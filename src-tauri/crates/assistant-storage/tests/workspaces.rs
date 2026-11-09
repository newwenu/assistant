mod common;

use assistant_storage::conversations::{insert_conversation, ConversationRow};
use assistant_storage::workspaces;
use common::conv;

fn temp_dir_path(dir: &tempfile::TempDir, name: &str) -> String {
    let path = dir.path().join(name);
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn upsert_inserts_then_refreshes_without_changing_name() {
    let conn = common::init_db();
    let dir = tempfile::tempdir().unwrap();
    let path = temp_dir_path(&dir, "alpha");

    let first = workspaces::upsert(&conn, &path, None).unwrap();
    assert_eq!(first.name, "alpha");
    assert_eq!(first.created_at, first.used_at);

    let second = workspaces::upsert(&conn, &path, None).unwrap();
    assert_eq!(
        second.created_at, first.created_at,
        "重复登记不得改创建时间"
    );
    assert!(second.used_at >= first.used_at, "重复登记必须刷新使用时间");
    assert_eq!(second.name, "alpha", "未给名字时保留既有显示名");

    let renamed = workspaces::upsert(&conn, &path, Some("我的项目")).unwrap();
    assert_eq!(renamed.name, "我的项目");
    assert_eq!(
        workspaces::get_name(&conn, &path).unwrap().as_deref(),
        Some("我的项目")
    );
}

#[test]
fn upsert_ignores_blank_name_and_falls_back_to_basename() {
    let conn = common::init_db();
    let dir = tempfile::tempdir().unwrap();
    let path = temp_dir_path(&dir, "beta");

    let entry = workspaces::upsert(&conn, &path, Some("   ")).unwrap();
    assert_eq!(entry.name, "beta", "空白名字视同未提供，退回 basename");
}

#[test]
fn list_orders_by_used_at_desc() {
    let conn = common::init_db();
    let dir = tempfile::tempdir().unwrap();
    let a = temp_dir_path(&dir, "a");
    let b = temp_dir_path(&dir, "b");

    workspaces::upsert(&conn, &a, None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    workspaces::upsert(&conn, &b, None).unwrap();
    let listed = workspaces::list(&conn).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].path, b, "最近使用的工作区排在最前");

    std::thread::sleep(std::time::Duration::from_millis(5));
    workspaces::upsert(&conn, &a, None).unwrap();
    let listed = workspaces::list(&conn).unwrap();
    assert_eq!(listed[0].path, a);
}

#[test]
fn forget_only_removes_book_entry_not_conversation_binding() {
    let conn = common::init_db();
    let dir = tempfile::tempdir().unwrap();
    let path = temp_dir_path(&dir, "gamma");

    let mut row = conv("c1");
    row.workspace_root = path.clone();
    insert_conversation(&conn, &row).unwrap();
    workspaces::upsert(&conn, &path, None).unwrap();

    workspaces::forget(&conn, &path).unwrap();
    assert!(workspaces::list(&conn).unwrap().is_empty(), "簿行应被删除");
    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1")
            .unwrap()
            .as_deref(),
        Some(path.as_str()),
        "会话绑定不得被删除收藏影响"
    );
}

#[test]
fn conversation_workspace_defaults_to_empty_and_clear_writes_empty() {
    let conn = common::init_db();
    insert_conversation(&conn, &conv("c1")).unwrap();

    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1").unwrap(),
        Some(String::new()),
        "新会话即未配置，读侧恒为 String"
    );

    workspaces::set_conversation_workspace(&conn, "c1", "/tmp/x").unwrap();
    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1")
            .unwrap()
            .as_deref(),
        Some("/tmp/x")
    );

    workspaces::set_conversation_workspace(&conn, "c1", workspaces::WORKSPACE_UNCONFIGURED)
        .unwrap();
    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1").unwrap(),
        Some(String::new()),
        "清除绑定即写回空串哨兵"
    );

    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "missing").unwrap(),
        None,
        "会话不存在返回 None"
    );
}

#[test]
fn duplicate_conversation_inherits_workspace_binding() {
    let conn = common::init_db();
    let dir = tempfile::tempdir().unwrap();
    let path = temp_dir_path(&dir, "delta");

    let mut row = conv("c1");
    row.workspace_root = path.clone();
    insert_conversation(&conn, &row).unwrap();

    let new_id =
        assistant_storage::conversations::ops::duplicate_conversation(&conn, "c1").unwrap();
    assert_eq!(
        workspaces::get_conversation_workspace(&conn, &new_id)
            .unwrap()
            .as_deref(),
        Some(path.as_str()),
        "复制产物继承源会话的工作区"
    );
}

#[test]
fn workspace_entry_struct_is_serializable() {
    let entry = workspaces::WorkspaceEntry {
        path: "p".to_string(),
        name: "n".to_string(),
        created_at: 1,
        used_at: 2,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: workspaces::WorkspaceEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back, entry);
}

