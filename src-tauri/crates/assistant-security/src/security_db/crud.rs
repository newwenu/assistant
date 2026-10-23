use rusqlite::params;

use crate::audit::{log as audit_log, AuditEvent};
use crate::types::{
    VaultCategory, VaultEntryCreate, VaultEntryDetail, VaultEntrySummary, VaultEntryUpdate,
    VaultError,
};
use crate::vault_crypto::types::{CipherAlgorithm, VaultPlaintext};
use crate::vault_crypto::VaultCrypto;

use super::SecurityDbState;

impl SecurityDbState {
    pub fn list_entries(
        &self,
        category: Option<VaultCategory>,
        search: Option<&str>,
        tag: Option<&str>,
    ) -> Result<Vec<VaultEntrySummary>, VaultError> {
        let conn_guard = self.conn.lock().unwrap();
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
        let conn_guard = self.conn.lock().unwrap();
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

    pub fn create_entry(
        &self,
        entry: VaultEntryCreate,
        algo: CipherAlgorithm,
    ) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let id = uuid::Uuid::new_v4().to_string();
            let entry_key = VaultCrypto::derive_entry_key(mk, &id);
            let now = chrono::Utc::now().timestamp_millis();

            let encrypted_password = entry
                .password
                .as_deref()
                .map(|p| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::new_str(p), entry_key.expose(), algo)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_notes = entry
                .notes
                .as_deref()
                .map(|n| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::new_str(n), entry_key.expose(), algo)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_totp = entry
                .totp_secret
                .as_deref()
                .map(|t| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::new_str(t), entry_key.expose(), algo)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let custom_fields_to_encrypt: Vec<serde_json::Value> = entry
                .custom_fields
                .iter()
                .map(|cf| {
                    if cf.is_protected {
                        let encrypted = VaultCrypto::encrypt_field(
                            &VaultPlaintext::new_str(&cf.value),
                            entry_key.expose(),
                            algo,
                        )
                        .unwrap_or_default();
                        serde_json::json!({
                            "name": cf.name,
                            "valueEncrypted": encrypted,
                            "isProtected": true
                        })
                    } else {
                        serde_json::json!({
                            "name": cf.name,
                            "value": cf.value,
                            "isProtected": false
                        })
                    }
                })
                .collect();
            let encrypted_custom_fields = serde_json::to_string(&custom_fields_to_encrypt)
                .map_err(|e| VaultError::Validation(e.to_string()))?;

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
            Ok(id)
        })
    }

    pub fn update_entry(
        &self,
        id: &str,
        entry: VaultEntryUpdate,
        algo: CipherAlgorithm,
    ) -> Result<(), VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let now = chrono::Utc::now().timestamp_millis();

            let encrypted_password = entry
                .password
                .as_deref()
                .map(|p| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::new_str(p),
                        entry_key.expose(),
                        algo,
                    )
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_notes = entry
                .notes
                .as_deref()
                .map(|n| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::new_str(n),
                        entry_key.expose(),
                        algo,
                    )
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_totp = entry
                .totp_secret
                .as_deref()
                .map(|t| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::new_str(t),
                        entry_key.expose(),
                        algo,
                    )
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let custom_fields_to_encrypt: Vec<serde_json::Value> = entry
                .custom_fields
                .iter()
                .map(|cf| {
                    if cf.is_protected {
                        let encrypted = VaultCrypto::encrypt_field(
                            &VaultPlaintext::new_str(&cf.value),
                            entry_key.expose(),
                            algo,
                        )
                        .unwrap_or_default();
                        serde_json::json!({
                            "name": cf.name,
                            "valueEncrypted": encrypted,
                            "isProtected": true
                        })
                    } else {
                        serde_json::json!({
                            "name": cf.name,
                            "value": cf.value,
                            "isProtected": false
                        })
                    }
                })
                .collect();
            let encrypted_custom_fields = serde_json::to_string(&custom_fields_to_encrypt)
                .map_err(|e| VaultError::Validation(e.to_string()))?;

            let tags_json = serde_json::to_string(&entry.tags)
                .map_err(|e| VaultError::Validation(e.to_string()))?;

            let affected = conn
                .execute(
                    "UPDATE vault_entries SET \
                     title = ?1, username = ?2, url = ?3, category = ?4, \
                     encrypted_password = ?5, encrypted_notes = ?6, \
                     encrypted_custom_fields = ?7, encrypted_totp_secret = ?8, \
                     tags = ?9, is_favorite = ?10, revision = revision + 1, updated_at = ?11 \
                     WHERE id = ?12 AND deleted_at IS NULL",
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
                        entry.is_favorite as i64,
                        now,
                        id,
                    ],
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;

            if affected == 0 {
                return Err(VaultError::NotFound(id.to_string()));
            }

            let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(id));
            Ok(())
        })
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), VaultError> {
        let conn_guard = self.conn.lock().unwrap();
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

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
}