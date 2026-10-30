use std::path::PathBuf;

use tauri::{command, AppHandle};

use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{
    VaultEntryCreate, VaultEntryDetail, VaultEntrySummary, VaultEntryUpdate, VaultErrorDto,
};
use assistant_vault::types::{VaultEntryCsvRow, VaultEntryExport, VaultEntrySnapshot, VaultError};

use assistant_crypto::vault_crypto::types::MasterKey;
use assistant_vault::sync_engine::SyncEngine;

fn export_entry_to_sync(
    engine: &SyncEngine,
    master_key: &MasterKey,
    snapshot: &VaultEntrySnapshot,
) -> Result<(), VaultError> {
    engine
        .vault_file_manager()
        .export_entry(master_key, snapshot, false)
        .map_err(|e| VaultError::Other(e.to_string()))
}

fn prepare_sync(engine: &SyncEngine) {
    if let Err(e) = engine.prepare() {
        log::warn!("Vault sync prepare failed: {}", e);
    }
}

fn finalize_sync(engine: &SyncEngine, msg: &str) {
    if let Err(e) = engine.finalize(msg) {
        log::warn!("Vault sync finalize failed ({}): {}", msg, e);
    }
}

// precondition: Unlocked
#[command]
pub fn vault_list_entries(
    app: AppHandle,
    category: Option<String>,
    search: Option<String>,
    tag: Option<String>,
) -> Result<Vec<VaultEntrySummary>, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let cat = category.and_then(|c| assistant_vault::types::VaultCategory::parse(&c));
        state
            .list_entries(cat, search.as_deref(), tag.as_deref())
            .map_err(VaultErrorDto::from)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_get_entry(app: AppHandle, id: String) -> Result<VaultEntryDetail, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        state.get_entry(&id).map_err(VaultErrorDto::from)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_create_entry(
    app: AppHandle,
    entry: VaultEntryCreate,
) -> Result<String, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .create_entry_tx(entry, super::cipher_algo(), |mk, snap| {
                export_entry_to_sync(&engine, mk, snap)
            })
            .map_err(VaultErrorDto::from)?;

        log::info!("Vault entry created: {}", snapshot.id);
        super::ssh::auto_add_entry(&app, &snapshot.id);
        finalize_sync(&engine, &format!("vault: create {}", snapshot.id));
        Ok(snapshot.id)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_update_entry(
    app: AppHandle,
    id: String,
    entry: VaultEntryUpdate,
) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .update_entry_tx(&id, entry, super::cipher_algo(), |mk, snap| {
                export_entry_to_sync(&engine, mk, snap)
            })
            .map_err(VaultErrorDto::from)?;

        log::info!("Vault entry updated: {}", snapshot.id);
        finalize_sync(&engine, &format!("vault: update {}", snapshot.id));
        Ok(())
    })
}

// precondition: Unlocked
#[command]
pub fn vault_delete_entry(app: AppHandle, id: String) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let fingerprint = super::ssh::entry_fingerprint(&app, &id);

        state
            .delete_entry_tx(&id, |entry_id| {
                engine
                    .vault_file_manager()
                    .remove_entry_file(entry_id)
                    .map_err(|e| VaultError::Other(e.to_string()))
            })
            .map_err(VaultErrorDto::from)?;

        if let Some(fp) = fingerprint {
            super::ssh::auto_remove_fingerprint(&app, &fp);
        }

        log::info!("Vault entry deleted: {}", id);
        finalize_sync(&engine, &format!("vault: delete {}", id));
        Ok(())
    })
}

// precondition: Unlocked
#[command]
pub fn vault_list_deleted_entries(app: AppHandle) -> Result<Vec<VaultEntrySummary>, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        state.list_deleted_entries().map_err(VaultErrorDto::from)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_restore_entry(app: AppHandle, id: String) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .restore_entry_tx(&id, |mk, snap| export_entry_to_sync(&engine, mk, snap))
            .map_err(VaultErrorDto::from)?;

        log::info!("Vault entry restored: {}", snapshot.id);
        finalize_sync(&engine, &format!("vault: restore {}", snapshot.id));
        Ok(())
    })
}

// precondition: Unlocked
#[command]
pub fn vault_purge_deleted(app: AppHandle) -> Result<u32, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let ids = state
            .list_deleted_entry_ids()
            .map_err(VaultErrorDto::from)?;
        if ids.is_empty() {
            return Ok(0);
        }

        for id in &ids {
            engine
                .vault_file_manager()
                .remove_entry_file(id)
                .map_err(|e| {
                    super::internal_error(format!(
                        "Failed to remove entry file {} while purging: {}",
                        id, e
                    ))
                })?;
        }

        let purged = state.purge_deleted().map_err(VaultErrorDto::from)?;

        log::info!("Vault purged {} deleted entries", purged);
        if purged > 0 {
            finalize_sync(&engine, &format!("vault: purge {} entries", purged));
        }
        Ok(purged)
    })
}

