use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use log::warn;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use assistant_crypto::error::VaultError;

use super::AuditEvent;

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
        };
        let mut line = serde_json::to_string(&record)
            .map_err(|e| VaultError::Other(format!("audit jsonl serialize failed: {}", e)))?;
        line.push('\n');

        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        restrict_permissions(&file);
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// 读取 `since_ms` 之后的记录（严格大于），按时间升序。
    ///
    /// 目录不存在 → 空集合；单行解析失败 → `warn` 后跳过，不整体失败。
    pub fn read_since(&self, since_ms: i64) -> Result<Vec<JsonlRecord>, VaultError> {
        if !self.dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| is_audit_file(path))
            .collect();
        files.sort();

        let mut records = Vec::new();
        for path in files {
            let file = std::fs::File::open(&path)?;
            for line in BufReader::new(file).lines() {
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
                    Ok(record) => {
                        if record.at > since_ms {
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

fn is_audit_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(FILE_PREFIX) && name.ends_with(FILE_SUFFIX))
        .unwrap_or(false)
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

/// 双写审计：DB 与 JSONL 两路都尝试，任一路成功即 `Ok`。
///
/// - 恢复通道：`conn = None`，只写 JSONL；
/// - 未初始化：`writer = None`，只写 DB；
/// - JSONL 失败只 `warn`，不得阻断主业务；DB 失败按原逻辑返回错误。
pub fn log_dual(
    conn: Option<&Connection>,
    writer: Option<&AuditJsonlWriter>,
    event: AuditEvent,
    detail: Option<&str>,
) -> Result<(), VaultError> {
    let at = chrono::Utc::now().timestamp_millis();

    let db_result = match conn {
        Some(conn) => super::write_db_at(conn, event, detail, at),
        None => Ok(()),
    };

    let jsonl_result = match writer {
        Some(writer) => {
            let result = writer.append_at(event, detail, at);
            if let Err(e) = &result {
                warn!("Audit jsonl append failed for '{}': {}", event, e);
            }
            result
        }
        None => Ok(()),
    };

    match (db_result, jsonl_result) {
        (Ok(()), _) => Ok(()),
        (Err(_), Ok(())) => Ok(()),
        (Err(db_err), Err(_)) => Err(db_err),
    }
}
