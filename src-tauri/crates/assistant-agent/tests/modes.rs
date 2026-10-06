use assistant_agent::modes::{AgentMode, ModeProfile};

#[test]
fn presentation_rank_order() {
    assert!(AgentMode::Quick.presentation_rank() < AgentMode::Standard.presentation_rank());
    assert!(AgentMode::Standard.presentation_rank() < AgentMode::Deep.presentation_rank());
}

#[test]
fn profile_from_mode_matches_all_fields() {
    for mode in [AgentMode::Quick, AgentMode::Standard, AgentMode::Deep] {
        let p = ModeProfile::from_mode(mode);
        assert_eq!(p.mode, mode);
        assert_eq!(p.system_prompt, mode.system_prompt());
        assert!((p.temperature - mode.default_temperature()).abs() < f32::EPSILON);
        assert_eq!(p.max_tokens, mode.default_max_tokens());
    }
}

#[test]
fn depth_ordering() {
    assert!(AgentMode::Quick.default_temperature() < AgentMode::Standard.default_temperature());
    assert!(AgentMode::Standard.default_temperature() < AgentMode::Deep.default_temperature());
    assert!(AgentMode::Quick.default_max_tokens() < AgentMode::Standard.default_max_tokens());
    assert!(AgentMode::Quick.system_prompt().len() < AgentMode::Deep.system_prompt().len());
}

#[test]
fn serde_roundtrip() {
    for mode in [AgentMode::Quick, AgentMode::Standard, AgentMode::Deep] {
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: AgentMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, decoded);
    }
}

#[test]
fn serde_unknown_mode_fails() {
    assert!(serde_json::from_str::<AgentMode>(r#"{"Quick":1}"#).is_err());
}
