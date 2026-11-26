// 参考: docs/store/I75-startup-layering-spec.md §5 — 创建 / 显隐 / 属性

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};
use assistant_config::orb_conf::OrbConf;

use crate::app_dirs::AppDirs;
use crate::boot::BootConf;

use super::geometry::{saved_normal_size, snap_position};
use super::mode::{set_mode, MainWindowMode};
use super::spec::spec;

static WINDOW_LOCKS: LazyLock<Mutex<HashMap<String, ()>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 启动期 / 运行期共用的建窗路径：规格全字段来自 `WINDOW_SPECS`，几何在同一条路径内套用。
///
/// 调用方只负责后续的 `show`：主窗口在 `Idle` 档建成后保持隐藏，等入口触发才显示。
pub(crate) fn create_window(app: &AppHandle, label: &str, dirs: &AppDirs) -> Result<(), String> {
    let window_spec = spec(label).ok_or_else(|| {
        let mut args = HashMap::new();
        args.insert("label", label);
        assistant_i18n::t_with_args("window.unknown_label", &args)
    })?;

    let resolved_title = match label {
        WINDOW_MAIN => assistant_i18n::t("window.title_main"),
        _ => window_spec.title.to_string(),
    };

    let mut builder = WebviewWindowBuilder::new(
        app,
        window_spec.label,
        WebviewUrl::App(window_spec.url.into()),
    )
    .title(&resolved_title)
    .inner_size(window_spec.width, window_spec.height)
    .visible(window_spec.visible)
    .resizable(window_spec.resizable)
    .decorations(window_spec.decorations)
    .transparent(window_spec.transparent)
    .shadow(window_spec.shadow)
    .always_on_top(window_spec.always_on_top)
    .skip_taskbar(window_spec.skip_taskbar)
    .maximizable(window_spec.maximizable)
    .minimizable(window_spec.minimizable)
    .accept_first_mouse(window_spec.accept_first_mouse)
    .zoom_hotkeys_enabled(window_spec.zoom_hotkeys_enabled)
    .focused(window_spec.focus);

    if window_spec.center {
        builder = builder.center();
    }

    if let Some(color) = window_spec.background_color {
        builder = builder.background_color(tauri::window::Color(
            u8::from_str_radix(&color[1..3], 16).unwrap_or(0),
            u8::from_str_radix(&color[3..5], 16).unwrap_or(0),
            u8::from_str_radix(&color[5..7], 16).unwrap_or(0),
            255,
        ));
    }

    // `E14` 接管落点（I67 §13.3）：显式声明 user data folder，不再依赖 Tauri 内部默认。
    // 值即 Tauri 强制默认（`app_local_data_dir()`），故落点不变、不迁移、不产生第二份
    // profile；`dirs` 取自启动期唯一解析结果（N15），取不到时保持 Tauri 默认并记 warn
    // （窗口创建不因此失败）。
    builder = builder.data_directory(dirs.webview_data_directory());

    // 首帧回传的 id 随 webview 初始化脚本注入，首帧关键路径上省一次 IPC 往返。
    // 主窗口是懒建轮次：id 带 `wake-` 前缀，起点就在此处打点，首帧回传即得 T_wake。
    let startup_id = if label == WINDOW_MAIN {
        let id = format!("wake-{}", uuid::Uuid::new_v4());
        app.state::<crate::boot::StartupReport>()
            .record_wake_start(&id, crate::boot::epoch_ms());
        id
    } else {
        app.state::<crate::boot::BootClock>()
            .startup_id()
            .to_string()
    };
    builder = builder.initialization_script(&format!("window.__STARTUP_ID__ = \"{startup_id}\";"));

    if let (Some(mw), Some(mh)) = (window_spec.min_width, window_spec.min_height) {
        builder = builder.min_inner_size(mw, mh);
    }

    #[cfg(not(debug_assertions))]
    let builder = builder.devtools(false);

    let window = match builder.build() {
        Ok(window) => window,
        Err(e) => {
            log::warn!("Failed to build window {}: {}", label, e);
            if let Some(window) = app.get_webview_window(label) {
                window.show().map_err(|e| e.to_string())?;
                window.set_focus().map_err(|e| e.to_string())?;
                return Ok(());
            }
            return Err(format!("failed to build window {label}: {e}"));
        }
    };

    match label {
        WINDOW_MAIN => apply_main_geometry(app, &window)?,
        WINDOW_ORB => apply_orb_geometry(app),
        _ => {}
    }

    // label 属于阶段行没有的上下文（同一阶段会建不同窗口），dev 档可见、release 档静默。
    log::debug!("Created window: {label}");

    Ok(())
}

