use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::fs;

/// 同一 ID 必落同一桶，桶碰撞只增加争用（I61 §5.5）。
const ID_LOCK_BUCKETS: usize = 64;

pub struct AttachmentStoreConfig {
    pub dir: PathBuf,
}

#[derive(Clone)]
pub struct AttachmentStore {
    config: Arc<AttachmentStoreConfig>,
    /// 锁数组与 `config` 同样进 `Arc`：`Clone` 出来的句柄必须共享同一组锁，
    /// 否则每个 clone 各拿一把锁，「同一 ID 必互斥」失效。
    locks: Arc<[Mutex<()>; ID_LOCK_BUCKETS]>,
}

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid media type: {0}")]
    InvalidMediaType(String),
    #[error("Image too large: {size} bytes (max {max})")]
    TooLarge { size: usize, max: usize },
    #[error("Too many images: {count} (max {max})")]
    TooManyImages { count: usize, max: usize },
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Invalid attachment id: {0}")]
    InvalidId(String),
}

/// 判据与路径构造在同一处，调用方无法绕过。
pub fn is_plain_file_name(id: &str) -> bool {
    if id.is_empty()
        || id.starts_with('.')
        || id.ends_with(".tmp")
        || id.contains(':')
        || id.contains('/')
        || id.contains('\\')
        || id.chars().any(|c| c == '\0' || c.is_control())
    {
        return false;
    }
    let mut components = Path::new(id).components();
    matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

fn ensure_plain(id: &str) -> Result<(), AttachmentError> {
    if is_plain_file_name(id) {
        Ok(())
    } else {
        Err(AttachmentError::InvalidId(id.to_string()))
    }
}

fn lock_bucket(id: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    usize::try_from(hasher.finish()).unwrap_or(usize::MAX) % ID_LOCK_BUCKETS
}

/// 部分写入残留，不计入条目数字，由 owner 回收时一并清理（I61 §3.9）。
const COMPANION_SUFFIXES: [&str; 5] = ["-journal", "-wal", "-shm", ".tmp", ".lock"];

fn is_companion_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    COMPANION_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

pub struct ImageAttachmentRef {
    pub id: String,
    pub media_type: String,
    pub bytes: u64,
}

pub struct AttachmentEntry {
    pub id: String,
    pub bytes: u64,
    pub created_at: u64,
}

/// 派生文本侧车目录的观测快照（I61 §3.10，条目 `E2`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttachmentDirStats {
    pub bytes: u64,
    pub count: u64,
    /// 侧车文件 `mtime` 最早 / 最晚时刻（秒）；无文件时为 `None`，不是 `0`。
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 派生文件（PDF 文本层等）的独立子目录：与附件根目录分开，
/// 避免 `list_all` / 孤儿清理把派生文件误判为附件。
const DERIVED_DIR: &str = "derived";
/// 抽取器版本：算法变更后旧侧车不会被读到。
const EXTRACTOR_VERSION: &str = "extract-v1";

/// 派生侧车的一条记录（I61 `E2` 的 `A5` 候选）。
///
/// `name` 是 `derived/` 下的**文件名**（不是路径）；`modified_at` 取不到为 `None`
/// （不是 `0`，G6）——回收侧按 fail-closed 处理「时间未知」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedEntry {
    pub name: String,
    pub bytes: u64,
    pub modified_at: Option<i64>,
    pub orphan: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DerivedPurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OrphanPurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
}

/// 与 [`AttachmentStore::derived_path`] 的拼接规则同源：改名规则只能有一处。
fn derived_stem(name: &str) -> String {
    let suffix = format!(".{}.txt", EXTRACTOR_VERSION);
    match name.strip_suffix(&suffix) {
        Some(stem) => stem.to_string(),
        None => Path::new(name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name)
            .to_string(),
    }
}

