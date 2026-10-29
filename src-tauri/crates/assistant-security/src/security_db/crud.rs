use rusqlite::{params, Connection, OptionalExtension};

use crate::audit::{log as audit_log, AuditEvent};
use crate::types::{
    CustomFieldInput, ImportOutcome, VaultCategory, VaultCryptoInfo, VaultEntryCreate,
    VaultEntryDetail, VaultEntryImport, VaultEntrySnapshot, VaultEntrySummary, VaultEntryUpdate,
    VaultError,
};
use crate::vault_crypto::types::{CipherAlgorithm, MasterKey, VaultPlaintext};
use crate::vault_crypto::VaultCrypto;

use super::SecurityDbState;

impl SecurityDbState {
    pub fn list_entries(
        &self,
        category: Option<VaultCategory>,
        search: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<VaultEntrySummary>, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let mut sql = String::from(
            "SELECT id, title, username, url, category, tags, is_favorite, updated_at \
             FROM vault_entries WHERE deleted_at IS NULL",
        );
        let mut param_idx = 1u32;

        if category.is_some() {
            sql.push_str(&format!(" AND category = ?{}", param_idx));
            param_idx += 1;
        }
        if search.is_some() {
            sql.push_str(&format!(
                " AND (title LIKE ?{} OR username LIKE ?{} OR url LIKE ?{})",
                param_idx,
                param_idx + 1,
                param_idx + 2
            ));
            param_idx += 3;
        }
        if tag.is_some() {
            sql.push_str(&format!(" AND tags LIKE ?{}", param_idx));
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| VaultError::Database(e.to_string()))?;

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(cat) = category {
            param_values.push(Box::new(cat.to_string()));
        }
        if let Some(s) = search {
            let pattern = format!("%{}%", s);
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern));
        }
        if let Some(t) = tag {
            param_values.push(Box::new(format!("%\"{}\"%", t)));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
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

    pub fn get_entry(&self, id: &str) -> Result<VaultEntryDetail, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let result: Result<VaultEntryDetail, rusqlite::Error> = conn.query_row(
            "SELECT id, title, username, url, category, encrypted_password, encrypted_notes, \
             encrypted_totp_secret, encrypted_custom_fields, tags, is_favorite, revision, \
             created_at, updated_at \
             FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |row| {
                let category_str: String = row.get("category")?;
                let tags_json: String = row.get("tags")?;
                let encrypted_password: Option<String> = row.get("encrypted_password")?;
                let encrypted_notes: Option<String> = row.get("encrypted_notes")?;
                let encrypted_totp: Option<String> = row.get("encrypted_totp_secret")?;
                let custom_fields_json: Option<String> = row.get("encrypted_custom_fields")?;

                let custom_field_names: Vec<String> = custom_fields_json
                    .and_then(|json| {
                        let fields: Vec<serde_json::Value> = serde_json::from_str(&json).ok()?;
                        Some(
                            fields
                                .iter()
                                .filter_map(|f| f.get("name")?.as_str().map(String::from))
                                .collect(),
                        )
                    })
                    .unwrap_or_default();

                Ok(VaultEntryDetail {
                    id: row.get("id")?,
                    category: VaultCategory::parse(&category_str).unwrap_or_default(),
                    title: row.get("title")?,
                    username: row.get("username")?,
                    url: row.get("url")?,
                    has_password: encrypted_password.is_some(),
                    has_notes: encrypted_notes.is_some(),
                    has_totp: encrypted_totp.is_some(),
                    custom_field_names,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    is_favorite: row.get::<_, i64>("is_favorite")? != 0,
                    revision: row.get::<_, i64>("revision")? as u64,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        );

        match result {
            Ok(detail) => Ok(detail),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(VaultError::NotFound(id.to_string())),
            Err(e) => Err(VaultError::Database(e.to_string())),
        }
    }

    pub fn list_entry_ids(&self, include_deleted: bool) -> Result<Vec<String>, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let sql = if include_deleted {
            "SELECT id FROM vault_entries ORDER BY created_at"
        } else {
            "SELECT id FROM vault_entries WHERE deleted_at IS NULL ORDER BY created_at"
        };

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

    pub fn create_entry(
        &self,
        entry: VaultEntryCreate,
        algo: CipherAlgorithm,
    ) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| create_entry_in(conn, mk, entry, algo).map(|s| s.id))
    }

    pub fn update_entry(
        &self,
        id: &str,
        entry: VaultEntryUpdate,
        algo: CipherAlgorithm,
    ) -> Result<(), VaultError> {
        self.with_conn_and_key(|conn, mk| update_entry_in(conn, mk, id, entry, algo))
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;
        delete_entry_in(conn, id)
    }

    pub fn delete_entries_tx(
        &self,
        ids: &[String],
        detail: Option<&str>,
    ) -> Result<usize, VaultError> {
        self.with_transaction(|conn, _mk| {
            let now = chrono::Utc::now().timestamp_millis();
            let mut deleted = 0usize;
            for id in ids {
                let affected = conn
                    .execute(
                        "UPDATE vault_entries SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                        params![now, id],
                    )
                    .map_err(|e| VaultError::Database(e.to_string()))?;
                if affected > 0 {
                    deleted += 1;
                    let _ = audit_log(
                        conn,
                        AuditEvent::VaultEntryDeleted,
                        Some(detail.unwrap_or(id.as_str())),
                    );
                }
            }
            Ok(deleted)
        })
    }

    pub(crate) fn with_transaction<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection, &MasterKey) -> Result<T, VaultError>,
    {
        let mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
        let mk = mk_guard.as_ref().ok_or(VaultError::Locked)?;
        let mut conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_mut().ok_or(VaultError::Locked)?;
        let tx = conn
            .transaction()
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let result = f(&tx, mk)?;
        tx.commit()
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(result)
    }

    pub fn create_entry_tx<F>(
        &self,
        entry: VaultEntryCreate,
        algo: CipherAlgorithm,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let snapshot = create_entry_in(conn, mk, entry, algo)?;
            export(mk, &snapshot)?;
            Ok(snapshot)
        })
    }

    pub fn update_entry_tx<F>(
        &self,
        id: &str,
        entry: VaultEntryUpdate,
        algo: CipherAlgorithm,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            update_entry_in(conn, mk, id, entry, algo)?;
            let snapshot = read_snapshot(conn, mk, id)?;
            export(mk, &snapshot)?;
            Ok(snapshot)
        })
    }

    pub fn delete_entry_tx<F>(&self, id: &str, export: F) -> Result<(), VaultError>
    where
        F: FnOnce(&str) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, _mk| {
            delete_entry_in(conn, id)?;
            export(id)?;
            Ok(())
        })
    }

    pub fn export_all_snapshots(&self) -> Result<Vec<VaultEntrySnapshot>, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let ids = query_ids(
                conn,
                "SELECT id FROM vault_entries WHERE deleted_at IS NULL ORDER BY created_at",
            )?;
            ids.iter()
                .map(|id| read_snapshot(conn, mk, id))
                .collect::<Result<Vec<_>, VaultError>>()
        })
    }

    pub fn snapshot(&self, id: &str) -> Result<VaultEntrySnapshot, VaultError> {
        self.with_conn_and_key(|conn, mk| read_snapshot(conn, mk, id))
    }

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
        source: &str,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            let now = chrono::Utc::now().timestamp_millis();
            let entry_key = VaultCrypto::derive_entry_key(mk, &entry.id);
            let algo = CipherAlgorithm::default();

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

    pub fn force_import_entry_tx<F>(
        &self,
        entry: VaultEntryImport,
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

            write_imported_entry_in(conn, mk, &entry, revision, CipherAlgorithm::default())?;
            let snapshot = read_snapshot(conn, mk, &entry.id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(&entry.id));
            Ok(snapshot)
        })
    }

    pub fn create_remote_copy_tx<F>(
        &self,
        entry: VaultEntryImport,
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

            write_imported_entry_in(conn, mk, &entry, revision, CipherAlgorithm::default())?;
            let snapshot = read_snapshot(conn, mk, &entry.id)?;
            export(mk, &snapshot)?;
            let _ = audit_log(conn, AuditEvent::VaultEntryCreated, Some(&entry.id));
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

    pub fn vault_crypto_info(
        &self,
        default_cipher: CipherAlgorithm,
    ) -> Result<VaultCryptoInfo, VaultError> {
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let master_key_salt = super::meta::get_salt(conn, "master_key_salt")
            .map_err(VaultError::Database)?
            .ok_or_else(|| VaultError::Database("master_key_salt not found".to_string()))?;
        let hash_salt = super::meta::get_salt(conn, "hash_salt")
            .map_err(VaultError::Database)?
            .ok_or_else(|| VaultError::Database("hash_salt not found".to_string()))?;
        let (argon2_m, argon2_t, argon2_p) = VaultCrypto::argon2_params();

        Ok(VaultCryptoInfo {
            master_key_salt,
            hash_salt,
            argon2_m,
            argon2_t,
            argon2_p,
            default_cipher,
        })
    }
}

