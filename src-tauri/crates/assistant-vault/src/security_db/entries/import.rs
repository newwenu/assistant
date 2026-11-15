use rusqlite::{params, Connection, OptionalExtension};

use assistant_crypto::vault_crypto::derive_entry_key;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};

use crate::audit::{log as audit_log, AuditEvent};
use crate::security_db::SecurityDbState;
use crate::types::{ImportOutcome, VaultEntryImport, VaultEntrySnapshot, VaultError};

use super::field_crypto::{
    encrypt_custom_fields, encrypt_opt, FIELD_NOTES, FIELD_PASSWORD, FIELD_TOTP,
};
use super::snapshot::read_snapshot;

impl SecurityDbState {
    pub fn force_import_entry_tx<F>(
        &self,
        entry: VaultEntryImport,
        algo: CipherAlgorithm,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            let local_revision: Option<i64> = conn
                .query_row(
                    "SELECT revision FROM vault_entries WHERE id = ?1",
                    params![entry.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let revision = match local_revision {
                Some(local) => local.max(entry.revision) + 1,
                None => entry.revision + 1,
            };

            write_imported_entry_in(conn, dek, &entry, revision, algo)?;
            let snapshot = read_snapshot(conn, dek, dek_version, &entry.id)?;
            export(dek, &snapshot)?;
            audit_log(conn, AuditEvent::VaultEntryUpdated, Some(&entry.id));
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
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            let max_revision: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(revision), 0) FROM vault_entries",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let revision = max_revision.max(entry.revision) + 1;

            write_imported_entry_in(conn, dek, &entry, revision, algo)?;
            let snapshot = read_snapshot(conn, dek, dek_version, &entry.id)?;
            export(dek, &snapshot)?;
            audit_log(conn, AuditEvent::VaultEntryCreated, Some(&entry.id));
            Ok(snapshot)
        })
    }

    pub fn upsert_imported_entries(
        &self,
        entries: Vec<VaultEntryImport>,
        algo: CipherAlgorithm,
    ) -> Result<Vec<ImportOutcome>, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let mut outcomes = Vec::with_capacity(entries.len());
            for entry in entries {
                outcomes.push(upsert_imported_in(&tx, dek, entry, algo)?);
            }
            tx.commit()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            Ok(outcomes)
        })
    }
}

pub(super) fn write_imported_entry_in(
    conn: &Connection,
    dek: &Dek,
    entry: &VaultEntryImport,
    revision: i64,
    algo: CipherAlgorithm,
) -> Result<(), VaultError> {
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
    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    let now = crate::types::now_millis();

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

    Ok(())
}

fn upsert_imported_in(
    conn: &Connection,
    dek: &Dek,
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
    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    match existing {
        Some((Some(_), _, _)) => Ok(ImportOutcome::SkippedLocallyDeleted),
        Some((None, revision, updated_at)) => {
            if entry.revision < revision
                || (entry.revision == revision && entry.updated_at <= updated_at)
            {
                return Ok(ImportOutcome::SkippedStale);
            }

            conn.execute(
                "UPDATE vault_entries SET title = ?1, username = ?2, url = ?3, category = ?4, \
                 encrypted_password = ?5, encrypted_notes = ?6, encrypted_custom_fields = ?7, \
                 encrypted_totp_secret = ?8, tags = ?9, \
                 is_favorite = ?10, revision = ?11, updated_at = ?12 \
                 WHERE id = ?13 AND deleted_at IS NULL",
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
                    entry.revision,
                    entry.updated_at,
                    entry.id,
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;

            audit_log(conn, AuditEvent::VaultEntryUpdated, Some(&entry.id));
            Ok(ImportOutcome::Updated)
        }
        None => {
            let now = crate::types::now_millis();
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
                    entry.revision,
                    created_at,
                    updated_at,
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;

            audit_log(conn, AuditEvent::VaultEntryCreated, Some(&entry.id));
            Ok(ImportOutcome::Inserted)
        }
    }
}
