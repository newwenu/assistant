mod common;

use assistant_storage::token_usage::{
    get_cache_stats, get_cumulative, get_occupancy, get_summary, insert_record, TokenUsageRecord,
};
use common::init_db;
use rusqlite::Connection;

fn chat_record(conv_id: &str, prompt: u32, ts: i64) -> TokenUsageRecord {
    TokenUsageRecord {
        conversation_id: conv_id.to_string(),
        kind: "chat".to_string(),
        prompt_tokens: prompt,
        completion_tokens: 0,
        created_at: ts,
        ..Default::default()
    }
}

fn insert(conn: &Connection, r: &TokenUsageRecord) {
    insert_record(conn, r).unwrap();
}

#[test]
fn occupancy_follows_branch() {
    let conn = init_db();

    let mut a = chat_record("c1", 1_000, 10);
    a.branch_leaf_id = Some("b1".to_string());
    insert(&conn, &a);

    let mut b = chat_record("c1", 2_000, 20);
    b.branch_leaf_id = Some("b1".to_string());
    insert(&conn, &b);

    let mut other = chat_record("c1", 3_000, 30);
    other.branch_leaf_id = Some("b2".to_string());
    insert(&conn, &other);

    let b1 = get_occupancy(&conn, "c1", Some("b1")).unwrap().unwrap();
    assert_eq!(b1.prompt_tokens, 2_000);

    let b2 = get_occupancy(&conn, "c1", Some("b2")).unwrap().unwrap();
    assert_eq!(b2.prompt_tokens, 3_000);

    assert!(get_occupancy(&conn, "c1", Some("b3")).unwrap().is_none());
}

#[test]
fn occupancy_ignores_other_conversations() {
    let conn = init_db();
    insert(&conn, &chat_record("c1", 100, 10));
    insert(&conn, &chat_record("c2", 900, 20));

    let row = get_occupancy(&conn, "c1", None).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 100);
}

#[test]
fn same_millisecond_breaks_tie_by_id() {
    let conn = init_db();
    insert(&conn, &chat_record("c1", 1_000, 10));
    insert(&conn, &chat_record("c1", 2_000, 10));

    let row = get_occupancy(&conn, "c1", None).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 2_000, "later insert wins on equal ts");
}

#[test]
fn estimated_record_does_not_override_reported() {
    let conn = init_db();

    insert(&conn, &chat_record("c1", 1_000, 10));

    let mut estimated = chat_record("c1", 9_000, 20);
    estimated.estimated = true;
    insert(&conn, &estimated);

    let row = get_occupancy(&conn, "c1", None).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 1_000);
    assert!(!row.estimated);
}

#[test]
fn estimated_record_is_fallback_when_branch_has_none_reported() {
    let conn = init_db();

    let mut estimated = chat_record("c1", 9_000, 20);
    estimated.estimated = true;
    estimated.branch_leaf_id = Some("b1".to_string());
    insert(&conn, &estimated);

    let mut reported_elsewhere = chat_record("c1", 1_000, 10);
    reported_elsewhere.branch_leaf_id = Some("b2".to_string());
    insert(&conn, &reported_elsewhere);

    let row = get_occupancy(&conn, "c1", Some("b1")).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 9_000);
    assert!(row.estimated);
}

#[test]
fn compaction_record_excluded_from_occupancy_but_counted_in_cumulative() {
    let conn = init_db();

    let mut chat = chat_record("c1", 100, 10);
    chat.completion_tokens = 50;
    insert(&conn, &chat);

    let mut compaction = chat_record("c1", 500, 20);
    compaction.kind = "compaction".to_string();
    insert(&conn, &compaction);

    let row = get_occupancy(&conn, "c1", None).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 100);

    assert_eq!(get_cumulative(&conn, "c1").unwrap(), Some(650));
}

#[test]
fn retry_attempts_accumulate_in_cumulative() {
    let conn = init_db();

    let mut first = chat_record("c1", 200, 10);
    first.message_id = Some("m1".to_string());
    first.attempt_id = Some("c1:0".to_string());
    first.completion_tokens = 10;
    insert(&conn, &first);

    let mut retry = chat_record("c1", 300, 20);
    retry.message_id = Some("m1".to_string());
    retry.attempt_id = Some("c1:1".to_string());
    retry.completion_tokens = 20;
    insert(&conn, &retry);

    assert_eq!(get_cumulative(&conn, "c1").unwrap(), Some(530));

    let row = get_occupancy(&conn, "c1", None).unwrap().unwrap();
    assert_eq!(row.prompt_tokens, 300);
}

#[test]
fn cache_denominator_requires_capable_and_reported_field() {
    let conn = init_db();

    let mut capable = chat_record("c1", 100, 10);
    capable.cache_capable = true;
    capable.cache_read_tokens = Some(50);
    insert(&conn, &capable);

    let mut incapable = chat_record("c1", 200, 20);
    incapable.cache_capable = false;
    incapable.cache_read_tokens = Some(80);
    insert(&conn, &incapable);

    let mut no_fields = chat_record("c1", 300, 30);
    no_fields.cache_capable = true;
    insert(&conn, &no_fields);

    let stats = get_cache_stats(&conn, "c1").unwrap().unwrap();
    assert_eq!(stats.hit_tokens, 50);
    assert_eq!(stats.denominator_tokens, 100);
    assert_eq!(stats.hit_rate(), Some(0.5));

    assert!(get_cache_stats(&conn, "c2").unwrap().is_none());
}

#[test]
fn summary_without_records_has_no_occupancy() {
    let conn = init_db();
    let summary = get_summary(&conn, "c1", None, Some(128_000)).unwrap();

    assert!(summary.occupancy.used_tokens.is_none());
    assert!(summary.occupancy.percentage.is_none());
    assert_eq!(summary.occupancy.context_window_tokens, Some(128_000));
    assert!(summary.cumulative.is_none());
}

#[test]
fn summary_unknown_window_reports_absolute_only() {
    let conn = init_db();
    insert(&conn, &chat_record("c1", 1_000, 10));

    let summary = get_summary(&conn, "c1", None, None).unwrap();
    assert_eq!(summary.occupancy.used_tokens, Some(1_000));
    assert!(summary.occupancy.context_window_tokens.is_none());
    assert!(summary.occupancy.percentage.is_none());
}

#[test]
fn summary_uses_record_window_and_computes_percentage() {
    let conn = init_db();

    let mut r = chat_record("c1", 64_000, 10);
    r.context_window_tokens = Some(128_000);
    insert(&conn, &r);

    let summary = get_summary(&conn, "c1", None, Some(64_000)).unwrap();
    assert_eq!(summary.occupancy.context_window_tokens, Some(128_000));
    assert_eq!(summary.occupancy.percentage, Some(0.5));
}

#[test]
fn ten_round_scenario_keeps_occupancy_and_cumulative_apart() {
    let conn = init_db();

    const S: u32 = 2_000;
    const D: u32 = 1_000;
    const C: u32 = 1_000;
    for round in 0..10u32 {
        let mut r = chat_record("c1", S + round * (D + C), 100 + round as i64);
        r.completion_tokens = C;
        insert(&conn, &r);
    }

    let summary = get_summary(&conn, "c1", None, Some(128_000)).unwrap();
    assert_eq!(summary.occupancy.used_tokens, Some(20_000));
    assert_eq!(summary.occupancy.percentage, Some(20_000.0 / 128_000.0));
    assert_eq!(summary.cumulative.map(|c| c.total_tokens), Some(120_000));
}
