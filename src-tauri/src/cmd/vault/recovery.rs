use tauri::{command, AppHandle, Manager};

use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::sync_engine::SyncEngine;

#[command]
pub fn vault_generate_recovery_key(app: AppHandle) -> Result<String, String> {
    let state = super::vault_state(&app)?;
    let master_key = state.get_master_key().map_err(|e| e.to_string())?;
    let mk = master_key.as_ref().ok_or("Vault is locked")?;
    let bootstrap_key = state.get_bootstrap_key().map_err(|e| e.to_string())?;
    let bk = bootstrap_key.as_ref().ok_or("Vault is locked")?;

    let (recovery_key, emergency) =
        assistant_vault::recovery_key::generate_recovery_key(mk, bk).map_err(|e| e.to_string())?;

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

    let emergency_data = assistant_vault::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let result = assistant_vault::recovery_key::unlock_with_recovery_key(
        &recovery_key,
        &emergency_data,
        &state,
    )
    .map_err(|e| {
        log::warn!("Vault unlock with recovery key failed: {}", e);
        e.to_string()
    })?;

    if result {
        let mut integrity_warning: Option<String> = None;
        if let Ok(master_key) = state.get_master_key() {
            if let Some(mk) = master_key.as_ref() {
                match engine.verify_vault_meta_integrity(mk) {
                    Ok(integrity) => {
                        if !integrity.valid {
                            log::warn!(
                                "Recovery key unlock: vault meta integrity mismatch — {}",
                                integrity.message
                            );
                            integrity_warning = Some(integrity.message);
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Recovery key unlock: vault meta integrity check failed: {}",
                            e
                        );
                        integrity_warning = Some(e.to_string());
                    }
                }
            }
        }
        if let Some(message) = integrity_warning {
            crate::protocol::events::emit_broadcast(
                &app,
                crate::protocol::events::event_name::VAULT_META_INTEGRITY_WARNING,
                crate::protocol::events::VaultMetaIntegrityWarningPayload { message },
            );
        }

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
) -> Result<assistant_vault::vault_file::IntegrityResult, String> {
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
    pub skipped_ids: Vec<String>,
}

const EMERGENCY_META_FILE: &str = ".vault-meta";

fn emergency_repo_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    Ok(data_dir.join("security").join("recovery"))
}

fn prepare_emergency_repo(dir: &std::path::Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .map_err(|e| format!("Failed to clear emergency repo {}: {}", dir.display(), e))?;
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create emergency repo {}: {}", dir.display(), e))
}

fn export_from_engine(
    engine: &SyncEngine,
    recovery_key: &str,
    export_path: &str,
) -> Result<DisasterRecoveryResult, String> {
    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    let emergency = emergency.ok_or("Recovery key not set in .vault-meta")?;

    let emergency_data = assistant_vault::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let (_bootstrap_key, master_key) =
        assistant_vault::recovery_key::decrypt_recovery_keys(recovery_key, &emergency_data)
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

    let (exports, skipped_ids) = engine.disaster_recovery_export(&master_key).map_err(|e| {
        log::warn!("Disaster recovery: export failed: {}", e);
        e.to_string()
    })?;

    let entry_count = exports.len() as u32;
    let json = serde_json::to_string_pretty(&exports).map_err(|e| e.to_string())?;
    std::fs::write(export_path, &json).map_err(|e| e.to_string())?;

    log::info!(
        "Disaster recovery export: {} entries to {} ({} skipped)",
        entry_count,
        export_path,
        skipped_ids.len()
    );

    Ok(DisasterRecoveryResult {
        file_path: export_path.to_string(),
        entry_count,
        integrity_warning,
        skipped_ids,
    })
}

#[command]
pub async fn vault_disaster_recovery_export(
    app: AppHandle,
    git_url: Option<String>,
    recovery_key: String,
    export_path: String,
    commit: Option<String>,
) -> Result<DisasterRecoveryResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = git_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let commit = commit
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        match url {
            Some(url) => {
                let repo_dir = emergency_repo_dir(&app)?;
                prepare_emergency_repo(&repo_dir)?;
                log::info!(
                    "Disaster recovery: cloning into emergency repo {}",
                    repo_dir.display()
                );

                let engine = SyncEngine::new(
                    repo_dir.clone(),
                    assistant_sync::SyncConfig {
                        remote_url: Some(url.clone()),
                        ..Default::default()
                    },
                );

                engine.clone_recovery_repo(&url).map_err(|e| {
                    log::warn!("Disaster recovery: clone failed: {}", e);
                    let _ = std::fs::remove_dir_all(&repo_dir);
                    format!("Failed to clone vault repository: {}", e)
                })?;

                if let Some(commit) = commit.as_deref() {
                    engine.checkout_recovery_repo(commit).map_err(|e| {
                        log::warn!("Disaster recovery: checkout {} failed: {}", commit, e);
                        let _ = std::fs::remove_dir_all(&repo_dir);
                        format!("Failed to checkout commit {}: {}", commit, e)
                    })?;
                }

                if !repo_dir.join(EMERGENCY_META_FILE).exists() {
                    let _ = std::fs::remove_dir_all(&repo_dir);
                    return Err(
                        "Cloned repository is not a vault repository: .vault-meta missing"
                            .to_string(),
                    );
                }

                export_from_engine(&engine, &recovery_key, &export_path)
            }
            None => {
                log::info!("Disaster recovery: reading local vault without git operations");
                let engine = app.state::<SyncEngine>();
                export_from_engine(&engine, &recovery_key, &export_path)
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
