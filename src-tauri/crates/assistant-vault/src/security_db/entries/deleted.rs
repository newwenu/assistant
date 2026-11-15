use rusqlite::{params, Connection, OptionalExtension};

use assistant_crypto::vault_crypto::derive_entry_key;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};

use crate::audit::{log as audit_log, AuditEvent};
use crate::dto::VaultEntrySummary;
use crate::security_db::SecurityDbState;
use crate::types::{VaultCategory, VaultEntryImport, VaultEntrySnapshot, VaultError};

use super::field_crypto::{
    encrypt_custom_fields, encrypt_opt, FIELD_NOTES, FIELD_PASSWORD, FIELD_TOTP,
};
use super::query_ids;
use super::snapshot::read_snapshot;

impl SecurityDbState {
    pub fn list_deleted_entry_ids(&self) -> Result<Vec<String>, VaultError> {
        self.with_conn(|conn| {
            query_ids(
                conn,
                "SELECT id FROM vault_entries WHERE deleted_at IS NOT NULL ORDER BY created_at",
            )
        })
    }

    pub fn list_deleted_entries(&self) -> Result<Vec<VaultEntrySummary>, VaultError> {
        self.with_conn(list_deleted_entries_in)
    }

    pub fn restore_entry_tx<F>(&self, id: &str, export: F) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            let now = crate::types::now_millis();
            let affected = conn
                .execute(
                    "UPDATE vault_entries SET deleted_at = NULL, revision = revision + 1, \
                     updated_at = ?1 WHERE id = ?2 AND deleted_at IS NOT NULL",
                    params![now, id],
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;

            if affected == 0 {
                return Err(VaultError::NotFound(id.to_string()));
            }

            let snapshot = read_snapshot(conn, dek, dek_version, id)?;
            export(dek, &snapshot)?;
            audit_log(conn, AuditEvent::VaultEntryRestored, Some(id));
            Ok(snapshot)
        })
    }

    pub fn restore_from_history_tx<F>(
        &self,
        entry: VaultEntryImport,
        algo: CipherAlgorithm,
        source: &str,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            let now = crate::types::now_millis();
            let entry_key = derive_entry_key(dek, &entry.id);

            let encrypted_password = encrypt_opt(
                entry.password.as_deref(),
                &entry_key,
                FIELD_PASSWORD,
                &entry.id,
                algo,
            )?;
            let encrypted_notes = encrypt_opt(
                entry.notes.as_deref(),
                &entry_key,
                FIELD_NOTES,
                &entry.id,
                algo,
            )?;
            let encrypted_totp = encrypt_opt(
                entry.totp_secret.as_deref(),
                &entry_key,
                FIELD_TOTP,
                &entry.id,
                algo,
            )?;
            let encrypted_custom_fields =
                encrypt_custom_fields(&entry.custom_fields, &entry_key, &entry.id, algo)?;
            let tags_json = serde_json::to_string(&entry.tags)
                .map_err(|e| VaultError::Validation(e.to_string()))?;

            let existing: Option<i64> = conn
                .query_row(
                    "SELECT revision FROM vault_entries WHERE id = ?1",
                    params![entry.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let revision = match existing {
                Some(r) => r + 1,
                None => {
                    let max_revision: i64 = conn
                        .query_row(
                            "SELECT COALESCE(MAX(revision), 0) FROM vault_entries",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(|e| VaultError::Database(e.to_string()))?;
                    max_revision.max(entry.revision) + 1
                }
            };

            if existing.is_some() {
                conn.execute(
                    "UPDATE vault_entries SET title = ?1, username = ?2, url = ?3, category = ?4, \
                     encrypted_password = ?5, encrypted_notes = ?6, encrypted_custom_fields = ?7, \
                     encrypted_totp_secret = ?8, tags = ?9, \
                     is_favorite = ?10, revision = ?11, updated_at = ?12, deleted_at = NULL \
                     WHERE id = ?13",
                    params![
                        entry.title,
                        entry.username,
                        entry.url,
                        entry.category.to_string(),
                        encrypted_password,
                        encrypted_notes,
                        encrypted_custom_fields,
                        encrypted_totp,
                        tags_json,
                        i64::from(entry.is_favorite),
                        revision,
                        now,
                        entry.id,
                    ],
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;
            } else {
                let created_at = if entry.created_at > 0 {
                    entry.created_at
                } else {
                    now
                };
                conn.execute(
                    "INSERT INTO vault_entries \
                     (id, title, username, url, category, encrypted_password, encrypted_notes, \
                      encrypted_custom_fields, encrypted_totp_secret, tags, \
                      is_favorite, revision, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        entry.id,
                        entry.title,
                        entry.username,
                        entry.url,
                        entry.category.to_string(),
                        encrypted_password,
                        encrypted_notes,
                        encrypted_custom_fields,
                        encrypted_totp,
                        tags_json,
                        i64::from(entry.is_favorite),
                        revision,
                        created_at,
                        now,
                    ],
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;
            }

            let snapshot = read_snapshot(conn, dek, dek_version, &entry.id)?;
            export(dek, &snapshot)?;
            audit_log(
                conn,
                AuditEvent::VaultEntryRestored,
                Some(&format!("{}@{}", entry.id, source)),
            );
            Ok(snapshot)
        })
    }

    pub fn purge_deleted(&self) -> Result<u32, VaultError> {
        self.with_transaction(|conn, _session| {
            let purged = conn
                .execute("DELETE FROM vault_entries WHERE deleted_at IS NOT NULL", [])
                .map_err(|e| VaultError::Database(e.to_string()))?;
            if purged > 0 {
                audit_log(
                    conn,
                    AuditEvent::VaultEntryPurged,
                    Some(&purged.to_string()),
                );
            }
            u32::try_from(purged)
                .map_err(|_| VaultError::Validation("purged entry count exceeds u32".to_string()))
        })
    }
}

fn list_deleted_entries_in(conn: &Connection) -> Result<Vec<VaultEntrySummary>, VaultError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, username, url, category, tags, is_favorite, updated_at \
             FROM vault_entries WHERE deleted_at IS NOT NULL ORDER BY updated_at DESC",
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let rows = stmt
        .query_map([], |row| {
            let category_str: String = row.get("category")?;
            let tags_json: String = row.get("tags")?;
            Ok(VaultEntrySummary {
                id: row.get("id")?,
                category: VaultCategory::parse(&category_str).unwrap_or_default(),
                title: row.get("title")?,
                username: row.get("username")?,
                url: row.get("url")?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                is_favorite: row.get::<_, i64>("is_favorite")? != 0,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(entries)
}
