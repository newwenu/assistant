#![cfg(feature = "test-doubles")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, FixtureSource, InMemoryStore, RecordingSink, ScriptedAdapter, ScriptedResponse,
    StepClock, StoreCall,
};
use assistant_agent::kernel::ports::EventSink;
use assistant_agent::kernel::{
    CompactionConfig, CompactionSink, MessageStatus, SessionState, SessionStates, TurnContext,
    TurnError, TurnEvent, TurnLimits, TurnRunner, TurnSpec, TurnStatus, TurnTasks,
};
use assistant_providers::base::{
    BaseProviderAdapter, ChatDelta, ProviderError, ProviderErrorCode, TokenUsage,
};
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
    fixture_input_with_models(conversation_id, adapter, "[]")
}

fn fixture_input_with_models(
    conversation_id: &str,
    adapter: Arc<dyn BaseProviderAdapter>,
    models_json: &str,
) -> FixtureSource {
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
            models_json: models_json.to_string(),
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
    harness_with_options(adapter, sink, store, tasks, "[]", None)
}

/// 允许指定 provider 的模型元数据（如 `contextWindow`）与压缩配置，用于验证终止式压缩触发。
fn harness_with_options<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
    models_json: &str,
    compaction: Option<CompactionConfig>,
) -> Harness<S> {
    let states = Arc::new(SessionStates::new());
    let runner = TurnRunner::new(
        sink.clone(),
        store.clone(),
        Arc::new(fixture_input_with_models("conv-1", adapter, models_json)),
        states.clone(),
        tasks.clone(),
        Arc::new(FixedClock::default()),
        compaction,
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

/// DT6：每次 `run` 恰好 1 次 `Terminated`，且它是最后一条事件。
fn assert_single_terminated(events: &[TurnEvent]) {
    assert_eq!(
        terminated_count(events),
        1,
        "each run must terminate exactly once: {events:?}"
    );
    assert!(
        matches!(events.last(), Some(TurnEvent::Terminated { .. })),
        "Terminated must be the last event: {events:?}"
    );
}

fn terminated_count(events: &[TurnEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, TurnEvent::Terminated { .. }))
        .count()
}

/// 收到首个 delta 立即取消当前轮，用于模拟「流式中途取消」。
struct CancelOnDeltaSink {
    tasks: Arc<TurnTasks>,
    events: Mutex<Vec<TurnEvent>>,
}

impl CancelOnDeltaSink {
    fn events(&self) -> Vec<TurnEvent> {
        self.events.lock().unwrap().clone()
    }
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
    assert_single_terminated(&events);

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
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            StoreCall::AppendTool(_) | StoreCall::SaveToolCalls(_, _)
        )),
        "chat 路径不得写 tool 行"
    );
}

/// DT5 / V19：同一输入跑两遍，`StoreCall` 序列与内容逐项相同（时间戳由注入时钟提供）。
#[tokio::test]
async fn store_call_sequence_is_identical_across_runs() {
    async fn run_once() -> Vec<StoreCall> {
        let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
        let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
        let ctx = started_ctx(&h.tasks, "conv-1");
        h.runner.run(&ctx, &TurnSpec::chat()).await;
        h.store.calls()
    }

    let first = run_once().await;
    let second = run_once().await;

    assert!(!first.is_empty(), "落库调用序列不得为空");
    assert_eq!(first, second, "同一输入两遍运行的落库调用序列必须逐项相同");
    assert!(matches!(first[0], StoreCall::Save(_)));
    assert_eq!(
        first.last(),
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
    assert!(
        h.sink.dropped_count() > 0,
        "V4：事件发射失败必须被记录，不得静默丢弃"
    );
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
    assert_single_terminated(&h.sink.events());
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
    assert_single_terminated(&h.sink.events());
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
    assert_single_terminated(&h.sink.events());
    assert_eq!(
        h.store.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Failed
        ))
    );
}

#[tokio::test]
async fn output_limit_truncation_is_not_retried() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![
        ScriptedResponse::Deltas(vec![ChatDelta {
            finish_reason: Some("length".to_string()),
            ..Default::default()
        }]),
        ScriptedResponse::Deltas(vec![delta_text("should not be requested")]),
    ]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let mut spec = TurnSpec::agent();
    spec.max_iterations = 1;
    spec.tools = None;
    let outcome = h.runner.run(&ctx, &spec).await;

    assert_eq!(outcome.status, TurnStatus::Failed);
    assert_eq!(outcome.iterations, 1);
    assert_eq!(outcome.model_calls, 1, "输出上限截断不得重发");
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
    let TurnError::Provider(message) = error else {
        panic!("unexpected error variant: {error:?}");
    };
    assert!(
        message.to_lowercase().contains("invalid api key"),
        "unexpected error text: {message}"
    );
    let events = h.sink.events();
    assert_single_terminated(&events);
    let terminated = events.last();
    let Some(TurnEvent::Terminated {
        status: TurnStatus::Failed,
        error: Some(payload),
        ..
    }) = terminated
    else {
        panic!("终止事件必须携带 provider 原始错误载荷：{terminated:?}");
    };
    assert_eq!(
        payload.code,
        ProviderErrorCode::AuthFailed,
        "错误码不得被降级为通用错误"
    );
    assert_eq!(payload.message, "Authentication failed: invalid api key");
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
    assert_single_terminated(&h.sink.events());
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
    assert_single_terminated(&h.sink.events());
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
    assert_eq!(
        terminated_count(&h.sink.events()),
        0,
        "代际不符时不得发终止事件（DT6）"
    );
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
    assert!(
        matches!(outcome.error, Some(TurnError::Timeout)),
        "超时必须走独立变体：{:?}",
        outcome.error
    );
    assert_single_terminated(&sink.events());
    assert_eq!(
        store.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Failed
        ))
    );
}

