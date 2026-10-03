// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use tauri::{App, Manager};

use assistant_config::{app_conf::AppConf, constant::WINDOW_ORB, orb_conf::OrbConf};

use super::tray;

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    if let Err(e) = AppConf::load() {
        log::error!("Failed to init app config: {}", e);
    }
    if let Err(e) = OrbConf::load() {
        log::error!("Failed to init orb config: {}", e);
    }

    if let Some(orb) = handle.get_webview_window(WINDOW_ORB) {
        let _ = orb.set_ignore_cursor_events(true);
    }

    tray::create_tray(handle);

    log::info!("Assistant started");
    Ok(())
}