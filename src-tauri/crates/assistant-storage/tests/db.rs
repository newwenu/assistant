use assistant_storage::db;

fn init_temp_db() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = db::init_db(&db_path).unwrap();
    (conn, dir)
}

#[test]
fn init_db_idempotent_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn1 = db::init_db(&db_path).unwrap();
    conn1
        .execute(
            "INSERT INTO providers (id,name,type,base_url,api_key_encrypted,models,default_model,updated_at)
             VALUES ('p1','test','openai','http://x','','[]','gpt-4',0)",
            [],
        )
        .unwrap();
    conn1
        .execute(
            "INSERT INTO conversations (id,title,title_source,model_id,created_at,updated_at)
             VALUES ('c1','hello','placeholder','gpt-4',0,0)",
            [],
        )
        .unwrap();
    drop(conn1);

    let conn2 = db::init_db(&db_path).unwrap();
    let provider_count: i64 = conn2
        .query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
        .unwrap();
    let conv_count: i64 = conn2
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(provider_count, 1);
    assert_eq!(conv_count, 1);
}

#[test]
fn init_db_schema_version_set_once() {
    let (conn, _dir) = init_temp_db();
    let v1: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(v1, "1");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM app_settings WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "schema_version should exist exactly once");
}

#[test]
fn init_db_creates_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a").join("b").join("c");
    let db_path = nested.join("test.db");
    let _conn = db::init_db(&db_path).unwrap();
    assert!(nested.exists(), "parent directories should be created");
}

#[test]
fn init_db_foreign_key_enforced() {
    let (conn, _dir) = init_temp_db();
    let result = conn.execute(
        "INSERT INTO messages (id,conversation_id,role,content,timestamp)
         VALUES ('m1','nonexistent-conv','user','hello',0)",
        [],
    );
    assert!(
        result.is_err(),
        "FK constraint should reject message with nonexistent conversation_id"
    );
}

#[test]
fn init_db_cascade_deletes_messages() {
    let (conn, _dir) = init_temp_db();
    conn.execute(
        "INSERT INTO conversations (id,title,title_source,model_id,created_at,updated_at)
         VALUES ('c1','t','placeholder','m',0,0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id,conversation_id,role,content,timestamp)
         VALUES ('m1','c1','user','hello',0)",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM conversations WHERE id = 'c1'", [])
        .unwrap();
    let msg_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        msg_count, 0,
        "CASCADE should delete messages when conversation is deleted"
    );
}

#[test]
fn init_db_creates_messages_status_column() {
    let (conn, _dir) = init_temp_db();
    let has_status: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'status'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(has_status, 1, "fresh db must have messages.status");
}

#[test]
fn init_db_adds_status_column_to_legacy_messages_table() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
             CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
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
             INSERT INTO messages (id, conversation_id, role, content, timestamp)
                 VALUES ('m1','c1','assistant','hello',0);",
        )
        .unwrap();
    }

    let conn = db::init_db(&db_path).unwrap();
    let has_status: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'status'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(has_status, 1, "legacy db must gain messages.status");

    let status: String = conn
        .query_row("SELECT status FROM messages WHERE id = 'm1'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(status, "completed", "existing rows default to completed");
}

#[test]
fn init_db_creates_permission_tables() {
    let (conn, _dir) = init_temp_db();
    for table in ["permission_grants", "permission_audit"] {
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{table}'"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "fresh db must have {table}");
    }
}

#[test]
fn init_db_adds_tool_columns_to_legacy_messages_table() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy_tool.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
             CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
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
             INSERT INTO messages (id, conversation_id, role, content, timestamp)
                 VALUES ('m1','c1','assistant','hello',0);",
        )
        .unwrap();
    }

    let conn = db::init_db(&db_path).unwrap();
    for column in ["tool_call_id", "is_error"] {
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = '{column}'"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "legacy db must gain messages.{column}");
    }

    let (tool_call_id, is_error): (Option<String>, i64) = conn
        .query_row(
            "SELECT tool_call_id, is_error FROM messages WHERE id = 'm1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        tool_call_id.is_none(),
        "existing rows keep NULL tool_call_id"
    );
    assert_eq!(is_error, 0, "existing rows default is_error to 0");
}

