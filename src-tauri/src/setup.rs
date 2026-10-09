// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use std::str::FromStr;
use tauri::{App, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use assistant_config::{
    app_conf::AppConf,
    constant::{
        ORB_CHROME_INSET, ORB_EXPANDED_HEIGHT, ORB_EXPANDED_WIDTH, ORB_HITBOX_SIZE, WINDOW_MAIN,
        WINDOW_ORB,
    },
    orb_conf::OrbConf,
};
use assistant_orb::placement;
use assistant_storage::db;

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

    spawn_screen_inspection(handle.clone());

    register_global_shortcuts(handle);

    log::info!("Assistant started");
    Ok(())
}

fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let mut modifiers = Modifiers::empty();
    let mut code: Option<Code> = None;

    for part in s.split('+') {
        let part = part.trim();
        match part.to_lowercase().as_str() {
            "meta" | "super" | "cmd" | "win" | "windows" => modifiers |= Modifiers::META,
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            _ => {
                let key_part = part.to_uppercase();
                if let Ok(c) = Code::from_str(&key_part) {
                    code = Some(c);
                } else if let Ok(c) = Code::from_str(&format!("Key{}", key_part)) {
                    code = Some(c);
                } else if let Ok(c) = Code::from_str(&format!("Digit{}", key_part)) {
                    code = Some(c);
                }
            }
        }
    }

    code.map(|c| Shortcut::new(Some(modifiers), c))
}

pub fn register_global_shortcuts(handle: &tauri::AppHandle) {
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };

    let bindings = &conf.keybindings;
    let gs = handle.global_shortcut();

    let _ = gs.unregister_all();

    let primary_mod = if cfg!(target_os = "macos") {
        "Meta"
    } else {
        "Control"
    };
    let default_toggle_orb = format!("{}+Shift+O", primary_mod);
    let default_toggle_main = format!("{}+Shift+M", primary_mod);
    let default_new_chat = format!("{}+Shift+N", primary_mod);

    let shortcut_ids: [(&str, &str); 3] = [
        ("toggle_orb", &default_toggle_orb),
        ("toggle_main", &default_toggle_main),
        ("new_chat", &default_new_chat),
    ];

    for (id, default) in shortcut_ids {
        let binding = bindings.get(id).map(|s| s.as_str()).unwrap_or(default);
        let Some(shortcut) = parse_shortcut(binding) else {
            log::warn!("Failed to parse shortcut binding for {}: {}", id, binding);
            continue;
        };

        if let Err(e) = gs.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            match id {
                "toggle_orb" => {
                    let _ = crate::window::toggle(app, WINDOW_ORB);
                }
                "toggle_main" => {
                    let _ = crate::window::toggle(app, WINDOW_MAIN);
                }
                "new_chat" => {
                    let _ = app.emit("orb-new-conversation", ());
                }
                _ => {}
            }
        }) {
            log::warn!("Failed to register shortcut {} ({}): {}", id, binding, e);
            let _ = handle.emit(
                "shortcut-warning",
                serde_json::json!({
                    "id": id,
                    "binding": binding,
                    "error": e.to_string(),
                }),
            );
        }
    }
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

fn spawn_screen_inspection(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(placement::PLACEMENT_INSPECTION_INTERVAL);
        if crate::SHOULD_EXIT.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
            continue;
        };
        let Ok(pos) = window.outer_position() else {
            continue;
        };
        let Ok(size) = window.outer_size() else {
            continue;
        };
        let screens = placement::get_available_screens(&handle);
        let visual_x = pos.x + ORB_CHROME_INSET as i32;
        let visual_y = pos.y + ORB_CHROME_INSET as i32;
        let visual_bounds =
            placement::WindowBounds::new(visual_x, visual_y, ORB_HITBOX_SIZE, ORB_HITBOX_SIZE);
        if placement::bounds_inside_screens(&screens, visual_bounds) {
            continue;
        }
        let bounds = placement::WindowBounds::new(pos.x, pos.y, size.width, size.height);
        let snapped = placement::snap_to_nearest_screen(&screens, bounds);
        let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
        if let Ok(mut conf) = OrbConf::load() {
            conf.position_x = Some(snapped.x);
            conf.position_y = Some(snapped.y);
            let _ = conf.save();
        }
    });
}