fn query_ids(conn: &Connection, sql: &str) -> Result<Vec<String>, VaultError> {
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

pub(crate) fn create_entry_in(
    conn: &Connection,
    mk: &MasterKey,
    entry: VaultEntryCreate,
    algo: CipherAlgorithm,
) -> Result<VaultEntrySnapshot, VaultError> {
    let id = uuid::Uuid::new_v4().to_string();
    let entry_key = VaultCrypto::derive_entry_key(mk, &id);
    let now = chrono::Utc::now().timestamp_millis();

    let encrypted_password = encrypt_opt(entry.password.as_deref(), entry_key.expose(), algo)?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), entry_key.expose(), algo)?;
    let encrypted_totp = encrypt_opt(entry.totp_secret.as_deref(), entry_key.expose(), algo)?;
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, entry_key.expose(), algo)?;
    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    conn.execute(
        "INSERT INTO vault_entries \
         (id, title, username, url, category, encrypted_password, encrypted_notes, \
          encrypted_custom_fields, encrypted_totp_secret, tags, is_favorite, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            id,
            entry.title,
            entry.username,
            entry.url,
            entry.category.to_string(),
            encrypted_password,
            encrypted_notes,
            encrypted_custom_fields,
            encrypted_totp,
            tags_json,
            entry.is_favorite as i64,
            now,
            now,
        ],
    )
    .map_err(|e| VaultError::Database(e.to_string()))?;

    let _ = audit_log(conn, AuditEvent::VaultEntryCreated, Some(&id));
    read_snapshot(conn, mk, &id)
}

