//! P4 循环执行器单测（I38b C4.24–C4.38 / A1–A23）。
//!
//! 只驱动 `LoopRoundExecutor::run_iterations`，不经过 `TurnRunner`：所有端口都由
//! `test_doubles` 提供，断言口径按 spec §5 与 impl-guide 附录 A。

#![cfg(feature = "test-doubles")]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_agent::agent::loop_executor::{LoopRoundExecutor, WRAP_UP_INSTRUCTION};
use assistant_agent::agent::round_executor::{
    RoundExecutor, RoundOutcome, ToolErrorClass, ToolExecCtx, ToolExecResult, ToolExecutionPort,
    ToolStage, TurnBudget, TurnObserver, TurnRunCtx,
};
use assistant_agent::agent::tool_call::FinalizedToolCall;
use assistant_agent::agent::tool_event::ToolEvent;
use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, FixtureSource, InMemoryStore, RecordedRequest, ReplayMessage, ScriptedAdapter,
    ScriptedResponse, ScriptedToolExec, StepClock, StoreCall,
};
use assistant_agent::kernel::{
    Clock, MessageStatus, ProviderCredential, ProviderDelta, TurnError, TurnGeneration, TurnInput,
    TurnInputSource, TurnLimits, TurnPhase, TurnSpec, TurnStatus,
};
use assistant_providers::base::{BaseProviderAdapter, ChatDelta, ProviderError};
use assistant_providers::types::ToolSpec;
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

const CONVERSATION_ID: &str = "conv-1";
const TURN_ID: &str = "conv-1:turn-1";
const INITIAL_MESSAGE_ID: &str = "assistant-1";

// ---------------------------------------------------------------------------
// 观察者替身：记录 phase / delta（带 iteration 标记）/ 工具事件
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DeltaRecord {
    seq: u32,
    iteration: u32,
    text: String,
}

#[derive(Default)]
struct RecordingObserver {
    phases: Mutex<Vec<TurnPhase>>,
    deltas: Mutex<Vec<DeltaRecord>>,
    tool_events: Mutex<Vec<ToolEvent>>,
    seq: AtomicU32,
    iteration: AtomicU32,
    /// 注入后首个 delta 立即取消，模拟「流式中途取消」。
    cancel_on_delta: Option<CancellationToken>,
    /// 到达指定迭代的首个 delta 时取消，模拟「第 N 迭代中途取消」。
    cancel_on_delta_at: Option<(u32, CancellationToken)>,
}

impl RecordingObserver {
    fn phases(&self) -> Vec<TurnPhase> {
        self.phases.lock().map(|g| g.clone()).unwrap_or_default()
    }

    fn deltas(&self) -> Vec<DeltaRecord> {
        self.deltas.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl TurnObserver for RecordingObserver {
    fn set_phase(&self, phase: TurnPhase) {
        if phase == TurnPhase::Streaming {
            self.iteration.fetch_add(1, Ordering::SeqCst);
        }
        if let Ok(mut guard) = self.phases.lock() {
            guard.push(phase);
        }
    }

    fn emit_tool_event(&self, event: ToolEvent) {
        if let Ok(mut guard) = self.tool_events.lock() {
            guard.push(event);
        }
    }

    fn delta(&self, delta: &ProviderDelta) -> u32 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let text = delta.delta_text.clone().unwrap_or_default();
        if let Ok(mut guard) = self.deltas.lock() {
            guard.push(DeltaRecord {
                seq,
                iteration: self.iteration.load(Ordering::SeqCst),
                text,
            });
        }
        if let Some(token) = &self.cancel_on_delta {
            token.cancel();
        }
        if let Some((target, token)) = &self.cancel_on_delta_at {
            if self.iteration.load(Ordering::SeqCst) >= *target {
                token.cancel();
            }
        }
        seq
    }
}

// ---------------------------------------------------------------------------
// 其余端口替身
// ---------------------------------------------------------------------------

struct GenerationFlag(bool);

impl TurnGeneration for GenerationFlag {
    fn is_current(&self) -> bool {
        self.0
    }
}

/// 固定返回顺序的端口：用于断言「执行器不重排批次结果」。
struct OrderedToolExec {
    results: Mutex<Vec<ToolExecResult>>,
    batches: Mutex<Vec<usize>>,
}

impl OrderedToolExec {
    fn new(results: Vec<ToolExecResult>) -> Self {
        Self {
            results: Mutex::new(results),
            batches: Mutex::new(Vec::new()),
        }
    }

