// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use std::str::FromStr;
use std::sync::Arc;
use tauri::{App, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use assistant_config::{
    app_conf::AppConf,
    constant::{
        ORB_CHROME_INSET, ORB_COLLAPSED_WINDOW_SIZE, ORB_HITBOX_SIZE, WINDOW_MAIN, WINDOW_ORB,
    },
    orb_conf::OrbConf,
};
use assistant_orb::placement;
use assistant_security::clipboard_guard::ClipboardGuard;
use assistant_security::reveal_guard::RevealGuard;
use assistant_security::screen_lock::ScreenLockMonitor;
use assistant_security::security_db::SecurityDbState;
use assistant_storage::db;
use assistant_tools::{os, ToolRegistry};

use super::tray;
use crate::protocol::ShortcutWarningPayloadDto;

pub struct DbState(pub std::sync::Mutex<rusqlite::Connection>);

pub struct SshAgentState {
    pub added_fingerprints: std::sync::Mutex<std::collections::HashSet<String>>,
}

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    if let Err(e) = AppConf::load() {
        log::error!("Failed to init app config: {}", e);
    }
    if let Err(e) = OrbConf::load() {
        log::error!("Failed to init orb config: {}", e);
    }

    {
        let locale = AppConf::load()
            .map(|c| c.language)
            .unwrap_or_else(|_| "zh-CN".to_string());
        assistant_i18n::init_global(&locale);
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

    handle.manage(assistant_agent::chat_tasks::ChatTasks::new());

    {
        let security_db_path = match handle.path().app_data_dir() {
            Ok(data_dir) => data_dir.join("security").join("security.db"),
            Err(_) => {
                log::error!("Failed to resolve app data dir for security db");
                std::path::PathBuf::from("security/security.db")
            }
        };
        handle.manage(SecurityDbState::new(
            security_db_path.to_str().unwrap_or("security/security.db"),
        ));
        handle.manage(RevealGuard::new());
        handle.manage(ClipboardGuard::new());
        handle.manage(SshAgentState {
            added_fingerprints: std::sync::Mutex::new(std::collections::HashSet::new()),
        });
        log::info!("SecurityDbState + RevealGuard + ClipboardGuard + SshAgentState initialized");
    }

    {
        let vault_dir = match handle.path().app_data_dir() {
            Ok(data_dir) => data_dir.join("security").join("vault"),
            Err(_) => std::path::PathBuf::from("security/vault"),
        };
        let sync_config = assistant_sync::SyncConfig::default();
        handle.manage(assistant_sync::sync_engine::SyncEngine::new(
            vault_dir,
            sync_config,
        ));
        log::info!("SyncEngine initialized");
    }

    {
        let monitor = ScreenLockMonitor::new();
        let app_handle = handle.clone();
        monitor.spawn_listener(move || {
            let state = app_handle.state::<SecurityDbState>();
            if let Err(e) = state.lock() {
                log::error!("Screen lock triggered vault_lock but failed: {}", e);
            } else {
                log::info!("Screen lock triggered vault_lock");
            }
            let _ = emit_broadcast(&app_handle, event_name::VAULT_LOCKED, ());
        });
        handle.manage(monitor);
        log::info!("ScreenLockMonitor initialized");
    }

    {
        let registry = Arc::new(ToolRegistry::new());
        os::register_os_tools(&registry);
        handle.manage(registry);
        log::info!("OS tool registry initialized");
    }

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
                    emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
                }
                _ => {}
            }
        }) {
            log::warn!("Failed to register shortcut {} ({}): {}", id, binding, e);
            emit_broadcast(
                handle,
                event_name::SHORTCUT_WARNING,
                ShortcutWarningPayloadDto {
                    id: id.to_string(),
                    binding: binding.to_string(),
                    error: e.to_string(),
                },
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
    let collapsed_phys = (ORB_COLLAPSED_WINDOW_SIZE as f64 * scale) as u32;

    if let Ok(conf) = OrbConf::load() {
        if let (Some(x), Some(y)) = (conf.position_x, conf.position_y) {
            if let Some(snapped) = placement::ensure_orb_on_screen(
                &screens,
                x,
                y,
                collapsed_phys,
                collapsed_phys,
                ORB_CHROME_INSET,
                ORB_HITBOX_SIZE,
            ) {
                let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
            } else {
                let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            }
            return;
        }
    }

    let Some(primary) = screens.first() else {
        return;
    };
    let edge_margin = 20;
    let x = primary.x + primary.width as i32 - collapsed_phys as i32 - edge_margin;
    let y = primary.y + primary.height as i32 - collapsed_phys as i32 - edge_margin;
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
        let Some(snapped) = placement::ensure_orb_on_screen(
            &screens,
            pos.x,
            pos.y,
            size.width,
            size.height,
            ORB_CHROME_INSET,
            ORB_HITBOX_SIZE,
        ) else {
            continue;
        };
        let _ = window.set_position(tauri::PhysicalPosition::new(snapped.x, snapped.y));
        if let Ok(mut conf) = OrbConf::load() {
            conf.position_x = Some(snapped.x);
            conf.position_y = Some(snapped.y);
            let _ = conf.save();
        }
    });
}