pub(crate) fn update_entry_in(
    conn: &Connection,
    mk: &MasterKey,
    id: &str,
    entry: VaultEntryUpdate,
    algo: CipherAlgorithm,
) -> Result<(), VaultError> {
    let entry_key = VaultCrypto::derive_entry_key(mk, id);
    let now = chrono::Utc::now().timestamp_millis();

    let encrypted_password = encrypt_opt(entry.password.as_deref(), entry_key.expose(), algo)?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), entry_key.expose(), algo)?;
    let encrypted_totp = encrypt_opt(entry.totp_secret.as_deref(), entry_key.expose(), algo)?;
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, entry_key.expose(), algo)?;

    let tags_json =
        serde_json::to_string(&entry.tags).map_err(|e| VaultError::Validation(e.to_string()))?;

    let mut set_clauses = vec![
        "title = ?1".to_string(),
        "username = ?2".to_string(),
        "url = ?3".to_string(),
        "category = ?4".to_string(),
        "encrypted_custom_fields = ?5".to_string(),
        "tags = ?6".to_string(),
        "is_favorite = ?7".to_string(),
        "revision = revision + 1".to_string(),
        "updated_at = ?8".to_string(),
    ];
    let mut param_idx = 9;

    if encrypted_password.is_some() {
        set_clauses.push(format!("encrypted_password = ?{}", param_idx));
        param_idx += 1;
    }
    if encrypted_notes.is_some() {
        set_clauses.push(format!("encrypted_notes = ?{}", param_idx));
        param_idx += 1;
    }
    if encrypted_totp.is_some() {
        set_clauses.push(format!("encrypted_totp_secret = ?{}", param_idx));
        param_idx += 1;
    }

    let sql = format!(
        "UPDATE vault_entries SET {} WHERE id = ?{} AND deleted_at IS NULL",
        set_clauses.join(", "),
        param_idx,
    );

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(entry.title.clone()),
        Box::new(entry.username.clone()),
        Box::new(entry.url.clone()),
        Box::new(entry.category.to_string()),
        Box::new(encrypted_custom_fields),
        Box::new(tags_json),
        Box::new(entry.is_favorite as i64),
        Box::new(now),
    ];

    if let Some(ep) = encrypted_password {
        params.push(Box::new(ep));
    }
    if let Some(en) = encrypted_notes {
        params.push(Box::new(en));
    }
    if let Some(et) = encrypted_totp {
        params.push(Box::new(et));
    }
    params.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let affected = conn
        .execute(&sql, param_refs.as_slice())
        .map_err(|e| VaultError::Database(e.to_string()))?;

    if affected == 0 {
        return Err(VaultError::NotFound(id.to_string()));
    }

    let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(id));
    Ok(())
}

pub(crate) fn delete_entry_in(conn: &Connection, id: &str) -> Result<(), VaultError> {
    let now = chrono::Utc::now().timestamp_millis();
    let affected = conn
        .execute(
            "UPDATE vault_entries SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    if affected == 0 {
        return Err(VaultError::NotFound(id.to_string()));
    }

    let _ = audit_log(conn, AuditEvent::VaultEntryDeleted, Some(id));
    Ok(())
}

