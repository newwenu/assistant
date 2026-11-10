#![cfg(feature = "test-doubles")]

use std::sync::Arc;

use assistant_agent::agent::tool_event::ToolEvent;
use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, FixtureSource, InMemoryStore, RecordingSink, ScriptedAdapter, ScriptedResponse,
};
use assistant_agent::kernel::{
    OnBusy, SessionStates, TurnContext, TurnEmitter, TurnEvent, TurnRunner, TurnSpec, TurnTasks,
};
use assistant_providers::base::BaseProviderAdapter;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;

fn fixture_input(conversation_id: &str, adapter: Arc<dyn BaseProviderAdapter>) -> FixtureSource {
    let conversation = ConversationRow {
        id: conversation_id.to_string(),
        title: "fixture".to_string(),
        title_source: "user".to_string(),
        provider_id: Some("fixture-provider".to_string()),
        model_id: "fixture-model".to_string(),
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: 0,
        updated_at: 0,
        is_full: false,
        active_branch_id: None,
        mode_id: None,
        tool_config_json: None,
        workspace_root: String::new(),
    };
    let messages = vec![MessageRow {
        id: "user-1".to_string(),
        conversation_id: conversation_id.to_string(),
        role: "user".to_string(),
        content: "hi".to_string(),
        tokens: None,
        timestamp: 0,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
        mode_id: None,
    }];
    let provider = assistant_agent::kernel::ProviderCredential {
        row: ProviderRow {
            id: "fixture-provider".to_string(),
            name: "fixture".to_string(),
            type_: "openai".to_string(),
            base_url: "scripted://local".to_string(),
            api_key_encrypted: String::new(),
            models_json: "[]".to_string(),
            default_model: "fixture-model".to_string(),
            updated_at: 0,
        },
        api_key: "fixture-key".to_string(),
    };
    FixtureSource::new(
        ConversationDetail {
            conversation,
            messages,
        },
        provider,
        "fixture-attachments",
        adapter,
    )
}

struct Harness {
    runner: TurnRunner,
    sink: Arc<RecordingSink>,
    tasks: Arc<TurnTasks>,
}

fn harness(scripts: Vec<ScriptedResponse>) -> Harness {
    let sink = Arc::new(RecordingSink::new());
    let store = Arc::new(InMemoryStore::new());
    let tasks = Arc::new(TurnTasks::new());
    let states = Arc::new(SessionStates::new());
    let adapter: Arc<dyn BaseProviderAdapter> = Arc::new(ScriptedAdapter::new(scripts));
    let runner = TurnRunner::new(
        sink.clone(),
        store,
        Arc::new(fixture_input("conv-1", adapter)),
        states,
        tasks.clone(),
        Arc::new(FixedClock::default()),
        None,
    );
    Harness {
        runner,
        sink,
        tasks,
    }
}

fn started_ctx(tasks: &TurnTasks, conversation_id: &str) -> TurnContext {
    let (turn_id, _token) = tasks
        .begin(conversation_id, OnBusy::Replace)
        .expect("begin");
    TurnContext::new(conversation_id, turn_id, "assistant-1")
}

/// `UserMessageConfirmed` 是会话级事件，其余事件都是轮级事件。
fn event_turn_id(event: &TurnEvent) -> Option<&str> {
    match event {
        TurnEvent::StreamStart { turn_id, .. }
        | TurnEvent::Delta { turn_id, .. }
        | TurnEvent::PhaseChanged { turn_id, .. }
        | TurnEvent::Tool(ToolEvent { turn_id, .. })
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
    let emitter = TurnEmitter::new(sink.clone(), tasks.clone(), "conv-1", turn_id);

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