#[tokio::test]
async fn wall_clock_timeout_halts_turn_before_model_call() {
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

    // 墙钟预算由 `StopHook` 判定，故必须用装了钩子的 agent spec（chat 不装钩子）。
    // `wrap_up = false` 走「根因摘要」分支，模型调用数保持 0（A21 预算耗尽出口）。
    let mut spec = TurnSpec::agent();
    spec.tools = None;
    spec.wrap_up = false;
    spec.limits = limits;
    let outcome = runner.run(&ctx, &spec).await;

    assert_eq!(outcome.status, TurnStatus::Completed);
    assert_eq!(outcome.model_calls, 0, "墙钟超时必须在模型调用前拦下");
    assert!(outcome.hit_cap, "预算耗尽出口必须标记 hit_cap");
    assert!(
        !outcome.text.trim().is_empty(),
        "预算耗尽出口不得返回空文本：{:?}",
        outcome.text
    );
    assert_single_terminated(&sink.events());
}

/// C5.30 golden：四组终态（正常完成 / 中途取消 / provider 报错 / 空响应）的事件序列、
/// `seq` 连续性、落库终态与错误文案不变量。不依赖已删除的 `chat_runtime` 旧基线。
fn event_shape(events: &[TurnEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| match event {
            TurnEvent::StreamStart { .. } => "stream_start".to_string(),
            TurnEvent::Delta { seq, .. } => format!("delta:{seq}"),
            TurnEvent::PhaseChanged { phase, .. } => format!("phase:{phase:?}"),
            TurnEvent::Tool(_) => "tool".to_string(),
            TurnEvent::CompactionStarted { .. } => "compaction_started".to_string(),
            TurnEvent::CompactionFinished { ok, .. } => format!("compaction_finished:{ok}"),
            TurnEvent::Terminated { status, .. } => format!("terminated:{status:?}"),
            TurnEvent::UserMessageConfirmed { .. } => "user_message_confirmed".to_string(),
        })
        .collect()
}

fn assert_turn_invariants(
    events: &[TurnEvent],
    calls: &[StoreCall],
    expected: TurnStatus,
    expected_status: MessageStatus,
) {
    assert_single_terminated(events);

    let Some(TurnEvent::Terminated {
        status, last_seq, ..
    }) = events.last()
    else {
        panic!("last event must be Terminated: {events:?}");
    };
    assert_eq!(*status, expected, "终态不符：{events:?}");

    let seqs: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::Delta { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    for (index, seq) in seqs.iter().enumerate() {
        assert_eq!(
            *seq,
            index as u32 + 1,
            "Delta seq 必须从 1 起连续递增：{seqs:?}"
        );
    }
    assert_eq!(
        *last_seq,
        seqs.last().copied().unwrap_or(0),
        "Terminated.last_seq 必须等于末条 Delta 的 seq：{events:?}"
    );

    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, StoreCall::Finalize(..)))
            .count(),
        1,
        "终态只能写一次：{calls:?}"
    );
    assert_eq!(
        calls.last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            expected_status
        )),
        "Finalize 必须是最后一次落库调用：{calls:?}"
    );
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            StoreCall::AppendTool(_) | StoreCall::SaveToolCalls(_, _)
        )),
        "chat 路径不得写 tool 行：{calls:?}"
    );
}

#[tokio::test]
async fn golden_event_sequence_completed() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![completed_script()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    let events = h.sink.events();
    assert_eq!(
        event_shape(&events),
        vec![
            "stream_start",
            "delta:1",
            "delta:2",
            "delta:3",
            "terminated:Completed"
        ]
    );
    assert_turn_invariants(
        &events,
        &h.store.calls(),
        TurnStatus::Completed,
        MessageStatus::Completed,
    );
    assert_eq!(outcome.text, "Hello world");
    assert!(outcome.error.is_none(), "成功终态不得携带错误");
}

#[tokio::test]
async fn golden_event_sequence_cancelled_mid_stream() {
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

    let events = h.sink.events();
    assert_eq!(
        event_shape(&events),
        vec!["stream_start", "delta:1", "terminated:Cancelled"]
    );
    assert_turn_invariants(
        &events,
        &h.store.calls(),
        TurnStatus::Cancelled,
        MessageStatus::Interrupted,
    );
    assert_eq!(outcome.text, "Hello", "已收到的部分内容必须保留");
    assert_eq!(h.states.get("conv-1"), Some(SessionState::Idle));
}

