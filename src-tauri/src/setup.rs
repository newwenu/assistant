// 参考: ChatGPT/src-tauri/src/core/setup.rs — setup 中初始化配置和窗口

use std::collections::HashMap;
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
    vault_conf::VaultConf,
};
use assistant_orb::placement;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::db;
use assistant_tools::{os, ToolRegistry};

use super::tray;
use crate::protocol::ShortcutWarningPayloadDto;

pub struct DbState(pub std::sync::Mutex<rusqlite::Connection>);

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    if let Err(e) = VaultConf::load() {
        log::error!("Failed to init vault config: {}", e);
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

    {
        let attachments_dir = match handle.path().app_data_dir() {
            Ok(data_dir) => data_dir.join("attachments"),
            Err(e) => {
                log::error!("Failed to resolve app data dir for attachments: {}", e);
                return Err(Box::new(e));
            }
        };
        std::fs::create_dir_all(&attachments_dir).expect("Failed to create attachments directory");
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

    handle.manage(assistant_agent::chat_tasks::ChatTasks::new());

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

/// 一条全局快捷键的静态定义：id 用于配置查找，action 是按下时执行的函数。
struct ShortcutSpec {
    id: &'static str,
    action: fn(&tauri::AppHandle),
}

static SHORTCUT_SPECS: &[ShortcutSpec] = &[
    ShortcutSpec {
        id: "toggle_orb",
        action: toggle_orb_action,
    },
    ShortcutSpec {
        id: "toggle_main",
        action: toggle_main_action,
    },
    ShortcutSpec {
        id: "new_chat",
        action: new_chat_action,
    },
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

struct ResolvedShortcut {
    id: &'static str,
    binding: String,
    shortcut: Shortcut,
    action: fn(&tauri::AppHandle),
}

struct ShortcutIssue {
    id: &'static str,
    binding: String,
    error: String,
}

/// 把配置与默认值解析成可注册项。纯函数，便于单测。
///
/// 规则：
/// - 配置里显式清空（空字符串）表示用户主动禁用，不回落默认值；
/// - 配置缺失时使用默认值；
/// - 解析失败、重复绑定都记录为 issue，不注册该条；
/// - 重复判定使用归一化后的字符串，后出现的条目让位给先出现的。
fn resolve_shortcut_bindings(
    configured: &HashMap<String, String>,
    defaults: &HashMap<String, String>,
) -> (Vec<ResolvedShortcut>, Vec<ShortcutIssue>) {
    let mut resolved = Vec::new();
    let mut issues = Vec::new();
    let mut seen: HashMap<String, &'static str> = HashMap::new();

    for spec in SHORTCUT_SPECS {
        let binding = match configured.get(spec.id) {
            Some(raw) if raw.trim().is_empty() => continue,
            Some(raw) => raw.trim().to_string(),
            None => match defaults.get(spec.id) {
                Some(default) => default.clone(),
                None => {
                    issues.push(ShortcutIssue {
                        id: spec.id,
                        binding: String::new(),
                        error: "no default binding".to_string(),
                    });
                    continue;
                }
            },
        };

        let Some(shortcut) = parse_shortcut(&binding) else {
            issues.push(ShortcutIssue {
                id: spec.id,
                binding,
                error: "invalid binding".to_string(),
            });
            continue;
        };

        let normalized = binding.to_lowercase().replace(' ', "");
        if let Some(existing) = seen.get(&normalized) {
            issues.push(ShortcutIssue {
                id: spec.id,
                binding,
                error: format!("duplicate of {}", existing),
            });
            continue;
        }

        seen.insert(normalized, spec.id);
        resolved.push(ResolvedShortcut {
            id: spec.id,
            binding,
            shortcut,
            action: spec.action,
        });
    }

    (resolved, issues)
}

fn emit_shortcut_warning(handle: &tauri::AppHandle, issue: &ShortcutIssue) {
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
    let (resolved, issues) = resolve_shortcut_bindings(&conf.keybindings, &defaults);
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
                &ShortcutIssue {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> HashMap<String, String> {
        assistant_config::app_conf::default_keybindings()
    }

    #[test]
    fn default_bindings_resolve() {
        let (resolved, issues) = resolve_shortcut_bindings(&HashMap::new(), &defaults());
        assert_eq!(resolved.len(), SHORTCUT_SPECS.len());
        assert!(issues.is_empty());
    }

    #[test]
    fn explicitly_cleared_binding_is_disabled() {
        let mut configured = defaults();
        configured.insert("new_chat".to_string(), "   ".to_string());
        let (resolved, issues) = resolve_shortcut_bindings(&configured, &defaults());
        assert_eq!(resolved.len(), SHORTCUT_SPECS.len() - 1);
        assert!(issues.is_empty());
        assert!(!resolved.iter().any(|item| item.id == "new_chat"));
    }

    #[test]
    fn invalid_binding_is_reported() {
        let mut configured = defaults();
        configured.insert("new_chat".to_string(), "NotAKey".to_string());
        let (resolved, issues) = resolve_shortcut_bindings(&configured, &defaults());
        assert_eq!(resolved.len(), SHORTCUT_SPECS.len() - 1);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "new_chat");
        assert_eq!(issues[0].error, "invalid binding");
    }

    #[test]
    fn duplicate_binding_is_rejected() {
        let mut configured = defaults();
        let orb_binding = configured
            .get("toggle_orb")
            .cloned()
            .expect("toggle_orb default");
        configured.insert("new_chat".to_string(), orb_binding);
        let (resolved, issues) = resolve_shortcut_bindings(&configured, &defaults());
        assert_eq!(resolved.len(), SHORTCUT_SPECS.len() - 1);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "new_chat");
        assert!(issues[0].error.starts_with("duplicate of toggle_orb"));
    }
}
