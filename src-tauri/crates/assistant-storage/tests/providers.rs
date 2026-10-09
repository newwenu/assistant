use assistant_storage::db;
use assistant_storage::providers::{self, ProviderRow};

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

fn provider(id: &str, ts: i64) -> ProviderRow {
    ProviderRow {
        id: id.to_string(),
        name: format!("provider-{}", id),
        type_: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        api_key_encrypted: String::new(),
        models_json: "[]".to_string(),
        default_model: "gpt-4".to_string(),
        updated_at: ts,
    }
}

#[test]
fn delete_provider_nullifies_all_referencing_conversations() {
    let conn = init_db();
    providers::insert_provider(&conn, &provider("p1", 0)).unwrap();
    conn.execute(
        "INSERT INTO conversations (id,title,title_source,provider_id,model_id,created_at,updated_at)
         VALUES ('c1','t','placeholder','p1','m',0,0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversations (id,title,title_source,provider_id,model_id,created_at,updated_at)
         VALUES ('c2','t','placeholder','p1','m',0,0)",
        [],
    )
    .unwrap();

    providers::delete_provider(&conn, "p1").unwrap();

    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE provider_id IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        null_count, 2,
        "both conversations should have provider_id = NULL"
    );
    assert!(providers::get_provider_by_id(&conn, "p1")
        .unwrap()
        .is_none());
}

#[test]
fn get_providers_ordered_by_updated_at_desc() {
    let conn = init_db();
    providers::insert_provider(&conn, &provider("p1", 100)).unwrap();
    providers::insert_provider(&conn, &provider("p2", 300)).unwrap();
    providers::insert_provider(&conn, &provider("p3", 200)).unwrap();

    let all = providers::get_providers(&conn).unwrap();
    let ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["p2", "p3", "p1"],
        "should be sorted by updated_at DESC"
    );
}

#[test]
fn update_provider_preserves_id_changes_other_fields() {
    let conn = init_db();
    providers::insert_provider(&conn, &provider("p1", 100)).unwrap();

    let mut p = providers::get_provider_by_id(&conn, "p1").unwrap().unwrap();
    p.name = "renamed".to_string();
    p.type_ = "anthropic".to_string();
    p.updated_at = 999;
    providers::update_provider(&conn, &p).unwrap();

    let after = providers::get_provider_by_id(&conn, "p1").unwrap().unwrap();
    assert_eq!(after.name, "renamed");
    assert_eq!(after.type_, "anthropic");
    assert_eq!(after.updated_at, 999);
}

#[test]
fn duplicate_provider_id_rejects() {
    let conn = init_db();
    providers::insert_provider(&conn, &provider("p1", 0)).unwrap();
    assert!(providers::insert_provider(&conn, &provider("p1", 1)).is_err());
}
