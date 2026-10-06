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
    assert_eq!(msg_count, 0, "CASCADE should delete messages when conversation is deleted");
}