    fn batches(&self) -> Vec<usize> {
        self.batches.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ToolExecutionPort for OrderedToolExec {
    async fn execute(
        &self,
        calls: Vec<FinalizedToolCall>,
        _ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult> {
        if let Ok(mut guard) = self.batches.lock() {
            guard.push(calls.len());
        }
        self.results.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

/// 执行时取消本轮的端口：模拟「工具执行中被取消」。
struct CancelOnExecToolExec {
    token: CancellationToken,
}

#[async_trait]
impl ToolExecutionPort for CancelOnExecToolExec {
    async fn execute(
        &self,
        _calls: Vec<FinalizedToolCall>,
        _ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult> {
        self.token.cancel();
        futures::future::pending::<()>().await;
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// fixture 构造
// ---------------------------------------------------------------------------

fn delta_text(text: &str) -> ChatDelta {
    ChatDelta {
        delta_text: Some(text.to_string()),
        ..Default::default()
    }
}

fn tool_call_delta(index: usize, id: &str, name: &str, args: &str) -> ChatDelta {
    ChatDelta {
        delta_tool_call_id: Some(id.to_string()),
        delta_tool_call_name: Some(name.to_string()),
        delta_tool_call_arguments: Some(args.to_string()),
        delta_tool_call_index: Some(index),
        ..Default::default()
    }
}

/// 单个工具调用 + `finish_reason = tool_calls`。
fn tool_call_script(id: &str, name: &str, args: &str) -> ScriptedResponse {
    multi_tool_script(&[(id, name, args)])
}

/// 同一批次多个工具调用（index 从 0 递增）。
fn multi_tool_script(calls: &[(&str, &str, &str)]) -> ScriptedResponse {
    let mut deltas: Vec<ChatDelta> = calls
        .iter()
        .enumerate()
        .map(|(index, (id, name, args))| tool_call_delta(index, id, name, args))
        .collect();
    deltas.push(ChatDelta {
        finish_reason: Some("tool_calls".to_string()),
        ..Default::default()
    });
    ScriptedResponse::Deltas(deltas)
}

fn ok_result(call_id: &str, tool: &str, content: &str) -> ToolExecResult {
    ToolExecResult {
        call_id: call_id.to_string(),
        tool_name: tool.to_string(),
        content: content.to_string(),
        is_error: false,
        stage: ToolStage::Execute,
        error_kind: None,
        confirmation_wait_ms: None,
    }
}

fn err_result(
    call_id: &str,
    tool: &str,
    content: &str,
    stage: ToolStage,
    kind: Option<ToolErrorClass>,
) -> ToolExecResult {
    ToolExecResult {
        call_id: call_id.to_string(),
        tool_name: tool.to_string(),
        content: content.to_string(),
        is_error: true,
        stage,
        error_kind: kind,
        confirmation_wait_ms: None,
    }
}

fn echo_tool() -> ToolSpec {
    ToolSpec {
        name: "echo".to_string(),
        description: "echo back".to_string(),
        input_schema: serde_json::json!({"type": "object"}),
    }
}

fn custom_mode() -> AgentModeRow {
    AgentModeRow {
        id: "mode-custom".to_string(),
        name: "custom".to_string(),
        description: String::new(),
        kind: "custom".to_string(),
        prompt: "you are a fixture".to_string(),
        model: None,
        temperature: None,
        max_tokens: Some(4096),
        reasoning_enabled: None,
        reasoning_effort: None,
        tools_json: "[]".to_string(),
        presentation_rank: 0,
        is_builtin: false,
        created_at: 0,
        updated_at: 0,
    }
}

fn fixture_source(adapter: Arc<dyn BaseProviderAdapter>) -> FixtureSource {
    let conversation = ConversationRow {
        id: CONVERSATION_ID.to_string(),
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
        conversation_id: CONVERSATION_ID.to_string(),
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
    }];
    let provider = ProviderCredential {
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

// ---------------------------------------------------------------------------
// 运行参数与结果
// ---------------------------------------------------------------------------

struct LoopArgs {
    scripts: Vec<ScriptedResponse>,
    tools: Vec<ToolSpec>,
    tool_results: Vec<ToolExecResult>,
    max_iterations: u32,
    limits: TurnLimits,
    wrap_up: bool,
    clock: Arc<dyn Clock>,
    tool_exec: Option<Arc<dyn ToolExecutionPort>>,
    cancel_on_delta: bool,
    cancel_on_exec: bool,
    cancel_on_delta_at: Option<u32>,
    generation_current: bool,
}

impl LoopArgs {
    fn new(scripts: Vec<ScriptedResponse>) -> Self {
        Self {
            scripts,
            tools: Vec::new(),
            tool_results: Vec::new(),
            max_iterations: 3,
            limits: TurnLimits {
                max_iterations: 3,
                max_tool_calls: 8,
                turn_wall_clock: None,
                model_call: None,
                empty_response_retries: 0,
                microcompact_keep_recent: 0,
            },
            wrap_up: false,
            clock: Arc::new(FixedClock::default()),
            tool_exec: None,
            cancel_on_delta: false,
            cancel_on_exec: false,
            cancel_on_delta_at: None,
            generation_current: true,
        }
    }

    fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    fn with_results(mut self, results: Vec<ToolExecResult>) -> Self {
        self.tool_results = results;
        self
    }

    fn with_iterations(mut self, max_iterations: u32) -> Self {
        self.max_iterations = max_iterations;
        self.limits.max_iterations = max_iterations;
        self
    }

    fn with_max_tool_calls(mut self, max_tool_calls: u32) -> Self {
        self.limits.max_tool_calls = max_tool_calls;
        self
    }

    fn with_empty_retries(mut self, retries: u32) -> Self {
        self.limits.empty_response_retries = retries;
        self
    }

    fn with_wall_clock(mut self, limit: Duration) -> Self {
        self.limits.turn_wall_clock = Some(limit);
        self
    }

    fn with_wrap_up(mut self) -> Self {
        self.wrap_up = true;
        self
    }

    fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    fn with_port(mut self, port: Arc<dyn ToolExecutionPort>) -> Self {
        self.tool_exec = Some(port);
        self
    }

    fn cancel_on_first_delta(mut self) -> Self {
        self.cancel_on_delta = true;
        self
    }

    fn cancel_on_exec(mut self) -> Self {
        self.cancel_on_exec = true;
        self
    }

    /// 指定迭代的首个 delta 到达后立即取消，用于「第 N 迭代中途取消」。
    fn cancel_on_delta_at(mut self, iteration: u32) -> Self {
        self.cancel_on_delta_at = Some(iteration);
        self
    }

    fn with_model_call_timeout(mut self, limit: Duration) -> Self {
        self.limits.model_call = Some(limit);
        self
    }

    fn stale_generation(mut self) -> Self {
        self.generation_current = false;
        self
    }
}

struct LoopRun {
    outcome: RoundOutcome,
    store: Arc<InMemoryStore>,
    adapter: Arc<ScriptedAdapter>,
    observer: Arc<RecordingObserver>,
}

impl LoopRun {
    fn calls(&self) -> Vec<StoreCall> {
        self.store.calls()
    }

    fn replay(&self) -> Vec<ReplayMessage> {
        self.store.replay_messages()
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.adapter.requests()
    }
}

async fn run_loop(args: LoopArgs) -> LoopRun {
    let adapter = Arc::new(ScriptedAdapter::new(args.scripts));
    let token = CancellationToken::new();
    let observer = Arc::new(RecordingObserver {
        cancel_on_delta: args.cancel_on_delta.then(|| token.clone()),
        cancel_on_delta_at: args.cancel_on_delta_at.map(|it| (it, token.clone())),
        ..Default::default()
    });
    let store = Arc::new(InMemoryStore::new());
    // 模拟 `TurnRunner` 在进入循环前已建好占位行，执行器直接复用该 id。
    store.seed_assistant_row(INITIAL_MESSAGE_ID, Some("user-1"));
    let clock = args.clock;
    let started_at = clock.now();

    let mut source = fixture_source(adapter.clone() as Arc<dyn BaseProviderAdapter>);
    let has_tools = !args.tools.is_empty();
    if has_tools {
        source = source.with_mode(custom_mode());
    }
    let input: TurnInput = source
        .conversation_detail(CONVERSATION_ID)
        .await
        .expect("fixture input");

    let catalog: Option<Vec<ToolSpec>> = has_tools.then(|| args.tools.clone());

    let tool_exec: Option<Arc<dyn ToolExecutionPort>> = if args.cancel_on_exec {
        Some(Arc::new(CancelOnExecToolExec {
            token: token.clone(),
        }))
    } else {
        match args.tool_exec {
            Some(port) => Some(port),
            None if has_tools => Some(Arc::new(ScriptedToolExec::new(args.tool_results))),
            None => None,
        }
    };

    let mut spec = TurnSpec {
        max_iterations: args.max_iterations,
        wrap_up: args.wrap_up,
        ..TurnSpec::agent()
    };
    spec.tools = catalog.clone();
    spec.limits = args.limits;

    let generation = GenerationFlag(args.generation_current);

    let ctx = TurnRunCtx {
        turn_id: TURN_ID,
        conversation_id: CONVERSATION_ID,
        initial_message_id: INITIAL_MESSAGE_ID,
        start_iteration: 1,
        remaining_turn_budget: TurnBudget {
            remaining_tool_calls: spec.limits.max_tool_calls,
            turn_deadline: spec
                .limits
                .turn_wall_clock
                .filter(|limit| !limit.is_zero())
                .map(|limit| started_at + limit),
        },
        turn_started_at: started_at,
        child_token: &token,
        tools: catalog.as_deref(),
        tool_exec: tool_exec.as_deref(),
        observer: observer.as_ref(),
        store: store.as_ref(),
        input: &input,
        clock: clock.as_ref(),
        generation: &generation,
        limits: &spec.limits,
        spec: &spec,
        provider_id: Some("fixture-provider"),
        model_id: Some("fixture-model"),
        model: "fixture-model",
        branch_leaf_id: Some("user-1"),
        cache_capable: false,
        agent_id: "fixture-agent",
        parent_turn_id: None,
        delegation_depth: 0,
    };

    let outcome = LoopRoundExecutor.run_iterations(ctx).await;
    LoopRun {
        outcome,
        store,
        adapter,
        observer,
    }
}

// ---------------------------------------------------------------------------
// 断言辅助
// ---------------------------------------------------------------------------

fn call_labels(calls: &[StoreCall]) -> Vec<String> {
    calls
        .iter()
        .map(|call| match call {
            StoreCall::Create(id) => format!("create:{id}"),
            StoreCall::Save(id) => format!("save:{id}"),
            StoreCall::Finalize(id, status) => format!("finalize:{id}:{status:?}"),
            StoreCall::Usage(n) => format!("usage:{n}"),
            StoreCall::SaveToolCalls(id, _) => format!("tool_calls:{id}"),
            StoreCall::AppendTool(id) => format!("append:{id}"),
        })
        .collect()
}

fn row<'a>(replay: &'a [ReplayMessage], id: &str) -> &'a ReplayMessage {
    replay
        .iter()
        .find(|msg| msg.id == id)
        .unwrap_or_else(|| panic!("row {id} not written; replay={replay:?}"))
}

fn tool_rows(replay: &[ReplayMessage]) -> Vec<&ReplayMessage> {
    replay.iter().filter(|msg| msg.role == "tool").collect()
}

fn assistant_rows(replay: &[ReplayMessage]) -> Vec<&ReplayMessage> {
    replay
        .iter()
        .filter(|msg| msg.role == "assistant")
        .collect()
}

/// 行链严格单链：按写入顺序，每行的 `parent_id` 必须是上一条写入行。
fn assert_single_chain(replay: &[ReplayMessage], first_parent: &str) {
    let mut prev = first_parent.to_string();
    for msg in replay {
        assert_eq!(
            msg.parent_id.as_deref(),
            Some(prev.as_str()),
            "row {} must chain onto {prev}; replay={replay:?}",
            msg.id
        );
        prev = msg.id.clone();
    }
}

fn last_user_contents(requests: &[RecordedRequest]) -> Vec<String> {
    requests
        .iter()
        .map(|req| {
            req.messages
                .last()
                .map(|msg| msg.content.clone())
                .unwrap_or_default()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// A1 / C4.24：三迭代闭环
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a1_three_iteration_loop_completes() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"b\"}"),
            ScriptedResponse::text(&["done"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            ok_result("call-1", "echo", "a"),
            ok_result("call-2", "echo", "b"),
        ]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.text, "done");
    assert_eq!(run.outcome.iterations, 3);
    assert_eq!(run.outcome.model_calls, 3);
    assert_eq!(run.outcome.tool_calls, 2);
    assert!(run.outcome.error.is_none());
    assert!(!run.outcome.hit_cap);
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some("conv-1:assistant:4"),
        "最终行是本迭代新建的 assistant 行"
    );
    assert_eq!(run.adapter.calls(), 3);

    assert_eq!(
        call_labels(&run.calls()),
        vec![
            "save:assistant-1",
            "tool_calls:assistant-1",
            "finalize:assistant-1:Completed",
            "append:conv-1:tool:1",
            "create:conv-1:assistant:2",
            "save:conv-1:assistant:2",
            "tool_calls:conv-1:assistant:2",
            "finalize:conv-1:assistant:2:Completed",
            "append:conv-1:tool:3",
            "create:conv-1:assistant:4",
            "save:conv-1:assistant:4",
            "finalize:conv-1:assistant:4:Completed",
        ],
        "第 1 迭代复用占位行，后续迭代各建一行"
    );

    let replay = run.replay();
    assert_eq!(tool_rows(&replay).len(), 2);
    assert_eq!(row(&replay, "conv-1:tool:1").content, "a");
    assert_eq!(row(&replay, "conv-1:tool:3").content, "b");
    assert_eq!(row(&replay, "conv-1:assistant:4").content, "done");
    assert!(row(&replay, "conv-1:assistant:2").tool_calls_json.is_some());

    // 第 2 / 3 次请求都带上配对 tool 消息（wire 不留悬空 tool_call）。
    let requests = run.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].tools.is_some(), "工具目录已注入");
    for req in &requests[1..] {
        let tool_msgs = req
            .messages
            .iter()
            .filter(|msg| msg.tool_call_id.is_some())
            .count();
        assert!(tool_msgs >= 1, "wire must pair tool results");
    }

    // 每次迭代都先 Thinking 再 Streaming。
    let phases = run.observer.phases();
    assert_eq!(
        phases
            .iter()
            .filter(|phase| **phase == TurnPhase::Streaming)
            .count(),
        3
    );
}

// ---------------------------------------------------------------------------
// A2 / C4.25：最后一轮收尾（撤 tools + 收尾指令 + 未执行回填）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a2_wrap_up_call_drops_tools_and_marks_skipped() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            multi_tool_script(&[
                ("call-2", "echo", "{\"q\":\"b\"}"),
                ("call-3", "echo", "{\"q\":\"c\"}"),
            ]),
            ScriptedResponse::text(&["final answer"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![ok_result("call-1", "echo", "a")])
        .with_iterations(2)
        .with_wrap_up(),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.text, "final answer");
    assert!(run.outcome.hit_cap);
    assert!(run.outcome.wrap_up_injected);
    assert_eq!(run.outcome.iterations, 2, "收尾调用不递增迭代");
    assert_eq!(run.outcome.model_calls, 3, "两次正常 + 一次收尾");
    assert_eq!(run.outcome.tool_calls, 3, "未执行的调用同样计入发起数");

    let requests = run.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].tools.is_some(), "正常迭代仍带 tools");
    assert!(requests[2].tools.is_none(), "收尾调用必须撤掉 tools");
    assert_eq!(
        requests[2].messages.last().map(|msg| msg.content.as_str()),
        Some(WRAP_UP_INSTRUCTION),
        "收尾指令只进本次请求"
    );