/// 基附件的 `file_stem`：`derived_path` 丢扩展名，因此「存在性」也按 `stem` 判。
fn base_stem(id: &str) -> Option<String> {
    Path::new(id)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
}

fn mtime_seconds(meta: &std::fs::Metadata) -> Option<i64> {
    let modified = meta.modified().ok()?;
    let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(i64::try_from(since_epoch.as_secs()).unwrap_or(i64::MAX))
}

fn media_type_to_ext(mt: &str) -> &'static str {
    match mt {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "text/csv" => "csv",
        "text/html" => "html",
        "application/json" => "json",
        "text/x-python" => "py",
        "text/javascript" => "js",
        "text/typescript" => "ts",
        "text/css" => "css",
        "text/yaml" => "yaml",
        "text/xml" => "xml",
        "text/x-rust" => "rs",
        "text/x-toml" => "toml",
        "text/x-java" => "java",
        "text/x-go" => "go",
        "text/x-c" => "c",
        "text/x-cpp" => "cpp",
        "text/x-shellscript" => "sh",
        "text/x-powershell" => "ps1",
        "application/sql" => "sql",
        _ => "bin",
    }
}

impl AttachmentStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            config: Arc::new(AttachmentStoreConfig { dir }),
            locks: Arc::new(std::array::from_fn(|_| Mutex::new(()))),
        }
    }

    /// 附件 ID 锁（I61 §5.5）：`insert_attachment_refs` / `delete_attachments` / `A1` 共用，
    /// 保证「引用写入」与「文件删除」对同一 ID 互斥。临界区内禁止 `.await`。
    ///
    /// 用 `std::sync::Mutex` 而非 `tokio`：调用方已持 `&Connection`，`blocking_lock()` 会 panic。
    pub fn id_guard(&self, id: &str) -> MutexGuard<'_, ()> {
        let bucket = lock_bucket(id);
        self.locks[bucket]
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// 内容寻址写入任意附件（图片、PDF、文本等），扩展名由 `media_type` 决定。
    pub async fn save_attachment(
        &self,
        data: &[u8],
        media_type: &str,
    ) -> Result<ImageAttachmentRef, AttachmentError> {
        let hash = Sha256::digest(data);
        let sha256_hex = format!("{:x}", hash);
        let ext = media_type_to_ext(media_type);
        let attachment_id = format!("{}.{}", sha256_hex, ext);
        let path = self.config.dir.join(&attachment_id);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => {
                use tokio::io::AsyncWriteExt;
                let mut writer = tokio::io::BufWriter::new(file);
                writer.write_all(data).await?;
                writer.flush().await?;
                drop(writer);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }

        Ok(ImageAttachmentRef {
            id: attachment_id,
            media_type: media_type.to_string(),
            bytes: data.len() as u64,
        })
    }

    pub async fn read(&self, id: &str) -> Result<Vec<u8>, AttachmentError> {
        let path = self.file_path(id)?;
        Ok(fs::read(&path).await?)
    }

    /// 入口即校验 id：非法 id 返回 `InvalidId`，不产出任何可 join 的路径。
    pub fn file_path(&self, id: &str) -> Result<PathBuf, AttachmentError> {
        ensure_plain(id)?;
        Ok(self.config.dir.join(id))
    }

    /// 保持私有：外部只能通过侧车方法读写，避免多一条未校验的路径入口。
    fn derived_path(&self, id: &str) -> Result<PathBuf, AttachmentError> {
        ensure_plain(id)?;
        let stem = Path::new(id)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(id);
        Ok(self
            .config
            .dir
            .join(DERIVED_DIR)
            .join(format!("{}.{}.txt", stem, EXTRACTOR_VERSION)))
    }

    /// 不存在 / 读取失败 / id 非法一律 `None`，由调用方回退。
    pub async fn read_derived_text(&self, id: &str) -> Option<String> {
        let path = self.derived_path(id).ok()?;
        match fs::read_to_string(&path).await {
            Ok(text) if !text.trim().is_empty() => Some(text),
            _ => None,
        }
    }

    /// 写入侧车文本；已存在时直接返回，避免并发重复写。
    pub async fn write_derived_text(&self, id: &str, text: &str) -> Result<(), AttachmentError> {
        let path = self.derived_path(id)?;
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp = path.with_extension("txt.tmp");
        fs::write(&tmp, text.as_bytes()).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn delete_derived_text(&self, id: &str) -> Result<(), AttachmentError> {
        let path = self.derived_path(id)?;
        let _ = fs::remove_file(&path).await;
        Ok(())
    }

    /// 派生侧车目录的观测（`E2`）：只认常规文件，符号链接与非常规文件跳过、不降级；
    /// 目录不存在是正常空态，返回全零而非错误。
    pub async fn derived_stats(&self) -> Result<AttachmentDirStats, AttachmentError> {
        let entries = self.list_derived().await?;
        let mut stats = AttachmentDirStats::default();
        for entry in &entries {
            stats.bytes = stats.bytes.saturating_add(entry.bytes);
            stats.count = stats.count.saturating_add(1);
            if let Some(seconds) = entry.modified_at {
                stats.oldest_at = Some(stats.oldest_at.map_or(seconds, |cur| cur.min(seconds)));
                stats.newest_at = Some(stats.newest_at.map_or(seconds, |cur| cur.max(seconds)));
            }
        }
        Ok(stats)
    }

    /// `derived/` 的逐文件清单（I61 `E2` 观测 / 回收共用同一口径）。
    ///
    /// `orphan` = `derived` 的 `stem` 找不到对应基附件（基附件已不存在，或扩展名不同）。
    pub async fn list_derived(&self) -> Result<Vec<DerivedEntry>, AttachmentError> {
        let bases: HashSet<String> = self
            .list_all()
            .await?
            .iter()
            .filter_map(|entry| base_stem(&entry.id))
            .collect();
        let dir = self.config.dir.join(DERIVED_DIR);
        let mut list = Vec::new();
        let mut entries = match fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(list),
            Err(error) => return Err(error.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).await?;
            if meta.file_type().is_symlink() || !meta.is_file() || is_companion_file(&path) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            list.push(DerivedEntry {
                orphan: !bases.contains(&derived_stem(name)),
                name: name.to_string(),
                bytes: meta.len(),
                modified_at: mtime_seconds(&meta),
            });
        }
        Ok(list)
    }

    /// 删除 `derived/` 下被计划点名的孤儿侧车（I61 `E2` / `A5`）。
    ///
    /// 执行侧重新校验、不信任计划里的数量：非单段文件名 / 伴生后缀 / 符号链接 / 非常规文件
    /// → `skipped`；`mtime` 取不到或 `>= sampledAt`（计划后被写过）→ `skipped`（fail-closed）；
    /// 删除失败 → `failed`。顺带清掉 `derived/` 下的伴生残留（I61 §3.9），计入 `removed`。
    pub async fn purge_derived_orphans(
        &self,
        names: &[String],
        sampled_at: i64,
    ) -> DerivedPurgeReport {
        let dir = self.config.dir.join(DERIVED_DIR);
        let mut report = DerivedPurgeReport::default();
        for name in names {
            if !is_plain_file_name(name) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            let target = dir.join(name);
            if is_companion_file(&target) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            let meta = match fs::symlink_metadata(&target).await {
                Ok(meta) => meta,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.skipped = report.skipped.saturating_add(1);
                    continue;
                }
                Err(_) => {
                    report.failed = report.failed.saturating_add(1);
                    continue;
                }
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            if mtime_seconds(&meta).is_none_or(|seconds| seconds >= sampled_at) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            match fs::remove_file(&target).await {
                Ok(()) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(meta.len());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.skipped = report.skipped.saturating_add(1);
                }
                Err(_) => report.failed = report.failed.saturating_add(1),
            }
        }
        let companions = self.clean_derived_companions(&dir, sampled_at).await;
        report.removed = report.removed.saturating_add(companions.removed);
        report.removed_bytes = report
            .removed_bytes
            .saturating_add(companions.removed_bytes);
        report.failed = report.failed.saturating_add(companions.failed);
        report.skipped = report.skipped.saturating_add(companions.skipped);
        report
    }

    /// `derived/` 下的伴生残留：同样按 `mtime < sampledAt` 判据，避免删掉正在写的中间态。
    async fn clean_derived_companions(&self, dir: &Path, sampled_at: i64) -> DerivedPurgeReport {
        let mut report = DerivedPurgeReport::default();
        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(_) => return report,
        };
        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            let path = entry.path();
            if !is_companion_file(&path) {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path).await else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                continue;
            }
            if mtime_seconds(&meta).is_none_or(|seconds| seconds >= sampled_at) {
                continue;
            }
            match fs::remove_file(&path).await {
                Ok(()) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(meta.len());
                }
                Err(_) => report.failed = report.failed.saturating_add(1),
            }
        }
        report
    }

    pub async fn list_all(&self) -> Result<Vec<AttachmentEntry>, AttachmentError> {
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&self.config.dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            // `file_type` 不跟随链接：符号链接按「非常规文件」跳过，
            // 否则统计会把指向外部的链接算成附件，`delete` 却删不掉（口径漂移）。
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() || !file_type.is_file() {
                continue;
            }
            let id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let metadata = entry.metadata().await?;
            let created_at = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            entries.push(AttachmentEntry {
                id,
                bytes: metadata.len(),
                created_at,
            });
        }
        Ok(entries)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AttachmentError> {
        ensure_plain(id)?;
        self.delete_derived_text(id).await?;
        let path = self.file_path(id)?;
        Ok(fs::remove_file(&path).await?)
    }

    /// 同步删除（I61 §5.5）：调用方必须已持有该 ID 的 [`AttachmentStore::id_guard`]，
    /// 临界区内禁止 `.await`，因此这里只用 `std::fs`。
    ///
    /// `Ok(Some(len))` = 已删并返回原文件字节；`Ok(None)` = 文件已不存在（不是错误）；
    /// 非法 id / 其它 IO 错 = `Err`。顺带清 `derived/{stem}.{EXTRACTOR_VERSION}.txt`。
    pub fn delete_blocking(&self, id: &str) -> Result<Option<u64>, AttachmentError> {
        ensure_plain(id)?;
        if let Ok(derived) = self.derived_path(id) {
            let _ = std::fs::remove_file(&derived);
        }
        let path = self.file_path(id)?;
        let len = match std::fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(Some(len)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// `E1` 的 `A1`：按 ID 删孤儿（I61 §5.2 第 4 条 / I61 §5.5）。
    ///
    /// 同步方法：锁内要查 DB，不能 async。每个 id 固定为：取 `id_guard` → **锁内**查引用
    /// （走 `idx_attachment_refs_att`）→ 仍被引用即 `skipped` → 确认存在且为常规文件 →
    /// `mtime` 不早于 `sampledAt` 即 `skipped`（挡住采样后新导入、引用尚未落库的窗口）→
    /// `delete_blocking` 累加字节；ID 非法 `skipped`，单条失败只计 `failed` 不中断。
    pub fn delete_orphans_by_ids(
        &self,
        conn: &Connection,
        ids: &[String],
        sampled_at: i64,
    ) -> OrphanPurgeReport {
        let mut report = OrphanPurgeReport::default();
        for id in ids {
            if !is_plain_file_name(id) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            let _guard = self.id_guard(id);
            match is_attachment_referenced(conn, id) {
                Ok(true) => {
                    report.skipped = report.skipped.saturating_add(1);
                    continue;
                }
                Ok(false) => {}
                Err(_) => {
                    report.failed = report.failed.saturating_add(1);
                    continue;
                }
            }
            let path = match self.file_path(id) {
                Ok(path) => path,
                Err(_) => {
                    report.skipped = report.skipped.saturating_add(1);
                    continue;
                }
            };
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.skipped = report.skipped.saturating_add(1);
                    continue;
                }
                Err(_) => {
                    report.failed = report.failed.saturating_add(1);
                    continue;
                }
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            if mtime_seconds(&meta).is_none_or(|seconds| seconds >= sampled_at) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            match self.delete_blocking(id) {
                Ok(Some(bytes)) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(bytes);
                }
                Ok(None) => report.skipped = report.skipped.saturating_add(1),
                Err(_) => report.failed = report.failed.saturating_add(1),
            }
        }
        report
    }

    pub async fn clean_tmp(&self) -> Result<(), AttachmentError> {
        let mut dir = fs::read_dir(&self.config.dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".tmp") {
                    let _ = fs::remove_file(&path).await;
                }
            }
        }
        Ok(())
    }
}

