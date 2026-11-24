use std::time::{Duration, Instant};

use tauri::{command, AppHandle, Manager};

use crate::protocol::WorkspaceDto;

/// `missing` 探测总耗时上限：网络盘/已拔出盘上的 `is_dir()` 可能长时间阻塞。
/// 超时项按 `missing = false` 处理且不报错（I38h §8 表 / C4.12）。
const PROBE_BUDGET: Duration = Duration::from_millis(500);

fn probe_missing(paths: &[String]) -> Vec<bool> {
    let total = paths.len();
    let mut missing = vec![false; total];
    if total == 0 {
        return missing;
    }

    let (tx, rx) = std::sync::mpsc::channel::<(usize, bool)>();
    let owned = paths.to_vec();
    std::thread::spawn(move || {
        for (index, path) in owned.into_iter().enumerate() {
            let missing = !std::path::Path::new(&path).is_dir();
            if tx.send((index, missing)).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + PROBE_BUDGET;
    for _ in 0..total {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok((index, is_missing)) => missing[index] = is_missing,
            Err(_) => break,
        }
    }
    missing
}

/// 目录簿列表（仅展示层缓存，不参与解析）。
///
/// 声明为 `async` + `spawn_blocking`：`probe_missing` 会为每个路径做一次 `is_dir()`，
/// 同步命令在主线程执行会把整个窗口卡住（I38h §8 表 / C4.12）。
#[command]
pub async fn list_workspaces(app: AppHandle) -> Result<Vec<WorkspaceDto>, String> {
    tauri::async_runtime::spawn_blocking(move || list_workspaces_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn list_workspaces_blocking(app: &AppHandle) -> Result<Vec<WorkspaceDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    // 先取快照并释放连接锁：探测最多等 500ms，不能把 DB 锁一起占住。
    let entries = {
        let conn = super::get_db_conn(&state)?;
        assistant_storage::workspaces::list(&conn)?
    };
    let paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let missing = probe_missing(&paths);
    Ok(entries
        .into_iter()
        .zip(missing)
        .map(|(entry, is_missing)| WorkspaceDto {
            path: entry.path,
            name: entry.name,
            missing: is_missing,
        })
        .collect())
}

/// 打开系统目录选择器，只返回所选路径，**不写库**（登记由 `register_workspace` 负责）。
///
/// 声明为 `async` 并把阻塞式对话框丢进 `spawn_blocking`：
/// 同步/阻塞调用不能占用主线程事件循环，否则窗口整段失去响应。
///
/// 非 UTF-8 路径直接报错，不用 `to_string_lossy` 改写（I44 §10.3）：
/// lossy 会返回一个与真实目录不同的字符串，后续登记必然失败或落到别处。
#[command]
pub async fn choose_workspace_dir(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || choose_workspace_dir_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

fn choose_workspace_dir_blocking(app: &AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(path) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    path.to_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| assistant_i18n::t("error.path_not_utf8"))
}

/// 登记（或刷新）目录簿条目；路径与 `patch_conversation` 写入值共用同一校验。
///
/// `validate_workspace_root` 里有 `canonicalize()`（网络盘上可能长时间阻塞），
/// 因此同样必须离开主线程。
#[command]
pub async fn register_workspace(
    app: AppHandle,
    path: String,
    name: Option<String>,
) -> Result<WorkspaceDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        register_workspace_blocking(&app, &path, name.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn register_workspace_blocking(
    app: &AppHandle,
    path: &str,
    name: Option<&str>,
) -> Result<WorkspaceDto, String> {
    let canonical = crate::tool_setup::validate_workspace_root(path)
        .ok_or_else(|| assistant_i18n::t("error.workspace_invalid"))?;
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let entry = assistant_storage::workspaces::upsert(&conn, &canonical, name)?;
    Ok(WorkspaceDto {
        path: entry.path,
        name: entry.name,
        missing: false,
    })
}

/// 只删目录簿行，不触碰任何会话绑定（I38h A11）。
#[command]
pub async fn forget_workspace(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        assistant_storage::workspaces::forget(&conn, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 在系统文件管理器里打开该会话绑定的工作区。
///
/// 参数只有会话 id：真实路径由 `resolve_workspace` 从库里推导，前端不传路径。
/// `open_path` 在 Windows 上是 `ShellExecute`，传可执行文件即执行，因此**绝不能让
/// 前端提供路径**——可达集合必须收窄为「某会话真实绑定、且已通过
/// `validate_workspace_root`（绝对路径 + `canonicalize` + `is_dir`）的目录」。
///
/// 返回 `false` 表示「未绑定 / 目录已失效」，与 `open_attachment` 同一约定：
/// 不可打开不升格为命令错误，由前端提示。
#[command]
pub async fn open_workspace_dir(app: AppHandle, conversation_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        let Some(root) = crate::tool_setup::resolve_workspace(&conn, &conversation_id) else {
            return Ok(false);
        };
        tauri_plugin_opener::open_path(std::path::Path::new(&root), None::<&str>)
            .map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}
