mod deleted;
pub(super) mod field_crypto;
mod import;
mod query;
mod snapshot;
mod tx;
mod write;

use std::fs;

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};
use assistant_sync::types::SyncError;
use rusqlite::Connection;

use crate::types::{CustomFieldInput, VaultCategory, VaultEntryImport, VaultError};
use crate::vault_file::types::ImportedEntry;
use crate::vault_file::VaultFileManager;

pub(super) fn query_ids(conn: &Connection, sql: &str) -> Result<Vec<String>, VaultError> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| VaultError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| VaultError::Database(e.to_string()))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(ids)
}

pub(super) fn rebuild_index(
    conn: &Connection,
    repo: &VaultFileManager,
    dek: &Dek,
    algo: CipherAlgorithm,
) -> Result<usize, VaultError> {
    let dir = repo.entries_dir();
    if !dir.exists() {
        return Ok(0);
    }

    let mut ids = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("enc") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            ids.push(stem.to_string());
        }
    }
    ids.sort();

    let mut count = 0usize;
    for id in ids {
        let imported = repo
            .import_entry(dek, &id)
            .map_err(sync_error_to_vault_error)?;
        let entry = imported_to_import(&imported)?;
        import::write_imported_entry_in(conn, dek, &entry, imported.revision, algo)?;
        count += 1;
    }
    Ok(count)
}

fn imported_to_import(imported: &ImportedEntry) -> Result<VaultEntryImport, VaultError> {
    let category = VaultCategory::parse(&imported.category).ok_or_else(|| {
        VaultError::EntryCorrupted(format!(
            "unknown category {} in entry file {}",
            imported.category, imported.id
        ))
    })?;
    let blob = &imported.blob;
    Ok(VaultEntryImport {
        id: imported.id.clone(),
        category,
        revision: imported.revision,
        title: blob.title.clone(),
        username: blob.username.clone(),
        url: blob.url.clone(),
        password: blob.password.clone(),
        notes: blob.notes.clone(),
        totp_secret: blob.totp.as_ref().map(|totp| totp.to_stored_payload()),
        custom_fields: blob
            .custom_fields
            .iter()
            .map(|field| CustomFieldInput {
                name: field.name.clone(),
                value: field.value.clone(),
                is_protected: field.is_secret,
            })
            .collect(),
        tags: blob.tags.clone(),
        is_favorite: blob.is_favorite,
        created_at: blob.created_at,
        updated_at: blob.updated_at,
    })
}

fn sync_error_to_vault_error(err: SyncError) -> VaultError {
    match err {
        SyncError::Io(msg) => VaultError::Io(std::io::Error::other(msg)),
        SyncError::EntryCorrupted(msg) => VaultError::EntryCorrupted(msg),
        SyncError::UnsupportedEntryFileVersion(msg) => VaultError::VaultCorrupted(msg),
        SyncError::VaultKeyMismatch(msg) => VaultError::Crypto(msg),
        other => VaultError::Other(other.to_string()),
    }
}
