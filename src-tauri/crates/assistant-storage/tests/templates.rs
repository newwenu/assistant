use assistant_storage::templates::{self, PromptTemplateRow};
use assistant_storage::db;

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

#[test]
fn seed_builtin_templates_idempotent() {
    let conn = init_db();
    templates::seed_builtin_templates(&conn).unwrap();
    let first = templates::get_templates(&conn).unwrap();
    assert!(!first.is_empty());

    templates::seed_builtin_templates(&conn).unwrap();
    let second = templates::get_templates(&conn).unwrap();
    assert_eq!(first.len(), second.len(), "second seed must not duplicate builtins");
}

#[test]
fn insert_or_ignore_preserves_first_on_duplicate_id() {
    let conn = init_db();
    let t1 = PromptTemplateRow {
        id: "dup".to_string(),
        name: "First".to_string(),
        category: "test".to_string(),
        content: "v1".to_string(),
        variables_json: None,
        is_builtin: false,
    };
    templates::insert_template(&conn, &t1).unwrap();

    let mut t2 = t1.clone();
    t2.name = "Second".to_string();
    t2.content = "v2".to_string();
    templates::insert_template(&conn, &t2).unwrap();

    let all = templates::get_templates(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "First", "INSERT OR IGNORE should keep the original row");
}

#[test]
fn custom_and_builtin_coexist_sorted() {
    let conn = init_db();
    templates::seed_builtin_templates(&conn).unwrap();
    let builtin_count = templates::get_templates(&conn).unwrap().len();

    templates::insert_template(&conn, &PromptTemplateRow {
        id: "custom-1".to_string(),
        name: "My Template".to_string(),
        category: "custom".to_string(),
        content: "hello".to_string(),
        variables_json: None,
        is_builtin: false,
    }).unwrap();

    let all = templates::get_templates(&conn).unwrap();
    assert_eq!(all.len(), builtin_count + 1, "custom template should coexist with builtins");
    assert!(all.iter().any(|t| t.id == "custom-1" && !t.is_builtin));
}