#[test]
fn init_db_adds_tool_config_columns_to_legacy_tables() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy_tool_config.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
             CREATE TABLE conversations (
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
             INSERT INTO conversations (id,title,title_source,model_id,created_at,updated_at)
                 VALUES ('c1','t','placeholder','m',0,0);
             CREATE TABLE agent_modes (
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
                updated_at INTEGER NOT NULL
             );
             INSERT INTO agent_modes (id,name,kind,created_at,updated_at)
                 VALUES ('user-legacy','legacy','deep',0,0);",
        )
        .unwrap();
    }

    let conn = db::init_db(&db_path).unwrap();
    for table in ["agent_modes", "conversations"] {
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = 'tool_config_json'"
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "legacy db must gain {table}.tool_config_json");
    }

    let legacy_kind: String = conn
        .query_row(
            "SELECT kind FROM agent_modes WHERE id = 'user-legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(legacy_kind, "deep", "用户自建行不得被改写");

    let (conv_tool_config, mode_tool_config): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT c.tool_config_json, m.tool_config_json
             FROM conversations c, agent_modes m
             WHERE c.id = 'c1' AND m.id = 'user-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(conv_tool_config.is_none(), "既有会话补列后为 NULL");
    assert!(mode_tool_config.is_none(), "既有模式补列后为 NULL");

    let builtin_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM agent_modes WHERE is_builtin = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(builtin_count, 3, "补列后仍能正常播种内置模式");
}

#[test]
fn init_db_adds_workspace_root_column_with_empty_default_and_does_not_migrate() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy_workspace.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
             CREATE TABLE conversations (
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
             INSERT INTO conversations (id,title,title_source,model_id,created_at,updated_at)
                 VALUES ('c1','t','placeholder','m',0,0);",
        )
        .unwrap();
    }

    let conn = db::init_db(&db_path).unwrap();

    let notnull: i64 = conn
        .query_row(
            "SELECT \"notnull\" FROM pragma_table_info('conversations') WHERE name = 'workspace_root'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(notnull, 1, "列必须 NOT NULL");

    let dflt: Option<String> = conn
        .query_row(
            "SELECT dflt_value FROM pragma_table_info('conversations') WHERE name = 'workspace_root'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dflt.as_deref(), Some("''"), "默认值必须为空串哨兵");

    let value: String = conn
        .query_row(
            "SELECT workspace_root FROM conversations WHERE id = 'c1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(value, "", "存量会话升级后为未配置，不做任何回填");

    let has_book: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'workspaces'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(has_book, 1, "目录簿表随初始化创建");
}

fn has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"),
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
        > 0
}

#[test]
fn init_db_creates_message_mode_id_and_mode_avatar_columns() {
    let (conn, _dir) = init_temp_db();
    assert!(
        has_column(&conn, "messages", "mode_id"),
        "A1: 新库必须有 messages.mode_id"
    );
    assert!(
        has_column(&conn, "agent_modes", "avatar"),
        "A1: 新库必须有 agent_modes.avatar"
    );
}

#[test]
fn init_db_adds_identity_columns_to_legacy_tables() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy_identity.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO app_settings (key, value) VALUES ('schema_version', '1');
             CREATE TABLE agent_modes (
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
                tool_config_json TEXT
             );
             INSERT INTO agent_modes (id,name,kind,created_at,updated_at)
                 VALUES ('user-legacy','legacy','chat',0,0);
             CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
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
                is_error INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO messages (id, conversation_id, role, content, timestamp)
                 VALUES ('m1','c1','assistant','hello',0);",
        )
        .unwrap();
    }

    let conn = db::init_db(&db_path).unwrap();
    assert!(
        has_column(&conn, "messages", "mode_id"),
        "A2: 旧库升级必须补上 messages.mode_id"
    );
    assert!(
        has_column(&conn, "agent_modes", "avatar"),
        "A2: 旧库升级必须补上 agent_modes.avatar"
    );

    let message_mode_id: Option<String> = conn
        .query_row("SELECT mode_id FROM messages WHERE id = 'm1'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(
        message_mode_id.is_none(),
        "A2: 存量消息不回填 mode_id，保持 NULL"
    );

    let legacy_avatar: Option<String> = conn
        .query_row(
            "SELECT avatar FROM agent_modes WHERE id = 'user-legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        legacy_avatar.is_none(),
        "A2: 存量模式行头像保持 NULL，渲染侧走降级链"
    );

    let default_avatar: Option<String> = conn
        .query_row(
            "SELECT avatar FROM agent_modes WHERE id = 'builtin-default'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        default_avatar.as_deref(),
        Some("builtin:chat"),
        "A2: 内置行按种子值补齐头像"
    );

    drop(conn);
    let conn = db::init_db(&db_path).unwrap();
    assert!(
        has_column(&conn, "messages", "mode_id"),
        "A2: 列已存在时必须跳过重复 ALTER"
    );
    assert!(
        has_column(&conn, "agent_modes", "avatar"),
        "A2: 列已存在时必须跳过重复 ALTER"
    );
}
