use tauri::{command, AppHandle, Manager};

use assistant_security::audit::{log as audit_log, AuditEvent};
use assistant_sync::sync_engine::SyncEngine;

#[command]
pub fn vault_generate_recovery_key(app: AppHandle) -> Result<String, String> {
    let state = super::vault_state(&app)?;
    let master_key = state.get_master_key().map_err(|e| e.to_string())?;
    let mk = master_key.as_ref().ok_or("Vault is locked")?;
    let bootstrap_key = state.get_bootstrap_key().map_err(|e| e.to_string())?;
    let bk = bootstrap_key.as_ref().ok_or("Vault is locked")?;

    let (recovery_key, emergency) = assistant_security::recovery_key::generate_recovery_key(mk, bk)
        .map_err(|e| e.to_string())?;

    let engine = super::sync_state(&app)?;
    engine
        .write_emergency_data(mk, &emergency)
        .map_err(|e| e.to_string())?;

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::RecoveryKeyGenerated, None);
        }
    }

    log::warn!("Recovery key generated and emergency data written");
    Ok(recovery_key)
}

#[command]
pub fn vault_unlock_with_recovery_key(
    app: AppHandle,
    recovery_key: String,
) -> Result<bool, String> {
    let state = super::vault_state(&app)?;
    if !state.is_locked() {
        return Err("Vault is already unlocked".to_string());
    }

    let engine = super::sync_state(&app)?;
    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    let emergency = emergency.ok_or("Recovery key not set")?;

    let emergency_data = assistant_security::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let result = assistant_security::recovery_key::unlock_with_recovery_key(
        &recovery_key,
        &emergency_data,
        &state,
    )
    .map_err(|e| {
        log::warn!("Vault unlock with recovery key failed: {}", e);
        e.to_string()
    })?;

    if result {
        super::ssh_auto_add_on_unlock(&app);
        crate::protocol::events::emit_broadcast(
            &app,
            crate::protocol::events::event_name::VAULT_UNLOCKED,
            (),
        );
        log::warn!("Vault unlocked via recovery key");
    }

    Ok(result)
}

#[command]
pub fn vault_has_recovery_key(app: AppHandle) -> Result<bool, String> {
    let engine = super::sync_state(&app)?;
    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    Ok(emergency.is_some())
}

#[command]
pub fn vault_verify_vault_meta_integrity(
    app: AppHandle,
) -> Result<assistant_sync::vault_file::IntegrityResult, String> {
    let state = super::vault_state(&app)?;
    let master_key = state.get_master_key().map_err(|e| e.to_string())?;
    let mk = master_key.as_ref().ok_or("Vault is locked")?;

    let engine = super::sync_state(&app)?;
    engine
        .verify_vault_meta_integrity(mk)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisasterRecoveryResult {
    pub file_path: String,
    pub entry_count: u32,
    pub integrity_warning: bool,
}

#[command]
pub async fn vault_disaster_recovery_export(
    app: AppHandle,
    git_url: Option<String>,
    recovery_key: String,
    export_path: String,
) -> Result<DisasterRecoveryResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<SyncEngine>();

        if let Some(url) = &git_url {
            engine.clone_repo(url).map_err(|e| {
                log::warn!("Disaster recovery: clone failed: {}", e);
                e.to_string()
            })?;
        } else {
            engine.prepare().map_err(|e| {
                log::warn!("Disaster recovery: prepare failed: {}", e);
                e.to_string()
            })?;
            let _ = engine.pull();
        }

        let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
        let emergency = emergency.ok_or("Recovery key not set in .vault-meta")?;

        let emergency_data = assistant_security::recovery_key::EmergencyData {
            token: emergency.token,
            salt: emergency.salt,
        };

        let (_bootstrap_key, master_key) =
            assistant_security::recovery_key::decrypt_recovery_keys(&recovery_key, &emergency_data)
                .map_err(|e| {
                    log::warn!("Disaster recovery: decrypt failed: {}", e);
                    e.to_string()
                })?;

        let mut integrity_warning = false;
        match engine.verify_vault_meta_integrity(&master_key) {
            Ok(result) => {
                if !result.valid {
                    integrity_warning = true;
                    log::warn!("Disaster recovery: HMAC mismatch — {}", result.message);
                }
            }
            Err(e) => {
                integrity_warning = true;
                log::warn!("Disaster recovery: integrity check error: {}", e);
            }
        }

        let exports = engine.disaster_recovery_export(&master_key).map_err(|e| {
            log::warn!("Disaster recovery: export failed: {}", e);
            e.to_string()
        })?;

        let entry_count = exports.len() as u32;

        let json = serde_json::to_string_pretty(&exports).map_err(|e| e.to_string())?;
        std::fs::write(&export_path, &json).map_err(|e| e.to_string())?;

        log::info!(
            "Disaster recovery export: {} entries to {}",
            entry_count,
            export_path
        );

        Ok(DisasterRecoveryResult {
            file_path: export_path,
            entry_count,
            integrity_warning,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
