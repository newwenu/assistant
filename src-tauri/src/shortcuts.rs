// 参考: docs/store/I75-startup-layering-spec.md §9 #12 — 全局快捷键实体（启动与运行期共用）

use std::str::FromStr;

use assistant_config::app_conf::AppConf;
use assistant_config::shortcut::{resolve_bindings, BindingIssue};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::ShortcutWarningPayloadDto;

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
    ("capture_screenshot", capture_screenshot_action),
];

fn capture_screenshot_action(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(code) = crate::screenshot::start(&handle).await {
            crate::screenshot::emit_error(&handle, &code);
        }
    });
}

fn toggle_window(app: &tauri::AppHandle, label: &str, name: &str) {
    let dirs = match crate::boot::dirs(app) {
        Ok(dirs) => dirs,
        Err(e) => {
            log::warn!("toggle {name} window failed: dirs not ready: {e}");
            return;
        }
    };
    if let Err(e) = crate::window::toggle(app, label, &dirs) {
        log::warn!("toggle {name} window failed: {e}");
    }
}

fn toggle_orb_action(app: &tauri::AppHandle) {
    toggle_window(app, assistant_config::constant::WINDOW_ORB, "orb");
}

fn toggle_main_action(app: &tauri::AppHandle) {
    toggle_window(app, assistant_config::constant::WINDOW_MAIN, "main");
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

/// 启动期传 `BootConf` 快照；运行期改配置由 `cmd/config.rs` 现读后传新快照。
pub fn register_global_shortcuts(handle: &tauri::AppHandle, conf: &AppConf) {
    let defaults = assistant_config::app_conf::default_keybindings();
    let (resolved, issues) =
        resolve_bindings(SHORTCUT_SPECS, &conf.keybindings, &defaults, parse_shortcut);
    for issue in &issues {
        emit_shortcut_warning(handle, issue);
    }

    let gs = handle.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        log::warn!("unregister global shortcuts failed: {e}");
    }

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
