use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use log::warn;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use assistant_crypto::error::VaultError;

use super::AuditEvent;
use crate::storage_stats::DirStats;

const FILE_PREFIX: &str = "audit-";
const FILE_SUFFIX: &str = ".jsonl";
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// JSONL 追加日志的单行记录。
///
/// `detail` 恒在（缺失时为 `null`），与 `security_audit_log` 的行语义一致。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonlRecord {
    pub at: i64,
    pub event: String,
    pub detail: Option<String>,
    /// 记录身份：`{文件名}:{行号}`，由 [`AuditJsonlWriter::read_since`] 在读入时填充。
    ///
    /// 只用于补录去重（同一毫秒的多条记录靠它区分），**不落盘**：
    /// 落盘后一旦重写/压缩文件，身份就会漂移，因此审计文件只允许追加。
    #[serde(skip)]
    pub origin: Option<String>,
}

/// 审计 JSONL 追加写入器。
///
/// 只持有目录路径，不持密钥、不持连接：恢复通道（无 `security.db`）也能写。
pub struct AuditJsonlWriter {
    dir: PathBuf,
}

impl AuditJsonlWriter {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `E11`：审计目录观测；口径与 [`Self::prune_before`] 一致（只认 `audit-*.jsonl` 常规文件）。
    pub fn stats(&self) -> std::io::Result<DirStats> {
        crate::storage_stats::dir_stats(&self.dir, is_audit_file)
    }

    /// 追加一条记录。
    ///
    /// 失败：目录不可创建、文件不可打开、序列化失败；调用方按「不阻断业务」处理。
    pub fn append(&self, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError> {
        self.append_at(event, detail, chrono::Utc::now().timestamp_millis())
    }

    /// 追加一条带显式时间戳的记录。
    ///
    /// 双写时与 DB 行共用同一个 `at`，补录才能按 `(event, timestamp)` 判重。
    pub fn append_at(
        &self,
        event: AuditEvent,
        detail: Option<&str>,
        at: i64,
    ) -> Result<(), VaultError> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.active_file_path();

        let record = JsonlRecord {
            at,
            event: event.to_string(),
            detail: detail.map(str::to_string),
            origin: None,
        };
        let mut line = serde_json::to_string(&record)
            .map_err(|e| VaultError::Other(format!("audit jsonl serialize failed: {}", e)))?;
        line.push('\n');

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        restrict_permissions(&file);
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// 读取 `since_ms` 及之后的记录（含等值，同毫秒记录靠 `origin` 去重），按时间升序。
    ///
    /// 目录不存在 → 空集合；单行解析失败 → `warn` 后跳过；单个文件打不开 → `warn` 后跳过，
    /// 不整体失败。候选文件按文件名日期裁剪到「游标所在 UTC 日期减 1 天」之后：
    /// 文件名用本地日期、记录用 UTC 毫秒，两者最多差 1 天，故留 1 天余量。
    pub fn read_since(&self, since_ms: i64) -> Result<Vec<JsonlRecord>, VaultError> {
        if !self.dir.is_dir() {
            return Ok(Vec::new());
        }

        let cutoff = file_date_cutoff(since_ms);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| is_audit_file(path))
            .filter(|path| match file_date(path) {
                Some(date) => date >= cutoff,
                None => true,
            })
            .collect();
        files.sort();

        let mut records = Vec::new();
        for path in files {
            let file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(e) => {
                    warn!("Skip unreadable audit jsonl {}: {}", path.display(), e);
                    continue;
                }
            };
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();
            for (index, line) in BufReader::new(file).lines().enumerate() {
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        warn!("Audit jsonl read error in {}: {}", path.display(), e);
                        continue;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<JsonlRecord>(&line) {
                    Ok(mut record) => {
                        if record.at >= since_ms {
                            record.origin = Some(format!("{}:{}", name, index + 1));
                            records.push(record);
                        }
                    }
                    Err(e) => warn!(
                        "Skip malformed audit jsonl line in {}: {}",
                        path.display(),
                        e
                    ),
                }
            }
        }
        records.sort_by_key(|record| record.at);
        Ok(records)
    }

    /// 删除「已补录且已超期」的日文件（规格 §5 G12）。
    ///
    /// 判据必须**同时**满足，缺一不可：
    /// - 文件名日期早于 `cursor_ms` 所在 UTC 日期：该日及之后的文件可能还含尚未补录的
    ///   记录（或尚未读入的记录），删掉即永久丢失，因此只允许删更早的；
    /// - 文件名日期早于「今天减 `retention_days`」，即确实超期。
    ///
    /// 判据只取文件名日期：追加永远写当日文件（[`Self::active_file_path`]），
    /// 因此「早于今天」的文件不可能再被追加，删除不会与新写入竞争。
    ///
    /// `retention_days == 0` 完全禁用；单文件删除失败只 `warn` 并计数，不抛出、
    /// 不影响补录结果（审计是观测，不是事实源）。符号链接与非普通文件一律不删。
    pub fn prune_before(&self, cursor_ms: i64, retention_days: u32) -> PruneReport {
        let mut report = PruneReport::default();
        if retention_days == 0 || !self.dir.is_dir() {
            return report;
        }
        // 任一日期推导失败都放弃清理：宁可不删，也不能拿错误判据删用户数据。
        let (Some(cursor_date), Some(retention_date)) = (
            utc_date_of(cursor_ms),
            retention_date_cutoff(retention_days),
        ) else {
            return report;
        };

        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    "Audit jsonl prune: read dir '{}' failed: {}",
                    self.dir.display(),
                    e
                );
                report.failed += 1;
                return report;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!(
                        "Audit jsonl prune: bad entry in '{}': {}",
                        self.dir.display(),
                        e
                    );
                    report.failed += 1;
                    continue;
                }
            };
            let path = entry.path();
            if !is_audit_file(&path) {
                continue;
            }
            // 文件名日期解析不出 → 无法证明已补录，保守保留。
            let Some(date) = file_date(&path) else {
                continue;
            };
            if date >= cursor_date || date >= retention_date {
                continue;
            }
            // `symlink_metadata` 不跟随链接：符号链接与目录一律不删。
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e) => {
                    warn!("Audit jsonl prune: stat '{}' failed: {}", path.display(), e);
                    report.failed += 1;
                    continue;
                }
            };
            if !meta.is_file() {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => report.removed += 1,
                Err(e) => {
                    warn!(
                        "Audit jsonl prune: remove '{}' failed: {}",
                        path.display(),
                        e
                    );
                    report.failed += 1;
                }
            }
        }
        report
    }

    /// 当日文件；超过 8 MiB 时顺延到 `audit-YYYYMMDD-{n}.jsonl`。
    fn active_file_path(&self) -> PathBuf {
        let date = chrono::Local::now().format("%Y%m%d").to_string();
        let mut candidate = self
            .dir
            .join(format!("{}{}{}", FILE_PREFIX, date, FILE_SUFFIX));
        let mut seq = 1u32;
        while let Ok(meta) = std::fs::metadata(&candidate) {
            if meta.len() < MAX_FILE_BYTES {
                return candidate;
            }
            candidate = self
                .dir
                .join(format!("{}{}-{}{}", FILE_PREFIX, date, seq, FILE_SUFFIX));
            seq += 1;
        }
        candidate
    }
}

