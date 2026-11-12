use std::fmt;

use log::error;
use rusqlite::{params, Connection};

use assistant_crypto::error::VaultError;

pub use crate::dto::audit::AuditLogEntry;

pub mod jsonl;

pub use jsonl::{log_dual, AuditJsonlWriter, JsonlRecord};

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
    LocalDbRepaired,
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
            Self::LocalDbRepaired => write!(f, "local_db_repaired"),
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

/// 仅写 `security.db` 的审计入口（现有调用点的别名，签名不变）。
pub fn log(conn: &Connection, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError> {
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
        error!("Audit log write failed for event '{}': {}", event_type, e);
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

pub fn query_log(conn: &Connection, limit: i64) -> Result<Vec<AuditLogEntry>, VaultError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, event_type, detail, timestamp FROM security_audit_log ORDER BY timestamp DESC LIMIT ?1",
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(AuditLogEntry {
                id: row.get("id")?,
                event_type: row.get("event_type")?,
                detail: row.get("detail")?,
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
