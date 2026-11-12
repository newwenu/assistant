use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use log::info;
use rusqlite::Connection;

use crate::audit::{log_dual, AuditEvent, AuditJsonlWriter};
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::derive_meta_key;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, DbKey, Dek, Kek, MetaKey};

pub mod biometric_ops;
pub mod crypto_ops;
pub mod entries;
pub mod import_export;
pub mod meta;
pub mod rekey;
pub mod schema;

pub use meta::Anchor;

pub const SQLCIPHER_PRAGMAS: &str =
    "PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter=1; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;";

/// 内存加固：密钥材料用后即清、临时表不落盘。必须在 `PRAGMA key` 之前生效。
pub const SQLCIPHER_HARDENING_PRAGMAS: &str =
    "PRAGMA cipher_memory_security = ON; PRAGMA temp_store = MEMORY;";

const MAX_BACKUPS: usize = 5;

/// JSONL 补录游标：`vault_meta` 的 KV 行，不改 DDL。
const JSONL_SYNC_KEY: &str = "audit_jsonl_synced_at";

/// 只补录「库不可用期间发生」的事件；其余事件解锁态本来就能直接写库。
const BACKFILL_EVENTS: &[&str] = &[
    "recovery_export_used",
    "dek_rotated",
    "local_db_rebuilt",
    "vault_repository_reset",
];

const BACKUP_PREFIX: &str = "security-";

const BACKUP_SUFFIX: &str = ".db.bak";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    User,
    AutoLock,
    ScreenLock,
    AppExit,
    Reset,
    Panic,
}

impl LockReason {
    /// 审计 detail 用的稳定字符串（封闭集合，禁止自由文本）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::AutoLock => "auto_lock",
            Self::ScreenLock => "screen_lock",
            Self::AppExit => "app_exit",
            Self::Reset => "reset",
            Self::Panic => "panic",
        }
    }
}

pub struct AnchorExpectation {
    pub meta_hash: String,
    pub dek_version: u32,
}

impl AnchorExpectation {
    pub fn new(meta_hash: impl Into<String>, dek_version: u32) -> Self {
        Self {
            meta_hash: meta_hash.into(),
            dek_version,
        }
    }

    pub fn from_anchor(anchor: &Anchor) -> Self {
        Self {
            meta_hash: anchor.meta_hash.clone(),
            dek_version: anchor.dek_version,
        }
    }
}

pub struct VaultSession {
    pub(crate) conn: Connection,
    pub(crate) dek: Dek,
    pub(crate) db_key: DbKey,
    pub(crate) meta_key: MetaKey,
    pub(crate) dek_version: u32,
    pub(crate) epoch: u64,
    pub(crate) kek: Option<Kek>,
}

impl VaultSession {
    pub fn new(conn: Connection, dek: Dek, db_key: DbKey, dek_version: u32) -> Self {
        let meta_key = derive_meta_key(&dek);
        Self {
            conn,
            dek,
            db_key,
            meta_key,
            dek_version,
            epoch: 0,
            kek: None,
        }
    }

    /// 口令解锁/初始化时缓存 `kek`：`rotate_dek` 重包 `dek_new` 需要当前 `KEK`，
    /// 生物识别与恢复码会话没有 `KEK`，此时 `kek` 为 `None`。
    pub fn with_kek(mut self, kek: Kek) -> Self {
        self.kek = Some(kek);
        self
    }

    pub fn kek(&self) -> Option<&Kek> {
        self.kek.as_ref()
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn dek(&self) -> &Dek {
        &self.dek
    }

    pub fn db_key(&self) -> &DbKey {
        &self.db_key
    }

    pub fn meta_key(&self) -> &MetaKey {
        &self.meta_key
    }

    pub fn dek_version(&self) -> u32 {
        self.dek_version
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

pub struct SecurityDbState {
    session: Mutex<Option<VaultSession>>,
    db_path: String,
    audit_dir: PathBuf,
    epoch: AtomicU64,
}

pub struct SessionConnGuard<'a> {
    guard: MutexGuard<'a, Option<VaultSession>>,
}

impl<'a> SessionConnGuard<'a> {
    pub fn as_ref(&self) -> Option<&Connection> {
        self.guard.as_ref().map(|session| &session.conn)
    }
}

pub struct SessionDekGuard<'a> {
    guard: MutexGuard<'a, Option<VaultSession>>,
}

impl<'a> SessionDekGuard<'a> {
    pub fn as_ref(&self) -> Option<&Dek> {
        self.guard.as_ref().map(|session| &session.dek)
    }