    let replay = run.replay();
    let tools = tool_rows(&replay);
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0].content, "a");
    for skipped in &tools[1..] {
        assert!(skipped.is_error);
        assert!(
            skipped.content.contains("未执行：已达迭代上限"),
            "skipped content: {}",
            skipped.content
        );
    }
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some("conv-1:assistant:5"),
        "已 finalize 的迭代行不能覆盖，收尾文本另起新行"
    );
    assert_eq!(row(&replay, "conv-1:assistant:5").content, "final answer");
}

// ---------------------------------------------------------------------------
// A3 / C4.26：空响应重试不递增迭代、不留空行
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a3_empty_response_retry_stays_in_iteration() {
    let run = run_loop(
        LoopArgs::new(vec![
            ScriptedResponse::empty(),
            ScriptedResponse::text(&["ok"]),
        ])
        .with_empty_retries(1)
        .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.text, "ok");
    assert_eq!(run.outcome.iterations, 1, "重试不递增迭代");
    assert_eq!(run.outcome.model_calls, 2);
    assert_eq!(run.adapter.calls(), 2);
    assert_eq!(
        call_labels(&run.calls()),
        vec!["save:assistant-1", "finalize:assistant-1:Completed",],
        "重试只落一次库，不留空白行"
    );
    assert_eq!(run.replay().len(), 1, "执行器不重复建行，只复用预置占位行");
}

