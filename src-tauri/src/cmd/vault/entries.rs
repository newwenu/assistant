use std::path::PathBuf;

use tauri::{command, AppHandle, Manager};

use assistant_security::audit::{log as audit_log, AuditEvent};
use assistant_security::types::{
    VaultEntryCreate, VaultEntryCsvRow, VaultEntryDetail, VaultEntryExport, VaultEntrySummary,
    VaultEntryUpdate,
};

use super::SecurityDbState;
use assistant_sync::sync_engine::SyncEngine;

#[command]
pub fn vault_list_entries(
    app: AppHandle,
    category: Option<String>,
    search: Option<String>,
    tag: Option<String>,
) -> Result<Vec<VaultEntrySummary>, String> {
    let state = super::vault_state(&app)?;
    let cat = category.and_then(|c| assistant_security::types::VaultCategory::parse(&c));
    state
        .list_entries(cat, search.as_deref(), tag.as_deref())
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_get_entry(app: AppHandle, id: String) -> Result<VaultEntryDetail, String> {
    let state = super::vault_state(&app)?;
    state.get_entry(&id).map_err(|e| e.to_string())
}

#[command]
pub fn vault_create_entry(app: AppHandle, entry: VaultEntryCreate) -> Result<String, String> {
    let state = super::vault_state(&app)?;
    let id = state
        .create_entry(entry, super::cipher_algo())
        .map_err(|e| e.to_string())?;
    log::info!("Vault entry created: {}", id);
    Ok(id)
}

#[command]
pub fn vault_update_entry(
    app: AppHandle,
    id: String,
    entry: VaultEntryUpdate,
) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    state
        .update_entry(&id, entry, super::cipher_algo())
        .map_err(|e| e.to_string())?;
    log::info!("Vault entry updated: {}", id);
    Ok(())
}

#[command]
pub fn vault_delete_entry(app: AppHandle, id: String) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    state.delete_entry(&id).map_err(|e| e.to_string())?;
    log::info!("Vault entry deleted: {}", id);
    Ok(())
}

#[command]
pub async fn vault_change_master_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        state
            .change_master_password(&current, &new, super::cipher_algo())
            .map_err(|e| e.to_string())?;

        {
            let engine = app.state::<SyncEngine>();
            let master_key = state.get_master_key().map_err(|e| e.to_string())?;
            let mk = master_key.as_ref().ok_or("Vault is locked")?;

            if let Err(e) = engine.clear_emergency_data() {
                log::warn!(
                    "Failed to clear emergency data after master password change: {}",
                    e
                );
            }

            if let Err(e) = engine.rewrite_vault_meta_with_new_key(mk) {
                log::warn!(
                    "Failed to rewrite vault-meta HMAC after master password change: {}",
                    e
                );
            }
        }

        {
            let guard = state.get_conn().map_err(|e| e.to_string())?;
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::RecoveryKeyInvalidated, None);
            }
        }

        log::info!("Vault master password changed");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn vault_export_to_file(
    app: AppHandle,
    format: String,
    path: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        let entries: Vec<VaultEntryExport> = state.export_entries().map_err(|e| e.to_string())?;
        log::info!(
            "Vault export to file: {} entries as {} -> {}",
            entries.len(),
            format,
            path
        );

        let content = match format.as_str() {
            "json" => serde_json::to_string_pretty(&entries)
                .map_err(|e| e.to_string())?
                .into_bytes(),
            "csv" => {
                let mut w = csv::Writer::from_writer(Vec::new());
                for e in &entries {
                    let row: VaultEntryCsvRow = e.clone().into();
                    w.serialize(&row).map_err(|e| e.to_string())?;
                }
                w.into_inner().map_err(|e| e.to_string())?
            }
            _ => return Err(format!("Unsupported export format: {}", format)),
        };

        std::fs::write(&path, &content).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn vault_import(app: AppHandle, path: String, format: String) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        let file_path = PathBuf::from(&path);
        let data = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;

        let entries: Vec<VaultEntryCreate> = match format.as_str() {
            "json" => serde_json::from_str(&data).map_err(|e| e.to_string())?,
            "csv" => {
                let mut reader = csv::Reader::from_reader(data.as_bytes());
                let mut result = Vec::new();
                for record in reader.deserialize() {
                    let row: VaultEntryCsvRow = record.map_err(|e| e.to_string())?;
                    result.push(row.into());
                }
                result
            }
            _ => return Err(format!("Unsupported import format: {}", format)),
        };

        let result = state
            .import_entries(entries, super::cipher_algo())
            .map_err(|e| e.to_string())?;
        log::info!(
            "Vault import: {} entries from {} ({})",
            result,
            path,
            format
        );
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
