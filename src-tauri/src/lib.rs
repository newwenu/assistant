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
use assistant_security::clipboard_guard::ClipboardGuard;
use tauri::Manager;

use crate::protocol::events::{emit_broadcast, event_name};

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

pub fn request_exit() {
    SHOULD_EXIT.store(true, Ordering::Relaxed);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            cmd::toggle_orb_visibility,
            cmd::set_orb_click_through,
            cmd::get_screen_size,
            cmd::snap_orb_to_screen,
            cmd::snap_orb_to_edge,
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
            cmd::get_conversation_all_messages,
            cmd::delete_message_persist,
            cmd::get_message_subtree_size,
            cmd::search_conversations,
            cmd::export_conversation,
            cmd::export_conversation_to_file,
            cmd::record_tool_usage,
            cmd::get_recent_tools,
            cmd::get_favorite_tools,
            cmd::toggle_tool_favorite,
            cmd::set_favorite_tools_order,
            cmd::toggle_conversation_pin,
            cmd::duplicate_conversation,
            cmd::batch_delete_conversations,
            cmd::purge_conversations,
            cmd::create_branch,
            cmd::switch_branch,
            cmd::get_message_siblings,
            cmd::fork_to_new_conversation,
            cmd::increment_generation,
            cmd::get_app_version,
            cmd::get_rustc_version,
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
            cmd::os_tool_exec,
            cmd::vault_init,
            cmd::vault_unlock,
            cmd::vault_is_locked,
            cmd::vault_is_initialized,
            cmd::vault_lock,
            cmd::vault_list_entries,
            cmd::vault_get_entry,
            cmd::vault_copy_password,
            cmd::vault_copy_field,
            cmd::vault_reveal_password,
            cmd::vault_reveal_notes,
            cmd::vault_create_entry,
            cmd::vault_update_entry,
            cmd::vault_delete_entry,
            cmd::vault_change_master_password,
            cmd::vault_export_to_file,
            cmd::vault_import,
            cmd::vault_totp_generate,
            cmd::vault_copy_totp,
            cmd::vault_totp_time_remaining,
            cmd::vault_totp_parse_uri,
            cmd::vault_query_audit_log,
            cmd::vault_hibp_check,
            cmd::vault_hibp_audit,
            cmd::vault_check_master_password_strength,
            cmd::vault_show_unlock_window,
            cmd::vault_is_biometric_available,
            cmd::vault_is_biometric_enrolled,
            cmd::vault_unlock_biometric,
            cmd::vault_enroll_biometric,
            cmd::vault_disenroll_biometric,
            cmd::vault_sync_pull,
            cmd::vault_sync_push,
            cmd::vault_sync_now,
            cmd::vault_sync_force_push,
            cmd::vault_get_sync_config,
            cmd::vault_set_sync_config,
            cmd::vault_resolve_conflict,
            cmd::vault_git_init,
            cmd::vault_git_clone,
            cmd::vault_git_add_remote,
            cmd::vault_git_set_remote,
            cmd::vault_sync_readyness,
            cmd::vault_sync_prepare,
            cmd::vault_sync_finalize,
            cmd::vault_ssh_add_to_agent,
            cmd::vault_ssh_remove_from_agent,
            cmd::vault_ssh_list_agent,
            cmd::vault_ssh_auto_add_all,
            cmd::vault_ssh_is_agent_running,
            cmd::vault_generate_recovery_key,
            cmd::vault_unlock_with_recovery_key,
            cmd::vault_has_recovery_key,
            cmd::vault_verify_vault_meta_integrity,
            cmd::vault_disaster_recovery_export,
            cmd::get_storage_breakdown,
            cmd::delete_attachments,
        ])
        .on_menu_event(|app, event| match event.id.as_ref() {
            "orb_new_chat" => {
                emit_broadcast(app, event_name::ORB_NEW_CONVERSATION, ());
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
                    use tauri::Emitter;
                    let _ = window.emit(event_name::ORB_BLUR, ());
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!("tauri.conf.json"))
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let guard = app_handle.state::<ClipboardGuard>();
                let _ = guard.force_clear();
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !SHOULD_EXIT.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
        });
}
