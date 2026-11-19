mod common;

use assistant_storage::agent_modes::{
    self, AgentModeRow, ApprovalLevel, ModeToolConfig, BUILTIN_AGENT_TOOL_CONFIG,
};
use assistant_storage::conversations;
use assistant_storage::db;
use common::{conv, init_db};
use rusqlite::Connection;

fn mode_row(id: &str, kind: &str, is_builtin: bool) -> AgentModeRow {
    AgentModeRow {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        kind: kind.to_string(),
        prompt: String::new(),
        model: None,
        temperature: None,
        max_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
        presentation_rank: 99,
        is_builtin,
        created_at: 1,
        updated_at: 1,
        tool_config_json: None,
        avatar: None,
    }
}

fn dump(conn: &Connection) -> String {
    let mut stmt = conn
        .prepare("SELECT id, kind, is_builtin, tool_config_json FROM agent_modes ORDER BY id")
        .unwrap();
    let modes: Vec<String> = stmt
        .query_map([], |r| {
            Ok(format!(
                "{}|{}|{}|{:?}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(stmt);

    let mut stmt = conn
        .prepare("SELECT id, mode_id FROM conversations ORDER BY id")
        .unwrap();
    let convs: Vec<String> = stmt
        .query_map([], |r| {
            Ok(format!(
                "{}|{:?}",
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    format!("{}::{}", modes.join(","), convs.join(","))
}

fn column_count(conn: &Connection, table: &str, column: &str) -> i64 {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"),
        [],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn init_db_seeds_three_builtin_modes() {
    let conn = init_db();
    let builtin: Vec<AgentModeRow> = agent_modes::get_modes(&conn)
        .unwrap()
        .into_iter()
        .filter(|m| m.is_builtin)
        .collect();

    assert_eq!(builtin.len(), 3);
    let ids: Vec<&str> = builtin.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"builtin-default"));
    assert!(ids.contains(&"builtin-agent"));
    assert!(ids.contains(&"builtin-translate"));
    assert!(!ids.contains(&"builtin-quick"));
    assert!(!ids.contains(&"builtin-deep"));

    let default = agent_modes::get_by_id(&conn, "builtin-default")
        .unwrap()
        .unwrap();
    assert_eq!(default.kind, "chat");

    let translate = agent_modes::get_by_id(&conn, "builtin-translate")
        .unwrap()
        .unwrap();
    assert_eq!(translate.kind, "chat");

    let agent = agent_modes::get_by_id(&conn, "builtin-agent")
        .unwrap()
        .unwrap();
    assert_eq!(agent.kind, "agent");
    assert_eq!(
        agent.tool_config_json.as_deref(),
        Some(agent_modes::encode_tool_config(&BUILTIN_AGENT_TOOL_CONFIG).as_str())
    );
}

#[test]
fn builtin_seed_tool_config_matches_defaults() {
    let conn = init_db();
    let default_config: ModeToolConfig = serde_json::from_str(
        agent_modes::get_by_id(&conn, "builtin-default")
            .unwrap()
            .unwrap()
            .tool_config_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(default_config, ModeToolConfig::default());

    let agent_config: ModeToolConfig = serde_json::from_str(
        agent_modes::get_by_id(&conn, "builtin-agent")
            .unwrap()
            .unwrap()
            .tool_config_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(agent_config, BUILTIN_AGENT_TOOL_CONFIG);
    assert_eq!(agent_config.approval_level, ApprovalLevel::DangerousOnly);
    assert!(agent_config.enabled_tools.is_empty());
}

#[test]
fn init_db_creates_tool_config_columns_once() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("tool_config.db");
    let conn = db::init_db(&db_path).unwrap();
    assert_eq!(column_count(&conn, "agent_modes", "tool_config_json"), 1);
    assert_eq!(column_count(&conn, "conversations", "tool_config_json"), 1);
    drop(conn);

    let conn = db::init_db(&db_path).unwrap();
    assert_eq!(column_count(&conn, "agent_modes", "tool_config_json"), 1);
    assert_eq!(column_count(&conn, "conversations", "tool_config_json"), 1);
}

#[test]
fn init_db_reopen_preserves_legacy_mode_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy_mode.db");
    let conn = db::init_db(&db_path).unwrap();
    agent_modes::insert_mode(&conn, &mode_row("user-legacy", "deep", false)).unwrap();
    let before = dump(&conn);
    drop(conn);

    let conn = db::init_db(&db_path).unwrap();

    assert_eq!(dump(&conn), before);
    let legacy = agent_modes::get_by_id(&conn, "user-legacy")
        .unwrap()
        .unwrap();
    assert_eq!(legacy.kind, "deep");
}

#[test]
fn init_db_reopen_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = db::init_db(&db_path).unwrap();
    agent_modes::insert_mode(&conn, &mode_row("user-deep", "deep", false)).unwrap();
    let mut c1 = conv("c1");
    c1.mode_id = Some("user-deep".to_string());
    conversations::insert_conversation(&conn, &c1).unwrap();
    let before = dump(&conn);
    drop(conn);

    let conn = db::init_db(&db_path).unwrap();
    assert_eq!(dump(&conn), before);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM agent_modes WHERE is_builtin = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 3, "内置行重复初始化不得新增");

    let user = agent_modes::get_by_id(&conn, "user-deep").unwrap().unwrap();
    assert_eq!(user.kind, "deep");
}

#[test]
fn avatar_roundtrips_through_insert_and_read() {
    let conn = init_db();
    let mut row = mode_row("user-avatar", "chat", false);
    row.avatar = Some("builtin:agent".to_string());
    agent_modes::insert_mode(&conn, &row).unwrap();

    let stored = agent_modes::get_by_id(&conn, "user-avatar")
        .unwrap()
        .unwrap();
    assert_eq!(stored.avatar.as_deref(), Some("builtin:agent"));

    let mut updated = stored.clone();
    updated.avatar = Some("builtin:translate".to_string());
    agent_modes::update_mode(&conn, &updated).unwrap();
    let reread = agent_modes::get_by_id(&conn, "user-avatar")
        .unwrap()
        .unwrap();
    assert_eq!(reread.avatar.as_deref(), Some("builtin:translate"));

    updated.avatar = None;
    agent_modes::update_mode(&conn, &updated).unwrap();
    assert_eq!(
        agent_modes::get_by_id(&conn, "user-avatar")
            .unwrap()
            .unwrap()
            .avatar,
        None,
        "update 写入 NULL 即清除头像"
    );
}

#[test]
fn reset_builtin_mode_restores_default_avatar() {
    let conn = init_db();
    let resolver = |key: &str| key.to_string();

    for (id, expected) in [
        ("builtin-default", "builtin:chat"),
        ("builtin-agent", "builtin:agent"),
        ("builtin-translate", "builtin:translate"),
    ] {
        let mut row = agent_modes::get_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(row.avatar.as_deref(), Some(expected), "种子头像");

        row.avatar = Some("builtin:default".to_string());
        row.name = "renamed".to_string();
        agent_modes::update_mode(&conn, &row).unwrap();

        agent_modes::reset_builtin_mode(&conn, id, &resolver).unwrap();
        let reset = agent_modes::get_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(
            reset.avatar.as_deref(),
            Some(expected),
            "A4: 重置后 {id} 头像恢复内置默认值"
        );
        assert_ne!(reset.name, "renamed", "A4: 重置同时恢复名称");
    }
}
