#![cfg(feature = "test-doubles")]

mod common;

use std::sync::Arc;

use assistant_agent::agent::node_event::{AgentNodeEvent, ROOT_AGENT_ID};
use assistant_agent::agent::tool_event::ToolEvent;
use assistant_agent::kernel::ports::test_doubles::{RecordingSink, ScriptedResponse};
use assistant_agent::kernel::{OnBusy, TurnEmitter, TurnEvent, TurnSpec, TurnTasks};
use common::harness::started_ctx;
use common::runner::{scripted_harness, Harness};

fn harness(scripts: Vec<ScriptedResponse>) -> Harness<RecordingSink> {
    scripted_harness(scripts)
}

/// `UserMessageConfirmed` 是会话级事件，其余事件都是轮级事件。
fn event_turn_id(event: &TurnEvent) -> Option<&str> {
    match event {
        TurnEvent::StreamStart { turn_id, .. }
        | TurnEvent::Delta { turn_id, .. }
        | TurnEvent::PhaseChanged { turn_id, .. }
        | TurnEvent::Tool(ToolEvent { turn_id, .. })
        | TurnEvent::Node(AgentNodeEvent { turn_id, .. })
        | TurnEvent::CompactionStarted { turn_id }
        | TurnEvent::CompactionFinished { turn_id, .. }
        | TurnEvent::Terminated { turn_id, .. } => Some(turn_id),
        TurnEvent::UserMessageConfirmed { .. } => None,
    }
}

fn delta_seqs(events: &[TurnEvent]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::Delta { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn every_turn_scoped_event_carries_the_turn_id() {
    let h = harness(vec![ScriptedResponse::text(&["Hello", " world"])]);
    let ctx = started_ctx(&h.tasks, "conv-1");

    h.runner.run(&ctx, &TurnSpec::chat()).await;

    let events = h.sink.events();
    assert!(!events.is_empty(), "turn must emit at least one event");
    for event in &events {
        if let Some(turn_id) = event_turn_id(event) {
            assert_eq!(
                turn_id, ctx.turn_id,
                "turn-scoped event must carry the current turn id: {event:?}"
            );
        }
    }
}

#[tokio::test]
async fn emitter_user_message_confirmed_is_conversation_scoped() {
    let sink = Arc::new(RecordingSink::new());
    let tasks = Arc::new(TurnTasks::new());
    let (turn_id, _token) = tasks.begin("conv-1", OnBusy::Replace).expect("begin");
    let emitter = TurnEmitter::new(
        sink.clone(),
        tasks.clone(),
        "conv-1",
        turn_id,
        ROOT_AGENT_ID,
        None,
    );

    emitter.user_message_confirmed("user-1");

    let events = sink.events();
    assert_eq!(events.len(), 1, "exactly one event expected");
    match &events[0] {
        TurnEvent::UserMessageConfirmed {
            conversation_id,
            user_msg_id,
        } => {
            assert_eq!(conversation_id, "conv-1");
            assert_eq!(user_msg_id, "user-1");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn seq_is_strictly_monotonic_within_a_turn_and_terminated_reuses_last_seq() {
    let h = harness(vec![ScriptedResponse::text(&["Hello", " world"])]);
    let ctx = started_ctx(&h.tasks, "conv-1");

    h.runner.run(&ctx, &TurnSpec::chat()).await;

    let events = h.sink.events();
    let seqs = delta_seqs(&events);
    assert_eq!(seqs, vec![1, 2, 3], "seq must start at 1 and increase by 1");

    let terminated = events
        .iter()
        .find_map(|event| match event {
            TurnEvent::Terminated { last_seq, .. } => Some(*last_seq),
            _ => None,
        })
        .expect("turn must terminate");
    assert_eq!(
        terminated,
        *seqs.last().unwrap(),
        "Terminated.last_seq must equal the last Delta seq"
    );
}

#[tokio::test]
async fn seq_restarts_at_one_for_the_next_turn() {
    let h = harness(vec![
        ScriptedResponse::text(&["Hello", " world"]),
        ScriptedResponse::text(&["Hello", " world"]),
    ]);

    let first = started_ctx(&h.tasks, "conv-1");
    h.runner.run(&first, &TurnSpec::chat()).await;
    let first_events = h.sink.events();
    assert_eq!(delta_seqs(&first_events), vec![1, 2, 3]);

    let offset = first_events.len();
    let second = started_ctx(&h.tasks, "conv-1");
    h.runner.run(&second, &TurnSpec::chat()).await;

    let all = h.sink.events();
    let second_events = &all[offset..];
    assert_eq!(
        delta_seqs(second_events),
        vec![1, 2, 3],
        "each turn owns a fresh seq counter"
    );
    for event in second_events {
        if let Some(turn_id) = event_turn_id(event) {
            assert_eq!(
                turn_id, second.turn_id,
                "second turn events must carry the second turn id"
            );
        }
    }
}

/// 事件轨迹的序列化文本（每行一条事件），用于逐字节比对。
async fn trace_of() -> Vec<String> {
    let h = harness(vec![ScriptedResponse::text(&["Hello", " world"])]);
    let ctx = started_ctx(&h.tasks, "conv-1");
    h.runner.run(&ctx, &TurnSpec::chat()).await;
    h.sink
        .events()
        .iter()
        .map(|event| serde_json::to_string(event).expect("TurnEvent must serialize"))
        .collect()
}

/// V17 / H9：同一输入、同一代际起点，两遍运行的事件轨迹逐字节相同。
#[tokio::test]
async fn event_trace_is_byte_identical_across_runs() {
    let first = trace_of().await;
    let second = trace_of().await;

    assert!(!first.is_empty(), "trace must not be empty");
    assert_eq!(first, second, "same input must yield the same trace");
    assert!(
        first.iter().any(|line| line.contains("\"kind\":\"delta\"")),
        "trace must be tagged by variant kind: {first:?}"
    );
}
