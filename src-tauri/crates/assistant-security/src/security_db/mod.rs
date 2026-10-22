use std::panic::catch_unwind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use log::info;
use rand::Rng;
use rusqlite::{params, Connection};
use zeroize::Zeroize;

use crate::audit::{log as audit_log, AuditEvent};
use crate::biometric::BiometricKeyStore;
use crate::types::{
    VaultCategory, VaultEntryCreate, VaultEntryDetail, VaultEntryExport, VaultEntrySummary,
    VaultEntryUpdate, VaultError,
};
use crate::vault_crypto::types::{CipherAlgorithm, MasterKey, VaultPlaintext};
use crate::vault_crypto::VaultCrypto;

pub mod meta;
pub mod schema;

const BOOTSTRAP_SALT: [u8; 32] = [
    0x56, 0x61, 0x75, 0x6C, 0x74, 0x42, 0x6F, 0x6F, 0x74, 0x73, 0x74, 0x72, 0x61, 0x70, 0x53, 0x61,
    0x6C, 0x74, 0x5F, 0x32, 0x30, 0x32, 0x35, 0x5F, 0x41, 0x72, 0x67, 0x6F, 0x6E, 0x32, 0x69, 0x64,
];

const SQLCIPHER_PRAGMAS: &str =
    "PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;";

pub struct SecurityDbState {
    conn: Mutex<Option<Connection>>,
    master_key: Mutex<Option<MasterKey>>,
    db_path: String,
}

impl SecurityDbState {
    pub fn new(db_path: &str) -> Self {
        Self {
            conn: Mutex::new(None),
            master_key: Mutex::new(None),
            db_path: db_path.to_string(),
        }
    }

    pub fn init(&self, password: &str) -> Result<(), VaultError> {
        let path = Path::new(&self.db_path);
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }

        {
            let strength = crate::password_strength::estimate_strength(password);
            if strength.score < 2 {
                return Err(VaultError::Validation(format!(
                    "Master password too weak (score {}/4, need ≥2). {}",
                    strength.score, strength.crack_time_display
                )));
            }
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bootstrap_key_bytes = VaultCrypto::derive_master_key(password, &BOOTSTRAP_SALT)
            .map_err(VaultError::Crypto)?;
        let bootstrap_key = MasterKey::new(bootstrap_key_bytes);

        let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&bootstrap_key);
        let sqlcipher_key_hex = hex::encode(sqlcipher_key);

        let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
        conn.execute_batch(SQLCIPHER_PRAGMAS)
            .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;

        {
            let mut mk_arr = sqlcipher_key;
            mk_arr.zeroize();
        }

        conn.execute_batch(schema::INIT_SQL)
            .map_err(|e| VaultError::Database(e.to_string()))?;
        meta::ensure_schema_version(&conn)?;

        let master_key_salt: [u8; 32] = rand::thread_rng().gen();
        let hash_salt: [u8; 32] = rand::thread_rng().gen();

        let master_key_bytes = VaultCrypto::derive_master_key(password, &master_key_salt)
            .map_err(VaultError::Crypto)?;
        let master_key_hash =
            VaultCrypto::derive_master_key(password, &hash_salt).map_err(VaultError::Crypto)?;

        meta::set_salt(&conn, "master_key_salt", &master_key_salt)?;
        meta::set_salt(&conn, "hash_salt", &hash_salt)?;
        meta::set_master_key_hash(&conn, &master_key_hash)?;

        let master_key = MasterKey::new(master_key_bytes);
        *self.master_key.lock().unwrap() = Some(master_key);
        *self.conn.lock().unwrap() = Some(conn);

