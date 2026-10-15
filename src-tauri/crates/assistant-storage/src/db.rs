use log::info;
use rusqlite::Connection;
use std::path::Path;

use crate::agent_modes;
use crate::migrations;

pub fn init_db(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key_encrypted TEXT NOT NULL,
            models TEXT NOT NULL,
            default_model TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            title_source TEXT NOT NULL DEFAULT 'placeholder',
            provider_id TEXT,
            model_id TEXT NOT NULL,
            system_prompt TEXT,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            generation INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            tokens INTEGER,
            timestamp INTEGER NOT NULL,
            parent_id TEXT,
            is_pinned INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tool_favorites (
            tool_id TEXT PRIMARY KEY
        );

        CREATE TABLE IF NOT EXISTS tool_history (
            tool_id TEXT PRIMARY KEY,
            used_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS compression_state (
            conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
            last_compression_at INTEGER,
            compression_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conv_ts
          ON messages(conversation_id, timestamp);",
    )
    .map_err(|e| e.to_string())?;

    agent_modes::init_table(&conn)?;

    let has_mode_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'mode_id'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_mode_id {
        conn.execute_batch("ALTER TABLE conversations ADD COLUMN mode_id TEXT;")
            .map_err(|e| e.to_string())?;
        info!("Added mode_id column to conversations table");
    }

    let has_is_full: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'is_full'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_is_full {
        conn.execute_batch(
            "ALTER TABLE conversations ADD COLUMN is_full INTEGER NOT NULL DEFAULT 0;",
        )
        .map_err(|e| e.to_string())?;
        info!("Added is_full column to conversations table");
    }

    agent_modes::seed_builtin_modes(&conn)?;

    let is_new_db: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM app_settings WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        == 0;

    if is_new_db {
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('schema_version', '1')",
            [],
        )
        .map_err(|e| e.to_string())?;
        info!("New database created with schema_version = 1");
    } else {
        #[allow(unused)]
        {
            migrations::run_migrations(&conn)?;
        }
    }

    info!("Database initialized at {:?}", db_path);
    Ok(conn)
}
