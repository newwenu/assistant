use assistant_storage::db;
use assistant_storage::tool_favorites;

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

#[test]
fn toggle_is_involution() {
    let conn = init_db();
    let r1 = tool_favorites::toggle_favorite(&conn, "a").unwrap();
    assert!(r1, "first toggle adds");
    let r2 = tool_favorites::toggle_favorite(&conn, "a").unwrap();
    assert!(!r2, "second toggle removes");
    assert!(tool_favorites::get_favorites(&conn).unwrap().is_empty());
    let r3 = tool_favorites::toggle_favorite(&conn, "a").unwrap();
    assert!(r3, "third toggle adds again");
}

#[test]
fn set_favorites_order_replaces_entire_set() {
    let conn = init_db();
    tool_favorites::toggle_favorite(&conn, "old-1").unwrap();
    tool_favorites::toggle_favorite(&conn, "old-2").unwrap();

    tool_favorites::set_favorites_order(&conn, &["c".into(), "a".into(), "b".into()]).unwrap();
    let favs = tool_favorites::get_favorites(&conn).unwrap();
    assert_eq!(
        favs,
        vec!["c", "a", "b"],
        "should completely replace, not merge"
    );
}

#[test]
fn set_favorites_order_rejects_duplicate_tool_ids() {
    let conn = init_db();
    let result = tool_favorites::set_favorites_order(&conn, &["x".into(), "x".into()]);
    assert!(
        result.is_err(),
        "duplicate tool_ids should fail due to PRIMARY KEY constraint"
    );
}

#[test]
fn set_favorites_order_should_preserve_old_data_on_failure() {
    let conn = init_db();
    tool_favorites::toggle_favorite(&conn, "keep-me").unwrap();
    assert_eq!(
        tool_favorites::get_favorites(&conn).unwrap(),
        vec!["keep-me"]
    );

    let result = tool_favorites::set_favorites_order(&conn, &["x".into(), "x".into()]);
    assert!(result.is_err());

    let favs = tool_favorites::get_favorites(&conn).unwrap();
    assert_eq!(
        favs, vec!["keep-me"],
        "on failure, old favorites should be preserved; current impl is non-atomic (DELETE then INSERT) so data is lost"
    );
}
