use std::fmt;

use log::warn;
use rusqlite::{params, Connection};

use assistant_crypto::error::VaultError;

pub use crate::dto::audit::AuditLogEntry;

pub mod jsonl;

pub use jsonl::{log_dual, AuditJsonlWriter, JsonlRecord, PruneReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEvent {
    VaultUnlock,
    VaultUnlockFailed,
    VaultLock,
    VaultReveal,
    VaultRevealRateLimited,
    VaultEntryCreated,
    VaultEntryUpdated,
    VaultEntryDeleted,
    VaultEntryRestored,
    VaultEntryPurged,
    VaultCopyPassword,
    VaultCopyField,
    HibpAudit,
    HibpLeakFound,
    MasterPasswordChange,
    RecoveryKeyExported,
    RecoveryExportUsed,
    DekRotated,
    LocalDbRebuilt,
    AnchorRollbackRejected,
    BiometricEnrolled,
    BiometricUnlocked,
    BiometricFailed,
    BiometricRevoked,
    SshKeyLoaded,
    SshKeyRemoved,
    VaultRepositoryReset,
    VaultUnsupportedVersion,
    VaultEntriesExported,
    VaultEntriesImported,
}

/// 全部事件变体（规格 §2.7.2 词表断言与前端字典一致性测试的唯一事实来源）。
///
/// 新增变体必须同步加到这里，否则 `audit_event_names_unique` 与字典一致性测试都会漏掉它。
pub const ALL_EVENTS: &[AuditEvent] = &[
    AuditEvent::VaultUnlock,
    AuditEvent::VaultUnlockFailed,
    AuditEvent::VaultLock,
    AuditEvent::VaultReveal,
    AuditEvent::VaultRevealRateLimited,
    AuditEvent::VaultEntryCreated,
    AuditEvent::VaultEntryUpdated,
    AuditEvent::VaultEntryDeleted,
    AuditEvent::VaultEntryRestored,
    AuditEvent::VaultEntryPurged,
    AuditEvent::VaultCopyPassword,
    AuditEvent::VaultCopyField,
    AuditEvent::HibpAudit,
    AuditEvent::HibpLeakFound,
    AuditEvent::MasterPasswordChange,
    AuditEvent::RecoveryKeyExported,
    AuditEvent::RecoveryExportUsed,
    AuditEvent::DekRotated,
    AuditEvent::LocalDbRebuilt,
    AuditEvent::AnchorRollbackRejected,
    AuditEvent::BiometricEnrolled,
    AuditEvent::BiometricUnlocked,
    AuditEvent::BiometricFailed,
    AuditEvent::BiometricRevoked,
    AuditEvent::SshKeyLoaded,
    AuditEvent::SshKeyRemoved,
    AuditEvent::VaultRepositoryReset,
    AuditEvent::VaultUnsupportedVersion,
    AuditEvent::VaultEntriesExported,
    AuditEvent::VaultEntriesImported,
];

impl fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VaultUnlock => write!(f, "vault_unlock"),
            Self::VaultUnlockFailed => write!(f, "vault_unlock_failed"),
            Self::VaultLock => write!(f, "vault_lock"),
            Self::VaultReveal => write!(f, "vault_reveal"),
            Self::VaultRevealRateLimited => write!(f, "vault_reveal_rate_limited"),
            Self::VaultEntryCreated => write!(f, "vault_entry_created"),
            Self::VaultEntryUpdated => write!(f, "vault_entry_updated"),
            Self::VaultEntryDeleted => write!(f, "vault_entry_deleted"),
            Self::VaultEntryRestored => write!(f, "vault_entry_restored"),
            Self::VaultEntryPurged => write!(f, "vault_entry_purged"),
            Self::VaultCopyPassword => write!(f, "vault_copy_password"),
            Self::VaultCopyField => write!(f, "vault_copy_field"),
            Self::HibpAudit => write!(f, "hibp_audit"),
            Self::HibpLeakFound => write!(f, "hibp_leak_found"),
            Self::MasterPasswordChange => write!(f, "master_password_change"),
            Self::RecoveryKeyExported => write!(f, "recovery_key_exported"),
            Self::RecoveryExportUsed => write!(f, "recovery_export_used"),
            Self::DekRotated => write!(f, "dek_rotated"),
            Self::LocalDbRebuilt => write!(f, "local_db_rebuilt"),
            Self::AnchorRollbackRejected => write!(f, "anchor_rollback_rejected"),
            Self::BiometricEnrolled => write!(f, "biometric_enrolled"),
            Self::BiometricUnlocked => write!(f, "biometric_unlocked"),
            Self::BiometricFailed => write!(f, "biometric_failed"),
            Self::BiometricRevoked => write!(f, "biometric_revoked"),
            Self::SshKeyLoaded => write!(f, "ssh_key_loaded"),
            Self::SshKeyRemoved => write!(f, "ssh_key_removed"),
            Self::VaultRepositoryReset => write!(f, "vault_repository_reset"),
            Self::VaultUnsupportedVersion => write!(f, "vault_unsupported_version"),
            Self::VaultEntriesExported => write!(f, "vault_entries_exported"),
            Self::VaultEntriesImported => write!(f, "vault_entries_imported"),
        }
    }
}

