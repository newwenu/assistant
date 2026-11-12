use std::path::{Path, PathBuf};

use assistant_tools::os::path::{resolve_external_input, ExternalInputMode};

use tauri::{command, AppHandle};

use assistant_vault::audit::{log as audit_log, path_digest, AuditEvent};
use assistant_vault::dto::{
    ExportResultDto, VaultEntryCreate, VaultEntryDetail, VaultEntrySummary, VaultEntryUpdate,
    VaultErrorDto,
};
use assistant_vault::plaintext_export::{validate_export_target, verify_export_token};
use assistant_vault::types::{VaultEntryCsvRow, VaultEntryExport, VaultEntrySnapshot, VaultError};

use assistant_crypto::vault_crypto::types::Dek;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;

fn export_entry_to_sync(
    engine: &SyncEngine,
    dek: &Dek,
    snapshot: &VaultEntrySnapshot,
) -> Result<(), VaultError> {
    engine
        .vault_file_manager()
        .export_entry(dek, snapshot)
        .map_err(|e| VaultError::Other(e.to_string()))
}

fn sync_meta(engine: &SyncEngine, dek: &Dek) -> Result<(), VaultError> {
    assistant_vault::lifecycle::sync_write_meta(engine.vault_file_manager(), dek)?;
    Ok(())
}

fn refresh_anchor(state: &SecurityDbState, engine: &SyncEngine) -> Result<(), VaultError> {
    assistant_vault::lifecycle::refresh_anchor(state, engine.vault_file_manager())
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
pub fn vault_count_entries(app: AppHandle) -> Result<u32, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        state.count_entries().map_err(VaultErrorDto::from)
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
) -> Result<VaultEntryDetail, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .create_entry_tx(entry, super::cipher_algo(), |dek, snap| {
                export_entry_to_sync(&engine, dek, snap)?;
                sync_meta(&engine, dek)
            })
            .map_err(VaultErrorDto::from)?;

        refresh_anchor(state, &engine).map_err(VaultErrorDto::from)?;

        log::info!("Vault entry created: {}", snapshot.id);
        super::ssh::auto_add_entry(&app, &snapshot.id);
        finalize_sync(&engine, &format!("vault: create {}", snapshot.id));
        Ok(VaultEntryDetail::from(&snapshot))
    })
}

// precondition: Unlocked
#[command]
pub fn vault_update_entry(
    app: AppHandle,
    id: String,
    entry: VaultEntryUpdate,
) -> Result<VaultEntryDetail, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .update_entry_tx(&id, entry, super::cipher_algo(), |dek, snap| {
                export_entry_to_sync(&engine, dek, snap)?;
                sync_meta(&engine, dek)
            })
            .map_err(VaultErrorDto::from)?;

        refresh_anchor(state, &engine).map_err(VaultErrorDto::from)?;

        log::info!("Vault entry updated: {}", snapshot.id);
        finalize_sync(&engine, &format!("vault: update {}", snapshot.id));
        Ok(VaultEntryDetail::from(&snapshot))
    })
}

// precondition: Unlocked
#[command]
pub fn vault_set_favorite(
    app: AppHandle,
    id: String,
    is_favorite: bool,
) -> Result<VaultEntryDetail, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let engine = super::sync_state(&app)?;

        prepare_sync(&engine);

        let snapshot = state
            .set_favorite_tx(&id, is_favorite, |dek, snap| {
                export_entry_to_sync(&engine, dek, snap)?;
                sync_meta(&engine, dek)
            })
            .map_err(VaultErrorDto::from)?;

        refresh_anchor(state, &engine).map_err(VaultErrorDto::from)?;

        log::info!(
            "Vault entry favorite set: {} -> {}",
            snapshot.id,
            is_favorite
        );
        finalize_sync(&engine, &format!("vault: favorite {}", snapshot.id));
        Ok(VaultEntryDetail::from(&snapshot))
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
            .delete_entry_tx(&id, |dek, entry_id| {
                engine
                    .vault_file_manager()
                    .remove_entry_file(entry_id)
                    .map_err(|e| VaultError::Other(e.to_string()))?;
                sync_meta(&engine, dek)
            })
            .map_err(VaultErrorDto::from)?;

        refresh_anchor(state, &engine).map_err(VaultErrorDto::from)?;

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
            .restore_entry_tx(&id, |dek, snap| {
                export_entry_to_sync(&engine, dek, snap)?;
                sync_meta(&engine, dek)
            })
            .map_err(VaultErrorDto::from)?;

        refresh_anchor(state, &engine).map_err(VaultErrorDto::from)?;

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
            {
                let dek_guard = state.get_dek().map_err(VaultErrorDto::from)?;
                let dek = dek_guard
                    .as_ref()
                    .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;
                sync_meta(&engine, dek).map_err(VaultErrorDto::from)?;
            }
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
            let engine = super::sync_state(&app)?;
            assistant_vault::lifecycle::change_password(
                state,
                engine.vault_file_manager(),
                &current,
                &new,
            )
            .map_err(VaultErrorDto::from)?;

            log::info!("Vault master password changed");
            Ok(())
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault task join failed: {}", e)))?
}