/// 主窗口启动期只恢复 normal 几何；形态本身是运行时状态、不落盘。
fn apply_main_geometry(app: &AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
    let conf = match crate::boot::boot_conf(app) {
        Ok(conf) => conf,
        Err(error) => {
            log::warn!("Failed to read boot conf for main window geometry: {error}");
            return Ok(());
        }
    };

    let (width, height) = saved_normal_size(&conf.app);
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;

    if let (Some(x), Some(y)) = (conf.app.main_window_x, conf.app.main_window_y) {
        snap_position(app, window, x, y)?;
    }

    set_mode(MainWindowMode::Normal);
    Ok(())
}

fn apply_orb_geometry(app: &AppHandle) {
    let conf: std::sync::Arc<BootConf> = match crate::boot::boot_conf(app) {
        Ok(conf) => conf,
        Err(error) => {
            log::warn!("Failed to read boot conf for orb window geometry: {error}");
            return;
        }
    };
    super::placement::position_orb_window(app, &conf.orb);
    apply_orb_window_attrs(app, &conf.orb);
}

/// 只 `show`，不抢焦点：Orb 启动期在坐标收敛后显示，抢焦点会打断用户当前窗口。
pub fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = get_or_error(app, label)?;
    window.show().map_err(|e| e.to_string())
}

fn show_window_focused(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = get_or_error(app, label)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

pub fn show_or_create(app: &AppHandle, label: &str, dirs: &AppDirs) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        return show_window_focused(app, label);
    }

    // 主窗口只经 `ensure(window_main)` 懒建：资源表项是唯一建窗入口，这里不重复建。
    if label == WINDOW_MAIN {
        show_or_create_main(app);
        return Ok(());
    }

    let _lock = WINDOW_LOCKS.lock().unwrap();

    if app.get_webview_window(label).is_some() {
        return show_window_focused(app, label);
    }

    create_window(app, label, dirs)?;
    show_window_focused(app, label)
}

/// 主窗口唯一触发路径：先 `ensure` 懒建，再显示并抢焦点。
pub async fn show_main_window(app: &AppHandle) -> Result<(), String> {
    crate::boot::ensure(app, "window_main")
        .await
        .map_err(|e| e.to_string())?;
    show_window_focused(app, WINDOW_MAIN)
}

pub fn toggle(app: &AppHandle, label: &str, dirs: &AppDirs) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        show_or_create(app, label, dirs)
    }
}

pub fn get_or_error(app: &AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("{} window not found", label))
}

pub fn destroy_all(app: &AppHandle, labels: &[&str]) {
    for label in labels {
        if let Some(w) = app.get_webview_window(label) {
            if let Err(e) = w.destroy() {
                log::warn!("destroy window {} failed: {e}", label);
            }
        }
    }
}

pub fn show_or_create_main(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = show_main_window(&handle).await {
            log::warn!(
                "show_or_create_main: show window {} failed: {e}",
                WINDOW_MAIN
            );
        }
    });
}

/// Orb 窗口常驻属性（I47 §6.1）：置顶与跨虚拟桌面/空间，二者同源于 `always_on_top`。
///
/// 失败只记 warn：窗口属性是尽力而为，不阻断启动。
/// 不加平台分支——tao 0.37.2 在 Windows 上 `set_visible_on_all_workspaces` 为空实现且不报错。
///
/// 配置由调用点传入：启动期用 `BootConf` 快照，运行期用刚保存的配置，本函数不再自行读盘。
pub fn apply_orb_window_attrs(app: &AppHandle, conf: &OrbConf) {
    if crate::screenshot::is_active() {
        return;
    }
    let Some(window) = app.get_webview_window(WINDOW_ORB) else {
        return;
    };
    if let Err(e) = window.set_always_on_top(conf.always_on_top) {
        log::warn!("Failed to set orb always on top: {}", e);
    }
    if let Err(e) = window.set_visible_on_all_workspaces(conf.always_on_top) {
        log::warn!("Failed to set orb visible on all workspaces: {}", e);
    }
}