/// 仅写 `security.db` 的审计入口（现有调用点的别名）。
///
/// 返回 `()`：审计失败只 `warn`，从类型上消除调用方用 `?` 传播的可能（规格 §4）。
pub fn log(conn: &Connection, event: AuditEvent, detail: Option<&str>) {
    log_dual(Some(conn), None, event, detail)
}

pub(crate) fn write_db_at(
    conn: &Connection,
    event: AuditEvent,
    detail: Option<&str>,
    timestamp: i64,
) -> Result<(), VaultError> {
    let id = uuid::Uuid::new_v4().to_string();
    let event_type = event.to_string();

    conn.execute(
        "INSERT INTO security_audit_log (id, event_type, detail, timestamp) VALUES (?1, ?2, ?3, ?4)",
        params![id, event_type, detail, timestamp],
    )
    .map_err(|e| {
        warn!("Audit log write failed for event '{}': {}", event_type, e);
        VaultError::Database(e.to_string())
    })?;

    Ok(())
}

pub fn path_digest(path: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hex::encode(hasher.finalize())
}

const QUERY_LIMIT_DEFAULT: i64 = 100;
const QUERY_LIMIT_MAX: i64 = 500;

pub fn clamp_query_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(value) if value > 0 => value.min(QUERY_LIMIT_MAX),
        _ => QUERY_LIMIT_DEFAULT,
    }
}

const DETAIL_CLOSED_TAGS: [&str; 4] = ["totp", "conflict", "json", "csv"];
const DETAIL_EXPORT_FORMATS: [&str; 2] = ["json", "csv"];
const UNKNOWN_SEGMENT: &str = "unknown";

fn is_closed_tag(segment: &str) -> bool {
    DETAIL_CLOSED_TAGS.contains(&segment)
}

fn is_export_format(segment: &str) -> bool {
    DETAIL_EXPORT_FORMATS.contains(&segment)
}

fn is_digits(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit())
}

fn parse_count(segment: &str) -> Option<i64> {
    if is_digits(segment) {
        segment.parse().ok()
    } else {
        None
    }
}

fn uuid_id(segment: &str) -> Option<String> {
    uuid::Uuid::parse_str(segment)
        .ok()
        .map(|_| segment.to_string())
}

fn is_base64_digest(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// 未校验入参写入 `detail` 前的形态校验：非 UUID 一律收敛为封闭哨兵值。
pub fn sanitized_entry_id(candidate: &str) -> String {
    uuid_id(candidate).unwrap_or_else(|| UNKNOWN_SEGMENT.to_string())
}

/// SSH 指纹入参写入 `detail` 前的形态校验：只接受 `SHA256:{base64}`（本应用唯一生成的指纹形态），
/// 其余（含 `MD5:` 冒号分段形态与任意前端串）一律收敛为封闭哨兵值。
pub fn sanitized_fingerprint(candidate: &str) -> String {
    match candidate.split_once(':') {
        Some(("SHA256", digest)) if is_base64_digest(digest) => candidate.to_string(),
        _ => UNKNOWN_SEGMENT.to_string(),
    }
}

/// §2.4 的 `detail` 结构形态；`Raw` 表示无法归类，前端按原文兜底展示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedDetail {
    Empty,
    Count(i64),
    EntryId(String),
    EntryWithSuffix {
        entry_id: Option<String>,
        suffix: String,
    },
    TaggedEntry {
        tag: String,
        entry_id: Option<String>,
    },
    EntryCount {
        entry_id: Option<String>,
        count: i64,
    },
    Export {
        format: String,
        count: i64,
        digest: String,
        failed: bool,
    },
    DekRotated {
        old: i64,
        new: i64,
        count: i64,
    },
    Conflict {
        entry_id: Option<String>,
        side: String,
        field: String,
        digest: String,
    },
    Fingerprint {
        key_type: String,
        digest: String,
    },
    Label(String),
    Raw(String),
}

