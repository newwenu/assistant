use log::info;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::agent_modes;
use crate::migrations;

/// 按保留期删除 `permission_audit` 的过期行，返回删除行数。
///
/// `retention_days = 0` 表示不清理（与 `result_retention_days` 的「0 = 禁用」一致）。
/// 审计是只追加表，没有删除机制就会随使用时间单调增长；清理只在启动时跑一次，
/// 不做后台定时，避免与工具决策的高频写入争用同一把连接锁。
///
/// `now_millis` 由调用方注入而非内部取时钟：边界行为可在单测里确定性验证。
pub fn cleanup_expired_audit(
    conn: &Connection,
    retention_days: u32,
    now_millis: i64,
) -> Result<usize, String> {
    if retention_days == 0 {
        return Ok(0);
    }
    let cutoff = now_millis.saturating_sub(i64::from(retention_days) * 86_400_000);
    conn.execute(
        "DELETE FROM permission_audit WHERE at_millis < ?1",
        params![cutoff],
    )
    .map_err(|e| e.to_string())
}

pub fn init_db(db_path: &Path) -> Result<Connection, String> {
    init_db_localized(db_path, &|key| key.to_string())
}

pub fn init_db_localized(
    db_path: &Path,
    resolve: &dyn Fn(&str) -> String,
) -> Result<Connection, String> {
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
            active_branch_id TEXT,
            tool_config_json TEXT,
            workspace_root TEXT NOT NULL DEFAULT '',
            -- 会话内下一个可用 turn 序号（I46 D2）。取号即自增，不回退、不复用。
            next_turn_index INTEGER NOT NULL DEFAULT 1
        );

        -- 模式级工具装配（`ModeToolConfig` 的 JSON）由 `agent_modes` 模块负责编解码；
        -- 内置行初始值见 `agent_modes::BUILTIN_AGENT_TOOL_CONFIG`。
        CREATE TABLE IF NOT EXISTS agent_modes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'custom',
            prompt TEXT NOT NULL DEFAULT '',
            model TEXT,
            temperature REAL,
            max_tokens INTEGER,
            reasoning_enabled INTEGER,
            reasoning_effort TEXT,
            presentation_rank INTEGER NOT NULL DEFAULT 99,
            is_builtin INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            tool_config_json TEXT,
            avatar TEXT
        );

        -- `tool_calls_json` 存 assistant 行的工具调用线格式（id / name / arguments），
        -- 结果文本仍由 `role = 'tool'` 行的 `content` 承载（I46 F15），禁止 DROP；
        -- 读取时必须经结构化 DTO 转换，不得把该列的 JSON 文本直接搬进 DTO。
        --
        -- 工具结果行的 `status` 额外承载执行阶段（D27）：`pending` = 已声明、未进入
        -- 执行队列；`running` = 已进入执行队列、结果未落库；`completed` = 有真实结果。
        -- 占位行在声明时即插入（正文为空），结果到达时原地回填——拓扑一经确定不再改写。
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
            model_id TEXT,
            status TEXT NOT NULL DEFAULT 'completed',
            tool_call_id TEXT,
            is_error INTEGER NOT NULL DEFAULT 0,
            mode_id TEXT,
            -- 节点坐标（I46 D2）：`turn_index` 为会话内单调 turn 序号，`NULL` = 未知
            -- （存量行 / 压缩摘要）；`iteration` 为 turn 内迭代序号，0 = 非迭代行。
            -- 两者只由后端写入，渲染层禁止猜测。
            turn_index INTEGER,
            iteration INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tool_favorites (
            tool_id TEXT PRIMARY KEY
        );

        CREATE TABLE IF NOT EXISTS tool_history (
            tool_id TEXT PRIMARY KEY,
            used_at INTEGER NOT NULL
        );

        -- MCP server 配置（I38e §6.1）：唯一运行时配置源。
        -- `id` 不可变（模型工具名与授权键都用它）；`name` 只是展示名。
        -- `config_encrypted` 是 `{command,args,cwd,env}` 的密文（base64 文本），
        -- `config_hash` 是明文规范 JSON 的 SHA-256。运行时状态（Ready / Error /
        -- 重试次数）不落库，只有 `enabled` 持久化。
        CREATE TABLE IF NOT EXISTS mcp_servers (
            id               TEXT PRIMARY KEY,
            name             TEXT NOT NULL,
            transport        TEXT NOT NULL DEFAULT 'stdio',
            config_encrypted TEXT NOT NULL,
            config_hash      TEXT NOT NULL,
            enabled          INTEGER NOT NULL DEFAULT 0,
            created_at       INTEGER NOT NULL,
            updated_at       INTEGER NOT NULL
        );

        -- 审批授权与审计（D14）：幂等建表，不做 ALTER 迁移。
        -- `grant_key` 为 `ApprovalKey` 的规范 JSON；审计只追加不更新，
        -- 过期行由 `cleanup_expired_audit` 按保留期删除。
        -- 归属坐标（conversation_id / turn_id / iteration）必须与决策同行落库：
        -- 靠时间戳事后关联不可行（毫秒既非唯一标识也不单调）。
        CREATE TABLE IF NOT EXISTS permission_grants (
            grant_key TEXT PRIMARY KEY,
            tool      TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS permission_audit (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            tool            TEXT NOT NULL,
            action          TEXT NOT NULL,
            resource        TEXT NOT NULL,
            outcome         TEXT NOT NULL,
            at_millis       INTEGER NOT NULL,
            conversation_id TEXT,
            turn_id         TEXT,
            iteration       INTEGER
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

        -- 工作区目录簿：只作会话头下拉的「最近工作区」候选与显示名缓存，
        -- **不参与解析**。会话绑定值存在 `conversations.workspace_root`，
        -- 删除簿行不得改变任何会话的行为。
        CREATE TABLE IF NOT EXISTS workspaces (
            path       TEXT PRIMARY KEY,
            name       TEXT,
            created_at INTEGER NOT NULL,
            used_at    INTEGER NOT NULL
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
            ON attachment_refs(attachment_id);

        CREATE TABLE IF NOT EXISTS token_usage_records (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id       TEXT    NOT NULL,
            message_id            TEXT,
            attempt_id            TEXT,
            branch_leaf_id        TEXT,
            kind                  TEXT    NOT NULL,
            provider_id           TEXT,
            model_id              TEXT,
            prompt_tokens         INTEGER NOT NULL,
            completion_tokens     INTEGER NOT NULL,
            cache_read_tokens     INTEGER,
            cache_write_tokens    INTEGER,
            reasoning_tokens      INTEGER,
            cache_capable         INTEGER NOT NULL DEFAULT 0,
            context_window_tokens INTEGER,
            estimated             INTEGER NOT NULL DEFAULT 0,
            created_at            INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_token_usage_conv
            ON token_usage_records (conversation_id, branch_leaf_id, created_at DESC, id DESC);",
    )
    .map_err(|e| e.to_string())?;

    let has_agent_tool_config: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('agent_modes') WHERE name = 'tool_config_json'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_agent_tool_config {
        conn.execute_batch("ALTER TABLE agent_modes ADD COLUMN tool_config_json TEXT;")
            .map_err(|e| e.to_string())?;
        info!("Added tool_config_json column to agent_modes table");
    }

    // 模式头像标识（I43 §3.1）：可空，`NULL` = 未配置，读取侧走渲染降级链。
    let has_agent_avatar: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('agent_modes') WHERE name = 'avatar'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_agent_avatar {
        conn.execute_batch("ALTER TABLE agent_modes ADD COLUMN avatar TEXT;")
            .map_err(|e| e.to_string())?;
        info!("Added avatar column to agent_modes table");
    }

    let has_conv_tool_config: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'tool_config_json'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_conv_tool_config {
        conn.execute_batch("ALTER TABLE conversations ADD COLUMN tool_config_json TEXT;")
            .map_err(|e| e.to_string())?;
        info!("Added tool_config_json column to conversations table");
    }

    // 工作区绑定列（I38h §2.1）：空串 = 未配置，列不允许 NULL。
    // 存量行在加列时直接落到 `''`，因此读取侧永远拿到 `String`。
    let has_conv_workspace_root: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'workspace_root'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_conv_workspace_root {
        conn.execute_batch(
            "ALTER TABLE conversations ADD COLUMN workspace_root TEXT NOT NULL DEFAULT '';",
        )
        .map_err(|e| e.to_string())?;
        info!("Added workspace_root column to conversations table");
    }

    // 会话级 turn 取号器（I46 §2.1）：存量会话初始化为 1，不回填历史行的
    // `messages.turn_index`（存量行保持 `NULL`，渲染侧走降级分支）。
    let has_conv_next_turn_index: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'next_turn_index'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !has_conv_next_turn_index {
        conn.execute_batch(
            "ALTER TABLE conversations ADD COLUMN next_turn_index INTEGER NOT NULL DEFAULT 1;",
        )
        .map_err(|e| e.to_string())?;
        info!("Added next_turn_index column to conversations table");
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_messages_parent_id
          ON messages(conversation_id, parent_id, sibling_index);",
    )
    .map_err(|e| e.to_string())?;

    agent_modes::seed_builtin_modes(&conn, resolve)?;

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

        let has_status: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'status'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_status {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN status TEXT NOT NULL DEFAULT 'completed';",
            )
            .map_err(|e| e.to_string())?;
            info!("Added status column to messages table");
        }

        let has_tool_call_id: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'tool_call_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_tool_call_id {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN tool_call_id TEXT;")
                .map_err(|e| e.to_string())?;
            info!("Added tool_call_id column to messages table");
        }

        let has_is_error: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'is_error'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_is_error {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN is_error INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(|e| e.to_string())?;
            info!("Added is_error column to messages table");
        }

        // 消息级模式（I43 §3.1）：`NULL` = 未知来源（存量行 / 压缩摘要 / user 行）。
        // 只追加列，不回填存量数据。
        let has_mode_id: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'mode_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_mode_id {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN mode_id TEXT;")
                .map_err(|e| e.to_string())?;
            info!("Added mode_id column to messages table");
        }

        // 节点坐标列（I46 §2.1）：只追加列，不回填存量行。
        let has_turn_index: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'turn_index'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_turn_index {
            conn.execute_batch("ALTER TABLE messages ADD COLUMN turn_index INTEGER;")
                .map_err(|e| e.to_string())?;
            info!("Added turn_index column to messages table");
        }

        let has_iteration: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'iteration'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !has_iteration {
            conn.execute_batch(
                "ALTER TABLE messages ADD COLUMN iteration INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(|e| e.to_string())?;
            info!("Added iteration column to messages table");
        }

        #[allow(unused)]
        {
            migrations::run_migrations(&conn)?;
        }

        // 审计归属坐标（I50 §5）：只追加可空列，不回填存量行。
        // 存量行没有会话信息是事实，不能用空串或占位 id 伪造归属。
        for column in ["conversation_id", "turn_id", "iteration"] {
            let has_column: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('permission_audit') WHERE name = ?1",
                    [column],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|e| e.to_string())?
                > 0;
            if !has_column {
                let ddl = if column == "iteration" {
                    "ALTER TABLE permission_audit ADD COLUMN iteration INTEGER;".to_string()
                } else {
                    format!("ALTER TABLE permission_audit ADD COLUMN {column} TEXT;")
                };
                conn.execute_batch(&ddl).map_err(|e| e.to_string())?;
                info!("Added {column} column to permission_audit table");
            }
        }
    }

    // 坐标索引（I46 §2.1）：必须在列补齐之后创建，否则存量库加列前会失败。
    // 顺序为 (conversation_id, turn_index, iteration)，`turn_index IS NULL` 的
    // 存量行落在索引尾部，不影响新行排序。
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_messages_conv_coord
          ON messages(conversation_id, turn_index, iteration);",
    )
    .map_err(|e| e.to_string())?;

    // 权限审计时间索引（I50 §5 O3）：查询面按 `at_millis` 范围过滤，无索引即全表扫描。
    // 与其它索引同批执行；`IF NOT EXISTS` 对存量库幂等，不需要 `has_*` 守卫。
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_permission_audit_at
          ON permission_audit(at_millis);",
    )
    .map_err(|e| e.to_string())?;

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

