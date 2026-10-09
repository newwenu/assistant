#![cfg(feature = "slow-tests")]

use assistant_agent::compaction::CompactionState;
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