/// 按 §2.4 的消歧规则解析 `detail`：段数 + 首段封闭标签有序匹配，禁止只按「第二段是否数字」单条判断。
pub fn parse_detail(detail: &str) -> ParsedDetail {
    if detail.is_empty() {
        return ParsedDetail::Empty;
    }

    let segments: Vec<&str> = detail.split(':').collect();
    let first = segments[0];

    if is_closed_tag(first) {
        if first == "conflict" {
            return if segments.len() == 5 {
                ParsedDetail::Conflict {
                    entry_id: uuid_id(segments[1]),
                    side: segments[2].to_string(),
                    field: segments[3].to_string(),
                    digest: segments[4].to_string(),
                }
            } else {
                ParsedDetail::Raw(detail.to_string())
            };
        }

        if is_export_format(first) {
            if segments.len() != 3 && segments.len() != 4 {
                return ParsedDetail::Raw(detail.to_string());
            }
            let Some(count) = parse_count(segments[1]) else {
                return ParsedDetail::Raw(detail.to_string());
            };
            let failed = segments.len() == 4;
            if failed && segments[3] != "failed" {
                return ParsedDetail::Raw(detail.to_string());
            }
            return ParsedDetail::Export {
                format: first.to_string(),
                count,
                digest: segments[2].to_string(),
                failed,
            };
        }

        return if segments.len() == 2 {
            ParsedDetail::TaggedEntry {
                tag: first.to_string(),
                entry_id: uuid_id(segments[1]),
            }
        } else {
            ParsedDetail::Raw(detail.to_string())
        };
    }

    if first == "SHA256" {
        return if segments.len() == 2 && is_base64_digest(segments[1]) {
            ParsedDetail::Fingerprint {
                key_type: first.to_string(),
                digest: segments[1].to_string(),
            }
        } else {
            ParsedDetail::Raw(detail.to_string())
        };
    }

    if let Some((head, suffix)) = detail.split_once('@') {
        return ParsedDetail::EntryWithSuffix {
            entry_id: uuid_id(head),
            suffix: suffix.to_string(),
        };
    }

    if segments.len() == 2 {
        if let Some(count) = parse_count(segments[1]) {
            return ParsedDetail::EntryCount {
                entry_id: uuid_id(first),
                count,
            };
        }
    }

    if segments.len() == 3 {
        if let (Some(old), Some(new), Some(count)) = (
            parse_count(segments[0]),
            parse_count(segments[1]),
            parse_count(segments[2]),
        ) {
            return ParsedDetail::DekRotated { old, new, count };
        }
    }

    if segments.len() == 1 {
        if let Some(count) = parse_count(first) {
            return ParsedDetail::Count(count);
        }
        if let Some(id) = uuid_id(first) {
            return ParsedDetail::EntryId(id);
        }
        return ParsedDetail::Label(first.to_string());
    }

    ParsedDetail::Raw(detail.to_string())
}

/// 按 §2.4 消歧规则从 `detail` 提取条目 ID；无法定位时返回 `None`。
pub fn detail_entry_id(detail: &str) -> Option<String> {
    match parse_detail(detail) {
        ParsedDetail::EntryId(id) => Some(id),
        ParsedDetail::EntryWithSuffix { entry_id, .. } => entry_id,
        ParsedDetail::TaggedEntry { entry_id, .. } => entry_id,
        ParsedDetail::EntryCount { entry_id, .. } => entry_id,
        ParsedDetail::Conflict { entry_id, .. } => entry_id,
        _ => None,
    }
}

pub fn query_log(
    conn: &Connection,
    limit: i64,
    since: Option<i64>,
    event_types: Option<&[String]>,
) -> Result<Vec<AuditLogEntry>, VaultError> {
    let mut sql = String::from("SELECT id, event_type, detail, timestamp FROM security_audit_log");
    let mut conditions: Vec<String> = Vec::new();
    if since.is_some() {
        conditions.push("timestamp >= ?".to_string());
    }
    let type_filter = event_types.filter(|types| !types.is_empty());
    if let Some(types) = type_filter {
        let placeholders = vec!["?"; types.len()].join(", ");
        conditions.push(format!("event_type IN ({})", placeholders));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY timestamp DESC, id DESC LIMIT ?");

    let mut values: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(since_ms) = since {
        values.push(rusqlite::types::Value::Integer(since_ms));
    }
    if let Some(types) = type_filter {
        for event_type in types {
            values.push(rusqlite::types::Value::Text(event_type.clone()));
        }
    }
    values.push(rusqlite::types::Value::Integer(limit));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            let detail: Option<String> = row.get("detail")?;
            Ok(AuditLogEntry {
                id: row.get("id")?,
                event_type: row.get("event_type")?,
                entry_id: detail.as_deref().and_then(detail_entry_id),
                detail,
                timestamp: row.get("timestamp")?,
            })
        })
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
    }
    Ok(entries)
}
