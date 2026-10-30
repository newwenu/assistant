use tauri::{command, AppHandle, Manager};

use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{DisasterRecoveryResultDto, IntegrityResult, VaultErrorDto};
use assistant_vault::sync_engine::SyncEngine;
use assistant_vault::types::VaultError;

// precondition: Unlocked
#[command]
pub fn vault_generate_recovery_key(app: AppHandle) -> Result<String, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let master_key = state.get_master_key().map_err(VaultErrorDto::from)?;
        let mk = master_key
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;
        let bootstrap_key = state.get_bootstrap_key().map_err(VaultErrorDto::from)?;
        let bk = bootstrap_key
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;

        let (recovery_key, emergency) =
            assistant_vault::recovery_key::generate_recovery_key(mk, bk)
                .map_err(VaultErrorDto::from)?;

        let engine = super::sync_state(&app)?;
        engine
            .write_emergency_data(mk, &emergency)
            .map_err(VaultErrorDto::from)?;

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::RecoveryKeyGenerated, None);
        }

        log::warn!("Recovery key generated and emergency data written");
        Ok(recovery_key)
    })
}

// precondition: AnyState
#[command]
pub fn vault_has_recovery_key(app: AppHandle) -> Result<bool, VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    let emergency = engine.read_emergency_data().map_err(VaultErrorDto::from)?;
    Ok(emergency.is_some())
}

// precondition: Unlocked
#[command]
pub fn vault_verify_vault_meta_integrity(app: AppHandle) -> Result<IntegrityResult, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let master_key = state.get_master_key().map_err(VaultErrorDto::from)?;
        let mk = master_key
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;

        let engine = super::sync_state(&app)?;
        engine
            .verify_vault_meta_integrity(mk)
            .map_err(VaultErrorDto::from)
    })
}

const EMERGENCY_META_FILE: &str = ".vault-meta";

fn emergency_repo_dir(app: &AppHandle) -> Result<std::path::PathBuf, VaultErrorDto> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| super::internal_error(format!("Failed to resolve app data dir: {}", e)))?;
    Ok(data_dir.join("security").join("recovery"))
}

fn prepare_emergency_repo(dir: &std::path::Path) -> Result<(), VaultErrorDto> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| {
            log::warn!("Failed to clear emergency repo {}: {}", dir.display(), e);
            VaultErrorDto::from(VaultError::Io(e))
        })?;
    }
    std::fs::create_dir_all(dir).map_err(|e| VaultErrorDto::from(VaultError::Io(e)))
}

fn export_from_engine(
    engine: &SyncEngine,
    recovery_key: &str,
    export_path: &str,
) -> Result<DisasterRecoveryResultDto, VaultErrorDto> {
    let emergency = engine.read_emergency_data().map_err(VaultErrorDto::from)?;
    let emergency = emergency.ok_or_else(|| VaultErrorDto::from(VaultError::RecoveryKeyNotSet))?;

    let emergency_data = assistant_vault::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let (_bootstrap_key, master_key) =
        assistant_vault::recovery_key::decrypt_recovery_keys(recovery_key, &emergency_data)
            .map_err(|e| {
                log::warn!("Disaster recovery: decrypt failed: {}", e);
                VaultErrorDto::from(e)
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
        VaultErrorDto::from(e)
    })?;

    let entry_count = exports.len() as u32;
    let json = serde_json::to_string_pretty(&exports).map_err(|e| {
        super::internal_error(format!("Failed to serialize recovery export: {}", e))
    })?;
    std::fs::write(export_path, &json).map_err(|e| VaultErrorDto::from(VaultError::Io(e)))?;

    log::info!(
        "Disaster recovery export: {} entries to {} ({} skipped)",
        entry_count,
        export_path,
        skipped_ids.len()
    );

    Ok(DisasterRecoveryResultDto {
        file_path: export_path.to_string(),
        entry_count,
        integrity_warning,
        skipped_ids,
    })
}

// precondition: AnyState
#[command]
pub async fn vault_disaster_recovery_export(
    app: AppHandle,
    git_url: Option<String>,
    recovery_key: String,
    export_path: String,
    commit: Option<String>,
) -> Result<DisasterRecoveryResultDto, VaultErrorDto> {
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
                    VaultErrorDto::from(e)
                })?;

                if let Some(commit) = commit.as_deref() {
                    engine.checkout_recovery_repo(commit).map_err(|e| {
                        log::warn!("Disaster recovery: checkout {} failed: {}", commit, e);
                        let _ = std::fs::remove_dir_all(&repo_dir);
                        VaultErrorDto::from(e)
                    })?;
                }

                if !repo_dir.join(EMERGENCY_META_FILE).exists() {
                    let _ = std::fs::remove_dir_all(&repo_dir);
                    return Err(VaultErrorDto::from(VaultError::NotInitialized));
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
    .map_err(|e| {
        super::internal_error(format!("Vault disaster recovery task join failed: {}", e))
    })?
}
