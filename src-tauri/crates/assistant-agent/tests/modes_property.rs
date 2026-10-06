#![cfg(feature = "slow-tests")]

use assistant_agent::modes::{AgentMode, ModeProfile};
use proptest::prelude::*;

proptest! {
    #[test]
    fn profile_fields_bounded(_mode in 0u8..3) {
        let mode = match _mode % 3 {
            0 => AgentMode::Quick,
            1 => AgentMode::Standard,
            _ => AgentMode::Deep,
        };
        let p = ModeProfile::from_mode(mode);
        assert!(p.temperature > 0.0 && p.temperature <= 1.0);
        assert!(p.max_tokens > 0);
        assert!(!p.system_prompt.is_empty());
    }
}