// ---------------------------------------------------------------------------
// A4 / C4.27：取消路径
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a4_stream_cancel_persists_partial_as_interrupted() {
    let run = run_loop(
        LoopArgs::new(vec![ScriptedResponse::Stall(vec![delta_text("part")])])
            .cancel_on_first_delta()
            .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Cancelled);
    assert_eq!(run.outcome.text, "part", "已产出的正文必须保留");
    assert!(run.outcome.error.is_none());
    assert_eq!(
        run.calls().last(),
        Some(&StoreCall::Finalize(
            "assistant-1".to_string(),
            MessageStatus::Interrupted
        )),
        "取消落 interrupted，不是 failed"
    );
}

#[tokio::test]
async fn a4_tool_exec_cancel_writes_cancelled_batch() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::text(&["unreachable"]),
        ])
        .with_tools(vec![echo_tool()])
        .cancel_on_exec(),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Cancelled);
    assert_eq!(run.outcome.tool_calls, 1, "被取消的调用同样计入发起数");

    let replay = run.replay();
    let tools = tool_rows(&replay);
    assert_eq!(tools.len(), 1, "取消批次仍写配对结果行");
    assert_eq!(tools[0].content, "已取消");
    assert!(tools[0].is_error);
    assert_eq!(tools[0].tool_call_id.as_deref(), Some("call-1"));

    let labels = call_labels(&run.calls());
    assert!(labels.contains(&"finalize:assistant-1:Interrupted".to_string()));
}

