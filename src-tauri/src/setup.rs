// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use tauri::{App, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use assistant_config::{
    app_conf::AppConf,
    constant::{ORB_EXPANDED_WIDTH, ORB_EXPANDED_HEIGHT, WINDOW_ORB},
    orb_conf::OrbConf,
};
use assistant_orb::placement;
use assistant_storage::db;
use assistant_storage::templates;

use super::tray;

pub struct DbState(pub std::sync::Mutex<rusqlite::Connection>);

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
            match db::init_db(&db_path) {
                Ok(conn) => {
                    if let Err(e) = templates::seed_builtin_templates(&conn) {
                        log::error!("Failed to seed builtin templates: {}", e);
                    }
                    handle.manage(DbState(std::sync::Mutex::new(conn)));
                }
                Err(e) => {
                    log::error!("Failed to init database: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to resolve app data dir: {}", e);
        }
    }

    handle.manage(crate::chat_tasks::ChatTasks::new());

    tray::create_tray(handle);

    position_orb_window(handle);

    let shortcut: Shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyK);
    if let Err(e) = handle.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window(WINDOW_ORB) {
                if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    }) {
        log::warn!("Failed to register global shortcut (may be occupied by another app): {}", e);
        if let Some(orb) = handle.get_webview_window(WINDOW_ORB) {
            let _ = orb.eval(&format!(
                "window.__shortcutError = '快捷键 Cmd/Ctrl+K 注册失败，可能被其它应用占用({})';",
                e
            ));
        }
    }

    log::info!("Assistant started");
    Ok(())
}

fn position_orb_window(handle: &tauri::AppHandle) {
    let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
        return;
    };

    let screens = placement::get_available_screens(handle);
    let scale = window.scale_factor().unwrap_or(1.0);
    let orb_w_phys = (ORB_EXPANDED_WIDTH as f64 * scale) as u32;
    let orb_h_phys = (ORB_EXPANDED_HEIGHT as f64 * scale) as u32;

    if let Ok(conf) = OrbConf::load() {
        if let (Some(x), Some(y)) = (conf.position_x, conf.position_y) {
            let bounds = placement::WindowBounds::new(x, y, orb_w_phys, orb_h_phys);
            if placement::bounds_inside_screens(&screens, bounds) {
                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
                return;
            }
            let snapped = placement::snap_to_nearest_screen(&screens, bounds);
            let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
            return;
        }
    }

    let Some(primary) = screens.first() else {
        return;
    };
    let edge_margin = 20;
    let x = primary.x + primary.width as i32 - orb_w_phys as i32 - edge_margin;
    let y = primary.y + primary.height as i32 - orb_h_phys as i32 - edge_margin;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}