use std::path::Path;

use rusqlite::Connection;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::DbKey;

use super::{SQLCIPHER_HARDENING_PRAGMAS, SQLCIPHER_PRAGMAS};

pub const SCHEMA_VERSION: u32 = 2;

pub const INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS vault_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_entries (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    username TEXT,
    url TEXT,
    category TEXT NOT NULL DEFAULT 'login',
    encrypted_password TEXT,
    encrypted_notes TEXT,
    encrypted_custom_fields TEXT,
    encrypted_totp_secret TEXT,
    totp_digits INTEGER NOT NULL DEFAULT 6,
    totp_period INTEGER NOT NULL DEFAULT 30,
    tags TEXT NOT NULL DEFAULT '[]',
    is_favorite INTEGER NOT NULL DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 1,
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS security_audit_log (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    detail TEXT,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_vault_entries_category
    ON vault_entries(category);
CREATE INDEX IF NOT EXISTS idx_vault_entries_favorite
    ON vault_entries(is_favorite);
CREATE INDEX IF NOT EXISTS idx_vault_entries_updated
    ON vault_entries(updated_at);
CREATE INDEX IF NOT EXISTS idx_vault_entries_deleted
    ON vault_entries(deleted_at);
CREATE INDEX IF NOT EXISTS idx_vault_entries_tags
    ON vault_entries(tags);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
    ON security_audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_event_type
    ON security_audit_log(event_type);
"#;

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
