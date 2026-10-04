// 参考: ChatGPT/src-tauri/src/core/cmd.rs — 命令集中管理
// 参考: WindowPet/cmd.rs — 鼠标位置获取

use std::sync::atomic::{AtomicBool, Ordering};

use log::error;
use mouse_position::mouse_position::Mouse;
use serde_json::json;
use tauri::{command, AppHandle, Emitter, Manager};

use assistant_config::{app_conf::AppConf, constant::WINDOW_ORB, orb_conf::OrbConf};
use assistant_orb::placement;
use crate::request_exit;

static ORB_CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

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

#[command]
pub fn emit_theme_changed(app: AppHandle, resolved: String) -> Result<(), String> {
    app.emit("theme-changed", serde_json::json!({ "resolved": resolved }))
        .map_err(|e| e.to_string())
}

#[command]
pub fn toggle_orb_visibility(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[command]
pub fn set_orb_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    if ORB_CLICK_THROUGH.load(Ordering::Relaxed) == enabled {
        return Ok(());
    }
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| e.to_string())?;
    ORB_CLICK_THROUGH.store(enabled, Ordering::Relaxed);
    log::debug!("Orb click-through set to {}", enabled);
    Ok(())
}

#[command]
pub fn get_screen_size(app: AppHandle, center_x: f64, center_y: f64, scale_factor: f64) -> serde_json::Value {
    let cx_phys = center_x * scale_factor;
    let cy_phys = center_y * scale_factor;
    let screens = placement::get_available_screens(&app);
    let best = screens.iter().min_by_key(|s| {
        let sx = s.x + s.width as i32 / 2;
        let sy = s.y + s.height as i32 / 2;
        let dx = cx_phys as i64 - sx as i64;
        let dy = cy_phys as i64 - sy as i64;
        dx * dx + dy * dy
    });
    match best {
        Some(s) => json!({
            "screen": { "x": s.x, "y": s.y, "width": s.width, "height": s.height }
        }),
        None => json!({ "screen": { "x": 0, "y": 0, "width": 1920, "height": 1080 } }),
    }
}

#[command]
pub fn snap_orb_to_screen(app: AppHandle) -> Result<(), String> {
    placement::check_and_snap_orb(&app, WINDOW_ORB);
    Ok(())
}

#[command]
pub fn save_orb_position(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let mut conf = OrbConf::load().map_err(|e| e.to_string())?;
    conf.position_x = Some(pos.x);
    conf.position_y = Some(pos.y);
    conf.save().map_err(|e| e.to_string())
}

#[command]
pub fn app_quit(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(WINDOW_ORB) {
        if let Ok(pos) = w.outer_position() {
            if let Ok(mut conf) = OrbConf::load() {
                conf.position_x = Some(pos.x);
                conf.position_y = Some(pos.y);
                let _ = conf.save();
            }
        }
        let _ = w.destroy();
    }
    request_exit();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.destroy();
    }
    app.exit(0);
    Ok(())
}