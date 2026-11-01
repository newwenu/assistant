use rusqlite::{params, OptionalExtension};

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_crypto::vault_crypto::VaultCrypto;

use crate::audit::{log as audit_log, AuditEvent};
use crate::dto::VaultEntrySummary;
use crate::security_db::SecurityDbState;
use crate::types::{VaultCategory, VaultEntryImport, VaultEntrySnapshot, VaultError};

use super::field_crypto::{encrypt_custom_fields, encrypt_opt};
use super::query_ids;
use super::snapshot::read_snapshot;

impl SecurityDbState {
    pub fn list_deleted_entry_ids(&self) -> Result<Vec<String>, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;
        query_ids(
            conn,
            "SELECT id FROM vault_entries WHERE deleted_at IS NOT NULL ORDER BY created_at",
        )
    }

    pub fn list_deleted_entries(&self) -> Result<Vec<VaultEntrySummary>, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

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

    pub fn restore_entry_tx<F>(&self, id: &str, export: F) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let now = chrono::Utc::now().timestamp_millis();
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

            let snapshot = read_snapshot(conn, mk, id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(conn, AuditEvent::VaultEntryRestored, Some(id));
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
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let now = chrono::Utc::now().timestamp_millis();
            let entry_key = VaultCrypto::derive_entry_key(mk, &entry.id);

            let encrypted_password =
                encrypt_opt(entry.password.as_deref(), entry_key.expose(), algo)?;
            let encrypted_notes = encrypt_opt(entry.notes.as_deref(), entry_key.expose(), algo)?;
            let encrypted_totp = encrypt_opt(entry.totp_secret.as_deref(), entry_key.expose(), algo)?;
            let encrypted_custom_fields =
                encrypt_custom_fields(&entry.custom_fields, entry_key.expose(), algo)?;
            let tags_json =
                serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

            let existing: Option<i64> = conn
                .query_row(
                    "SELECT revision FROM vault_entries WHERE id = ?1",
                    params![entry.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let revision = match existing {
                Some(r) => r as u64 + 1,
                None => {
                    let max_revision: i64 = conn
                        .query_row(
                            "SELECT COALESCE(MAX(revision), 0) FROM vault_entries",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(|e| VaultError::Database(e.to_string()))?;
                    max_revision.max(entry.revision as i64) as u64 + 1
                }
            };

            if existing.is_some() {
                conn.execute(
                    "UPDATE vault_entries SET title = ?1, username = ?2, url = ?3, category = ?4, \
                     encrypted_password = ?5, encrypted_notes = ?6, encrypted_custom_fields = ?7, \
                     encrypted_totp_secret = ?8, totp_digits = ?9, totp_period = ?10, tags = ?11, \
                     is_favorite = ?12, revision = ?13, updated_at = ?14, deleted_at = NULL \
                     WHERE id = ?15",
                    params![
                        entry.title,
                        entry.username,
                        entry.url,
                        entry.category.to_string(),
                        encrypted_password,
                        encrypted_notes,
                        encrypted_custom_fields,
                        encrypted_totp,
                        entry.totp_digits as i64,
                        entry.totp_period as i64,
                        tags_json,
                        entry.is_favorite as i64,
                        revision as i64,
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
                      encrypted_custom_fields, encrypted_totp_secret, totp_digits, totp_period, tags, \
                      is_favorite, revision, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
                        entry.totp_digits as i64,
                        entry.totp_period as i64,
                        tags_json,
                        entry.is_favorite as i64,
                        revision as i64,
                        created_at,
                        now,
                    ],
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;
            }

            let snapshot = read_snapshot(conn, mk, &entry.id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(
                conn,
                AuditEvent::VaultEntryRestored,
                Some(&format!("{}@{}", entry.id, source)),
            );
            Ok(snapshot)
        })
    }

    pub fn purge_deleted(&self) -> Result<u32, VaultError> {
        self.with_transaction(|conn, _mk| {
            let purged = conn
                .execute("DELETE FROM vault_entries WHERE deleted_at IS NOT NULL", [])
                .map_err(|e| VaultError::Database(e.to_string()))?;
            if purged > 0 {
                let _ = audit_log(
                    conn,
                    AuditEvent::VaultEntryPurged,
                    Some(&purged.to_string()),
                );
            }
            Ok(purged as u32)
        })
    }
}