fn resolve_export_target(path: &str) -> Result<PathBuf, VaultErrorDto> {
    let target =
        resolve_external_input(path, ExternalInputMode::Write).map_err(super::validation_error)?;

    let dirs = crate::app_dirs::AppDirs::resolve()
        .map_err(|e| super::internal_error(format!("Failed to resolve app dirs: {}", e)))?;
    let security = super::security_dir(&dirs);
    let repo = super::repo_dir(&dirs);

    validate_export_target(&target, &[security.as_path(), repo.as_path()])
        .map_err(VaultErrorDto::from)
}

fn write_private_file(target: &Path, content: &[u8]) -> Result<(), VaultError> {
    assistant_vault::plaintext_export::write_private_file(target, content)
}

// precondition: Unlocked
#[command]
pub async fn vault_export_to_file(
    app: AppHandle,
    format: String,
    path: String,
    confirm_token: String,
) -> Result<ExportResultDto, VaultErrorDto> {
    verify_export_token(&confirm_token).map_err(VaultErrorDto::from)?;

    tauri::async_runtime::spawn_blocking(move || {
        let target = resolve_export_target(&path)?;

        super::with_unlocked(&app, |state| {
            let entries: Vec<VaultEntryExport> =
                state.export_entries().map_err(VaultErrorDto::from)?;

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

            let digest = path_digest(&target.to_string_lossy());
            let audit_detail = format!("{}:{}:{}", format, entries.len(), digest);

            if let Err(err) = write_private_file(&target, &content) {
                if let Ok(guard) = state.get_conn() {
                    if let Some(conn) = guard.as_ref() {
                        let _ = audit_log(
                            conn,
                            AuditEvent::VaultEntriesExported,
                            Some(&format!("{}:failed", audit_detail)),
                        );
                    }
                }
                log::warn!("Vault export to file failed: {}", err);
                return Err(VaultErrorDto::from(err));
            }

            if let Ok(guard) = state.get_conn() {
                if let Some(conn) = guard.as_ref() {
                    let _ = audit_log(conn, AuditEvent::VaultEntriesExported, Some(&audit_detail));
                }
            }

            log::info!(
                "Vault export to file: {} entries as {} -> {}",
                entries.len(),
                format,
                target.display()
            );

            Ok(ExportResultDto {
                path: target.to_string_lossy().to_string(),
                count: entries.len() as u32,
            })
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
            let file_path = resolve_external_input(&path, ExternalInputMode::Read)
                .map_err(super::validation_error)?;
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

            let engine = super::sync_state(&app)?;
            prepare_sync(&engine);

            let result = state
                .import_entries(entries, super::cipher_algo(), |mk, snapshot| {
                    export_entry_to_sync(&engine, mk, snapshot)
                })
                .map_err(VaultErrorDto::from)?;

            if result > 0 {
                {
                    let dek_guard = state.get_dek().map_err(VaultErrorDto::from)?;
                    let dek = dek_guard
                        .as_ref()
                        .ok_or_else(|| VaultErrorDto::from(VaultError::Locked))?;
                    sync_meta(&engine, dek).map_err(VaultErrorDto::from)?;
                }
                finalize_sync(&engine, &format!("vault: import {} entries", result));
            }

            if let Ok(guard) = state.get_conn() {
                if let Some(conn) = guard.as_ref() {
                    let _ = audit_log(
                        conn,
                        AuditEvent::VaultEntriesImported,
                        Some(&format!(
                            "{}:{}:{}",
                            format,
                            result,
                            path_digest(&file_path.to_string_lossy())
                        )),
                    );
                }
            }

            log::info!(
                "Vault import: {} entries from {} ({})",
                result,
                file_path.display(),
                format
            );
            Ok(result)
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault task join failed: {}", e)))?
}