#[test]
fn conv_row_workspace_root_is_string_not_option() {
    let row: ConversationRow = conv("c1");
    assert_eq!(row.workspace_root, "", "ConversationRow 的值域是 String");
}

fn insert_grant(conn: &rusqlite::Connection, grant_key: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO permission_grants (grant_key, tool, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![grant_key, "fs_read", 1i64],
    )
    .unwrap();
}

fn has_grant(conn: &rusqlite::Connection, grant_key: &str) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM permission_grants WHERE grant_key = ?1",
            rusqlite::params![grant_key],
            |row| row.get(0),
        )
        .unwrap();
    count == 1
}

fn session_grant_keys(conn: &rusqlite::Connection, conversation_id: &str) -> Vec<String> {
    let prefix = format!("s:{conversation_id}|");
    let mut stmt = conn
        .prepare("SELECT grant_key FROM permission_grants WHERE instr(grant_key, ?1) = 1")
        .unwrap();
    stmt.query_map(rusqlite::params![prefix], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<String>, _>>()
        .unwrap()
}

#[test]
fn clear_session_grants_removes_only_that_conversation_and_keeps_global() {
    let conn = common::init_db();
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_read\"}");
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_write\"}");
    insert_grant(&conn, "s:c2|{\"tool\":\"fs_read\"}");
    insert_grant(&conn, "g|{\"tool\":\"fs_read\"}");

    let removed = workspaces::clear_session_grants(&conn, "c1").unwrap();
    assert_eq!(removed, 2, "只删该会话的两条会话级授权");
    assert!(
        session_grant_keys(&conn, "c1").is_empty(),
        "会话级授权全消失"
    );
    assert_eq!(
        session_grant_keys(&conn, "c2").len(),
        1,
        "其他会话的授权不受影响"
    );
    assert!(
        has_grant(&conn, "g|{\"tool\":\"fs_read\"}"),
        "全局授权是用户显式决策，不得删除"
    );
}

#[test]
fn clear_session_grants_respects_conversation_id_boundary() {
    let conn = common::init_db();
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_read\"}");
    insert_grant(&conn, "s:c10|{\"tool\":\"fs_read\"}");

    workspaces::clear_session_grants(&conn, "c1").unwrap();
    assert!(session_grant_keys(&conn, "c1").is_empty());
    assert_eq!(
        session_grant_keys(&conn, "c10").len(),
        1,
        "前缀必须带 `|` 边界，不能误删 c10"
    );
}

#[test]
fn clear_binding_also_clears_session_grants() {
    let conn = common::init_db();
    insert_conversation(&conn, &conv("c1")).unwrap();
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_read\"}");

    workspaces::set_conversation_workspace(&conn, "c1", workspaces::WORKSPACE_UNCONFIGURED)
        .unwrap();
    workspaces::clear_session_grants(&conn, "c1").unwrap();

    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1").unwrap(),
        Some(String::new()),
        "清除绑定写回空串哨兵"
    );
    assert!(
        session_grant_keys(&conn, "c1").is_empty(),
        "清除绑定同样清理会话级授权"
    );
}

#[test]
fn workspace_write_and_grant_clear_commit_together() {
    let conn = common::init_db();
    insert_conversation(&conn, &conv("c1")).unwrap();
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_read\"}");
    insert_grant(&conn, "g|{\"tool\":\"fs_read\"}");

    let tx = conn.unchecked_transaction().unwrap();
    workspaces::set_conversation_workspace(&tx, "c1", "/tmp/x").unwrap();
    workspaces::clear_session_grants(&tx, "c1").unwrap();
    tx.commit().unwrap();

    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1")
            .unwrap()
            .as_deref(),
        Some("/tmp/x")
    );
    assert!(session_grant_keys(&conn, "c1").is_empty());
    assert!(has_grant(&conn, "g|{\"tool\":\"fs_read\"}"));
}

#[test]
fn workspace_write_and_grant_clear_roll_back_together() {
    let conn = common::init_db();
    insert_conversation(&conn, &conv("c1")).unwrap();
    insert_grant(&conn, "s:c1|{\"tool\":\"fs_read\"}");

    let tx = conn.unchecked_transaction().unwrap();
    workspaces::set_conversation_workspace(&tx, "c1", "/tmp/x").unwrap();
    workspaces::clear_session_grants(&tx, "c1").unwrap();
    tx.rollback().unwrap();

    assert_eq!(
        workspaces::get_conversation_workspace(&conn, "c1").unwrap(),
        Some(String::new()),
        "回滚后绑定保持原值"
    );
    assert_eq!(
        session_grant_keys(&conn, "c1").len(),
        1,
        "回滚后授权仍在，不允许出现「边界已换、授权仍在」"
    );
}