// ---------------------------------------------------------------------------
// A5 / C4.28：工具失败回填后继续
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a5_validation_failure_nudges_and_continues() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::text(&["recovered"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![err_result(
            "call-1",
            "echo",
            "bad args",
            ToolStage::Validate,
            Some(ToolErrorClass::Business),
        )]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.text, "recovered");
    assert_eq!(run.outcome.iterations, 2);

    let replay = run.replay();
    let tools = tool_rows(&replay);
    assert_eq!(tools.len(), 1);
    assert!(tools[0].is_error, "失败结果以 is_error 回填");
    assert_eq!(tools[0].content, "bad args");

    let last = last_user_contents(&run.requests());
    assert!(
        last[1].contains("上次调用未通过校验"),
        "nudge 只进第 2 次请求：{}",
        last[1]
    );
}

#[tokio::test]
async fn a5_permission_denied_halts_immediately() {
    let run = run_loop(
        LoopArgs::new(vec![tool_call_script("call-1", "echo", "{\"q\":\"a\"}")])
            .with_tools(vec![echo_tool()])
            .with_results(vec![err_result(
                "call-1",
                "echo",
                "denied",
                ToolStage::Gateway,
                Some(ToolErrorClass::Denied),
            )]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.iterations, 1);
    assert!(!run.outcome.hit_cap, "无进展出口不是预算出口");
    assert_eq!(
        run.outcome.error,
        Some(TurnError::NoProgress {
            reason: "工具 echo 连续失败 1 次，已停止本轮：denied".to_string()
        })
    );
    assert_eq!(
        run.outcome.text,
        "工具 echo 连续失败 1 次，已停止本轮：denied"
    );
}

// ---------------------------------------------------------------------------
// A12 / C4.29：三维预算钩子独立触发
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a12_iteration_limit_ends_turn_with_summary() {
    let run = run_loop(
        LoopArgs::new(vec![tool_call_script("call-1", "echo", "{\"q\":\"a\"}")])
            .with_tools(vec![echo_tool()])
            .with_results(vec![ok_result("call-1", "echo", "a")])
            .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert!(run.outcome.hit_cap);
    assert!(!run.outcome.wrap_up_injected);
    assert_eq!(run.outcome.iterations, 1);
    assert_eq!(run.outcome.text, "已达迭代上限，本轮已停止。");
    assert!(run.outcome.error.is_none());
}

#[tokio::test]
async fn a12_tool_call_limit_brakes_next_iteration() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::text(&["never"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![ok_result("call-1", "echo", "a")])
        .with_iterations(3)
        .with_max_tool_calls(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert!(run.outcome.hit_cap);
    assert_eq!(run.outcome.iterations, 2, "第 2 迭代开头即刹车");
    assert_eq!(run.outcome.model_calls, 1);
    assert_eq!(run.outcome.text, "已达工具调用上限，本轮已停止。");
}

#[tokio::test]
async fn a12_turn_wall_clock_brakes_before_first_call() {
    let clock = Arc::new(StepClock::starting_now(Duration::from_secs(2)));
    let run = run_loop(
        LoopArgs::new(vec![ScriptedResponse::text(&["never"])])
            .with_clock(clock)
            .with_wall_clock(Duration::from_secs(1))
            .with_iterations(3),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert!(run.outcome.hit_cap);
    assert_eq!(run.outcome.iterations, 1);
    assert_eq!(run.outcome.model_calls, 0, "刹车发生在模型调用之前");
    assert_eq!(run.outcome.text, "本轮已超时，已停止。");
}

// ---------------------------------------------------------------------------
// A12b / C4.30：预算内裁剪
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a12b_budget_trims_batch_and_backfills_skipped() {
    let run = run_loop(
        LoopArgs::new(vec![multi_tool_script(&[
            ("call-1", "echo", "{\"q\":\"1\"}"),
            ("call-2", "echo", "{\"q\":\"2\"}"),
            ("call-3", "echo", "{\"q\":\"3\"}"),
            ("call-4", "echo", "{\"q\":\"4\"}"),
            ("call-5", "echo", "{\"q\":\"5\"}"),
        ])])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            ok_result("call-1", "echo", "1"),
            ok_result("call-2", "echo", "2"),
        ])
        .with_iterations(3)
        .with_max_tool_calls(2),
    )
    .await;

    let replay = run.replay();
    let tools = tool_rows(&replay);
    assert_eq!(tools.len(), 5, "超额调用也必须写配对结果行");
    assert_eq!(tools[0].content, "1");
    assert_eq!(tools[1].content, "2");
    for skipped in &tools[2..] {
        assert!(skipped.is_error);
        assert!(
            skipped
                .content
                .contains("本轮工具调用预算已用尽（max_tool_calls = 2）"),
            "skipped content: {}",
            skipped.content
        );
    }

    // 裁剪后仍是完整一批：第 2 迭代的 wire 里 5 个 tool_call 都有配对消息。
    let first = &run.requests()[0];
    let paired = first
        .messages
        .iter()
        .filter(|msg| msg.tool_call_id.is_some())
        .count();
    assert_eq!(paired, 0, "首个请求尚无工具结果");

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert!(run.outcome.hit_cap);
    assert_eq!(run.outcome.tool_calls, 5, "发起数计入全部调用");
    assert_eq!(run.outcome.iterations, 2);
    assert_eq!(run.outcome.text, "已达工具调用上限，本轮已停止。");
}

// ---------------------------------------------------------------------------
// A13 / C4.31：同参重复失败阶梯（nudge 不落库）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a13_generic_ladder_nudges_then_halts() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-3", "echo", "{\"q\":\"a\"}"),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            err_result("call-1", "echo", "boom", ToolStage::Persist, None),
            err_result("call-2", "echo", "boom", ToolStage::Persist, None),
            err_result("call-3", "echo", "boom", ToolStage::Persist, None),
        ]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.iterations, 3);
    assert_eq!(run.outcome.model_calls, 3, "halt 后不再调用模型");
    assert_eq!(
        run.outcome.text,
        "工具 echo 连续失败 3 次，已停止本轮：boom"
    );

    let last = last_user_contents(&run.requests());
    assert_eq!(last.len(), 3);
    assert!(
        !last[0].contains("重复了同一调用"),
        "首次失败不 nudge：{}",
        last[0]
    );
    assert!(
        !last[1].contains("重复了同一调用"),
        "nudge 在失败后注入下一轮：{}",
        last[1]
    );
    assert!(
        last[2].contains("你刚才重复了同一调用（echo）"),
        "第 2 次失败后注入 nudge：{}",
        last[2]
    );

    // nudge 只进请求，不落库：唯一 user 行是命令层的 user-1。
    assert_eq!(run.replay().iter().filter(|m| m.role == "user").count(), 0);
}

