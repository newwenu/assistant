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
#[command]
pub fn list_workspaces(app: AppHandle) -> Result<Vec<WorkspaceDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let entries = assistant_storage::workspaces::list(&conn)?;
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
/// 声明为 `async`：阻塞式对话框不能占用主线程事件循环。
///
/// 非 UTF-8 路径直接报错，不用 `to_string_lossy` 改写（I44 §10.3）：
/// lossy 会返回一个与真实目录不同的字符串，后续登记必然失败或落到别处。
#[command]
pub async fn choose_workspace_dir(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(path) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    let value = path
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| assistant_i18n::t("error.path_not_utf8"))?;
    Ok(Some(value))
}

/// 登记（或刷新）目录簿条目；路径与 `patch_conversation` 写入值共用同一校验。
#[command]
pub fn register_workspace(
    app: AppHandle,
    path: String,
    name: Option<String>,
) -> Result<WorkspaceDto, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let canonical = crate::tool_setup::validate_workspace_root(&path)
        .ok_or_else(|| assistant_i18n::t("error.workspace_invalid"))?;
    let entry = assistant_storage::workspaces::upsert(&conn, &canonical, name.as_deref())?;
    Ok(WorkspaceDto {
        path: entry.path,
        name: entry.name,
        missing: false,
    })
}

/// 只删目录簿行，不触碰任何会话绑定（I38h A11）。
#[command]
pub fn forget_workspace(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    assistant_storage::workspaces::forget(&conn, &path)
}
