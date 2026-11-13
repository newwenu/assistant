// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use std::str::FromStr;
use std::sync::Arc;
use tauri::{App, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use assistant_config::shortcut::{resolve_bindings, BindingIssue};
use assistant_config::{
    app_conf::AppConf,
    constant::{
        ORB_CHROME_INSET, ORB_COLLAPSED_WINDOW_SIZE, ORB_HITBOX_SIZE, WINDOW_MAIN, WINDOW_ORB,
    },
    orb_conf::OrbConf,
};
use assistant_orb::placement;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::db;

use super::tray;
use crate::protocol::ShortcutWarningPayloadDto;

pub struct DbState(pub std::sync::Mutex<rusqlite::Connection>);

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    for mount in crate::feature_mounts::mounts() {
        if let Err(e) = (mount.on_setup)(handle) {
            log::error!("[{}] {}", mount.id, e);
        }
    }
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

    let dirs = crate::app_dirs::AppDirs::resolve()?;

    {
        let conn = db::init_db_localized(&dirs.database(), &|key| assistant_i18n::t(key))
            .map_err(Box::<dyn std::error::Error>::from)?;
        match assistant_storage::conversations::mark_streaming_messages_interrupted(&conn) {
            Ok(0) => {}
            Ok(count) => {
                log::warn!("startup sweep: {count} streaming messages -> interrupted");
            }
            Err(e) => {
                log::warn!("startup sweep failed: {e}");
            }
        }
        handle.manage(DbState(std::sync::Mutex::new(conn)));
    }

    {
        let attachments_dir = dirs.attachments();
        std::fs::create_dir_all(&attachments_dir)?;
        let store = AttachmentStore::new(attachments_dir);
        handle.manage(store);
        log::info!("AttachmentStore initialized");

        {
            let store = handle.state::<AttachmentStore>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = store.clean_tmp().await {
                    log::warn!("Failed to clean tmp attachments: {}", e);
                }
            });
        }
    }

    handle.manage(Arc::new(assistant_agent::kernel::TurnTasks::new()));
    handle.manage(Arc::new(assistant_agent::kernel::SessionStates::new()));
    handle.manage(std::sync::Arc::new(crate::clock::ClockRuntime::new()));

    crate::tool_setup::init(handle)?;

    tray::create_tray(handle);

    position_orb_window(handle);
    if let Err(e) = crate::window::restore_main_window(handle) {
        log::warn!("Failed to restore main window geometry: {}", e);
    }
    crate::window::apply_orb_window_attrs(handle);
    show_orb_window(handle);

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

/// 按下快捷键时执行的回调
type ShortcutAction = fn(&tauri::AppHandle);

/// 全局快捷键的静态定义：id 用于配置查找，第二项是按下时执行的回调。
static SHORTCUT_SPECS: &[(&str, ShortcutAction)] = &[
    ("toggle_orb", toggle_orb_action),
    ("toggle_main", toggle_main_action),
    ("new_chat", new_chat_action),
];

fn toggle_orb_action(app: &tauri::AppHandle) {
    let _ = crate::window::toggle(app, WINDOW_ORB);
}

fn toggle_main_action(app: &tauri::AppHandle) {
    let _ = crate::window::toggle(app, WINDOW_MAIN);
}

fn new_chat_action(app: &tauri::AppHandle) {
    emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
}

fn emit_shortcut_warning(handle: &tauri::AppHandle, issue: &BindingIssue) {
    log::warn!(
        "Shortcut {} ({}) unavailable: {}",
        issue.id,
        issue.binding,
        issue.error
    );
    emit_broadcast(
        handle,
        event_name::SHORTCUT_WARNING,
        ShortcutWarningPayloadDto {
            id: issue.id.to_string(),
            binding: issue.binding.clone(),
            error: issue.error.clone(),
        },
    );
}

pub fn register_global_shortcuts(handle: &tauri::AppHandle) {
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };

    let defaults = assistant_config::app_conf::default_keybindings();
    let (resolved, issues) =
        resolve_bindings(SHORTCUT_SPECS, &conf.keybindings, &defaults, parse_shortcut);
    for issue in &issues {
        emit_shortcut_warning(handle, issue);
    }

    let gs = handle.global_shortcut();
    let _ = gs.unregister_all();

    for item in resolved {
        let action = item.action;
        let id = item.id;
        let binding = item.binding.clone();
        if let Err(e) = gs.on_shortcut(item.shortcut, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            action(app);
        }) {
            emit_shortcut_warning(
                handle,
                &BindingIssue {
                    id,
                    binding,
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

    let area = placement::get_primary_work_area(handle).or_else(|| screens.first().copied());
    let Some(area) = area else {
        return;
    };
    let edge_margin = 20;
    let (x, y) = placement::anchor_bottom_right(&area, collapsed_phys, edge_margin);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// 两个窗口在 `tauri.conf.json` 中均为 `visible: false`：
/// 主窗口只在托盘/Orb/快捷键入口唤起时显示，Orb 则在坐标收敛完成后再显示，
/// 避免"先出现在系统默认位置、再跳到保存位置"的启动跳动。
fn show_orb_window(handle: &tauri::AppHandle) {
    let Some(window) = handle.get_webview_window(WINDOW_ORB) else {
        return;
    };
    if let Err(e) = window.show() {
        log::warn!("Failed to show orb window: {}", e);
    }
}

fn spawn_screen_inspection(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(placement::PLACEMENT_INSPECTION_INTERVAL);
        if crate::lifecycle::should_exit() {
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
