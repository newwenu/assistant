use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::info;
use rand::Rng;
use rusqlite::Connection;
use zeroize::Zeroize;

use crate::audit::{log as audit_log, AuditEvent};
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_crypto::vault_crypto::VaultCrypto;

pub mod biometric_ops;
pub mod crypto_ops;
pub mod entries;
pub mod import_export;
pub mod meta;
pub mod rekey;
pub mod schema;

const BOOTSTRAP_SALT: [u8; 32] = [
    0x56, 0x61, 0x75, 0x6C, 0x74, 0x42, 0x6F, 0x6F, 0x74, 0x73, 0x74, 0x72, 0x61, 0x70, 0x53, 0x61,
    0x6C, 0x74, 0x5F, 0x32, 0x30, 0x32, 0x35, 0x5F, 0x41, 0x72, 0x67, 0x6F, 0x6E, 0x32, 0x69, 0x64,
];

const SQLCIPHER_PRAGMAS: &str =
    "PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;";

const MAX_BACKUPS: usize = 5;

const BACKUP_PREFIX: &str = "security-";

const BACKUP_SUFFIX: &str = ".db.bak";

pub struct SecurityDbState {
    conn: Mutex<Option<Connection>>,
    master_key: Mutex<Option<MasterKey>>,
    bootstrap_key: Mutex<Option<MasterKey>>,
    db_path: String,
}

impl SecurityDbState {
    pub fn new(db_path: &str) -> Self {
        Self {
            conn: Mutex::new(None),
            master_key: Mutex::new(None),
            bootstrap_key: Mutex::new(None),
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
        *self.master_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(master_key);
        *self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(bootstrap_key);
        *self.conn.lock().unwrap_or_else(|e| e.into_inner()) = Some(conn);

        info!("Vault initialized successfully (SQLCipher encrypted, two-phase unlock)");
        Ok(())
    }

    pub fn unlock(&self, password: &str) -> Result<(), VaultError> {
        if self
            .master_key
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
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
        let bootstrap_key = MasterKey::new(bootstrap_key_bytes);
        *self.master_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(master_key);
        *self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(bootstrap_key);
        *self.conn.lock().unwrap_or_else(|e| e.into_inner()) = Some(conn);

        {
            let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(c) = guard.as_ref() {
                let _ = audit_log(c, AuditEvent::VaultUnlock, None);
            }
        }

        info!("Vault unlocked successfully");
        Ok(())
    }

    pub fn lock(&self) -> Result<(), VaultError> {
        {
            let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultLock, None);
            }
        }

        {
            let mut mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
            *mk_guard = None;
        }
        {
            let mut bk_guard = self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner());
            *bk_guard = None;
        }
        {
            let mut conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            *conn_guard = None;
        }

        info!("Vault locked");
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.master_key
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
    }

    pub fn get_conn(&self) -> Result<std::sync::MutexGuard<'_, Option<Connection>>, VaultError> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(guard)
    }

    pub fn get_master_key(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<MasterKey>>, VaultError> {
        let guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(guard)
    }

    pub fn backup(&self, db_path: &Path) -> Result<PathBuf, VaultError> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = guard.as_ref().ok_or(VaultError::Locked)?;
        Self::backup_conn(conn, db_path)
    }

    fn backup_conn(conn: &rusqlite::Connection, db_path: &Path) -> Result<PathBuf, VaultError> {
        let backup_dir = db_path.parent().unwrap_or(Path::new(".")).join("backups");
        std::fs::create_dir_all(&backup_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
        let mut seq = 0u32;
        let mut backup_path = backup_dir.join(format!(
            "{}{}-{:03}{}",
            BACKUP_PREFIX, timestamp, seq, BACKUP_SUFFIX
        ));
        while backup_path.exists() {
            seq += 1;
            backup_path = backup_dir.join(format!(
                "{}{}-{:03}{}",
                BACKUP_PREFIX, timestamp, seq, BACKUP_SUFFIX
            ));
        }

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

        Self::prune_backups(&backup_dir, MAX_BACKUPS)?;

        Ok(backup_path)
    }

    fn prune_backups(dir: &Path, keep: usize) -> Result<(), VaultError> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(BACKUP_PREFIX) && name.ends_with(BACKUP_SUFFIX))
                    .unwrap_or(false)
            })
            .collect();

        if files.len() <= keep {
            return Ok(());
        }

        files.sort();
        let excess = files.len() - keep;
        for path in files.into_iter().take(excess) {
            if let Err(err) = std::fs::remove_file(&path) {
                log::warn!("Failed to prune old backup {}: {}", path.display(), err);
            }
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        Ok(Path::new(&self.db_path).exists())
    }

    pub fn cipher_algorithm_from_conf(conf_val: &str) -> CipherAlgorithm {
        match conf_val.to_ascii_lowercase().as_str() {
            "chacha20poly1305" => CipherAlgorithm::ChaCha20Poly1305,
            _ => CipherAlgorithm::Aes256Gcm,
        }
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    pub fn set_conn_and_key(
        &self,
        conn: Connection,
        master_key: MasterKey,
        bootstrap_key: MasterKey,
    ) {
        *self.conn.lock().unwrap_or_else(|e| e.into_inner()) = Some(conn);
        *self.master_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(master_key);
        *self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(bootstrap_key);
    }

    pub fn get_bootstrap_key(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<MasterKey>>, VaultError> {
        let guard = self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(guard)
    }

    fn with_conn_and_key<F, T>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection, &MasterKey) -> Result<T, VaultError>,
    {
        let mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
        let mk = mk_guard.as_ref().ok_or(VaultError::Locked)?;
        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;
        f(conn, mk)
    }
}
