use rusqlite::{params, Connection, OptionalExtension};

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_crypto::vault_crypto::VaultCrypto;

use crate::audit::{log as audit_log, AuditEvent};
use crate::security_db::SecurityDbState;
use crate::types::{ImportOutcome, VaultEntryImport, VaultEntrySnapshot, VaultError};

use super::field_crypto::{encrypt_custom_fields, encrypt_opt};
use super::snapshot::read_snapshot;

impl SecurityDbState {
    pub fn force_import_entry_tx<F>(
        &self,
        entry: VaultEntryImport,
        algo: CipherAlgorithm,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let local_revision: Option<i64> = conn
                .query_row(
                    "SELECT revision FROM vault_entries WHERE id = ?1",
                    params![entry.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let revision = match local_revision {
                Some(local) => (local as u64).max(entry.revision) + 1,
                None => entry.revision + 1,
            };

            write_imported_entry_in(conn, mk, &entry, revision, algo)?;
            let snapshot = read_snapshot(conn, mk, &entry.id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(&entry.id));
            Ok(snapshot)
        })
    }

    pub fn create_remote_copy_tx<F>(
        &self,
        entry: VaultEntryImport,
        algo: CipherAlgorithm,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let max_revision: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(revision), 0) FROM vault_entries",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let revision = max_revision.max(entry.revision as i64) as u64 + 1;

            write_imported_entry_in(conn, mk, &entry, revision, algo)?;
            let snapshot = read_snapshot(conn, mk, &entry.id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(conn, AuditEvent::VaultEntryCreated, Some(&entry.id));
            Ok(snapshot)
        })
    }

    pub fn upsert_imported_entries(
        &self,
        entries: Vec<VaultEntryImport>,
        algo: CipherAlgorithm,
    ) -> Result<Vec<ImportOutcome>, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let mut outcomes = Vec::with_capacity(entries.len());
            for entry in entries {
                outcomes.push(upsert_imported_in(&tx, mk, entry, algo)?);
            }
            tx.commit()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            Ok(outcomes)
        })
    }
}

fn write_imported_entry_in(
    conn: &Connection,
    mk: &MasterKey,
    entry: &VaultEntryImport,
    revision: u64,
    algo: CipherAlgorithm,
) -> Result<(), VaultError> {
    let entry_key = VaultCrypto::derive_entry_key(mk, &entry.id);
    let encrypted_password = encrypt_opt(entry.password.as_deref(), entry_key.expose(), algo)?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), entry_key.expose(), algo)?;
    let encrypted_totp = encrypt_opt(entry.totp_secret.as_deref(), entry_key.expose(), algo)?;
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, entry_key.expose(), algo)?;
    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    let now = chrono::Utc::now().timestamp_millis();

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM vault_entries WHERE id = ?1",
            params![entry.id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| VaultError::Database(e.to_string()))?
        .is_some();

    if exists {
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

    Ok(())
}

fn upsert_imported_in(
    conn: &Connection,
    mk: &MasterKey,
    entry: VaultEntryImport,
    algo: CipherAlgorithm,
) -> Result<ImportOutcome, VaultError> {
    let existing: Option<(Option<i64>, i64, i64)> = conn
        .query_row(
            "SELECT deleted_at, revision, updated_at FROM vault_entries WHERE id = ?1",
            params![entry.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let entry_key = VaultCrypto::derive_entry_key(mk, &entry.id);
    let encrypted_password = encrypt_opt(entry.password.as_deref(), entry_key.expose(), algo)?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), entry_key.expose(), algo)?;
    let encrypted_totp = encrypt_opt(entry.totp_secret.as_deref(), entry_key.expose(), algo)?;
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, entry_key.expose(), algo)?;
    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    match existing {
        Some((Some(_), _, _)) => Ok(ImportOutcome::SkippedLocallyDeleted),
        Some((None, revision, updated_at)) => {
            if entry.revision < revision as u64
                || (entry.revision == revision as u64 && entry.updated_at <= updated_at)
            {
                return Ok(ImportOutcome::SkippedStale);
            }

            conn.execute(
                "UPDATE vault_entries SET title = ?1, username = ?2, url = ?3, category = ?4, \
                 encrypted_password = ?5, encrypted_notes = ?6, encrypted_custom_fields = ?7, \
                 encrypted_totp_secret = ?8, totp_digits = ?9, totp_period = ?10, tags = ?11, \
                 is_favorite = ?12, revision = ?13, updated_at = ?14 \
                 WHERE id = ?15 AND deleted_at IS NULL",
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
                    entry.revision as i64,
                    entry.updated_at,
                    entry.id,
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;

            Ok(ImportOutcome::Updated)
        }
        None => {
            let now = chrono::Utc::now().timestamp_millis();
            let created_at = if entry.created_at > 0 {
                entry.created_at
            } else {
                now
            };
            let updated_at = if entry.updated_at > 0 {
                entry.updated_at
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
                    entry.revision as i64,
                    created_at,
                    updated_at,
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;

            let _ = audit_log(conn, AuditEvent::VaultEntryCreated, Some(&entry.id));
            Ok(ImportOutcome::Inserted)
        }
    }
}
