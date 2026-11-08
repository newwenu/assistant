use assistant_agent::compaction::{evaluate_compaction, CompactDecision, COMPACTION_SAFETY_TOKENS};

// W=10000, reserved=0 → budget = 10000 - 0 - 1000 = 9000；80% → 7200
#[test]
fn below_threshold() {
    assert_eq!(
        evaluate_compaction(7199, Some(10_000), true, 80, 0),
        CompactDecision::NotNeeded
    );
}

#[test]
fn at_threshold() {
    assert_eq!(
        evaluate_compaction(7200, Some(10_000), true, 80, 0),
        CompactDecision::Terminate
    );
}

#[test]
fn denominator_deducts_output_reserve() {
    // W=10000, reserved=2000 → budget = 7000；80% → 5600
    // 旧口径（分母为完整窗口）阈值 8000，新口径阈值 5600
    assert_eq!(
        evaluate_compaction(5600, Some(10_000), true, 80, 2000),
        CompactDecision::Terminate
    );
    assert_eq!(
        evaluate_compaction(5599, Some(10_000), true, 80, 2000),
        CompactDecision::NotNeeded
    );
}

#[test]
fn auto_compact_disabled() {
    assert_eq!(
        evaluate_compaction(9000, Some(10_000), false, 80, 0),
        CompactDecision::NotNeeded
    );
}

#[test]
fn zero_prompt_tokens() {
    assert_eq!(
        evaluate_compaction(0, Some(10_000), true, 80, 0),
        CompactDecision::NotNeeded
    );
}

#[test]
fn unknown_window_is_undetermined() {
    // 窗口未知 → 不可判定，不得当成「不超限」静默跳过
    assert_eq!(
        evaluate_compaction(500, None, true, 80, 0),
        CompactDecision::Undetermined
    );
    assert_eq!(
        evaluate_compaction(500, Some(0), true, 80, 0),
        CompactDecision::Undetermined
    );
}

#[test]
fn reserve_consumes_entire_window() {
    // 预留 + 安全裕度 ≥ 窗口 → 没有任何输入预算，直接终止
    assert_eq!(
        evaluate_compaction(1, Some(2_000), true, 80, 2_000),
        CompactDecision::Terminate
    );
    assert_eq!(
        evaluate_compaction(1, Some(COMPACTION_SAFETY_TOKENS), true, 80, 0),
        CompactDecision::Terminate
    );
}
