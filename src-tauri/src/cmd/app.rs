use tauri::{command, AppHandle};

use crate::protocol::events::{emit_broadcast, event_name};

use assistant_config::{
    constant::{WINDOW_MAIN, WINDOW_ORB},
    orb_conf::OrbConf,
};

#[command]
pub fn app_quit(app: AppHandle) -> Result<(), String> {
    perform_exit(&app);
    Ok(())
}

pub fn perform_exit(app: &AppHandle) {
    if let Err(e) = crate::window::persist_main_geometry(app) {
        log::warn!("perform_exit: persist main geometry failed: {e}");
    }
    if let Ok(w) = crate::window::get_or_error(app, WINDOW_ORB) {
        if let Ok(pos) = w.outer_position() {
            if let Ok(mut conf) = OrbConf::load() {
                conf.position_x = Some(pos.x);
                conf.position_y = Some(pos.y);
                if let Err(e) = conf.save() {
                    log::warn!("perform_exit: save orb position failed: {e}");
                }
            }
        }
    }
    crate::lifecycle::request_exit();
    crate::window::destroy_all(app, &[WINDOW_ORB, WINDOW_MAIN]);
    app.exit(0);
}

#[command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[command]
pub fn get_rustc_version() -> String {
    rustc_version_runtime::version().to_string()
}

/// 通用动作：主窗口懒建后导航。托盘 / 菜单 / 前端命令共用一份，避免各入口各写一遍。
pub(crate) async fn show_main_and_navigate(app: &AppHandle, route: &str) -> Result<(), String> {
    crate::window::show_main_window(app).await?;
    emit_broadcast(app, event_name::NAVIGATE, route);
    Ok(())
}

/// 同步回调（托盘 / 菜单事件）入口：建窗是 async，转投 runtime；失败只记 warn。
pub(crate) fn show_main_and_navigate_sync(app: &AppHandle, route: &'static str) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = show_main_and_navigate(&handle, route).await {
            log::warn!("show main window and navigate {route} failed: {e}");
        }
    });
}

#[command]
pub async fn open_main_window_and_navigate(app: AppHandle, route: String) -> Result<(), String> {
    show_main_and_navigate(&app, &route).await
}
