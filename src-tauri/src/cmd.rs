// 参考: ChatGPT/src-tauri/src/core/cmd.rs — 命令集中管理
// 参考: WindowPet/cmd.rs — 鼠标位置获取

use log::error;
use mouse_position::mouse_position::Mouse;
use serde_json::json;
use tauri::command;

use assistant_config::{app_conf::AppConf, orb_conf::OrbConf};

#[command]
pub fn get_mouse_position() -> serde_json::Value {
    let position = Mouse::get_mouse_position();
    match position {
        Mouse::Position { x, y } => json!({
            "clientX": x,
            "clientY": y
        }),
        Mouse::Error => {
            error!("Error getting mouse position");
            json!(null)
        }
    }
}

#[command]
pub fn get_app_conf() -> serde_json::Value {
    match AppConf::load() {
        Ok(conf) => serde_json::to_value(&conf).unwrap_or(json!(null)),
        Err(e) => {
            error!("Failed to load app config: {}", e);
            json!(null)
        }
    }
}

#[command]
pub fn set_app_conf(conf: AppConf) -> Result<(), String> {
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn get_orb_conf() -> serde_json::Value {
    match OrbConf::load() {
        Ok(conf) => serde_json::to_value(&conf).unwrap_or(json!(null)),
        Err(e) => {
            error!("Failed to load orb config: {}", e);
            json!(null)
        }
    }
}

#[command]
pub fn set_orb_conf(conf: OrbConf) -> Result<(), String> {
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn set_theme(theme: String) -> Result<(), String> {
    let mut conf = AppConf::load().map_err(|e| e.to_string())?;
    conf.theme = theme;
    conf.save().map_err(|e| e.to_string())
}