/// 保留清理结果：只用于日志与测试断言，不参与任何业务判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub removed: usize,
    pub failed: usize,
}

fn is_audit_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(FILE_PREFIX) && name.ends_with(FILE_SUFFIX))
        .unwrap_or(false)
}

/// 文件名中的本地日期（`YYYYMMDD` 的整数形式）；解析不出返回 `None`（调用方按「不过滤」处理）。
fn file_date(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_prefix(FILE_PREFIX)?.strip_suffix(FILE_SUFFIX)?;
    let date: String = stem.chars().take(8).collect();
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    date.parse::<u32>().ok()
}

/// 游标所在 UTC 日期减 1 天（含该日）；留 1 天余量覆盖「本地日期 vs UTC 毫秒」的跨日差。
fn file_date_cutoff(since_ms: i64) -> u32 {
    let utc = chrono::DateTime::from_timestamp_millis(since_ms)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp_millis(0).unwrap());
    let date = (utc - chrono::Days::new(1)).format("%Y%m%d").to_string();
    date.parse::<u32>().unwrap_or(0)
}

/// 毫秒时间戳所在的 UTC 日期（`YYYYMMDD` 的整数形式）；时间戳非法返回 `None`。
///
/// 与 [`file_date`] 同一编码，故可直接比较大小（`YYYYMMDD` 的整数序与日期序一致）。
fn utc_date_of(ms: i64) -> Option<u32> {
    let utc = chrono::DateTime::from_timestamp_millis(ms)?;
    utc.format("%Y%m%d").to_string().parse::<u32>().ok()
}

/// 「今天减 `retention_days`」的 UTC 日期（`YYYYMMDD` 的整数形式）；越界返回 `None`。
fn retention_date_cutoff(retention_days: u32) -> Option<u32> {
    let cutoff =
        chrono::Utc::now().checked_sub_days(chrono::Days::new(u64::from(retention_days)))?;
    cutoff.format("%Y%m%d").to_string().parse::<u32>().ok()
}

#[cfg(unix)]
fn restrict_permissions(file: &std::fs::File) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = file.set_permissions(std::fs::Permissions::from_mode(0o600)) {
        warn!("Failed to restrict audit jsonl permissions: {}", e);
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_file: &std::fs::File) {}

/// 双写审计：DB 与 JSONL 两路都尝试，任一路成功即视为写入成功。
///
/// - 恢复通道：`conn = None`，只写 JSONL；
/// - 未初始化：`writer = None`，只写 DB；
/// - 两侧失败都只 `warn`，不阻断主业务。
///
/// 返回 `()`：从类型上消除调用方用 `?` 把审计失败传播成业务错误的可能（规格 §4）。
/// 时间戳只取一次 `now_millis`，DB 行与 JSONL 行必须同值，否则 §2.5 的补录去重会失效。
pub fn log_dual(
    conn: Option<&Connection>,
    writer: Option<&AuditJsonlWriter>,
    event: AuditEvent,
    detail: Option<&str>,
) {
    let at = crate::types::now_millis();

    if let Some(conn) = conn {
        if let Err(e) = super::write_db_at(conn, event, detail, at) {
            warn!("Audit db write failed for '{}': {}", event, e);
        }
    }

    if let Some(writer) = writer {
        if let Err(e) = writer.append_at(event, detail, at) {
            warn!("Audit jsonl append failed for '{}': {}", event, e);
        }
    }
}
