use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use log::info;
use rusqlite::{Connection, OptionalExtension};

use crate::audit::{log_dual, AuditEvent, AuditJsonlWriter};
use crate::storage_stats;
pub use crate::storage_stats::DirStats;
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, DbKey, Dek, Kek};

use super::schema::{open_connection, probe_read};
use super::{
    meta, schema, Anchor, AnchorExpectation, LockReason, SessionConnGuard, SessionDekGuard,
    VaultSession, BACKFILL_EVENTS, BACKUP_PREFIX, BACKUP_SUFFIX, JSONL_SYNC_KEY, MAX_BACKUPS,
};

/// `E8` 的裁剪结果：只用于回收结果回报，不参与任何业务判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
}

pub struct SecurityDbState {
    pub(super) session: Mutex<Option<VaultSession>>,
    db_path: String,
    audit_dir: PathBuf,
    epoch: AtomicU64,
    audit_retention_days: AtomicU32,
    /// 保留策略的生效时刻（秒）：策略注入时写入一次，之后只读。
    ///
    /// 与 `epoch` 分开：`epoch` 是会话代次（锁/解锁递增），这里是「策略从何时开始算」，
    /// 观测条目 `E11` 的 `policy.effective_since` 直接用它，不重新读时钟。
    audit_effective_since: AtomicI64,
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
            // 未注入前不清理：库层不读配置，也不在无显式保留策略时删用户数据。
            audit_retention_days: AtomicU32::new(0),
            audit_effective_since: AtomicI64::new(chrono::Utc::now().timestamp()),
        }
    }

    pub fn audit_dir(&self) -> &Path {
        &self.audit_dir
    }

    /// 设置 JSONL 审计文件的保留天数（`0` = 不清理），并把生效时刻记为本刻。
    ///
    /// 由 app 层从 `AppConf` 注入：库层不读配置文件，避免在库内部做隐式 I/O，
    /// 也避免测试被本机配置污染。生效时刻与天数必须**同一次**写入，
    /// 否则 `E11` 的 `policy.effectiveSince` 与实际裁剪截止时刻对不上（I61 §3.5）。
    pub fn set_audit_retention_days(&self, days: u32) {
        self.audit_retention_days.store(days, Ordering::SeqCst);
        self.audit_effective_since
            .store(chrono::Utc::now().timestamp(), Ordering::SeqCst);
    }

    pub fn audit_retention_days(&self) -> u32 {
        self.audit_retention_days.load(Ordering::SeqCst)
    }

    pub fn audit_writer(&self) -> AuditJsonlWriter {
        AuditJsonlWriter::new(self.audit_dir.clone())
    }

    /// 双写审计：已解锁时同时写 DB 与 JSONL，未解锁时只写 JSONL。
    ///
    /// 恢复通道（无连接）用 [`Self::audit_jsonl`]，不要走这里。
    /// 返回 `()`：审计失败只 `warn`，不阻断业务（规格 §4）。
    pub fn audit_dual(&self, event: AuditEvent, detail: Option<&str>) {
        let writer = self.audit_writer();
        let guard = self.get_conn().ok();
        let conn = guard.as_ref().and_then(|guard| guard.as_ref());
        log_dual(conn, Some(&writer), event, detail)
    }

    /// 只写 JSONL：库不可用（恢复通道、重置后）的审计落点。
    ///
    /// 返回 `()`：审计失败只 `warn`，不阻断业务（规格 §4）。
    pub fn audit_jsonl(&self, event: AuditEvent, detail: Option<&str>) {
        let writer = self.audit_writer();
        log_dual(None, Some(&writer), event, detail)
    }

    /// 解锁后补录 JSONL 中「库不可用期间」产生的事件。
    ///
    /// 去重键 `jsonl-{at}-{event}-{文件名}:{行号}` 保证重复解锁不重复插入；失败只 `warn`，不阻断解锁。
    fn backfill_jsonl_audit(&self, conn: &Connection) {
        if let Err(e) = self.try_backfill_jsonl_audit(conn) {
            log::warn!("Audit jsonl backfill failed: {}", e);
        }
    }

    fn try_backfill_jsonl_audit(&self, conn: &Connection) -> Result<(), VaultError> {
        let stored = meta::get_meta(conn, JSONL_SYNC_KEY)
            .map_err(VaultError::Database)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        // 游标在未来 = 时钟回拨或元数据损坏：按损坏处理，本次全量扫描一次（去重键挡重复），
        // 否则「回拨期间写入」的记录会因早于游标而永久漏补。补录结束后游标被改写成实际 max(at)，
        // 下一轮自动回到增量路径。
        let now = crate::types::now_millis();
        let since_ms = if stored > now { 0 } else { stored };
        let records = self.audit_writer().read_since(since_ms)?;
        let max_at = records.iter().map(|record| record.at).max();

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| VaultError::Database(e.to_string()))?;

        for record in records
            .iter()
            .filter(|record| BACKFILL_EVENTS.contains(&record.event.as_str()))
        {
            // 自回响防护：既双写又在白名单内的事件（`dek_rotated` / `local_db_rebuilt`）在
            // 原生行里已存在，按原生键跳过。只认非 `jsonl-` 行，否则同毫秒的补录行会互相
            // 挡掉（那正是 V2.3 要解决的场景）。
            let native_exists: bool = tx
                .query_row(
                    "SELECT 1 FROM security_audit_log \
                     WHERE event_type = ?1 AND timestamp = ?2 AND id NOT LIKE 'jsonl-%' \
                     LIMIT 1",
                    rusqlite::params![record.event, record.at],
                    |_| Ok(true),
                )
                .optional()
                .map_err(|e| VaultError::Database(e.to_string()))?
                .unwrap_or(false);
            if native_exists {
                continue;
            }

            // 去重键必须能区分同毫秒多条记录：`(文件名, 行号)` 是记录在文件里的稳定身份，
            // 且历史行无需迁移（读入时现算）。前提是审计文件只追加、不重写/不压缩。
            let origin = record.origin.as_deref().unwrap_or("unknown:0");
            let id = format!("jsonl-{}-{}-{}", record.at, record.event, origin);
            tx.execute(
                "INSERT OR IGNORE INTO security_audit_log (id, event_type, detail, timestamp) \
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, record.event, record.detail, record.at],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        }

        // 游标推进到「已处理的最大 at」：若用 now()，则本批尚未写入的记录会因时间更早而永久漏补。
        // 无记录时保持原游标不动。
        if let Some(max_at) = max_at {
            meta::set_meta(&tx, JSONL_SYNC_KEY, &max_at.to_string())
                .map_err(VaultError::Database)?;
        }
        tx.commit()
            .map_err(|e| VaultError::Database(e.to_string()))?;

        // 保留清理（规格 §5 G12）：补录成功后才谈得上「文件已被补录」，判据用刚推进的
        // 游标，**禁止用 `now`** —— 用 `now` 会把尚未补录的历史文件一起判为「可删」。
        // 无记录时游标不动，也不清理（缺少「已补录到哪」的判据）。
        if let Some(max_at) = max_at {
            let report = self
                .audit_writer()
                .prune_before(max_at, self.audit_retention_days());
            if report.removed > 0 || report.failed > 0 {
                log::info!(
                    "Audit jsonl prune before cursor {}: removed {} file(s), {} failure(s)",
                    max_at,
                    report.removed,
                    report.failed
                );
            }
        }
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
            log_dual(
                Some(&session.conn),
                Some(&writer),
                AuditEvent::AnchorRollbackRejected,
                Some("meta_or_dek_version"),
            );
            return Err(VaultError::VaultRollbackDetected);
        }

        let writer = self.audit_writer();
        log_dual(
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
        log_dual(
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

    pub(super) fn backup_dir_for(db_path: &Path) -> Result<PathBuf, VaultError> {
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
            if is_backup_file(&path) {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// `E8` 的对外裁剪入口（T3）：按备份目录重新计数后裁到 `keep` 份。
    ///
    /// 三条纪律：①`keep` 至少 1，任何情况下都保留最新一份（§5.5 `E8` 行「不得删最新一份」）；
    /// ②只按 `BACKUP_PREFIX` / `BACKUP_SUFFIX` 匹配，跳符号链接与非常规文件；
    /// ③目录不存在是正常空态（全零），单份删除失败只计数不中断（`partial` 语义）。
    pub fn prune_backups_to(db_path: &Path, keep: usize) -> Result<PruneReport, VaultError> {
        let backup_dir = Self::backup_dir_for(db_path)?;
        let keep = keep.max(1);
        let entries = match std::fs::read_dir(&backup_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PruneReport::default())
            }
            Err(error) => return Err(VaultError::Io(error)),
        };

        let mut files: Vec<(PathBuf, u64)> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            // `symlink_metadata` 不跟随链接：符号链接按「非常规文件」跳过，绝不删目标。
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() || !meta.is_file() || !is_backup_file(&path) {
                continue;
            }
            files.push((path, meta.len()));
        }

        let mut report = PruneReport::default();
        if files.len() <= keep {
            return Ok(report);
        }
        // 文件名前缀是 `%Y%m%d-%H%M%S-%3f`：升序即「最旧在前」，裁掉最旧的超限部分。
        files.sort_by(|left, right| left.0.cmp(&right.0));
        let excess = files.len() - keep;
        for (path, bytes) in files.into_iter().take(excess) {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(bytes);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    log::warn!("Failed to prune backup {}: {}", path.display(), error);
                    report.failed = report.failed.saturating_add(1);
                }
            }
        }
        Ok(report)
    }

    pub(super) fn prune_backups(dir: &Path, keep: usize) -> Result<(), VaultError> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| is_backup_file(path))
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

    /// `E7`：安全库文件观测（只 `metadata`，不解锁、不开库、不复制）。
    pub fn db_file_stats(&self) -> std::io::Result<DirStats> {
        storage_stats::file_stats(Path::new(&self.db_path))
    }

    /// `E8`：备份目录观测；口径与 `prune_backups` / `purge_backups` 一致（只认备份文件名）。
    pub fn backup_stats(&self) -> std::io::Result<DirStats> {
        let backup_dir = Self::backup_dir_for(Path::new(&self.db_path))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        storage_stats::dir_stats(&backup_dir, is_backup_file)
    }

    /// `E11`：审计 JSONL 目录观测；口径与 `prune_before` 一致（只认 `audit-*.jsonl`）。
    pub fn audit_stats(&self) -> std::io::Result<DirStats> {
        self.audit_writer().stats()
    }

    /// `E11` 的策略自述：保留天数 + 策略生效时刻（秒）。
    ///
    /// 恒为 `Some`：`SecurityDbState` 一定持有一份「当前生效」的策略快照，
    /// 取不到状态的场景由 provider 层返回 `None`（`policy` 两数字填 `null`）。
    pub fn audit_policy(&self) -> Option<(u32, i64)> {
        Some((
            self.audit_retention_days(),
            self.audit_effective_since.load(Ordering::SeqCst),
        ))
    }
}

/// 备份文件名判据：`prune_backups` 与 `backup_stats` 共用，避免「统计多少」与「能删多少」漂移。
fn is_backup_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(BACKUP_PREFIX) && name.ends_with(BACKUP_SUFFIX))
        .unwrap_or(false)
}