    pub fn dek_version(&self) -> Option<u32> {
        self.guard.as_ref().map(|session| session.dek_version)
    }
}

impl SecurityDbState {
    pub fn new(db_path: &str) -> Self {
        // 审计 JSONL 与 `security.db` 同级（`<data>/security/audit/`），不进仓库、不加密。
        let audit_dir = Path::new(db_path)
            .parent()
            .map(|parent| parent.join("audit"))
            .unwrap_or_else(|| PathBuf::from("audit"));
        Self {
            session: Mutex::new(None),
            db_path: db_path.to_string(),
            audit_dir,
            epoch: AtomicU64::new(0),
        }
    }

    pub fn audit_dir(&self) -> &Path {
        &self.audit_dir
    }

    pub fn audit_writer(&self) -> AuditJsonlWriter {
        AuditJsonlWriter::new(self.audit_dir.clone())
    }

    /// 双写审计：已解锁时同时写 DB 与 JSONL，未解锁时只写 JSONL。
    ///
    /// 恢复通道（无连接）用 [`Self::audit_jsonl`]，不要走这里。
    pub fn audit_dual(&self, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError> {
        let writer = self.audit_writer();
        let guard = self.get_conn().ok();
        let conn = guard.as_ref().and_then(|guard| guard.as_ref());
        log_dual(conn, Some(&writer), event, detail)
    }

    /// 只写 JSONL：库不可用（恢复通道、重置后）的审计落点。
    pub fn audit_jsonl(&self, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError> {
        let writer = self.audit_writer();
        log_dual(None, Some(&writer), event, detail)
    }

    /// 解锁后补录 JSONL 中「库不可用期间」产生的事件。
    ///
    /// 去重键 `jsonl-{at}-{event}` 保证重复解锁不重复插入；失败只 `warn`，不阻断解锁。
    fn backfill_jsonl_audit(&self, conn: &Connection) {
        if let Err(e) = self.try_backfill_jsonl_audit(conn) {
            log::warn!("Audit jsonl backfill failed: {}", e);
        }
    }

    fn try_backfill_jsonl_audit(&self, conn: &Connection) -> Result<(), VaultError> {
        let since_ms = meta::get_meta(conn, JSONL_SYNC_KEY)
            .map_err(VaultError::Database)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let now = crate::types::now_millis();
        let records = self.audit_writer().read_since(since_ms)?;

        for record in records
            .iter()
            .filter(|record| BACKFILL_EVENTS.contains(&record.event.as_str()))
        {
            let id = format!("jsonl-{}-{}", record.at, record.event);
            conn.execute(
                "INSERT OR IGNORE INTO security_audit_log (id, event_type, detail, timestamp)
                 SELECT ?1, ?2, ?3, ?4
                 WHERE NOT EXISTS (
                     SELECT 1 FROM security_audit_log WHERE event_type = ?2 AND timestamp = ?4
                 )",
                rusqlite::params![id, record.event, record.detail, record.at],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        }

        meta::set_meta(conn, JSONL_SYNC_KEY, &now.to_string()).map_err(VaultError::Database)?;
        Ok(())
    }

    pub fn init(
        &self,
        dek: Dek,
        db_key: DbKey,
        dek_version: u32,
        vault_id: &str,
        anchor: &Anchor,
        kek: Option<Kek>,
    ) -> Result<(), VaultError> {
        let path = Path::new(&self.db_path);
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }
        if self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return Err(VaultError::AlreadyUnlocked);
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = open_connection(path, &db_key)?;
        conn.execute_batch(schema::INIT_SQL)
            .map_err(|e| VaultError::Database(e.to_string()))?;
        meta::ensure_schema_version(&conn).map_err(VaultError::Database)?;
        meta::set_vault_id(&conn, vault_id).map_err(VaultError::Database)?;
        meta::set_anchor(&conn, anchor).map_err(VaultError::Database)?;

        let epoch = self.epoch.load(Ordering::SeqCst);
        let mut session = VaultSession::new(conn, dek, db_key, dek_version);
        session.kek = kek;
        session.epoch = epoch;
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);

        info!(
            "Vault initialized successfully (dek session, anchor revision {})",
            anchor.revision
        );
        Ok(())
    }

    pub fn unlock(
        &self,
        session: VaultSession,
        expected: &AnchorExpectation,
    ) -> Result<(), VaultError> {
        if self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return Err(VaultError::AlreadyUnlocked);
        }

        probe_read(&session.conn)?;

        let anchor = meta::get_anchor(&session.conn).map_err(VaultError::Database)?;
        let anchor = anchor.ok_or(VaultError::VaultRollbackDetected)?;
        if anchor.meta_hash != expected.meta_hash || anchor.dek_version != expected.dek_version {
            let writer = self.audit_writer();
            let _ = log_dual(
                Some(&session.conn),
                Some(&writer),
                AuditEvent::AnchorRollbackRejected,
                Some("meta_or_dek_version"),
            );
            return Err(VaultError::VaultRollbackDetected);
        }

        let writer = self.audit_writer();
        let _ = log_dual(
            Some(&session.conn),
            Some(&writer),
            AuditEvent::VaultUnlock,
            None,
        );
        self.backfill_jsonl_audit(&session.conn);

        let epoch = self.epoch.load(Ordering::SeqCst);
        let mut session = session;
        session.epoch = epoch;
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session);

