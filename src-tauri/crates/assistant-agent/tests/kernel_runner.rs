#![cfg(feature = "test-doubles")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, FixtureSource, InMemoryStore, RecordingSink, ScriptedAdapter, ScriptedResponse,
    StepClock, StoreCall,
};
use assistant_agent::kernel::ports::EventSink;
use assistant_agent::kernel::{
    MessageStatus, SessionState, SessionStates, TurnContext, TurnError, TurnEvent, TurnLimits,
    TurnRunner, TurnSpec, TurnStatus, TurnTasks,
};
use assistant_providers::base::{BaseProviderAdapter, ChatDelta, ProviderError, TokenUsage};
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;

fn delta_text(text: &str) -> ChatDelta {
    ChatDelta {
        delta_text: Some(text.to_string()),
        ..Default::default()
    }
}

/// 两段正文 + 带 usage 的收尾：正文拼成 `Hello world`，usage 为 10/4/14。
fn completed_script() -> ScriptedResponse {
    ScriptedResponse::Deltas(vec![
        delta_text("Hello"),
        delta_text(" world"),
        ChatDelta {
            finish_reason: Some("stop".to_string()),
            usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 4,
                total_tokens: 14,
                ..Default::default()
            }),
            ..Default::default()
        },
    ])
}

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

struct Harness<S: EventSink + 'static> {
    runner: TurnRunner,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
    states: Arc<SessionStates>,
}

fn harness(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: RecordingSink,
    store: InMemoryStore,
) -> Harness<RecordingSink> {
    harness_with(
        adapter,
        Arc::new(sink),
        Arc::new(store),
        Arc::new(TurnTasks::new()),
    )
}

fn harness_with<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
) -> Harness<S> {
    let states = Arc::new(SessionStates::new());
    let runner = TurnRunner::new(
        sink.clone(),
        store.clone(),
        Arc::new(fixture_input("conv-1", adapter)),
        states.clone(),
        tasks.clone(),
        Arc::new(FixedClock::default()),
        None,
    );
    Harness {
        runner,
        sink,
        store,
        tasks,
        states,
    }
}

fn started_ctx(tasks: &TurnTasks, conversation_id: &str) -> TurnContext {
    let (turn_id, _token) = tasks
        .begin(conversation_id, assistant_agent::kernel::OnBusy::Replace)
        .expect("begin");
    TurnContext::new(conversation_id, turn_id, "assistant-1")
}

/// 收到首个 delta 立即取消当前轮，用于模拟「流式中途取消」。
struct CancelOnDeltaSink {
    tasks: Arc<TurnTasks>,
    events: Mutex<Vec<TurnEvent>>,
}

impl EventSink for CancelOnDeltaSink {
    fn emit(&self, event: TurnEvent) {
        if matches!(event, TurnEvent::Delta { .. }) {
            self.tasks.cancel("conv-1");
        }
        self.events.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn chat_turn_completes_end_to_end_with_doubles() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(adapter.clone(), RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Completed);
    assert_eq!(outcome.text, "Hello world");
    assert_eq!(outcome.model_calls, 1);
    assert_eq!(outcome.iterations, 1);
    assert_eq!(outcome.prompt_tokens, Some(10));
    assert_eq!(outcome.total_tokens, Some(14));
    assert_eq!(adapter.calls(), 1, "恰好一次模型调用");

    let events = h.sink.events();
    assert!(matches!(
        events.first(),
        Some(TurnEvent::StreamStart { .. })
    ));
    let seqs: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::Delta { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(seqs, vec![1, 2, 3]);
    assert!(matches!(
        events.last(),
        Some(TurnEvent::Terminated {
            status: TurnStatus::Completed,
            ..
        })
    ));

    let calls = h.store.calls();
    assert!(matches!(calls[0], StoreCall::Save(_)));
    assert!(matches!(calls[1], StoreCall::Usage(1)));
    assert_eq!(
        calls.last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Completed
        ))
    );
}

#[tokio::test]
async fn sink_failure_does_not_abort_turn() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(
        adapter,
        RecordingSink::failing_after(1),
        InMemoryStore::new(),
    );
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Completed);
    assert_eq!(outcome.text, "Hello world");
    assert!(h.store.calls().contains(&StoreCall::Finalize(
        "assistant-1".to_string(),
        MessageStatus::Completed
    )));
}

#[tokio::test]
async fn store_failure_marks_turn_failed_and_finalizes_failed() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::failing());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert!(h.store.calls().contains(&StoreCall::Finalize(
        "assistant-1".to_string(),
        MessageStatus::Failed
    )));
}

#[tokio::test]
async fn chat_spec_does_not_retry_empty_response() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::empty()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert_eq!(outcome.iterations, 1);
    assert_eq!(outcome.model_calls, 1);
}

#[tokio::test]
async fn empty_response_retries_once_then_fails_without_bumping_iteration() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![
        ScriptedResponse::empty(),
        ScriptedResponse::empty(),
    ]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let mut spec = TurnSpec::agent();
    spec.max_iterations = 1;
    spec.tools = None;
    let outcome = h.runner.run(&ctx, &spec).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert_eq!(outcome.iterations, 1);
    assert_eq!(outcome.model_calls, 2);
    assert_eq!(
        h.store.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Failed
        ))
    );
}

