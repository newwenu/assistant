use assistant_storage::db;
use assistant_storage::tool_history;

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

#[test]
fn record_usage_deduplicates_by_tool_id() {
    let conn = init_db();
    tool_history::record_usage(&conn, "tool-a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    tool_history::record_usage(&conn, "tool-a").unwrap();

    let recent = tool_history::get_recent_tools(&conn, 10).unwrap();
    assert_eq!(
        recent.len(),
        1,
        "INSERT OR REPLACE should not duplicate tool_id"
    );
}

#[test]
fn get_recent_ordered_by_used_at_desc() {
    let conn = init_db();
    for i in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        tool_history::record_usage(&conn, &format!("tool-{}", i)).unwrap();
    }

    let top3 = tool_history::get_recent_tools(&conn, 3).unwrap();
    assert_eq!(top3.len(), 3);
    assert_eq!(top3[0], "tool-4", "most recently used should come first");
}

#[test]
fn cleanup_removes_only_entries_older_than_90_days() {
    let conn = init_db();
    let ninety_one_days_ago = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(91))
        .unwrap()
        .timestamp_millis();
    conn.execute(
        "INSERT INTO tool_history (tool_id, used_at) VALUES ('old-tool', ?1)",
        rusqlite::params![ninety_one_days_ago],
    )
    .unwrap();

    tool_history::record_usage(&conn, "new-tool").unwrap();

    let removed = tool_history::cleanup_old_history(&conn).unwrap();
    assert_eq!(removed, 1);

    let recent = tool_history::get_recent_tools(&conn, 10).unwrap();
    assert_eq!(recent, vec!["new-tool"]);
}

#[test]
fn cleanup_keeps_entries_within_90_days() {
    let conn = init_db();
    let eighty_nine_days_ago = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(89))
        .unwrap()
        .timestamp_millis();
    conn.execute(
        "INSERT INTO tool_history (tool_id, used_at) VALUES ('recent', ?1)",
        rusqlite::params![eighty_nine_days_ago],
    )
    .unwrap();

    let removed = tool_history::cleanup_old_history(&conn).unwrap();
    assert_eq!(
        removed, 0,
        "entries within 90 days should NOT be cleaned up"
    );
}
