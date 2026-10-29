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
            mode_id TEXT,
            is_full INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            active_branch_id TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            tokens INTEGER,
            timestamp INTEGER NOT NULL,
            parent_id TEXT,
            sibling_index INTEGER NOT NULL DEFAULT 0,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            reasoning TEXT,
            tool_calls_json TEXT,
            provider_id TEXT,
            model_id TEXT
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
          ON messages(conversation_id, timestamp);

        CREATE TABLE IF NOT EXISTS attachment_refs (
            attachment_id   TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            message_id      TEXT NOT NULL,
            PRIMARY KEY (attachment_id, conversation_id, message_id)
        );

        CREATE INDEX IF NOT EXISTS idx_attachment_refs_conv
            ON attachment_refs(conversation_id);

        CREATE INDEX IF NOT EXISTS idx_attachment_refs_att
            ON attachment_refs(attachment_id);",
    )
    .map_err(|e| e.to_string())?;

    agent_modes::init_table(&conn)?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_messages_parent_id
          ON messages(conversation_id, parent_id, sibling_index);",
    )
    .map_err(|e| e.to_string())?;

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
        let has_provider_id: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'provider_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_provider_id {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN provider_id TEXT;")
                .map_err(|e| e.to_string())?;
            info!("Added provider_id column to messages table");
        }

        let has_model_id: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'model_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_model_id {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN model_id TEXT;")
                .map_err(|e| e.to_string())?;
            info!("Added model_id column to messages table");
        }

        #[allow(unused)]
        {
            migrations::run_migrations(&conn)?;
        }
    }

    info!("Database initialized at {:?}", db_path);
    Ok(conn)
}

// ── Migration pattern ──────────────────────────────────────────────
// This project only supports ADD COLUMN migrations (no DROP / RENAME / data
// rewrite). The workflow is:
//
//   1. Add the new column to the CREATE TABLE statement above (with DEFAULT
//      so existing rows get a value automatically on fresh DB creation).
//   2. Add a pragma_table_info guard + ALTER TABLE below so **existing**
//      databases that lack the column get it added on next launch.
//
// Template (copy & rename for each new column):
//
//     let has_{column}: bool = conn
//         .query_row(
//             "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'",
//             [],
//             |row| row.get::<_, i64>(0),
//         )
//         .map_err(|e| e.to_string())?
//         > 0;
//     if !has_{column} {
//         conn.execute_batch(
//             "ALTER TABLE {table} ADD COLUMN {column} {type} {constraints};",
//         )
//         .map_err(|e| e.to_string())?;
//         info!("Added {column} column to {table} table");
//     }
//
// Once every user has upgraded (column is guaranteed present), the guard can
// be removed and only the CREATE TABLE definition remains.
// ───────────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn migrate_chain_parent_ids(_conn: &Connection) -> Result<(), String> {
    log::info!("migrate_chain_parent_ids disabled — breaks branch sibling semantics");
    Ok(())
}