// ---------------------------------------------------------------------------
// A14 / C4.32：单批次执行 + 结果顺序保持
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a14_batch_runs_once_and_keeps_port_order() {
    let port = Arc::new(OrderedToolExec::new(vec![
        ok_result("call-3", "echo", "c"),
        ok_result("call-1", "echo", "a"),
        ok_result("call-2", "echo", "b"),
    ]));
    let run = run_loop(
        LoopArgs::new(vec![
            multi_tool_script(&[
                ("call-1", "echo", "{\"q\":\"a\"}"),
                ("call-2", "echo", "{\"q\":\"b\"}"),
                ("call-3", "echo", "{\"q\":\"c\"}"),
            ]),
            ScriptedResponse::text(&["done"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_port(port.clone()),
    )
    .await;

    assert_eq!(port.batches(), vec![3], "同批次一次执行，不拆批");
    let replay = run.replay();
    let tools = tool_rows(&replay);
    assert_eq!(
        tools
            .iter()
            .map(|msg| msg.tool_call_id.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["call-3", "call-1", "call-2"],
        "结果顺序按端口返回顺序落库，执行器不重排"
    );

    let second = &run.requests()[1];
    let paired: Vec<String> = second
        .messages
        .iter()
        .filter_map(|msg| msg.tool_call_id.clone())
        .collect();
    assert_eq!(
        paired,
        vec!["call-3", "call-1", "call-2"],
        "wire 与落库同序"
    );

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.tool_calls, 3);
}

// ---------------------------------------------------------------------------
// A17 / C4.33：分类阶梯（预算 + 成功清零）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a17_validation_nudges_once_then_halts() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"a\"}"),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            err_result(
                "call-1",
                "echo",
                "bad",
                ToolStage::Validate,
                Some(ToolErrorClass::Business),
            ),
            err_result(
                "call-2",
                "echo",
                "bad",
                ToolStage::Validate,
                Some(ToolErrorClass::Business),
            ),
        ])
        .with_iterations(2),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.iterations, 2);
    assert_eq!(run.outcome.text, "工具 echo 连续失败 2 次，已停止本轮：bad");
    let last = last_user_contents(&run.requests());
    assert!(
        last[1].contains("上次调用未通过校验"),
        "预算 1 → 只 nudge 一次"
    );
}

#[tokio::test]
async fn a17_transient_allows_two_retries_without_nudge() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-3", "echo", "{\"q\":\"a\"}"),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            err_result(
                "call-1",
                "echo",
                "flaky",
                ToolStage::Execute,
                Some(ToolErrorClass::Internal),
            ),
            err_result(
                "call-2",
                "echo",
                "flaky",
                ToolStage::Execute,
                Some(ToolErrorClass::Internal),
            ),
            err_result(
                "call-3",
                "echo",
                "flaky",
                ToolStage::Execute,
                Some(ToolErrorClass::Internal),
            ),
        ]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.iterations, 3, "预算 2 → 第 3 次才 halt");
    let last = last_user_contents(&run.requests());
    assert!(
        last.iter().all(|text| !text.contains("上次调用未通过校验")),
        "Transient 无分类 nudge：{last:?}"
    );
}

