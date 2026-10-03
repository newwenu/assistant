// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use tauri::{App, Manager};

use assistant_config::{app_conf::AppConf, orb_conf::OrbConf};
use assistant_storage::db;

use super::tray;

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    if let Err(e) = AppConf::load() {
        log::error!("Failed to init app config: {}", e);
    }
    if let Err(e) = OrbConf::load() {
        log::error!("Failed to init orb config: {}", e);
    }

    match handle.path().app_data_dir() {
        Ok(data_dir) => {
            let db_path = data_dir.join("assistant.db");
            if let Err(e) = db::init_db(&db_path) {
                log::error!("Failed to init database: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to resolve app data dir: {}", e);
        }
    }

    tray::create_tray(handle);

    log::info!("Assistant started");
    Ok(())
}