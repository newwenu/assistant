use assistant_agent::compaction::should_terminate;

#[test]
fn should_terminate_below_threshold() {
    assert!(!should_terminate(799, 1000, true, 80));
}

#[test]
fn should_terminate_at_threshold() {
    assert!(should_terminate(800, 1000, true, 80));
}

#[test]
fn should_terminate_auto_compact_disabled() {
    assert!(!should_terminate(900, 1000, false, 80));
}

#[test]
fn should_terminate_zero_tokens() {
    assert!(!should_terminate(0, 1000, true, 80));
}

#[test]
fn should_terminate_zero_context_window() {
    assert!(!should_terminate(500, 0, true, 80));
}

#[test]
fn should_terminate_exact_boundary() {
    assert!(should_terminate(800, 1000, true, 80));
    assert!(!should_terminate(799, 1000, true, 80));
}
