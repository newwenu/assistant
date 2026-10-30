// 参考: ChatGPT/src-tauri/src/main.rs — Builder 链式配置 + setup + invoke_handler
// 参考: BongoCat/crates/bongocat-app/src/lib.rs — 模块化组织

mod cmd;
pub mod protocol;
mod setup;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};

use assistant_config::constant::{WINDOW_MAIN, WINDOW_ORB};

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
            cmd::get_vault_conf,
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
            cmd::patch_conversation,
            cmd::send_chat_message,
            cmd::cancel_chat,
            cmd::delete_message_persist,
            cmd::get_message_subtree_size,
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
            #[cfg(feature = "vault")]
            cmd::vault_init,
            #[cfg(feature = "vault")]
            cmd::vault_unlock,
            #[cfg(feature = "vault")]
            cmd::vault_is_locked,
            #[cfg(feature = "vault")]
            cmd::vault_is_initialized,
            #[cfg(feature = "vault")]
            cmd::vault_lock,
            #[cfg(feature = "vault")]
            cmd::vault_list_entries,
            #[cfg(feature = "vault")]
            cmd::vault_get_entry,
            #[cfg(feature = "vault")]
            cmd::vault_copy_password,
            #[cfg(feature = "vault")]
            cmd::vault_copy_field,
            #[cfg(feature = "vault")]
            cmd::vault_reveal_password,
            #[cfg(feature = "vault")]
            cmd::vault_reveal_notes,
            #[cfg(feature = "vault")]
            cmd::vault_create_entry,
            #[cfg(feature = "vault")]
            cmd::vault_update_entry,
            #[cfg(feature = "vault")]
            cmd::vault_delete_entry,
            #[cfg(feature = "vault")]
            cmd::vault_list_deleted_entries,
            #[cfg(feature = "vault")]
            cmd::vault_restore_entry,
            #[cfg(feature = "vault")]
            cmd::vault_purge_deleted,
            #[cfg(feature = "vault")]
            cmd::vault_change_master_password,
            #[cfg(feature = "vault")]
            cmd::vault_export_to_file,
            #[cfg(feature = "vault")]
            cmd::vault_import,
            #[cfg(feature = "vault")]
            cmd::vault_totp_generate,
            #[cfg(feature = "vault")]
            cmd::vault_copy_totp,
            #[cfg(feature = "vault")]
            cmd::vault_totp_time_remaining,
            #[cfg(feature = "vault")]
            cmd::vault_totp_parse_uri,
            #[cfg(feature = "vault")]
            cmd::vault_query_audit_log,
            #[cfg(feature = "vault")]
            cmd::vault_hibp_check,
            #[cfg(feature = "vault")]
            cmd::vault_hibp_audit,
            #[cfg(feature = "vault")]
            cmd::vault_check_master_password_strength,
            #[cfg(feature = "vault")]
            cmd::vault_show_unlock_window,
            #[cfg(feature = "vault")]
            cmd::vault_is_biometric_available,
            #[cfg(feature = "vault")]
            cmd::vault_is_biometric_enrolled,
            #[cfg(feature = "vault")]
            cmd::vault_unlock_biometric,
            #[cfg(feature = "vault")]
            cmd::vault_enroll_biometric,
            #[cfg(feature = "vault")]
            cmd::vault_disenroll_biometric,
            #[cfg(feature = "vault")]
            cmd::vault_sync,
            #[cfg(feature = "vault")]
            cmd::vault_sync_last_time,
            #[cfg(feature = "vault")]
            cmd::vault_get_sync_config,
            #[cfg(feature = "vault")]
            cmd::vault_set_sync_config,
            #[cfg(feature = "vault")]
            cmd::vault_resolve_conflict,
            #[cfg(feature = "vault")]
            cmd::vault_sync_abort,
            #[cfg(feature = "vault")]
            cmd::vault_sync_discard,
            #[cfg(feature = "vault")]
            cmd::vault_git_init,
            #[cfg(feature = "vault")]
            cmd::vault_git_remove_remote,
            #[cfg(feature = "vault")]
            cmd::vault_git_test_connection,
            #[cfg(feature = "vault")]
            cmd::vault_git_remote,
            #[cfg(feature = "vault")]
            cmd::vault_sync_readyness,
            #[cfg(feature = "vault")]
            cmd::vault_sync_conflicts,
            #[cfg(feature = "vault")]
            cmd::vault_sync_pending_removals,
            #[cfg(feature = "vault")]
            cmd::vault_sync_confirm_removals,
            #[cfg(feature = "vault")]
            cmd::vault_history_list,
            #[cfg(feature = "vault")]
            cmd::vault_history_preview,
            #[cfg(feature = "vault")]
            cmd::vault_history_restore,
            #[cfg(feature = "vault")]
            cmd::vault_conflict_preview,
            #[cfg(feature = "vault")]
            cmd::vault_conflict_reveal,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_add_to_agent,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_remove_from_agent,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_list_agent,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_auto_add_all,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_is_agent_running,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_parse_key,
            #[cfg(feature = "vault")]
            cmd::vault_ssh_key_meta,
            #[cfg(feature = "vault")]
            cmd::vault_generate_recovery_key,
            #[cfg(feature = "vault")]
            cmd::vault_has_recovery_key,
            #[cfg(feature = "vault")]
            cmd::vault_verify_vault_meta_integrity,
            #[cfg(feature = "vault")]
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
                #[cfg(feature = "vault")]
                cmd::vault_cleanup_on_exit(app_handle);
                #[cfg(not(feature = "vault"))]
                let _ = app_handle;
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !SHOULD_EXIT.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
        });
}
