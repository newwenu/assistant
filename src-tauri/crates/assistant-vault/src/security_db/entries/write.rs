use rusqlite::{params, Connection};

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_crypto::vault_crypto::VaultCrypto;

use crate::audit::{log as audit_log, AuditEvent};
use crate::dto::{VaultEntryCreate, VaultEntryUpdate};
use crate::security_db::SecurityDbState;
use crate::types::{VaultEntrySnapshot, VaultError};

use super::field_crypto::{encrypt_custom_fields, encrypt_opt};
use super::snapshot::read_snapshot;

impl SecurityDbState {
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

    pub fn set_favorite(
        &self,
        id: &str,
        is_favorite: bool,
    ) -> Result<VaultEntrySnapshot, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            set_favorite_in(conn, id, is_favorite)?;
            read_snapshot(conn, mk, id)
        })
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

    pub fn set_favorite_tx<F>(
        &self,
        id: &str,
        is_favorite: bool,
        export: F,
    ) -> Result<VaultEntrySnapshot, VaultError>
    where
        F: FnOnce(&MasterKey, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, mk| {
            set_favorite_in(conn, id, is_favorite)?;
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
}

fn create_entry_in(
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

fn update_entry_in(
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

fn set_favorite_in(conn: &Connection, id: &str, is_favorite: bool) -> Result<(), VaultError> {
    let now = chrono::Utc::now().timestamp_millis();
    let affected = conn
        .execute(
            "UPDATE vault_entries SET is_favorite = ?1, revision = revision + 1, updated_at = ?2 \
             WHERE id = ?3 AND deleted_at IS NULL",
            params![is_favorite as i64, now, id],
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    if affected == 0 {
        return Err(VaultError::NotFound(id.to_string()));
    }

    let _ = audit_log(conn, AuditEvent::VaultEntryUpdated, Some(id));
    Ok(())
}

fn delete_entry_in(conn: &Connection, id: &str) -> Result<(), VaultError> {
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
