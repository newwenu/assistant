use assistant_agent::compaction::CompactionState;

#[test]
fn new_initializes_defaults() {
    let s = CompactionState::new(2000);
    assert_eq!(s.total_tokens, 0);
    assert_eq!(s.max_tokens, 2000);
    assert_eq!(s.compaction_threshold_percent, 80);
}

#[test]
fn needs_compaction_threshold_boundary() {
    let mut s = CompactionState::new(1000);
    s.update_token_count(799);
    assert!(!s.needs_compaction());
    s.update_token_count(800);
    assert!(s.needs_compaction());
}

#[test]
fn update_token_count_replaces() {
    let mut s = CompactionState::new(1000);
    s.update_token_count(500);
    s.update_token_count(300);
    assert_eq!(s.total_tokens, 300);
}

#[test]
fn serde_roundtrip() {
    let mut s = CompactionState::new(4096);
    s.update_token_count(3000);
    s.messages_before_compaction = 10;
    s.messages_after_compaction = 3;
    let json = serde_json::to_string(&s).unwrap();
    let decoded: CompactionState = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.total_tokens, 3000);
    assert_eq!(decoded.max_tokens, 4096);
    assert_eq!(decoded.messages_before_compaction, 10);
}

#[test]
fn very_large_max_tokens() {
    let s = CompactionState::new(u32::MAX);
    assert!(!s.needs_compaction());
    let mut s2 = CompactionState::new(u32::MAX);
    s2.update_token_count(u32::MAX);
    assert!(s2.needs_compaction());
}

#[test]
fn max_tokens_zero_should_not_need_compaction() {
    let s = CompactionState::new(0);
    assert!(!s.needs_compaction());
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn needs_compaction_matches_threshold(max_tokens in 1u32..100_000, tokens in 0u32..100_000) {
            let mut s = CompactionState::new(max_tokens);
            s.update_token_count(tokens);
            let threshold = max_tokens as f32 * 0.8;
            assert_eq!(s.needs_compaction(), tokens as f32 >= threshold);
        }

        #[test]
        fn serde_preserves_semantics(max_tokens in 1u32..100_000, tokens in 0u32..100_000) {
            let mut s = CompactionState::new(max_tokens);
            s.update_token_count(tokens);
            let json = serde_json::to_string(&s).unwrap();
            let decoded: CompactionState = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded.needs_compaction(), s.needs_compaction());
            assert_eq!(decoded.total_tokens, s.total_tokens);
        }
    }
}