        info!("Vault initialized successfully (SQLCipher encrypted, two-phase unlock)");
        Ok(())
    }

    pub fn unlock(&self, password: &str) -> Result<(), VaultError> {
        if self.master_key.lock().unwrap().is_some() {
            return Err(VaultError::AlreadyUnlocked);
        }

        let path = Path::new(&self.db_path);

        let bootstrap_key_bytes = VaultCrypto::derive_master_key(password, &BOOTSTRAP_SALT)
            .map_err(VaultError::Crypto)?;
        let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&MasterKey::new(bootstrap_key_bytes));
        let sqlcipher_key_hex = hex::encode(sqlcipher_key);

        let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
            .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
        conn.execute_batch(SQLCIPHER_PRAGMAS)
            .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;

        {
            let mut mk_arr = sqlcipher_key;
            mk_arr.zeroize();
        }

        let master_key_salt = match meta::get_salt(&conn, "master_key_salt") {
            Ok(Some(salt)) => salt,
            Ok(None) => {
                return Err(VaultError::Database(
                    "master_key_salt not found".to_string(),
                ))
            }
            Err(_) => return Err(VaultError::InvalidPassword),
        };

        let hash_salt = match meta::get_salt(&conn, "hash_salt") {
            Ok(Some(salt)) => salt,
            Ok(None) => return Err(VaultError::Database("hash_salt not found".to_string())),
            Err(_) => return Err(VaultError::InvalidPassword),
        };

        let stored_hash = match meta::get_master_key_hash(&conn) {
            Ok(Some(hash)) => hash,
            Ok(None) => {
                return Err(VaultError::Database(
                    "master_key_hash not found".to_string(),
                ))
            }
            Err(_) => return Err(VaultError::InvalidPassword),
        };

        let master_key_bytes = VaultCrypto::derive_master_key(password, &master_key_salt)
            .map_err(VaultError::Crypto)?;

        let valid = VaultCrypto::verify_master_password(password, &stored_hash, &hash_salt)
            .map_err(VaultError::Crypto)?;

        if !valid {
            let _ = audit_log(&conn, AuditEvent::VaultUnlockFailed, None);
            return Err(VaultError::InvalidPassword);
        }

        let master_key = MasterKey::new(master_key_bytes);
        *self.master_key.lock().unwrap() = Some(master_key);
        *self.conn.lock().unwrap() = Some(conn);

        {
            let guard = self.conn.lock().unwrap();
            if let Some(c) = guard.as_ref() {
                let _ = audit_log(c, AuditEvent::VaultUnlock, None);
            }
        }

        info!("Vault unlocked successfully");
        Ok(())
    }

    pub fn lock(&self) -> Result<(), VaultError> {
        {
            let guard = self.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultLock, None);
            }
        }

        let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
            *self.master_key.lock().unwrap() = None;
        }));

        match result {
            Ok(()) => {
                *self.conn.lock().unwrap() = None;
                info!("Vault locked");
                Ok(())
            }
            Err(_) => {
                *self.conn.lock().unwrap() = None;
                Err(VaultError::Database(
                    "Panic during vault lock — master_key zeroize may have failed".to_string(),
                ))
            }
        }
    }

    pub fn is_locked(&self) -> bool {
        self.master_key.lock().unwrap().is_none()
    }

    pub fn get_conn(&self) -> Result<std::sync::MutexGuard<'_, Option<Connection>>, VaultError> {
        let guard = self.conn.lock().unwrap();
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(guard)
    }

    pub fn get_master_key(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<MasterKey>>, VaultError> {
        let guard = self.master_key.lock().unwrap();
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(guard)
    }

    pub fn backup(&self, backup_path: &Path) -> Result<(), VaultError> {
        let guard = self.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or(VaultError::Locked)?;
        conn.execute_batch(&format!(
            "VACUUM INTO '{}'",
            backup_path.to_str().ok_or_else(|| {
                VaultError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid backup path",
                ))
            })?
        ))
        .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        Ok(Path::new(&self.db_path).exists())
    }

    fn with_conn_and_key<F, T>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection, &MasterKey) -> Result<T, VaultError>,
    {
        let mk_guard = self.master_key.lock().unwrap();
        let mk = mk_guard.as_ref().ok_or(VaultError::Locked)?;
        let conn_guard = self.conn.lock().unwrap();
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;
        f(conn, mk)
    }

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
                    category: VaultCategory::from_str(&category_str).unwrap_or_default(),
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
                    category: VaultCategory::from_str(&category_str).unwrap_or_default(),
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

    pub fn create_entry(&self, entry: VaultEntryCreate) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let id = uuid::Uuid::new_v4().to_string();
            let entry_key = VaultCrypto::derive_entry_key(mk, &id);
            let now = chrono::Utc::now().timestamp();

            let encrypted_password = entry
                .password
                .as_deref()
                .map(|p| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::from_str(p), entry_key.expose(), CipherAlgorithm::Aes256Gcm)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_notes = entry
                .notes
                .as_deref()
                .map(|n| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::from_str(n), entry_key.expose(), CipherAlgorithm::Aes256Gcm)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_totp = entry
                .totp_secret
                .as_deref()
                .map(|t| {
                    VaultCrypto::encrypt_field(&VaultPlaintext::from_str(t), entry_key.expose(), CipherAlgorithm::Aes256Gcm)
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let custom_fields_to_encrypt: Vec<serde_json::Value> = entry
                .custom_fields
                .iter()
                .map(|cf| {
                    if cf.is_protected {
                        let encrypted = VaultCrypto::encrypt_field(
                            &VaultPlaintext::from_str(&cf.value),
                            entry_key.expose(),
                            CipherAlgorithm::Aes256Gcm,
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

    pub fn update_entry(&self, id: &str, entry: VaultEntryUpdate) -> Result<(), VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let now = chrono::Utc::now().timestamp();

            let encrypted_password = entry
                .password
                .as_deref()
                .map(|p| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::from_str(p),
                        entry_key.expose(),
                        CipherAlgorithm::Aes256Gcm,
                    )
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_notes = entry
                .notes
                .as_deref()
                .map(|n| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::from_str(n),
                        entry_key.expose(),
                        CipherAlgorithm::Aes256Gcm,
                    )
                })
                .transpose()
                .map_err(VaultError::Crypto)?;

            let encrypted_totp = entry
                .totp_secret
                .as_deref()
                .map(|t| {
                    VaultCrypto::encrypt_field(
                        &VaultPlaintext::from_str(t),
                        entry_key.expose(),
                        CipherAlgorithm::Aes256Gcm,
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
                            &VaultPlaintext::from_str(&cf.value),
                            entry_key.expose(),
                            CipherAlgorithm::Aes256Gcm,
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

        let now = chrono::Utc::now().timestamp();
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

    pub fn decrypt_password(&self, id: &str) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_password FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(String::from).map_err(|_| {
                VaultError::Crypto("Decrypted password is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_notes(&self, id: &str) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_notes FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(String::from).map_err(|_| {
                VaultError::Crypto("Decrypted notes is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_totp_secret(&self, id: &str) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_totp_secret FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(String::from).map_err(|_| {
                VaultError::Crypto("Decrypted TOTP secret is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_field(&self, id: &str, field_name: &str) -> Result<String, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let custom_fields_json: String = conn
                .query_row(
                    "SELECT encrypted_custom_fields FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let fields: Vec<serde_json::Value> = serde_json::from_str(&custom_fields_json)
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let field = fields
                .iter()
                .find(|f| f.get("name").and_then(|n| n.as_str()) == Some(field_name))
                .ok_or_else(|| VaultError::NotFound(field_name.to_string()))?;

            if field.get("isProtected").and_then(|v| v.as_bool()) == Some(true) {
                let encrypted = field
                    .get("valueEncrypted")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| VaultError::Validation("Protected field has no encrypted value".to_string()))?;
                let entry_key = VaultCrypto::derive_entry_key(mk, id);
                let plaintext = VaultCrypto::decrypt_field(encrypted, entry_key.expose())
                    .map_err(VaultError::Crypto)?;
                plaintext.to_str().map(String::from).map_err(|_| {
                    VaultError::Crypto("Decrypted field is not valid UTF-8".to_string())
                })
            } else {
                field
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| VaultError::Validation("Unprotected field has no value".to_string()))
            }
        })
    }

    pub fn change_master_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), VaultError> {
        {
            let strength = crate::password_strength::estimate_strength(new_password);
            if strength.score < 2 {
                return Err(VaultError::Validation(format!(
                    "New master password too weak (score {}/4, need ≥2). {}",
                    strength.score, strength.crack_time_display
                )));
            }
        }

        let conn_guard = self.conn.lock().unwrap();
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let hash_salt = meta::get_salt(conn, "hash_salt")?
            .ok_or_else(|| VaultError::Database("hash_salt not found".to_string()))?;
        let stored_hash = meta::get_master_key_hash(conn)?
            .ok_or_else(|| VaultError::Database("master_key_hash not found".to_string()))?;
        let valid = VaultCrypto::verify_master_password(current_password, &stored_hash, &hash_salt)
            .map_err(VaultError::Crypto)?;
        if !valid {
            return Err(VaultError::InvalidPassword);
        }

        let backup_dir = Path::new(&self.db_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups");
        std::fs::create_dir_all(&backup_dir)?;
        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let backup_path = backup_dir.join(format!("security-{}.db.bak", timestamp));
        conn.execute_batch(&format!(
            "VACUUM INTO '{}'",
            backup_path.to_str().ok_or_else(|| {
                VaultError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid backup path",
                ))
            })?
        ))
        .map_err(|e| VaultError::Database(e.to_string()))?;

        let new_master_key_salt: [u8; 32] = rand::thread_rng().gen();
        let new_hash_salt: [u8; 32] = rand::thread_rng().gen();
        let new_master_key_bytes =
            VaultCrypto::derive_master_key(new_password, &new_master_key_salt)
                .map_err(VaultError::Crypto)?;
        let new_master_key_hash = VaultCrypto::derive_master_key(new_password, &new_hash_salt)
            .map_err(VaultError::Crypto)?;
        let new_master_key = MasterKey::new(new_master_key_bytes);

        let entry_ids: Vec<String>;
        {
            let mk_guard = self.master_key.lock().unwrap();
            let old_master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;

            entry_ids = {
                let mut stmt = conn
                    .prepare("SELECT id FROM vault_entries WHERE deleted_at IS NULL")
                    .map_err(|e| VaultError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map([], |row| row.get(0))
                    .map_err(|e| VaultError::Database(e.to_string()))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            conn.execute_batch("BEGIN TRANSACTION")
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let re_encrypt_result: Result<(), VaultError> = (|| {
                for eid in &entry_ids {
                    let old_entry_key = VaultCrypto::derive_entry_key(old_master_key, eid);
                    let new_entry_key = VaultCrypto::derive_entry_key(&new_master_key, eid);

                    let re_encrypt = |col: &str| -> Result<Option<String>, VaultError> {
                        let encrypted: Option<String> = conn
                            .query_row(
                                &format!("SELECT {} FROM vault_entries WHERE id = ?1", col),
                                params![eid],
                                |row| row.get(0),
                            )
                            .map_err(|e| VaultError::Database(e.to_string()))?;
                        match encrypted {
                            Some(enc) => {
                                let pt = VaultCrypto::decrypt_field(&enc, old_entry_key.expose())
                                    .map_err(VaultError::Crypto)?;
                                let re_enc = VaultCrypto::encrypt_field(
                                    &pt,
                                    new_entry_key.expose(),
                                    CipherAlgorithm::Aes256Gcm,
                                )
                                .map_err(VaultError::Crypto)?;
                                Ok(Some(re_enc))
                            }
                            None => Ok(None),
                        }
                    };

                    let new_password_enc = re_encrypt("encrypted_password")?;
                    let new_notes_enc = re_encrypt("encrypted_notes")?;
                    let new_totp_enc = re_encrypt("encrypted_totp_secret")?;

                    let new_custom_fields_enc: Option<String> = {
                        let cf_json: Option<String> = conn
                            .query_row(
                                "SELECT encrypted_custom_fields FROM vault_entries WHERE id = ?1",
                                params![eid],
                                |row| row.get(0),
                            )
                            .map_err(|e| VaultError::Database(e.to_string()))?;
                        match cf_json {
                            Some(json) => {
                                let fields: Vec<serde_json::Value> = serde_json::from_str(&json)
                                    .map_err(|e| VaultError::Database(e.to_string()))?;
                                let re_encrypted: Vec<serde_json::Value> = fields
                                    .iter()
                                    .map(|f| {
                                        if f.get("isProtected").and_then(|v| v.as_bool())
                                            == Some(true)
                                        {
                                            if let Some(enc_val) =
                                                f.get("valueEncrypted").and_then(|v| v.as_str())
                                            {
                                                match VaultCrypto::decrypt_field(
                                                    enc_val,
                                                    old_entry_key.expose(),
                                                ) {
                                                    Ok(pt) => {
                                                        match VaultCrypto::encrypt_field(
                                                            &pt,
                                                            new_entry_key.expose(),
                                                            CipherAlgorithm::Aes256Gcm,
                                                        ) {
                                                            Ok(re_enc) => serde_json::json!({
                                                                "name": f.get("name"),
                                                                "valueEncrypted": re_enc,
                                                                "isProtected": true
                                                            }),
                                                            Err(_) => f.clone(),
                                                        }
                                                    }
                                                    Err(_) => f.clone(),
                                                }
                                            } else {
                                                f.clone()
                                            }
                                        } else {
                                            f.clone()
                                        }
                                    })
                                    .collect();
                                Some(
                                    serde_json::to_string(&re_encrypted)
                                        .map_err(|e| VaultError::Validation(e.to_string()))?,
                                )
                            }
                            None => None,
                        }
                    };

                    conn.execute(
                        "UPDATE vault_entries SET encrypted_password = ?1, encrypted_notes = ?2, encrypted_totp_secret = ?3, encrypted_custom_fields = ?4 WHERE id = ?5",
                        params![new_password_enc, new_notes_enc, new_totp_enc, new_custom_fields_enc, eid],
                    ).map_err(|e| VaultError::Database(e.to_string()))?;
                }
                Ok(())
            })();

            if let Err(e) = re_encrypt_result {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }

            meta::set_salt(conn, "master_key_salt", &new_master_key_salt)?;
            meta::set_salt(conn, "hash_salt", &new_hash_salt)?;
            meta::set_master_key_hash(conn, &new_master_key_hash)?;

            if let Some(test_id) = entry_ids.first() {
                let test_entry_key = VaultCrypto::derive_entry_key(&new_master_key, test_id);
                let test_enc: Option<String> = conn
                    .query_row(
                        "SELECT encrypted_password FROM vault_entries WHERE id = ?1",
                        params![test_id],
                        |row| row.get(0),
                    )
                    .ok()
                    .flatten();
                if let Some(enc) = test_enc {
                    if VaultCrypto::decrypt_field(&enc, test_entry_key.expose()).is_err() {
                        let _ = conn.execute_batch("ROLLBACK");
                        return Err(VaultError::Crypto(
                            "Verification failed after re-encryption, transaction rolled back"
                                .to_string(),
                        ));
                    }
                }
            }

            conn.execute_batch("COMMIT")
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }

        let new_bootstrap_key_bytes = VaultCrypto::derive_master_key(new_password, &BOOTSTRAP_SALT)
            .map_err(VaultError::Crypto)?;
        let new_bootstrap_key = MasterKey::new(new_bootstrap_key_bytes);
        let new_sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&new_bootstrap_key);
        let new_sqlcipher_key_hex = hex::encode(new_sqlcipher_key);
        conn.execute_batch(&format!("PRAGMA rekey = \"x'{}'\"", new_sqlcipher_key_hex))
            .map_err(|e| {
                VaultError::Database(format!(
                    "PRAGMA rekey failed: {}. Backup at: {}",
                    e,
                    backup_path.display()
                ))
            })?;
        {
            let mut key_arr = new_sqlcipher_key;
            key_arr.zeroize();
        }

        {
            let mut mk_guard = self.master_key.lock().unwrap();
            *mk_guard = Some(new_master_key);
        }

        let _ = audit_log(conn, AuditEvent::MasterPasswordChange, None);
        info!("Master password changed successfully (PRAGMA rekey + entry re-encryption + custom_fields re-encryption)");

        let key_store = crate::biometric::create_key_store();
        let _ = self.disenroll_biometric(key_store.as_ref());

        Ok(())
    }

    pub fn export_entries(&self) -> Result<Vec<VaultEntryExport>, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, title, username, url, category, encrypted_password, \
                     encrypted_notes, encrypted_totp_secret, encrypted_custom_fields, \
                     tags, is_favorite, created_at, updated_at \
                     FROM vault_entries WHERE deleted_at IS NULL",
                )
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>("id")?,
                        row.get::<_, String>("title")?,
                        row.get::<_, Option<String>>("username")?,
                        row.get::<_, Option<String>>("url")?,
                        row.get::<_, String>("category")?,
                        row.get::<_, Option<String>>("encrypted_password")?,
                        row.get::<_, Option<String>>("encrypted_notes")?,
                        row.get::<_, Option<String>>("encrypted_totp_secret")?,
                        row.get::<_, Option<String>>("encrypted_custom_fields")?,
                        row.get::<_, String>("tags")?,
                        row.get::<_, i64>("is_favorite")?,
                        row.get::<_, i64>("created_at")?,
                        row.get::<_, i64>("updated_at")?,
                    ))
                })
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let mut exports = Vec::new();
            for row in rows {
                let (
                    id,
                    title,
                    username,
                    url,
                    category_str,
                    enc_pw,
                    enc_notes,
                    enc_totp,
                    enc_cf,
                    tags_json,
                    is_fav,
                    created,
                    updated,
                ) = row.map_err(|e| VaultError::Database(e.to_string()))?;

                let entry_key = VaultCrypto::derive_entry_key(mk, &id);

                let password = enc_pw
                    .map(|enc| {
                        VaultCrypto::decrypt_field(&enc, entry_key.expose()).and_then(|pt| {
                            pt.to_str()
                                .map(String::from)
                                .map_err(|_| "UTF-8 error".to_string())
                        })
                    })
                    .transpose()
                    .ok()
                    .flatten();

                let notes = enc_notes
                    .map(|enc| {
                        VaultCrypto::decrypt_field(&enc, entry_key.expose()).and_then(|pt| {
                            pt.to_str()
                                .map(String::from)
                                .map_err(|_| "UTF-8 error".to_string())
                        })
                    })
                    .transpose()
                    .ok()
                    .flatten();

                let totp_secret = enc_totp
                    .map(|enc| {
                        VaultCrypto::decrypt_field(&enc, entry_key.expose()).and_then(|pt| {
                            pt.to_str()
                                .map(String::from)
                                .map_err(|_| "UTF-8 error".to_string())
                        })
                    })
                    .transpose()
                    .ok()
                    .flatten();

                let custom_fields: Vec<crate::types::CustomFieldInput> = enc_cf
                    .and_then(|json| {
                        let fields: Vec<serde_json::Value> = serde_json::from_str(&json).ok()?;
                        Some(
                            fields
                                .iter()
                                .filter_map(|f| {
                                    let name = f.get("name")?.as_str()?.to_string();
                                    if f.get("isProtected").and_then(|v| v.as_bool()) == Some(true)
                                    {
                                        let enc = f.get("valueEncrypted")?.as_str()?;
                                        let pt =
                                            VaultCrypto::decrypt_field(enc, entry_key.expose())
                                                .ok()?;
                                        let val = pt.to_str().ok()?.to_string();
                                        Some(crate::types::CustomFieldInput {
                                            name,
                                            value: val,
                                            is_protected: true,
                                        })
                                    } else {
                                        let val = f.get("value")?.as_str()?.to_string();
                                        Some(crate::types::CustomFieldInput {
                                            name,
                                            value: val,
                                            is_protected: false,
                                        })
                                    }
                                })
                                .collect(),
                        )
                    })
                    .unwrap_or_default();

                exports.push(VaultEntryExport {
                    category: VaultCategory::from_str(&category_str).unwrap_or_default(),
                    title,
                    username,
                    url,
                    password,
                    notes,
                    totp_secret,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    custom_fields,
                    is_favorite: is_fav != 0,
                    created_at: created,
                    updated_at: updated,
                });
            }
            Ok(exports)
        })
    }

    pub fn import_entries(&self, entries: Vec<VaultEntryCreate>) -> Result<u32, VaultError> {
        let mut count = 0u32;
        for entry in entries {
            self.create_entry(entry)?;
            count += 1;
        }
        Ok(count)
    }

    fn biometric_key_path(&self) -> PathBuf {
        Path::new(&self.db_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("biometric-key.enc")
    }

    fn unlock_with_master_key(&self, master_key: MasterKey) -> Result<(), VaultError> {
        let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&master_key);
        let sqlcipher_key_hex = hex::encode(sqlcipher_key);

        let path = Path::new(&self.db_path);
        let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\"", sqlcipher_key_hex))
            .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
        conn.execute_batch(SQLCIPHER_PRAGMAS)
            .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;

        {
            let mut key_arr = sqlcipher_key;
            key_arr.zeroize();
        }

        let _hash_salt = meta::get_salt(&conn, "hash_salt")?
            .ok_or_else(|| VaultError::Database("hash_salt not found".to_string()))?;

        *self.master_key.lock().unwrap() = Some(master_key);
        *self.conn.lock().unwrap() = Some(conn);

        Ok(())
    }

    pub fn is_biometric_available(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        key_store.is_available()
    }

    pub fn enroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if self.is_locked() {
            return Err(VaultError::Locked);
        }

        if !key_store.is_available()? {
            return Err(VaultError::BiometricUnavailable);
        }

        if self.is_biometric_enrolled(key_store)? {
            self.disenroll_biometric(key_store)?;
        }

        let mk_guard = self.master_key.lock().unwrap();
        let master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;

        #[cfg(target_os = "macos")]
        {
            key_store.set_key("vault-master-key", master_key.expose())?;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let mut wrapping_key = [0u8; 32];
            rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);

            let nonce: [u8; 12] = rand::thread_rng().gen();
            let encrypted_master_key = VaultCrypto::encrypt_field(
                &VaultPlaintext::from(master_key.expose().to_vec()),
                &wrapping_key,
                CipherAlgorithm::Aes256Gcm,
            )
            .map_err(VaultError::Crypto)?;

            let mut combined = Vec::with_capacity(12 + encrypted_master_key.len());
            combined.extend_from_slice(&nonce);
            combined.extend_from_slice(encrypted_master_key.as_bytes());
            let biometric_key_enc = B64.encode(&combined);

            key_store.set_key("vault-master-key", &wrapping_key)?;

            let biometric_path = self.biometric_key_path();
            std::fs::write(&biometric_path, &biometric_key_enc)?;

            wrapping_key.zeroize();
        }

        {
            let guard = self.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::BiometricEnrolled, None);
            }
        }

        info!("Biometric enrolled successfully");
        Ok(())
    }

    pub fn unlock_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if !self.is_locked() {
            return Err(VaultError::AlreadyUnlocked);
        }

        if !key_store.is_available()? {
            return Err(VaultError::BiometricUnavailable);
        }

        #[cfg(target_os = "macos")]
        {
            let master_key_bytes = key_store.get_key("vault-master-key")?;
            if master_key_bytes.len() != 32 {
                return Err(VaultError::Crypto(
                    "Invalid master key length from biometric decrypt".to_string(),
                ));
            }
            let mut mk_arr = [0u8; 32];
            mk_arr.copy_from_slice(&master_key_bytes);
            let master_key = MasterKey::new(mk_arr);

            self.unlock_with_master_key(master_key)?;

            {
                let guard = self.conn.lock().unwrap();
                if let Some(c) = guard.as_ref() {
                    let _ = audit_log(c, AuditEvent::VaultUnlock, Some("biometric"));
                }
            }

            info!("Vault unlocked via biometric (macOS single-layer)");
        }

        #[cfg(not(target_os = "macos"))]
        {
            let biometric_path = self.biometric_key_path();
            if !biometric_path.exists() {
                return Err(VaultError::NotFound("biometric_key_enc".to_string()));
            }

            let wrapping_key_vec = key_store.get_key("vault-master-key")?;
            if wrapping_key_vec.len() != 32 {
                return Err(VaultError::Crypto(
                    "Invalid wrapping key length from biometric store".to_string(),
                ));
            }
            let mut wrapping_key = [0u8; 32];
            wrapping_key.copy_from_slice(&wrapping_key_vec);

            let biometric_key_enc = std::fs::read_to_string(&biometric_path)?;

            let combined = B64.decode(&biometric_key_enc).map_err(|e| {
                VaultError::Crypto(format!("Failed to decode biometric_key_enc: {}", e))
            })?;

            if combined.len() < 12 {
                return Err(VaultError::Crypto(
                    "biometric_key_enc too short".to_string(),
                ));
            }

            let encrypted_mk_str = String::from_utf8_lossy(&combined[12..]).to_string();

            let master_key_plaintext = VaultCrypto::decrypt_field(&encrypted_mk_str, &wrapping_key)
                .map_err(|e| {
                    VaultError::Crypto(format!("Biometric key decryption failed: {}", e))
                })?;

            if master_key_plaintext.expose().len() != 32 {
                return Err(VaultError::Crypto(
                    "Invalid master key length from biometric decrypt".to_string(),
                ));
            }

            let mut mk_arr = [0u8; 32];
            mk_arr.copy_from_slice(master_key_plaintext.expose());
            let master_key = MasterKey::new(mk_arr);

            self.unlock_with_master_key(master_key)?;

            self.check_biometric_consistency(key_store)?;

            {
                let guard = self.conn.lock().unwrap();
                if let Some(c) = guard.as_ref() {
                    let _ = audit_log(c, AuditEvent::VaultUnlock, Some("biometric"));
                }
            }

            wrapping_key.zeroize();

            info!("Vault unlocked via biometric (two-layer)");
        }

        Ok(())
    }

    pub fn disenroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        let _ = key_store.delete_key("vault-master-key");

        #[cfg(not(target_os = "macos"))]
        {
            let biometric_path = self.biometric_key_path();
            if biometric_path.exists() {
                let _ = std::fs::remove_file(&biometric_path);
            }
        }

        {
            let guard = self.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultLock, Some("biometric_disenrolled"));
            }
        }

        info!("Biometric disenrolled");
        Ok(())
    }

    pub fn is_biometric_enrolled(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        #[cfg(target_os = "macos")]
        {
            return key_store.has_key("vault-master-key");
        }

        #[cfg(not(target_os = "macos"))]
        {
            let has_os_key = key_store.has_key("vault-master-key")?;
            let has_enc_file = self.biometric_key_path().exists();
            Ok(has_os_key && has_enc_file)
        }
    }

    pub fn check_biometric_consistency(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        #[cfg(target_os = "macos")]
        {
            return Ok(());
        }

        #[cfg(not(target_os = "macos"))]
        {
            let has_os_key = key_store.has_key("vault-master-key")?;
            let has_enc_file = self.biometric_key_path().exists();

            match (has_os_key, has_enc_file) {
                (true, true) => {}
                (true, false) => {
                    let mk_guard = self.master_key.lock().unwrap();
                    let master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;

                    let mut wrapping_key_vec = key_store.get_key("vault-master-key")?;
                    if wrapping_key_vec.len() != 32 {
                        let _ = self.disenroll_biometric(key_store);
                        return Ok(());
                    }
                    let mut wrapping_key = [0u8; 32];
                    wrapping_key.copy_from_slice(&wrapping_key_vec);

                    let nonce: [u8; 12] = rand::thread_rng().gen();
                    let encrypted_master_key = VaultCrypto::encrypt_field(
                        &VaultPlaintext::from(master_key.expose().to_vec()),
                        &wrapping_key,
                        CipherAlgorithm::Aes256Gcm,
                    )
                    .map_err(VaultError::Crypto)?;

                    let mut combined = Vec::with_capacity(12 + encrypted_master_key.len());
                    combined.extend_from_slice(&nonce);
                    combined.extend_from_slice(encrypted_master_key.as_bytes());
                    let biometric_key_enc = B64.encode(&combined);

                    let biometric_path = self.biometric_key_path();
                    std::fs::write(&biometric_path, &biometric_key_enc)?;

                    wrapping_key.zeroize();
                    wrapping_key_vec.zeroize();

                    info!("Biometric self-healing: regenerated biometric-key.enc");
                }
                (false, true) => {
                    let biometric_path = self.biometric_key_path();
                    let _ = std::fs::remove_file(&biometric_path);
                    info!("Biometric self-healing: removed orphaned biometric-key.enc");
                }
                (false, false) => {}
            }

            Ok(())
        }
    }
}
