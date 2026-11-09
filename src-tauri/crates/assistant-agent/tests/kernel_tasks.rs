use assistant_agent::kernel::{Busy, OnBusy, TurnTasks};

#[test]
fn replace_cancels_previous_turn_and_bumps_counter() {
    let tasks = TurnTasks::new();
    let (first_id, first_token) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    assert_eq!(first_id, "conv:1");
    assert!(!first_token.is_cancelled());

    let (second_id, second_token) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    assert_eq!(second_id, "conv:2");
    assert!(
        first_token.is_cancelled(),
        "replace must cancel the old token"
    );
    assert!(!second_token.is_cancelled());
    assert_eq!(tasks.current("conv").as_deref(), Some("conv:2"));
}

#[test]
fn reject_returns_busy_and_keeps_old_turn_running() {
    let tasks = TurnTasks::new();
    let (first_id, first_token) = tasks.begin("conv", OnBusy::Reject).expect("begin");

    assert_eq!(tasks.begin("conv", OnBusy::Reject), Err(Busy));
    assert!(
        !first_token.is_cancelled(),
        "rejected begin must not cancel"
    );
    assert_eq!(tasks.current("conv"), Some(first_id));

    tasks.cancel("conv");
    assert!(first_token.is_cancelled());
}

#[test]
fn cancel_only_flags_token_and_reports_presence() {
    let tasks = TurnTasks::new();
    assert!(!tasks.cancel("missing"));
    let (_, token) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    assert!(tasks.cancel("conv"));
    assert!(token.is_cancelled());
    assert_eq!(
        tasks.current("conv").as_deref(),
        Some("conv:1"),
        "cancel keeps the entry so the turn can still persist its terminal state"
    );
}

#[test]
fn finish_with_stale_turn_id_keeps_new_generation() {
    let tasks = TurnTasks::new();
    let (old_id, _) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    let (new_id, new_token) = tasks.begin("conv", OnBusy::Replace).expect("begin");

    tasks.finish("conv", &old_id);
    assert_eq!(tasks.current("conv"), Some(new_id.clone()));
    assert!(!new_token.is_cancelled());

    tasks.finish("conv", &new_id);
    assert_eq!(tasks.current("conv"), None);
}

#[test]
fn finish_on_unknown_conversation_is_noop() {
    let tasks = TurnTasks::new();
    tasks.finish("missing", &"missing:1".to_string());
    assert_eq!(tasks.current("missing"), None);
}

#[test]
fn counter_only_grows_and_ids_are_conversation_scoped() {
    let tasks = TurnTasks::new();
    let mut last = String::new();
    for expected in 1..=5 {
        let (id, _) = tasks.begin("conv", OnBusy::Replace).expect("begin");
        assert_eq!(id, format!("conv:{expected}"));
        last = id;
    }
    assert_eq!(tasks.current("conv"), Some(last));

    let (other, _) = tasks.begin("other", OnBusy::Replace).expect("begin");
    assert_eq!(other, "other:1");
    assert_eq!(tasks.current("conv").as_deref(), Some("conv:5"));
}

#[test]
fn child_token_is_cancelled_with_parent() {
    let tasks = TurnTasks::new();
    let (_, token) = tasks.begin("conv", OnBusy::Replace).expect("begin");

    let model = TurnTasks::child_token(&token);
    let tool = TurnTasks::child_token(&token);
    let approval = TurnTasks::child_token(&token);

    tasks.cancel("conv");
    assert!(model.is_cancelled());
    assert!(tool.is_cancelled());
    assert!(approval.is_cancelled());
}

#[test]
fn child_token_can_be_cancelled_without_killing_parent() {
    let tasks = TurnTasks::new();
    let (_, token) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    let model = TurnTasks::child_token(&token);

    model.cancel();
    assert!(model.is_cancelled());
    assert!(
        !token.is_cancelled(),
        "child cancel must not cancel the turn"
    );
}

#[test]
fn generation_guard_matches_current_turn() {
    let tasks = TurnTasks::new();
    let (turn, _) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    assert!(tasks.is_current("conv", &turn));
    assert!(!tasks.is_current("conv", &"conv:2".to_string()));
    assert!(!tasks.is_current("missing", &turn));
}

#[test]
fn is_busy_ignores_cancelled_generation() {
    let tasks = TurnTasks::new();
    assert!(!tasks.is_busy("conv"));
    let (_, token) = tasks.begin("conv", OnBusy::Reject).expect("begin");
    assert!(tasks.is_busy("conv"));

    tasks.cancel("conv");
    assert!(token.is_cancelled());
    assert!(
        !tasks.is_busy("conv"),
        "cancel must release mutual exclusion before finish"
    );
    assert_eq!(tasks.current("conv").as_deref(), Some("conv:1"));
}

#[test]
fn reject_after_cancel_replaces_stale_generation() {
    let tasks = TurnTasks::new();
    let (old_id, old_token) = tasks.begin("conv", OnBusy::Reject).expect("begin");
    tasks.cancel("conv");
    assert!(old_token.is_cancelled());

    let (new_id, new_token) = tasks.begin("conv", OnBusy::Reject).expect("begin");
    assert_eq!(new_id, "conv:2");
    assert!(!new_token.is_cancelled());
    assert_eq!(tasks.current("conv"), Some(new_id));
    assert!(
        !tasks.is_current("conv", &old_id),
        "stale generation must lose the generation guard"
    );
}

#[tokio::test]
async fn cancelled_token_completes_select_branch() {
    let tasks = TurnTasks::new();
    let (_, token) = tasks.begin("conv", OnBusy::Replace).expect("begin");
    let child = TurnTasks::child_token(&token);
    tasks.cancel("conv");

    let winner = tokio::select! {
        biased;
        _ = child.cancelled() => "cancelled",
        _ = std::future::pending::<()>() => "elapsed",
    };
    assert_eq!(winner, "cancelled");
}
