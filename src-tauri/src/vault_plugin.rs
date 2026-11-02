// 参考: docs/store/I24-feature-extension-spec.md §3.8.3 — vault 走插件边界，可选性只体现一次

use tauri::plugin::TauriPlugin;

#[cfg(feature = "vault")]
pub fn init() -> TauriPlugin<tauri::Wry> {
    crate::cmd::register_conf_applier(
        "vault",
        crate::cmd::vault::apply_conf as crate::cmd::ConfApplier,
    );
    tauri::plugin::Builder::<tauri::Wry>::new("vault")
        .setup(|app, _api| {
            crate::cmd::vault::vault_setup(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::cmd::vault::vault_init,
            crate::cmd::vault::vault_unlock,
            crate::cmd::vault::vault_is_locked,
            crate::cmd::vault::vault_is_initialized,
            crate::cmd::vault::vault_lock,
            crate::cmd::vault::vault_list_entries,
            crate::cmd::vault::vault_get_entry,
            crate::cmd::vault::vault_copy_password,
            crate::cmd::vault::vault_copy_field,
            crate::cmd::vault::vault_reveal_password,
            crate::cmd::vault::vault_reveal_notes,
            crate::cmd::vault::vault_create_entry,
            crate::cmd::vault::vault_update_entry,
            crate::cmd::vault::vault_set_favorite,
            crate::cmd::vault::vault_delete_entry,
            crate::cmd::vault::vault_list_deleted_entries,
            crate::cmd::vault::vault_restore_entry,
            crate::cmd::vault::vault_purge_deleted,
            crate::cmd::vault::vault_change_master_password,
            crate::cmd::vault::vault_export_to_file,
            crate::cmd::vault::vault_import,
            crate::cmd::vault::vault_totp_generate,
            crate::cmd::vault::vault_copy_totp,
            crate::cmd::vault::vault_totp_time_remaining,
            crate::cmd::vault::vault_totp_parse_uri,
            crate::cmd::vault::vault_query_audit_log,
            crate::cmd::vault::vault_hibp_check,
            crate::cmd::vault::vault_hibp_audit,
            crate::cmd::vault::vault_check_master_password_strength,
            crate::cmd::vault::vault_show_unlock_window,
            crate::cmd::vault::vault_is_biometric_available,
            crate::cmd::vault::vault_is_biometric_enrolled,
            crate::cmd::vault::vault_unlock_biometric,
            crate::cmd::vault::vault_enroll_biometric,
            crate::cmd::vault::vault_disenroll_biometric,
            crate::cmd::vault::vault_sync,
            crate::cmd::vault::vault_sync_last_time,
            crate::cmd::vault::vault_get_sync_config,
            crate::cmd::vault::vault_set_sync_config,
            crate::cmd::vault::vault_resolve_conflict,
            crate::cmd::vault::vault_sync_abort,
            crate::cmd::vault::vault_sync_discard,
            crate::cmd::vault::vault_git_init,
            crate::cmd::vault::vault_git_remove_remote,
            crate::cmd::vault::vault_git_test_connection,
            crate::cmd::vault::vault_git_remote,
            crate::cmd::vault::vault_sync_readyness,
            crate::cmd::vault::vault_sync_remote_status,
            crate::cmd::vault::vault_sync_conflicts,
            crate::cmd::vault::vault_sync_pending_removals,
            crate::cmd::vault::vault_sync_confirm_removals,
            crate::cmd::vault::vault_history_list,
            crate::cmd::vault::vault_history_preview,
            crate::cmd::vault::vault_history_restore,
            crate::cmd::vault::vault_conflict_preview,
            crate::cmd::vault::vault_conflict_reveal,
            crate::cmd::vault::vault_ssh_add_to_agent,
            crate::cmd::vault::vault_ssh_remove_from_agent,
            crate::cmd::vault::vault_ssh_list_agent,
            crate::cmd::vault::vault_ssh_auto_add_all,
            crate::cmd::vault::vault_ssh_is_agent_running,
            crate::cmd::vault::vault_ssh_parse_key,
            crate::cmd::vault::vault_ssh_key_meta,
            crate::cmd::vault::vault_generate_recovery_key,
            crate::cmd::vault::vault_has_recovery_key,
            crate::cmd::vault::vault_verify_vault_meta_integrity,
            crate::cmd::vault::vault_disaster_recovery_export,
        ])
        .build()
}

#[cfg(not(feature = "vault"))]
pub fn init() -> TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::<tauri::Wry>::new("vault").build()
}