#[tokio::test]
async fn provider_error_marks_turn_failed_with_error_text() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Error(
        ProviderError::AuthFailed("invalid api key".to_string()),
    )]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    let error = outcome.error.as_ref().expect("error must be reported");
    let TurnError::Other(message) = error else {
        panic!("unexpected error variant: {error:?}");
    };
    assert!(
        message.to_lowercase().contains("invalid api key"),
        "unexpected error text: {message}"
    );
    let terminated = h.sink.events().pop();
    assert!(matches!(
        terminated,
        Some(TurnEvent::Terminated {
            status: TurnStatus::Failed,
            error: Some(_),
            ..
        })
    ));
    assert_eq!(
        h.store.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Failed
        ))
    );
}

#[tokio::test]
async fn cancelled_turn_marks_message_interrupted() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");
    h.tasks.cancel("conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Cancelled);
    assert!(h.store.calls().contains(&StoreCall::Finalize(
        "assistant-1".to_string(),
        MessageStatus::Interrupted
    )));
    assert_eq!(
        h.states.get("conv-1"),
        Some(SessionState::Idle),
        "取消后会话状态必须回到 Idle"
    );
}

#[tokio::test]
async fn cancel_mid_stream_saves_partial_then_finalizes_interrupted() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Stall(vec![
        delta_text("Hello"),
    ])]));
    let tasks = Arc::new(TurnTasks::new());
    let sink = Arc::new(CancelOnDeltaSink {
        tasks: tasks.clone(),
        events: Mutex::new(Vec::new()),
    });
    let h = harness_with(adapter, sink, Arc::new(InMemoryStore::new()), tasks.clone());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Cancelled);
    assert_eq!(outcome.text, "Hello", "已收到的部分内容必须保留");
    let calls = h.store.calls();
    let save_at = calls
        .iter()
        .position(|c| matches!(c, StoreCall::Save(_)))
        .expect("取消时已收到的内容必须先落库");
    let finalize_at = calls
        .iter()
        .position(|c| matches!(c, StoreCall::Finalize(_, MessageStatus::Interrupted)))
        .expect("取消后必须写 interrupted 终态");
    assert!(
        save_at < finalize_at,
        "先落库后改状态：Save 必须在 Finalize 之前"
    );
    assert_eq!(
        h.states.get("conv-1"),
        Some(SessionState::Idle),
        "取消后会话状态必须回到 Idle"
    );
}

#[tokio::test]
async fn stale_turn_writes_nothing_and_emits_nothing() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let stale = started_ctx(&h.tasks, "conv-1");
    // 新消息顶替旧轮，`stale.turn_id` 不再是当前代际。
    let _newer = h
        .tasks
        .begin("conv-1", assistant_agent::kernel::OnBusy::Replace)
        .expect("begin newer");

    let outcome = h.runner.run(&stale, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Interrupted);
    assert!(h.store.calls().is_empty());
    assert!(h.sink.events().is_empty());
}

#[tokio::test(start_paused = true)]
async fn model_call_timeout_fails_turn_after_call_started() {
    // 暂停时钟：脚本里的 400ms 等待与 120ms 超时都由虚拟时间推进，用例不真等。
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Delayed(
        Duration::from_millis(400),
        vec![delta_text("late")],
    )]));
    let sink = Arc::new(RecordingSink::new());
    let store = Arc::new(InMemoryStore::new());
    let tasks = Arc::new(TurnTasks::new());
    let limits = TurnLimits {
        turn_wall_clock: None,
        model_call: Some(Duration::from_millis(120)),
        ..TurnLimits::default()
    };
    let runner = TurnRunner::new(
        sink.clone(),
        store.clone(),
        Arc::new(fixture_input("conv-1", adapter)),
        Arc::new(SessionStates::new()),
        tasks.clone(),
        Arc::new(FixedClock::default()),
        None,
    );
    let (turn_id, _token) = tasks
        .begin("conv-1", assistant_agent::kernel::OnBusy::Replace)
        .expect("begin");
    let ctx = TurnContext::new("conv-1", turn_id, "assistant-1");

    let mut spec = TurnSpec::chat();
    spec.limits = limits;
    let outcome = runner.run(&ctx, &spec).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert_eq!(outcome.model_calls, 1);
    assert!(matches!(
        outcome.error,
        Some(TurnError::Other(ref message)) if message.contains("timed out")
    ));
    assert_eq!(
        store.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Failed
        ))
    );
}

#[tokio::test]
async fn wall_clock_timeout_fails_turn_before_model_call() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let sink = Arc::new(RecordingSink::new());
    let store = Arc::new(InMemoryStore::new());
    let tasks = Arc::new(TurnTasks::new());
    let limits = TurnLimits {
        turn_wall_clock: Some(Duration::from_secs(1)),
        model_call: Some(Duration::from_secs(5)),
        ..TurnLimits::default()
    };
    let runner = TurnRunner::new(
        sink.clone(),
        store.clone(),
        Arc::new(fixture_input("conv-1", adapter)),
        Arc::new(SessionStates::new()),
        tasks.clone(),
        Arc::new(StepClock::starting_now(Duration::from_secs(2))),
        None,
    );
    let (turn_id, _token) = tasks
        .begin("conv-1", assistant_agent::kernel::OnBusy::Replace)
        .expect("begin");
    let ctx = TurnContext::new("conv-1", turn_id, "assistant-1");

    let mut spec = TurnSpec::chat();
    spec.limits = limits;
    let outcome = runner.run(&ctx, &spec).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert_eq!(outcome.model_calls, 0);
}
