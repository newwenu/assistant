use assistant_agent::kernel::persist::build_usage_records;
use assistant_agent::kernel::{PersistCtx, UsageSnapshot};

fn snapshot(prompt: Option<u32>, completion: Option<u32>, total: Option<u32>) -> UsageSnapshot {
    UsageSnapshot {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: total,
        ..Default::default()
    }
}

fn ctx(kind: &str) -> PersistCtx {
    PersistCtx {
        conversation_id: "c1".to_string(),
        turn_id: "c1:1".to_string(),
        message_id: "m1".to_string(),
        branch_leaf_id: Some("b1".to_string()),
        kind: kind.to_string(),
        provider_id: Some("p1".to_string()),
        model_id: Some("gpt-4".to_string()),
        cache_capable: true,
        created_at_ms: 1_700_000_000_000,
    }
}

#[test]
fn reported_prompt_keeps_provider_flag() {
    let (prompt, completion, estimated) = snapshot(Some(100), Some(20), Some(120)).record_tokens();
    assert_eq!((prompt, completion), (100, 20));
    assert!(!estimated);
}

#[test]
fn derived_prompt_is_marked_estimated() {
    let (prompt, completion, estimated) = snapshot(None, Some(20), Some(120)).record_tokens();
    assert_eq!((prompt, completion), (100, 20));
    assert!(estimated);
}

#[test]
fn missing_prompt_and_total_is_zero_and_estimated() {
    let (prompt, completion, estimated) = snapshot(None, None, None).record_tokens();
    assert_eq!((prompt, completion), (0, 0));
    assert!(estimated);
}

#[test]
fn derived_prompt_never_goes_negative() {
    let (prompt, _, estimated) = snapshot(None, Some(200), Some(120)).record_tokens();
    assert_eq!(prompt, 0);
    assert!(estimated);
}

#[test]
fn every_attempt_produces_its_own_record() {
    let snapshots = vec![
        snapshot(Some(100), Some(10), Some(110)),
        snapshot(Some(120), Some(20), Some(140)),
    ];
    let records = build_usage_records(&snapshots, &ctx("chat"));
    assert_eq!(
        records.len(),
        2,
        "retry must not overwrite the first attempt"
    );
    assert_eq!(records[0].prompt_tokens, 100);
    assert_eq!(records[1].prompt_tokens, 120);
    assert_eq!(records[0].attempt_id.as_deref(), Some("c1:1"));
}

#[test]
fn compaction_produces_single_record_with_kind() {
    let snapshots = vec![snapshot(Some(800), Some(40), Some(840))];
    let records = build_usage_records(&snapshots, &ctx("compaction"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, "compaction");
    assert_eq!(records[0].prompt_tokens, 800);
    assert_eq!(records[0].completion_tokens, 40);
}

#[test]
fn record_carries_all_snapshot_fields() {
    let snapshot = UsageSnapshot {
        prompt_tokens: Some(100),
        completion_tokens: Some(20),
        total_tokens: Some(130),
        cache_read_tokens: Some(60),
        cache_write_tokens: Some(10),
        reasoning_tokens: Some(5),
        estimated: false,
        context_window_tokens: Some(128_000),
    };
    let records = build_usage_records(&[snapshot], &ctx("chat"));
    let r = &records[0];
    assert_eq!(r.conversation_id, "c1");
    assert_eq!(r.message_id.as_deref(), Some("m1"));
    assert_eq!(r.branch_leaf_id.as_deref(), Some("b1"));
    assert_eq!(r.provider_id.as_deref(), Some("p1"));
    assert_eq!(r.model_id.as_deref(), Some("gpt-4"));
    assert_eq!(r.cache_read_tokens, Some(60));
    assert_eq!(r.cache_write_tokens, Some(10));
    assert_eq!(r.reasoning_tokens, Some(5));
    assert_eq!(r.context_window_tokens, Some(128_000));
    assert!(r.cache_capable);
    assert!(!r.estimated);
}

#[test]
fn record_uses_injected_timestamp() {
    let records = build_usage_records(&[snapshot(Some(10), Some(2), Some(12))], &ctx("chat"));
    assert_eq!(records[0].created_at, 1_700_000_000_000);
}
