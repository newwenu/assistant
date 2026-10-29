mod common;

use assistant_storage::tool_history;
use common::init_db;

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
fn cleanup_keeps_only_10_most_recent() {
    let conn = init_db();
    for i in 0..15 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        tool_history::record_usage(&conn, &format!("tool-{}", i)).unwrap();
    }

    let removed = tool_history::cleanup_old_history(&conn).unwrap();
    assert_eq!(removed, 5, "should remove 5 oldest entries beyond 10");

    let recent = tool_history::get_recent_tools(&conn, 20).unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0], "tool-14", "most recent should survive");
    assert_eq!(recent[9], "tool-5", "10th most recent should survive");
}

#[test]
fn cleanup_noop_when_at_or_below_capacity() {
    let conn = init_db();
    for i in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        tool_history::record_usage(&conn, &format!("tool-{}", i)).unwrap();
    }

    let removed = tool_history::cleanup_old_history(&conn).unwrap();
    assert_eq!(removed, 0, "at capacity, nothing should be removed");

    let recent = tool_history::get_recent_tools(&conn, 20).unwrap();
    assert_eq!(recent.len(), 10);
}