pub(crate) fn write_imported_entry_in(
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

pub(crate) fn upsert_imported_in(
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

pub(crate) fn read_snapshot(
    conn: &Connection,
    mk: &MasterKey,
    id: &str,
) -> Result<VaultEntrySnapshot, VaultError> {
    let row = conn
        .query_row(
            "SELECT id, title, username, url, category, encrypted_password, encrypted_notes, \
             encrypted_totp_secret, encrypted_custom_fields, totp_digits, totp_period, tags, \
             is_favorite, revision, created_at, updated_at \
             FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, i64>(14)?,
                    row.get::<_, i64>(15)?,
                ))
            },
        )
        .optional()
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let (
        id,
        title,
        username,
        url,
        category,
        enc_password,
        enc_notes,
        enc_totp,
        enc_custom_fields,
        totp_digits,
        totp_period,
        tags_json,
        is_favorite,
        revision,
        created_at,
        updated_at,
    ) = row.ok_or_else(|| VaultError::NotFound(id.to_string()))?;

    let entry_key = VaultCrypto::derive_entry_key(mk, &id);

    Ok(VaultEntrySnapshot {
        id,
        category: VaultCategory::parse(&category).unwrap_or_default(),
        revision: revision as u64,
        title,
        username,
        url,
        password: decrypt_opt(enc_password.as_deref(), entry_key.expose())?,
        notes: decrypt_opt(enc_notes.as_deref(), entry_key.expose())?,
        totp_secret: decrypt_opt(enc_totp.as_deref(), entry_key.expose())?,
        totp_digits: totp_digits as u32,
        totp_period: totp_period as u32,
        custom_fields: decrypt_custom_fields(enc_custom_fields.as_deref(), entry_key.expose())?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        is_favorite: is_favorite != 0,
        created_at,
        updated_at,
    })
}

fn encrypt_opt(
    value: Option<&str>,
    entry_key: &[u8; 32],
    algo: CipherAlgorithm,
) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            VaultCrypto::encrypt_field(&VaultPlaintext::new_str(v), entry_key, algo)
                .map_err(VaultError::Crypto)
        })
        .transpose()
}

fn decrypt_opt(value: Option<&str>, entry_key: &[u8; 32]) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            let plaintext = VaultCrypto::decrypt_field(v, entry_key).map_err(VaultError::Crypto)?;
            Ok(plaintext.to_str().unwrap_or_default().to_string())
        })
        .transpose()
}

fn encrypt_custom_fields(
    fields: &[CustomFieldInput],
    entry_key: &[u8; 32],
    algo: CipherAlgorithm,
) -> Result<String, VaultError> {
    let values: Vec<serde_json::Value> = fields
        .iter()
        .map(|cf| -> Result<serde_json::Value, VaultError> {
            if cf.is_protected {
                let encrypted = VaultCrypto::encrypt_field(
                    &VaultPlaintext::new_str(&cf.value),
                    entry_key,
                    algo,
                )
                .map_err(VaultError::Crypto)?;
                Ok(serde_json::json!({
                    "name": cf.name,
                    "valueEncrypted": encrypted,
                    "isProtected": true
                }))
            } else {
                Ok(serde_json::json!({
                    "name": cf.name,
                    "value": cf.value,
                    "isProtected": false
                }))
            }
        })
        .collect::<Result<Vec<_>, VaultError>>()?;

    serde_json::to_string(&values).map_err(|e| VaultError::Validation(e.to_string()))
}

fn decrypt_custom_fields(
    json: Option<&str>,
    entry_key: &[u8; 32],
) -> Result<Vec<CustomFieldInput>, VaultError> {
    let Some(json) = json else {
        return Ok(Vec::new());
    };

    let values: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| VaultError::Database(e.to_string()))?;

    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let is_protected = value
            .get("isProtected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let plain = if is_protected {
            let encrypted = value
                .get("valueEncrypted")
                .and_then(|v| v.as_str())
                .ok_or_else(|| VaultError::Database("missing valueEncrypted".to_string()))?;
            let plaintext =
                VaultCrypto::decrypt_field(encrypted, entry_key).map_err(VaultError::Crypto)?;
            plaintext.to_str().unwrap_or_default().to_string()
        } else {
            value
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        fields.push(CustomFieldInput {
            name,
            value: plain,
            is_protected,
        });
    }

    Ok(fields)
}