#[cfg(test)]
mod audit_retention_tests {
    use super::*;

    fn temp_conn() -> (Connection, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = init_db(&dir.path().join("audit.db")).unwrap();
        (conn, dir)
    }

    fn insert(conn: &Connection, at_millis: i64) {
        conn.execute(
            "INSERT INTO permission_audit (tool, action, resource, outcome, at_millis)
             VALUES ('agent_write', 'write', '…/a.txt', 'allowed', ?1)",
            params![at_millis],
        )
        .unwrap();
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM permission_audit", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    /// `0` 是「不清理」而不是「删光」：与 `result_retention_days` 的约定一致。
    #[test]
    fn zero_retention_keeps_everything() {
        let (conn, _dir) = temp_conn();
        insert(&conn, 0);
        assert_eq!(
            cleanup_expired_audit(&conn, 0, 10_000_000_000_000).unwrap(),
            0
        );
        assert_eq!(count(&conn), 1);
    }

    /// 边界行（恰好 `retention_days` 天前）必须保留：判据是严格早于截断点。
    #[test]
    fn deletes_rows_older_than_cutoff_and_keeps_boundary() {
        let (conn, _dir) = temp_conn();
        let day = 86_400_000i64;
        let now = 100 * day;
        insert(&conn, now - 91 * day);
        insert(&conn, now - 90 * day);
        insert(&conn, now);

        assert_eq!(cleanup_expired_audit(&conn, 90, now).unwrap(), 1);
        assert_eq!(count(&conn), 2);
    }

    /// 存量库没有归属列时必须由迁移补齐：`CREATE TABLE IF NOT EXISTS` 不会改结构。
    #[test]
    fn migration_adds_ownership_columns_to_existing_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
                 CREATE TABLE permission_audit (
                    id        INTEGER PRIMARY KEY AUTOINCREMENT,
                    tool      TEXT NOT NULL,
                    action    TEXT NOT NULL,
                    resource  TEXT NOT NULL,
                    outcome   TEXT NOT NULL,
                    at_millis INTEGER NOT NULL
                );",
            )
            .unwrap();
        }

        let conn = init_db(&path).unwrap();
        for column in ["conversation_id", "turn_id", "iteration"] {
            let present: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('permission_audit') WHERE name = ?1",
                    [column],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, 1, "{column} 必须由迁移补齐");
        }
    }
}
