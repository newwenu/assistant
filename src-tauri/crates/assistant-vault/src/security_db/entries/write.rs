use rusqlite::{params, Connection};

use assistant_crypto::vault_crypto::derive_entry_key;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};

use crate::audit::{log as audit_log, AuditEvent};
use crate::dto::totp::payload_from_input;
use crate::dto::{VaultEntryCreate, VaultEntryUpdate};
use crate::security_db::SecurityDbState;
use crate::types::{VaultEntrySnapshot, VaultError};

use super::field_crypto::{
    encrypt_custom_fields, encrypt_opt, FIELD_NOTES, FIELD_PASSWORD, FIELD_TOTP,
};
use super::snapshot::read_snapshot;

impl SecurityDbState {
    pub fn create_entry(
        &self,
        entry: VaultEntryCreate,
        algo: CipherAlgorithm,
    ) -> Result<String, VaultError> {
        self.with_conn_and_dek(|conn, dek, dv| {
            create_entry_in(conn, dek, dv, entry, algo).map(|s| s.id)
        })
    }

    pub fn update_entry(
        &self,
        id: &str,
        entry: VaultEntryUpdate,
        algo: CipherAlgorithm,
    ) -> Result<(), VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| update_entry_in(conn, dek, id, entry, algo))
    }

    pub fn set_favorite(
        &self,
        id: &str,
        is_favorite: bool,
    ) -> Result<VaultEntrySnapshot, VaultError> {
        self.with_conn_and_dek(|conn, dek, dv| {
            set_favorite_in(conn, id, is_favorite)?;
            read_snapshot(conn, dek, dv, id)
        })
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), VaultError> {
        self.with_conn(|conn| delete_entry_in(conn, id))
    }

    /// 批量软删除：`source` 是**来源标签**（如 `remote`），不是整条 `detail`。
    ///
    /// 每条成功删除的行各写一行 `vault_entry_deleted`，`detail` 为 `{entryId}@{source}`（规格 §2.4）：
    /// 条目 ID 必须落在可定位固定段，否则面板无法按条目检索。
    pub fn delete_entries_tx(
        &self,
        ids: &[String],
        source: Option<&str>,
    ) -> Result<usize, VaultError> {
        self.with_transaction(|conn, _session| {
            let now = crate::types::now_millis();
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
                    let audit_detail = match source {
                        Some(source) => format!("{}@{}", id, source),
                        None => id.clone(),
                    };
                    audit_log(
                        conn,
                        AuditEvent::VaultEntryDeleted,
                        Some(&audit_detail),
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
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            let snapshot = create_entry_in(conn, dek, dek_version, entry, algo)?;
            export(dek, &snapshot)?;
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
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            update_entry_in(conn, dek, id, entry, algo)?;
            let snapshot = read_snapshot(conn, dek, dek_version, id)?;
            export(dek, &snapshot)?;
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
        F: FnOnce(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            let dek = &session.dek;
            let dek_version = session.dek_version;
            set_favorite_in(conn, id, is_favorite)?;
            let snapshot = read_snapshot(conn, dek, dek_version, id)?;
            export(dek, &snapshot)?;
            Ok(snapshot)
        })
    }

    pub fn delete_entry_tx<F>(&self, id: &str, export: F) -> Result<(), VaultError>
    where
        F: FnOnce(&Dek, &str) -> Result<(), VaultError>,
    {
        self.with_transaction(|conn, session| {
            delete_entry_in(conn, id)?;
            export(&session.dek, id)?;
            Ok(())
        })
    }
}

fn create_entry_in(
    conn: &Connection,
    dek: &Dek,
    dek_version: u32,
    entry: VaultEntryCreate,
    algo: CipherAlgorithm,
) -> Result<VaultEntrySnapshot, VaultError> {
    let id = uuid::Uuid::new_v4().to_string();
    let entry_key = derive_entry_key(dek, &id);
    let now = crate::types::now_millis();

    let totp_payload = payload_from_input(entry.totp.as_ref());
    let encrypted_password = encrypt_opt(
        entry.password.as_deref(),
        &entry_key,
        FIELD_PASSWORD,
        &id,
        algo,
    )?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), &entry_key, FIELD_NOTES, &id, algo)?;
    let encrypted_totp = encrypt_opt(totp_payload.as_deref(), &entry_key, FIELD_TOTP, &id, algo)?;
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, &entry_key, &id, algo)?;
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
            i64::from(entry.is_favorite),
            now,
            now,
        ],
    )
    .map_err(|e| VaultError::Database(e.to_string()))?;

    audit_log(conn, AuditEvent::VaultEntryCreated, Some(&id));
    read_snapshot(conn, dek, dek_version, &id)
}

fn update_entry_in(
    conn: &Connection,
    dek: &Dek,
    id: &str,
    entry: VaultEntryUpdate,
    algo: CipherAlgorithm,
) -> Result<(), VaultError> {
    let entry_key = derive_entry_key(dek, id);
    let now = crate::types::now_millis();

    let encrypted_password = encrypt_opt(
        entry.password.as_deref(),
        &entry_key,
        FIELD_PASSWORD,
        id,
        algo,
    )?;
    let encrypted_notes = encrypt_opt(entry.notes.as_deref(), &entry_key, FIELD_NOTES, id, algo)?;
    // None = 保持不变；Some(None) = 清除；Some(Some(_)) = 整体替换
    let encrypted_totp: Option<Option<String>> = match entry.totp.as_ref() {
        None => None,
        Some(input) => match payload_from_input(Some(input)) {
            Some(payload) => Some(encrypt_opt(
                Some(payload.as_str()),
                &entry_key,
                FIELD_TOTP,
                id,
                algo,
            )?),
            None => Some(None),
        },
    };
    let encrypted_custom_fields =
        encrypt_custom_fields(&entry.custom_fields, &entry_key, id, algo)?;

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
    match &encrypted_totp {
        None => {}
        Some(None) => set_clauses.push("encrypted_totp_secret = NULL".to_string()),
        Some(Some(_)) => {
            set_clauses.push(format!("encrypted_totp_secret = ?{}", param_idx));
            param_idx += 1;
        }
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
        Box::new(i64::from(entry.is_favorite)),
        Box::new(now),
    ];

    if let Some(ep) = encrypted_password {
        params.push(Box::new(ep));
    }
    if let Some(en) = encrypted_notes {
        params.push(Box::new(en));
    }
    if let Some(Some(et)) = encrypted_totp {
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

    audit_log(conn, AuditEvent::VaultEntryUpdated, Some(id));
    Ok(())
}

fn set_favorite_in(conn: &Connection, id: &str, is_favorite: bool) -> Result<(), VaultError> {
    let now = crate::types::now_millis();
    let affected = conn
        .execute(
            "UPDATE vault_entries SET is_favorite = ?1, revision = revision + 1, updated_at = ?2 \
             WHERE id = ?3 AND deleted_at IS NULL",
            params![i64::from(is_favorite), now, id],
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    if affected == 0 {
        return Err(VaultError::NotFound(id.to_string()));
    }

    audit_log(conn, AuditEvent::VaultEntryUpdated, Some(id));
    Ok(())
}

fn delete_entry_in(conn: &Connection, id: &str) -> Result<(), VaultError> {
    let now = crate::types::now_millis();
    let affected = conn
        .execute(
            "UPDATE vault_entries SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    if affected == 0 {
        return Err(VaultError::NotFound(id.to_string()));
    }

    audit_log(conn, AuditEvent::VaultEntryDeleted, Some(id));
    Ok(())
}