#[tokio::test]
async fn a17_success_resets_classified_budget() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-3", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::text(&["done"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            err_result(
                "call-1",
                "echo",
                "flaky",
                ToolStage::Execute,
                Some(ToolErrorClass::Internal),
            ),
            ok_result("call-2", "echo", "ok"),
            err_result(
                "call-3",
                "echo",
                "flaky",
                ToolStage::Execute,
                Some(ToolErrorClass::Internal),
            ),
        ])
        .with_iterations(4),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.iterations, 4, "成功清零后重新计数，不提前 halt");
    assert_eq!(run.outcome.text, "done");
}

// ---------------------------------------------------------------------------
// A20 / C4.34：行链严格单链，不出现假分支
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a20_row_chain_is_single_line() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            tool_call_script("call-2", "echo", "{\"q\":\"b\"}"),
            ScriptedResponse::text(&["done"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![
            ok_result("call-1", "echo", "a"),
            ok_result("call-2", "echo", "b"),
        ]),
    )
    .await;

    let replay = run.replay();
    assert_single_chain(&replay, "user-1");
    assert_eq!(assistant_rows(&replay).len(), 3, "预置占位行 + 两次新建");
    assert_eq!(
        assistant_rows(&replay)
            .iter()
            .filter(|msg| msg.parent_id.as_deref() == Some("user-1"))
            .count(),
        1,
        "只有占位行挂在 user 上，不得出现第二条假分支"
    );
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some(replay.last().expect("last row").id.as_str()),
        "终态行就是最后写入的 assistant 行"
    );
}