/// 单 ID 引用存在性查询：走 `idx_attachment_refs_att`，只判有无。
fn is_attachment_referenced(conn: &Connection, id: &str) -> Result<bool, String> {
    match conn.query_row(
        "SELECT 1 FROM attachment_refs WHERE attachment_id = ?1 LIMIT 1",
        params![id],
        |_| Ok(()),
    ) {
        Ok(()) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

/// 写引用（I61 §5.5）：与删除共用同一 ID 锁，且锁内确认文件仍在，否则整条写入失败、
/// **不**落悬空引用（内容寻址去重下「先删后写」的兜底）。锁顺序恒为「DB 连接锁 → 附件 ID 锁」。
pub fn insert_attachment_refs(
    store: &AttachmentStore,
    conn: &Connection,
    attachment_ids: &[String],
    conversation_id: &str,
    message_id: &str,
) -> Result<(), String> {
    for aid in attachment_ids {
        ensure_plain(aid).map_err(|error| error.to_string())?;
        let _guard = store.id_guard(aid);
        let path = store.file_path(aid).map_err(|error| error.to_string())?;
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => return Err("attachment file missing".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err("attachment file missing".to_string());
            }
            Err(error) => return Err(error.to_string()),
        }
        conn.execute(
            "INSERT OR IGNORE INTO attachment_refs (attachment_id, conversation_id, message_id) VALUES (?1, ?2, ?3)",
            params![aid, conversation_id, message_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_attachment_refs_by_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM attachment_refs WHERE conversation_id = ?1",
        params![conversation_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_attachment_refs_by_message(
    conn: &Connection,
    message_ids: &[String],
) -> Result<(), String> {
    for mid in message_ids {
        conn.execute(
            "DELETE FROM attachment_refs WHERE message_id = ?1",
            params![mid],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_attachment_refs_by_attachment_id(
    conn: &Connection,
    attachment_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM attachment_refs WHERE attachment_id = ?1",
        params![attachment_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_all_referenced_attachment_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT attachment_id FROM attachment_refs")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

pub fn get_conversation_attachment_ids(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT attachment_id FROM attachment_refs WHERE conversation_id = ?1")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![conversation_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

pub fn duplicate_attachment_refs(
    conn: &Connection,
    source_conversation_id: &str,
    new_conversation_id: &str,
    message_id_map: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT attachment_id, message_id FROM attachment_refs WHERE conversation_id = ?1")
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, String)> = stmt
        .query_map(params![source_conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    for (attachment_id, old_message_id) in rows {
        if let Some(new_message_id) = message_id_map.get(&old_message_id) {
            conn.execute(
                "INSERT OR IGNORE INTO attachment_refs (attachment_id, conversation_id, message_id) VALUES (?1, ?2, ?3)",
                params![attachment_id, new_conversation_id, new_message_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
