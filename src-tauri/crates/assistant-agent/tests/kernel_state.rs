use assistant_agent::kernel::{
    SessionState, SessionStates, StateError, TurnError, TurnOutcome, TurnPhase, TurnStatus,
};

fn turn_outcome(status: TurnStatus, error: Option<TurnError>) -> TurnOutcome {
    TurnOutcome {
        status,
        error,
        ..TurnOutcome::new()
    }
}

#[test]
fn unrecorded_conversation_can_start_and_reads_none() {
    let states = SessionStates::new();
    assert!(states.get("conv").is_none());
    assert!(states.can_start_new_turn("conv"));
}

#[test]
fn begin_turn_enters_processing_starting() {
    let states = SessionStates::new();
    states
        .begin_turn("conv", &"conv:1".to_string())
        .expect("begin");
    assert_eq!(
        states.get("conv"),
        Some(SessionState::Processing {
            turn_id: "conv:1".to_string(),
            phase: TurnPhase::Starting,
        })
    );
}

#[test]
fn ensure_idle_does_not_clobber_existing_state() {
    let states = SessionStates::new();
    states
        .begin_turn("conv", &"conv:1".to_string())
        .expect("begin");
    states.ensure_idle("conv");
    assert!(matches!(
        states.get("conv"),
        Some(SessionState::Processing { .. })
    ));

    states.ensure_idle("fresh");
    assert_eq!(states.get("fresh"), Some(SessionState::Idle));
}

#[test]
fn phase_walk_matches_migration_table() {
    let states = SessionStates::new();
    let turn = "conv:1".to_string();
    states.begin_turn("conv", &turn).expect("begin");

    for phase in [
        TurnPhase::Thinking,
        TurnPhase::Streaming,
        TurnPhase::ToolCalling,
        TurnPhase::AwaitingApproval,
        TurnPhase::ToolCalling,
        TurnPhase::Compacting,
        TurnPhase::Thinking,
        TurnPhase::Finalizing,
    ] {
        states.set_phase("conv", &turn, phase);
        assert_eq!(
            states.get("conv"),
            Some(SessionState::Processing {
                turn_id: turn.clone(),
                phase,
            }),
            "phase {} not applied",
            phase as u8
        );
    }

    states.finish_turn("conv", &turn, &turn_outcome(TurnStatus::Completed, None));
    assert_eq!(states.get("conv"), Some(SessionState::Idle));
}

#[test]
fn stale_turn_id_is_ignored_for_phase_and_finish() {
    let states = SessionStates::new();
    states
        .begin_turn("conv", &"conv:2".to_string())
        .expect("begin");

    states.set_phase("conv", &"conv:1".to_string(), TurnPhase::Streaming);
    assert_eq!(
        states.get("conv"),
        Some(SessionState::Processing {
            turn_id: "conv:2".to_string(),
            phase: TurnPhase::Starting,
        })
    );

    states.finish_turn(
        "conv",
        &"conv:1".to_string(),
        &turn_outcome(TurnStatus::Completed, None),
    );
    assert!(matches!(
        states.get("conv"),
        Some(SessionState::Processing { .. })
    ));
}

#[test]
fn processing_rejects_new_turn_but_recoverable_error_allows() {
    let states = SessionStates::new();
    states
        .begin_turn("conv", &"conv:1".to_string())
        .expect("begin");
    assert_eq!(
        states.begin_turn("conv", &"conv:2".to_string()),
        Err(StateError::Busy)
    );
    assert!(!states.can_start_new_turn("conv"));

    states.finish_turn(
        "conv",
        &"conv:1".to_string(),
        &turn_outcome(
            TurnStatus::Failed,
            Some(TurnError::Other("boom".to_string())),
        ),
    );
    assert!(states.can_start_new_turn("conv"));
    states
        .begin_turn("conv", &"conv:2".to_string())
        .expect("recoverable error allows a new turn");
}

#[test]
fn failed_turn_records_error_with_message_and_cancelled_returns_idle() {
    let states = SessionStates::new();
    let turn = "conv:1".to_string();

    states.begin_turn("conv", &turn).expect("begin");
    states.finish_turn(
        "conv",
        &turn,
        &turn_outcome(
            TurnStatus::Failed,
            Some(TurnError::MiddlewareStop {
                name: "hook".to_string(),
                reason: "stop".to_string(),
            }),
        ),
    );
    match states.get("conv") {
        Some(SessionState::Error {
            message,
            recoverable,
        }) => {
            assert!(message.contains("hook"), "message must carry the error");
            assert!(recoverable);
        }
        other => panic!("expected error state, got {other:?}"),
    }

    states
        .begin_turn("conv", &"conv:2".to_string())
        .expect("begin after error");
    states.finish_turn(
        "conv",
        &"conv:2".to_string(),
        &turn_outcome(TurnStatus::Cancelled, None),
    );
    assert_eq!(states.get("conv"), Some(SessionState::Idle));

    states
        .begin_turn("conv", &"conv:3".to_string())
        .expect("begin");
    states.finish_turn(
        "conv",
        &"conv:3".to_string(),
        &turn_outcome(TurnStatus::Interrupted, None),
    );
    assert_eq!(states.get("conv"), Some(SessionState::Idle));
}

#[test]
fn remove_restores_unrecorded_state() {
    let states = SessionStates::new();
    states
        .begin_turn("conv", &"conv:1".to_string())
        .expect("begin");
    states.remove("conv");
    assert!(states.get("conv").is_none());
    assert!(states.can_start_new_turn("conv"));
    states
        .begin_turn("conv", &"conv:2".to_string())
        .expect("begin after remove");
}