// ---------------------------------------------------------------------------
// A21 / C4.35：双 Halt 出口的形状与可读文本
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a21_budget_exit_is_completed_and_readable() {
    let run = run_loop(
        LoopArgs::new(vec![tool_call_script("call-1", "echo", "{\"q\":\"a\"}")])
            .with_tools(vec![echo_tool()])
            .with_results(vec![ok_result("call-1", "echo", "a")])
            .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert!(run.outcome.hit_cap);
    assert!(run.outcome.error.is_none());
    assert!(!run.outcome.text.trim().is_empty());
    for token in ["iteration-limit", "tool-call-limit", "turn-wall-clock"] {
        assert!(
            !run.outcome.text.contains(token),
            "用户可见文本不得混入内部标识：{}",
            run.outcome.text
        );
    }
}

#[tokio::test]
async fn a21_no_progress_exit_is_failed_and_readable() {
    let run = run_loop(
        LoopArgs::new(vec![tool_call_script("call-1", "echo", "{\"q\":\"a\"}")])
            .with_tools(vec![echo_tool()])
            .with_results(vec![err_result(
                "call-1",
                "echo",
                "denied",
                ToolStage::Gateway,
                Some(ToolErrorClass::Denied),
            )]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert!(!run.outcome.hit_cap);
    assert!(matches!(
        run.outcome.error,
        Some(TurnError::NoProgress { .. })
    ));
    assert!(!run.outcome.text.trim().is_empty());
    for token in ["repeated-failure", "middleware", "after_iteration"] {
        assert!(
            !run.outcome.text.contains(token),
            "用户可见文本不得混入内部标识：{}",
            run.outcome.text
        );
    }
}

// ---------------------------------------------------------------------------
// A22 / C4.37：第 2 迭代取消 / 超时 / provider 错误时新建行并保留第 1 迭代行
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a22_second_iteration_failure_keeps_first_row() {
    let run = run_loop(
        LoopArgs::new(vec![
            ScriptedResponse::Deltas(vec![
                delta_text("step1"),
                tool_call_delta(0, "call-1", "echo", "{\"q\":\"a\"}"),
                ChatDelta {
                    finish_reason: Some("tool_calls".to_string()),
                    ..Default::default()
                },
            ]),
            ScriptedResponse::Error(ProviderError::RequestFailed("boom".to_string())),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![ok_result("call-1", "echo", "a")]),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert!(matches!(run.outcome.error, Some(TurnError::Provider(_))));
    assert!(run.outcome.provider_error.is_some(), "原始错误载荷单列保存");
    assert_eq!(run.outcome.iterations, 2);

    let labels = call_labels(&run.calls());
    assert_eq!(
        labels
            .iter()
            .filter(|label| label == &&"save:assistant-1".to_string())
            .count(),
        1,
        "第 1 迭代行不得被第 2 迭代覆盖：{labels:?}"
    );
    assert!(
        labels.contains(&"finalize:assistant-1:Completed".to_string()),
        "第 1 迭代行以 completed 收尾：{labels:?}"
    );

    let replay = run.replay();
    assert_eq!(row(&replay, "assistant-1").content, "step1");
    let failed_row = row(&replay, "conv-1:assistant:2");
    assert_eq!(failed_row.parent_id.as_deref(), Some("conv-1:tool:1"));
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some("conv-1:assistant:2")
    );
}

#[tokio::test]
async fn a22_second_iteration_cancel_keeps_first_row() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::text(&["p2", "q2"]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![ok_result("call-1", "echo", "a")])
        .cancel_on_delta_at(2),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Cancelled);
    assert!(run.outcome.error.is_none(), "取消不算失败");
    assert!(run.outcome.provider_error.is_some(), "原始错误载荷单列保存");
    assert_eq!(run.outcome.iterations, 2);

    let labels = call_labels(&run.calls());
    assert_eq!(
        labels
            .iter()
            .filter(|label| label == &&"save:assistant-1".to_string())
            .count(),
        1,
        "第 1 迭代行不得被第 2 迭代覆盖：{labels:?}"
    );
    assert!(
        labels.contains(&"finalize:assistant-1:Completed".to_string()),
        "第 1 迭代行以 completed 收尾：{labels:?}"
    );

    let replay = run.replay();
    assert!(
        row(&replay, "assistant-1").tool_calls_json.is_some(),
        "第 1 迭代行保持纯工具调用形态，未被改写"
    );
    let cancelled_row = row(&replay, "conv-1:assistant:2");
    assert_eq!(cancelled_row.parent_id.as_deref(), Some("conv-1:tool:1"));
    assert_eq!(cancelled_row.content, "p2", "只保留已收到的增量");
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some("conv-1:assistant:2")
    );
}

#[tokio::test]
async fn a22_second_iteration_timeout_keeps_first_row() {
    let run = run_loop(
        LoopArgs::new(vec![
            tool_call_script("call-1", "echo", "{\"q\":\"a\"}"),
            ScriptedResponse::Delayed(Duration::from_millis(60), vec![delta_text("slow")]),
        ])
        .with_tools(vec![echo_tool()])
        .with_results(vec![ok_result("call-1", "echo", "a")])
        .with_model_call_timeout(Duration::from_millis(10)),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert!(matches!(run.outcome.error, Some(TurnError::Timeout)));
    assert!(run.outcome.provider_error.is_some(), "原始错误载荷单列保存");
    assert_eq!(run.outcome.iterations, 2);

    let labels = call_labels(&run.calls());
    assert_eq!(
        labels
            .iter()
            .filter(|label| label == &&"save:assistant-1".to_string())
            .count(),
        1,
        "第 1 迭代行不得被第 2 迭代覆盖：{labels:?}"
    );
    assert!(
        labels.contains(&"finalize:assistant-1:Completed".to_string()),
        "第 1 迭代行以 completed 收尾：{labels:?}"
    );

    let replay = run.replay();
    assert!(
        row(&replay, "assistant-1").tool_calls_json.is_some(),
        "第 1 迭代行保持纯工具调用形态，未被改写"
    );
    let timed_out_row = row(&replay, "conv-1:assistant:2");
    assert_eq!(timed_out_row.parent_id.as_deref(), Some("conv-1:tool:1"));
    assert_eq!(
        run.outcome.final_message_id.as_deref(),
        Some("conv-1:assistant:2")
    );
}

// ---------------------------------------------------------------------------
// A23 / C4.38：delta 唯一路径 + 代际校验
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a23_deltas_flow_only_through_observer_with_iteration_tag() {
    let run = run_loop(
        LoopArgs::new(vec![
            ScriptedResponse::text(&["he", "llo"]),
            ScriptedResponse::text(&["done"]),
        ])
        .with_iterations(1),
    )
    .await;

    let deltas = run.observer.deltas();
    let text_deltas: Vec<_> = deltas.iter().filter(|d| !d.text.is_empty()).collect();
    assert_eq!(text_deltas.len(), 2, "两个正文增量都经 observer");
    assert_eq!(text_deltas[0].seq, 1);
    assert_eq!(text_deltas[1].seq, 2, "seq 单调递增");
    assert_eq!(text_deltas[0].iteration, 1, "带迭代标记");
    assert!(
        deltas.windows(2).all(|pair| pair[0].seq < pair[1].seq),
        "observer 收到的 seq 严格递增：{deltas:?}"
    );
    assert_eq!(
        text_deltas
            .iter()
            .map(|d| d.text.as_str())
            .collect::<String>(),
        run.outcome.text,
        "observer 收到的增量拼回正文"
    );
    assert_eq!(run.outcome.text, "hello");
}

#[tokio::test]
async fn a23_stale_generation_skips_all_persist() {
    let run = run_loop(
        LoopArgs::new(vec![ScriptedResponse::text(&["done"])])
            .with_iterations(1)
            .stale_generation(),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Completed);
    assert_eq!(run.outcome.text, "done");
    assert_eq!(run.outcome.final_message_id, None, "旧代际不得落库");
    assert!(run.calls().is_empty(), "旧代际不产生任何写调用");
    assert_eq!(run.replay().len(), 1, "旧代际不写新行，只剩预置占位行");
}

// ---------------------------------------------------------------------------
// A15 / C4.38：空响应重试用尽 + 输出上限截断不重发
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a15_empty_retries_exhausted_fails_without_extra_row() {
    let run = run_loop(
        LoopArgs::new(vec![ScriptedResponse::empty(), ScriptedResponse::empty()])
            .with_empty_retries(1)
            .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.error, Some(TurnError::EmptyResponse));
    assert_eq!(run.outcome.model_calls, 2, "重试用尽即失败");
    assert_eq!(run.outcome.iterations, 1);
    assert_eq!(
        call_labels(&run.calls()),
        vec!["save:assistant-1", "finalize:assistant-1:Failed"],
        "重试不留空白行，只以占位行收尾"
    );
    let replay = run.replay();
    assert_eq!(replay.len(), 1, "重试不建新行");
    assert_eq!(row(&replay, INITIAL_MESSAGE_ID).content, "");
}

#[tokio::test]
async fn a15_output_limit_truncation_is_not_retried() {
    let run = run_loop(
        LoopArgs::new(vec![ScriptedResponse::Deltas(vec![ChatDelta {
            finish_reason: Some("length".to_string()),
            ..Default::default()
        }])])
        .with_empty_retries(2)
        .with_iterations(1),
    )
    .await;

    assert_eq!(run.outcome.status, TurnStatus::Failed);
    assert_eq!(run.outcome.error, Some(TurnError::EmptyResponse));
    assert_eq!(run.outcome.model_calls, 1, "输出上限截断不得重发");
    assert_eq!(run.adapter.calls(), 1);
}