// precondition: Unlocked
#[command]
pub async fn vault_change_master_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |state| {
            state
                .change_master_password(&current, &new, super::cipher_algo())
                .map_err(VaultErrorDto::from)?;

            {
                let key_store = super::biometric::get_key_store();
                if state
                    .is_biometric_enrolled(key_store.as_ref())
                    .unwrap_or(false)
                {
                    let _ = state.disenroll_biometric(key_store.as_ref());
                }
            }

            {
                let engine = super::sync_state(&app)?;
                let master_key = state.get_master_key().map_err(VaultErrorDto::from)?;
                let mk = master_key
                    .as_ref()
                    .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;

                if let Err(e) = engine.clear_emergency_data(mk) {
                    log::warn!(
                        "Failed to clear emergency data after master password change: {}",
                        e
                    );
                }
            }

            {
                let engine = super::sync_state(&app)?;
                if let Err(e) = engine.export_all_entries(state, true, &[]) {
                    log::warn!(
                        "Failed to re-export vault entries after master password change: {}",
                        e
                    );
                }
                finalize_sync(&engine, "vault: master password changed");
            }

            {
                let guard = state.get_conn().map_err(VaultErrorDto::from)?;
                if let Some(conn) = guard.as_ref() {
                    let _ = audit_log(conn, AuditEvent::RecoveryKeyInvalidated, None);
                }
            }

            log::info!("Vault master password changed");
            Ok(())
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault task join failed: {}", e)))?
}

// precondition: Unlocked
#[command]
pub async fn vault_export_to_file(
    app: AppHandle,
    format: String,
    path: String,
) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |state| {
            let entries: Vec<VaultEntryExport> =
                state.export_entries().map_err(VaultErrorDto::from)?;
            log::info!(
                "Vault export to file: {} entries as {} -> {}",
                entries.len(),
                format,
                path
            );

            let content = match format.as_str() {
                "json" => serde_json::to_string_pretty(&entries)
                    .map_err(|e| {
                        super::internal_error(format!("Failed to serialize entries: {}", e))
                    })?
                    .into_bytes(),
                "csv" => {
                    let mut w = csv::Writer::from_writer(Vec::new());
                    for e in &entries {
                        let row: VaultEntryCsvRow = e.clone().into();
                        w.serialize(&row).map_err(|e| {
                            super::internal_error(format!("Failed to write CSV row: {}", e))
                        })?;
                    }
                    w.into_inner().map_err(|e| {
                        super::internal_error(format!("Failed to finalize CSV: {}", e))
                    })?
                }
                _ => {
                    return Err(super::validation_error(format!(
                        "Unsupported export format: {}",
                        format
                    )))
                }
            };

            std::fs::write(&path, &content).map_err(|e| VaultErrorDto::from(VaultError::Io(e)))
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault task join failed: {}", e)))?
}

// precondition: Unlocked
#[command]
pub async fn vault_import(
    app: AppHandle,
    path: String,
    format: String,
) -> Result<u32, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |state| {
            let file_path = PathBuf::from(&path);
            let data = std::fs::read_to_string(&file_path)
                .map_err(|e| VaultErrorDto::from(VaultError::Io(e)))?;

            let entries: Vec<VaultEntryCreate> = match format.as_str() {
                "json" => serde_json::from_str(&data).map_err(|e| {
                    super::validation_error(format!("Failed to parse JSON import file: {}", e))
                })?,
                "csv" => {
                    let mut reader = csv::Reader::from_reader(data.as_bytes());
                    let mut result = Vec::new();
                    for record in reader.deserialize() {
                        let row: VaultEntryCsvRow = record.map_err(|e| {
                            super::validation_error(format!("Failed to parse CSV record: {}", e))
                        })?;
                        result.push(row.into());
                    }
                    result
                }
                _ => {
                    return Err(super::validation_error(format!(
                        "Unsupported import format: {}",
                        format
                    )))
                }
            };

            let result = state
                .import_entries(entries, super::cipher_algo())
                .map_err(VaultErrorDto::from)?;
            log::info!(
                "Vault import: {} entries from {} ({})",
                result,
                path,
                format
            );
            Ok(result)
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault task join failed: {}", e)))?
}
