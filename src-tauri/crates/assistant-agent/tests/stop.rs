use std::sync::Arc;
use std::time::Duration;

use assistant_agent::agent::stop::{
    stop_hooks_all_continue, IterationLimitHook, ToolCallLimitHook, TurnTokenBudgetHook,
    TurnWallClockHook,
};
use assistant_agent::kernel::{StopDecision, StopHook, TurnState};

fn state<'a>(
    iteration: u32,
    tool_calls: u32,
    prompt_tokens: u32,
    completion_tokens: u32,
    elapsed: Duration,
    turn_deadline: Option<Duration>,
    model: &'a str,
) -> TurnState<'a> {
    TurnState {
        iteration,
        max_iterations: 12,
        tool_calls,
        max_tool_calls: 96,
        prompt_tokens,
        completion_tokens,
        elapsed,
        turn_deadline,
        model,
    }
}

fn baseline() -> TurnState<'static> {
    state(
        1,
        0,
        100,
        20,
        Duration::from_secs(1),
        Some(Duration::from_secs(1800)),
        "custom",
    )
}

fn all_hooks() -> Vec<Arc<dyn StopHook>> {
    vec![
        Arc::new(IterationLimitHook),
        Arc::new(ToolCallLimitHook),
        Arc::new(TurnWallClockHook),
    ]
}

#[test]
fn all_hooks_continue_within_budget() {
    let hooks = all_hooks();
    let state = baseline();
    assert_eq!(stop_hooks_all_continue(&hooks, &state), None);
}

#[test]
fn iteration_limit_hook_stops_when_over() {
    let state = state(13, 0, 0, 0, Duration::ZERO, None, "custom");
    match IterationLimitHook.check(&state) {
        StopDecision::Stop { reason } => assert_eq!(reason, "iteration-limit"),
        StopDecision::Continue => panic!("iteration-limit should have stopped"),
    }
}

#[test]
fn iteration_limit_hook_continues_at_boundary() {
    let state = state(12, 0, 0, 0, Duration::ZERO, None, "custom");
    assert_eq!(IterationLimitHook.check(&state), StopDecision::Continue);
}

#[test]
fn tool_call_limit_hook_stops_when_reached() {
    let mut state = baseline();
    state.tool_calls = 96;
    match ToolCallLimitHook.check(&state) {
        StopDecision::Stop { reason } => assert_eq!(reason, "tool-call-limit"),
        StopDecision::Continue => panic!("tool-call-limit should have stopped"),
    }
}

#[test]
fn tool_call_limit_hook_ignores_zero_limit() {
    let mut state = baseline();
    state.tool_calls = 5;
    state.max_tool_calls = 0;
    assert_eq!(ToolCallLimitHook.check(&state), StopDecision::Continue);
}

#[test]
fn wall_clock_hook_stops_when_elapsed_reaches_limit() {
    let state = state(
        1,
        0,
        0,
        0,
        Duration::from_secs(1800),
        Some(Duration::from_secs(1800)),
        "custom",
    );
    match TurnWallClockHook.check(&state) {
        StopDecision::Stop { reason } => assert_eq!(reason, "turn-wall-clock"),
        StopDecision::Continue => panic!("turn-wall-clock should have stopped"),
    }
}

#[test]
fn wall_clock_hook_ignores_unset_limit() {
    let unset = state(1, 0, 0, 0, Duration::from_secs(1800), None, "custom");
    assert_eq!(TurnWallClockHook.check(&unset), StopDecision::Continue);

    let zero_limit = state(
        1,
        0,
        0,
        0,
        Duration::from_secs(1800),
        Some(Duration::ZERO),
        "custom",
    );
    assert_eq!(TurnWallClockHook.check(&zero_limit), StopDecision::Continue);
}

#[test]
fn token_budget_hook_is_additive() {
    let hook = TurnTokenBudgetHook::new(1000);
    let mut state = baseline();
    state.prompt_tokens = 900;
    state.completion_tokens = 100;
    match hook.check(&state) {
        StopDecision::Stop { reason } => assert_eq!(reason, "token-budget"),
        StopDecision::Continue => panic!("token-budget should have stopped"),
    }

    let mut state = baseline();
    state.prompt_tokens = 400;
    state.completion_tokens = 50;
    assert_eq!(hook.check(&state), StopDecision::Continue);
}

#[test]
fn each_dimension_triggers_independently() {
    let hooks = all_hooks();

    let mut over_iterations = baseline();
    over_iterations.iteration = 13;
    let hit = stop_hooks_all_continue(&hooks, &over_iterations).expect("iteration should stop");
    assert_eq!(hit.0, "iteration-limit");

    let mut over_tools = baseline();
    over_tools.tool_calls = 96;
    let hit = stop_hooks_all_continue(&hooks, &over_tools).expect("tool calls should stop");
    assert_eq!(hit.0, "tool-call-limit");

    let mut over_wall_clock = baseline();
    over_wall_clock.elapsed = Duration::from_secs(1800);
    let hit = stop_hooks_all_continue(&hooks, &over_wall_clock).expect("wall clock should stop");
    assert_eq!(hit.0, "turn-wall-clock");
}

#[test]
fn first_stop_short_circuits_in_registration_order() {
    let hooks = all_hooks();
    let mut state = baseline();
    state.iteration = 13;
    state.tool_calls = 96;
    state.elapsed = Duration::from_secs(1800);
    let hit = stop_hooks_all_continue(&hooks, &state).expect("should stop");
    assert_eq!(hit.0, "iteration-limit");
}

#[test]
fn empty_hook_list_always_continues() {
    let hooks: Vec<Arc<dyn StopHook>> = Vec::new();
    assert_eq!(stop_hooks_all_continue(&hooks, &baseline()), None);
}