        info!("Vault unlocked successfully (epoch {})", epoch);
        Ok(())
    }

    pub fn set_session(&self, mut session: VaultSession) -> Result<(), VaultError> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            return Err(VaultError::AlreadyUnlocked);
        }
        session.epoch = self.epoch.load(Ordering::SeqCst);
        *guard = Some(session);
        Ok(())
    }

    pub fn lock_session(&self, reason: LockReason) -> Result<(), VaultError> {
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;

        let taken = {
            let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            guard.take()
        };

        let Some(session) = taken else {
            info!("Vault already locked ({:?})", reason);
            return Ok(());
        };

        let writer = self.audit_writer();
        let _ = log_dual(
            Some(&session.conn),
            Some(&writer),
            AuditEvent::VaultLock,
            Some(reason.as_str()),
        );

        let VaultSession {
            conn,
            dek,
            db_key,
            meta_key,
            dek_version: _,
            epoch: _,
            kek,
        } = session;

        if let Err((returned, err)) = conn.close() {
            log::warn!("Failed to close SQLCipher connection: {}", err);
            drop(returned);
        }
        drop(dek);
        drop(db_key);
        drop(meta_key);
        drop(kek);

        info!("Vault locked ({:?}, epoch {})", reason, epoch);
        Ok(())
    }

    pub fn lock(&self) -> Result<(), VaultError> {
        self.lock_session(LockReason::User)
    }

    pub fn is_locked(&self) -> bool {
        self.session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
    }

    pub fn current_epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    pub fn with_session<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&VaultSession) -> Result<T, VaultError>,
    {
        let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let session = guard.as_ref().ok_or(VaultError::Locked)?;
        let epoch = session.epoch;
        let result = f(session)?;
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(VaultError::Locked);
        }
        Ok(result)
    }

    pub fn with_session_mut<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&mut VaultSession) -> Result<T, VaultError>,
    {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let session = guard.as_mut().ok_or(VaultError::Locked)?;
        let epoch = session.epoch;
        let result = f(session)?;
        if self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(VaultError::Locked);
        }
        Ok(result)
    }

    pub fn with_conn<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection) -> Result<T, VaultError>,
    {
        self.with_session(|session| f(&session.conn))
    }

    pub(crate) fn with_conn_and_dek<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection, &Dek, u32) -> Result<T, VaultError>,
    {
        self.with_session(|session| f(&session.conn, &session.dek, session.dek_version))
    }

    pub fn get_conn(&self) -> Result<SessionConnGuard<'_>, VaultError> {
        let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(SessionConnGuard { guard })
    }

    pub fn get_dek(&self) -> Result<SessionDekGuard<'_>, VaultError> {
        let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            return Err(VaultError::Locked);
        }
        Ok(SessionDekGuard { guard })
    }

    pub fn vault_id(&self) -> Result<Option<String>, VaultError> {
        self.with_conn(|conn| meta::get_vault_id(conn).map_err(VaultError::Database))
    }

    pub fn set_vault_id(&self, vault_id: &str) -> Result<(), VaultError> {
        self.with_conn(|conn| meta::set_vault_id(conn, vault_id).map_err(VaultError::Database))
    }

    pub fn refresh_anchor(&self, meta_hash: &str, dek_version: u32) -> Result<(), VaultError> {
        self.with_conn(|conn| {
            let revision = meta::get_anchor(conn)
                .map_err(VaultError::Database)?
                .map(|anchor| anchor.revision)
                .unwrap_or(0)
                .saturating_add(1);
            let anchor = Anchor {
                meta_hash: meta_hash.to_string(),
                dek_version,
                revision,
            };
            meta::set_anchor(conn, &anchor).map_err(VaultError::Database)
        })
    }

    pub fn backup(&self, db_path: &Path) -> Result<PathBuf, VaultError> {
        self.with_session(|session| Self::backup_conn(&session.conn, db_path))
    }

    fn backup_conn(conn: &Connection, db_path: &Path) -> Result<PathBuf, VaultError> {
        let backup_dir = Self::backup_dir_for(db_path)?;
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

    fn backup_dir_for(db_path: &Path) -> Result<PathBuf, VaultError> {
        // 备份目录以安全库所在目录为根：路径必须是绝对路径且父目录非空，
        // 否则 `backups/` 会落到进程 cwd，产生调用方不可见的副本。
        let parent = db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                VaultError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Security db path has no parent directory",
                ))
            })?;
        if !parent.is_absolute() {
            return Err(VaultError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Security db path must be absolute",
            )));
        }
        Ok(parent.join("backups"))
    }

    pub(crate) fn purge_backups(db_path: &Path) -> Result<usize, VaultError> {
        let backup_dir = Self::backup_dir_for(db_path)?;
        if !backup_dir.exists() {
            return Ok(0);
        }

        let mut removed = 0usize;
        for entry in std::fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_backup = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(BACKUP_PREFIX) && name.ends_with(BACKUP_SUFFIX))
                .unwrap_or(false);
            if is_backup {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
        }
        Ok(removed)
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
}

pub(crate) fn open_connection(path: &Path, db_key: &DbKey) -> Result<Connection, VaultError> {
    let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
    apply_db_key(&conn, db_key)?;
    Ok(conn)
}

pub(crate) fn apply_db_key(conn: &Connection, db_key: &DbKey) -> Result<(), VaultError> {
    conn.execute_batch(SQLCIPHER_HARDENING_PRAGMAS)
        .map_err(|e| VaultError::Database(format!("PRAGMA hardening failed: {}", e)))?;
    let key_hex = hex::encode(db_key.expose());
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex))
        .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
    conn.execute_batch(SQLCIPHER_PRAGMAS)
        .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;
    Ok(())
}

pub(crate) fn probe_read(conn: &Connection) -> Result<(), VaultError> {
    conn.query_row(
        "SELECT value FROM vault_meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .map(|_| ())
    .map_err(|_| VaultError::LocalDbKeyMismatch)
}
