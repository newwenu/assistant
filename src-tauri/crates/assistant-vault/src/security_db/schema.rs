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