#[tokio::test]
async fn golden_event_sequence_provider_error() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Error(
        ProviderError::AuthFailed("invalid api key".to_string()),
    )]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let _outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    let events = h.sink.events();
    assert_eq!(
        event_shape(&events),
        vec!["stream_start", "terminated:Failed"]
    );
    assert_turn_invariants(
        &events,
        &h.store.calls(),
        TurnStatus::Failed,
        MessageStatus::Failed,
    );
    let Some(TurnEvent::Terminated {
        error: Some(payload),
        ..
    }) = events.last()
    else {
        panic!("provider 报错的终止事件必须携带错误载荷：{events:?}");
    };
    assert_eq!(payload.code, ProviderErrorCode::AuthFailed);
    assert_eq!(payload.message, "Authentication failed: invalid api key");
}

#[tokio::test]
async fn golden_event_sequence_empty_response() {
    let adapter = Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::empty()]));
    let h = harness(adapter, RecordingSink::new(), InMemoryStore::new());
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    let events = h.sink.events();
    assert_eq!(
        event_shape(&events),
        vec!["stream_start", "terminated:Failed"]
    );
    assert_turn_invariants(
        &events,
        &h.store.calls(),
        TurnStatus::Failed,
        MessageStatus::Failed,
    );
    assert!(
        matches!(outcome.error, Some(TurnError::EmptyResponse)),
        "空响应必须走独立错误变体：{:?}",
        outcome.error
    );
    let Some(TurnEvent::Terminated {
        error: Some(payload),
        ..
    }) = events.last()
    else {
        panic!("空响应的终止事件必须携带错误载荷：{events:?}");
    };
    assert!(
        payload.message.to_lowercase().contains("empty response"),
        "unexpected error text: {}",
        payload.message
    );
}

/// 终止式压缩触发端的替身：只记录调用目标，压缩本身由宿主异步执行。
struct CountingCompactionSink {
    calls: Mutex<Vec<String>>,
}

impl CountingCompactionSink {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl CompactionSink for CountingCompactionSink {
    async fn terminate(&self, conversation_id: &str) {
        self.calls.lock().unwrap().push(conversation_id.to_string());
    }
}

/// 已知窗口 `contextWindow` = 20000 → 预算 14904，阈值 50% 即 7452；
/// 下述脚本的 usage 为 20000 prompt tokens，必定判定为 `Terminate`。
const COMPACTION_MODELS_JSON: &str = r#"[{"id":"fixture-model","contextWindow":20000}]"#;

fn heavy_usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: 20_000,
        completion_tokens: 4,
        total_tokens: 20_004,
        ..Default::default()
    }
}

fn compaction_config(sink: Arc<CountingCompactionSink>) -> CompactionConfig {
    CompactionConfig {
        sink,
        auto_compact: true,
        threshold_percent: 50,
    }
}

#[tokio::test]
async fn completed_turn_triggers_terminal_compaction() {
    let sink = Arc::new(CountingCompactionSink::new());
    let h = harness_with_options(
        Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Deltas(vec![
            delta_text("Hello"),
            ChatDelta {
                finish_reason: Some("stop".to_string()),
                usage: Some(heavy_usage()),
                ..Default::default()
            },
        ])])),
        Arc::new(RecordingSink::new()),
        Arc::new(InMemoryStore::new()),
        Arc::new(TurnTasks::new()),
        COMPACTION_MODELS_JSON,
        Some(compaction_config(sink.clone())),
    );
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Completed);
    assert_eq!(
        sink.calls(),
        vec!["conv-1".to_string()],
        "完成的轮必须触发一次终止式压缩"
    );
}

#[tokio::test]
async fn cancelled_turn_never_triggers_compaction() {
    let sink = Arc::new(CountingCompactionSink::new());
    let tasks = Arc::new(TurnTasks::new());
    let h = harness_with_options(
        Arc::new(ScriptedAdapter::new(vec![ScriptedResponse::Stall(vec![
            ChatDelta {
                delta_text: Some("Hello".to_string()),
                usage: Some(heavy_usage()),
                ..Default::default()
            },
        ])])),
        Arc::new(CancelOnDeltaSink {
            tasks: tasks.clone(),
            events: Mutex::new(Vec::new()),
        }),
        Arc::new(InMemoryStore::new()),
        tasks.clone(),
        COMPACTION_MODELS_JSON,
        Some(compaction_config(sink.clone())),
    );
    let ctx = started_ctx(&h.tasks, "conv-1");

    let outcome = h.runner.run(&ctx, &TurnSpec::chat()).await;

    assert_eq!(outcome.status, TurnStatus::Cancelled);
    assert!(
        outcome.prompt_tokens.is_some(),
        "取消前已收到 usage，压缩判定已产生：{:?}",
        outcome.prompt_tokens
    );
    assert!(
        sink.calls().is_empty(),
        "取消是用户意图，不得随即自动发起摘要调用（I41 §4）"
    );
}