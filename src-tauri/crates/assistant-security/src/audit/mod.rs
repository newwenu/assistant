use std::fmt;

use rusqlite::{params, Connection};

use crate::types::VaultError;

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
    VaultCopyPassword,
    VaultCopyField,
    HibpAudit,
    HibpLeakFound,
    MasterPasswordChange,
    RecoveryKeyGenerated,
    BiometricEnrolled,
    SshKeyLoaded,
    SshKeyRemoved,
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
            Self::VaultCopyPassword => write!(f, "vault_copy_password"),
            Self::VaultCopyField => write!(f, "vault_copy_field"),
            Self::HibpAudit => write!(f, "hibp_audit"),
            Self::HibpLeakFound => write!(f, "hibp_leak_found"),
            Self::MasterPasswordChange => write!(f, "master_password_change"),
            Self::RecoveryKeyGenerated => write!(f, "recovery_key_generated"),
            Self::BiometricEnrolled => write!(f, "biometric_enrolled"),
            Self::SshKeyLoaded => write!(f, "ssh_key_loaded"),
            Self::SshKeyRemoved => write!(f, "ssh_key_removed"),
        }
    }
}

pub fn log(conn: &Connection, event: AuditEvent, detail: Option<&str>) -> Result<(), VaultError> {
    let id = uuid::Uuid::new_v4().to_string();
    let event_type = event.to_string();
    let timestamp = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO security_audit_log (id, event_type, detail, timestamp) VALUES (?1, ?2, ?3, ?4)",
        params![id, event_type, detail, timestamp],
    )
    .map_err(|e| VaultError::Database(e.to_string()))?;

    Ok(())
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

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub id: String,
    pub event_type: String,
    pub detail: Option<String>,
    pub timestamp: i64,
}
