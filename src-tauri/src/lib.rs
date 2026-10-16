// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式配置 + setup + invoke_handler
// 参考: BongoCat/crates/bongocat-app/src/lib.rs — 模块化组织

mod cmd;
pub mod crash_dump;
pub mod protocol;
mod setup;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};

use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};
use tauri::Emitter;

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

pub fn request_exit() {
    SHOULD_EXIT.store(true, Ordering::Relaxed);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show_or_create_main(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(setup::init)
        .invoke_handler(tauri::generate_handler![
            cmd::get_mouse_position,
            cmd::get_app_conf,
            cmd::get_orb_conf,
            cmd::get_mascot_conf,
            cmd::patch_conf,
            cmd::set_theme,
            cmd::emit_theme_changed,
            cmd::set_locale,
            cmd::toggle_orb_visibility,
            cmd::set_orb_click_through,
            cmd::get_screen_size,
            cmd::snap_orb_to_screen,
            cmd::snap_orb_to_edge,
            cmd::save_orb_position,
            cmd::app_quit,
            window::open_main_window,
            window::open_settings,
            window::expand_orb_panel,
            window::collapse_orb_panel,
            window::show_orb_context_menu,
            cmd::list_providers,
            cmd::save_provider,
            cmd::delete_provider,
            cmd::list_models_cmd,
            cmd::preview_models_cmd,
            cmd::create_conversation,
            cmd::list_conversations,
            cmd::get_conversation,
            cmd::delete_conversation,
            cmd::rename_conversation,
            cmd::update_conversation_model,
            cmd::update_conversation_provider,
            cmd::send_chat_message,
            cmd::cancel_chat,
            cmd::get_conversation_paginated,
            cmd::delete_message_persist,
            cmd::search_conversations,
            cmd::export_conversation,
            cmd::record_tool_usage,
            cmd::get_recent_tools,
            cmd::get_favorite_tools,
            cmd::toggle_tool_favorite,
            cmd::set_favorite_tools_order,
            cmd::toggle_conversation_pin,
            cmd::duplicate_conversation,
            cmd::batch_delete_conversations,
            cmd::purge_conversations,
            cmd::edit_message,
            cmd::regenerate_message,
            cmd::reload_shortcuts,
            cmd::get_app_version,
            cmd::get_rustc_version,
            cmd::set_launch_at_login,
            cmd::open_main_window_and_navigate,
            cmd::list_agent_modes,
            cmd::save_agent_mode,
            cmd::delete_agent_mode,
            cmd::reset_builtin_mode,
            cmd::get_conversation_token_usage,
            cmd::install_mascot_asset,
            cmd::uninstall_mascot_asset,
            cmd::manual_compact_conversation,
            cmd::unlock_conversation,
        ])
        .on_menu_event(|app, event| match event.id.as_ref() {
            "orb_new_chat" => {
                let _ = app.emit("orb-new-conversation", ());
            }
            "orb_open_main" => {
                let _ = window::show_or_create(app, WINDOW_MAIN);
            }
            "orb_quit" => {
                cmd::perform_exit(app);
            }
            _ => {}
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == WINDOW_MAIN {
                    let close_to_tray = assistant_config::app_conf::AppConf::load()
                        .map(|c| c.close_to_tray)
                        .unwrap_or(true);
                    if close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
            tauri::WindowEvent::Focused(false) => {
                if window.label() == WINDOW_ORB {
                    let _ = window.emit("orb-blur", ());
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !SHOULD_EXIT.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
        });
}