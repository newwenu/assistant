//! V1-e 验收：节点事件的写侧（I55 G1 / G3 / G5 / G12 / O2）。
//!
//! 走完整 `TurnRunner`（不是只跑循环执行器），因为 `admit` 由 `TurnRunner` 补记、
//! 且只有 `EventRoute::Broadcast` 才发节点事件。断言只看 `TurnEvent::Node`，
//! 与 `Delta` / `Tool` 事件的关系另按 `seq` 校验（G3）。

#![cfg(feature = "test-doubles")]

mod common;

use std::sync::Arc;

use assistant_agent::agent::node_event::{AgentNode, AgentNodeEvent, NodeState};
use assistant_agent::agent::round_executor::{
    ToolExecCtx, ToolExecResult, ToolExecutionPort, ToolStage,
};
use assistant_agent::agent::tool_call::FinalizedToolCall;
use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, InMemoryStore, RecordingSink, ScriptedAdapter, ScriptedResponse, ScriptedToolExec,
};
use assistant_agent::kernel::{
    SessionStates, TurnEvent, TurnRunner, TurnSpec, TurnStatus, TurnTasks,
};
use assistant_providers::base::BaseProviderAdapter;
use assistant_providers::types::ToolSpec;
use async_trait::async_trait;
use common::chat::{delta_finish, delta_tool_call_at, tool_spec};
use common::fixtures::fixture_input;
use common::harness::started_ctx;
use common::modes::mode_row;

const CONVERSATION_ID: &str = "conv-1";

// ---------------------------------------------------------------------------
// 装配
// ---------------------------------------------------------------------------

struct NodeHarness {
    runner: TurnRunner,
    sink: Arc<RecordingSink>,
    tasks: Arc<TurnTasks>,
}

/// agent 链路的完整装配：`Broadcast` 路由（否则不发节点事件）+ 注入 tool exec 端口。
///
/// 输入源必须带 agent mode，否则 `request::build` 不注入 tools，循环退化成单次调用。
fn agent_harness(
    scripts: Vec<ScriptedResponse>,
    tool_exec: Arc<dyn ToolExecutionPort>,
) -> NodeHarness {
    agent_harness_with_sink(scripts, tool_exec, Arc::new(RecordingSink::new()))
}

fn agent_harness_with_sink(
    scripts: Vec<ScriptedResponse>,
    tool_exec: Arc<dyn ToolExecutionPort>,
    sink: Arc<RecordingSink>,
) -> NodeHarness {
    let adapter: Arc<dyn BaseProviderAdapter> = Arc::new(ScriptedAdapter::new(scripts));
    let store: Arc<dyn assistant_agent::kernel::MessageStore> = Arc::new(InMemoryStore::new());
    let tasks = Arc::new(TurnTasks::new());
    let states = Arc::new(SessionStates::new());
    let source = fixture_input(CONVERSATION_ID, adapter).with_mode(mode_row("agent"));
    let runner = TurnRunner::new(
        sink.clone() as Arc<dyn assistant_agent::kernel::ports::EventSink>,
        store,
        Arc::new(source),
        states,
        tasks.clone(),
        Arc::new(FixedClock::default()),
        None,
    )
    .with_tool_exec(tool_exec);
    NodeHarness {
        runner,
        sink,
        tasks,
    }
}

fn echo_tool() -> ToolSpec {
    tool_spec("echo", "echo back", serde_json::json!({"type": "object"}))
}

/// 一次「调用工具」的模型响应：工具调用片段 + `finish_reason: tool_calls`。
fn tool_call_script(id: &str, name: &str) -> ScriptedResponse {
    ScriptedResponse::Deltas(vec![
        delta_tool_call_at(0, id, name, "{}"),
        delta_finish("tool_calls"),
    ])
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

fn scripted_tool_exec(results: Vec<ToolExecResult>) -> Arc<dyn ToolExecutionPort> {
    Arc::new(ScriptedToolExec::new(results))
}

/// 工具执行期间触发取消：先取消子令牌再永久挂起，让 `cancellable` 的 `biased`
/// 分支（令牌优先）稳定命中，落 `cancelled_tool_exec` 出口。
struct CancelOnExecToolExec;

#[async_trait]
impl ToolExecutionPort for CancelOnExecToolExec {
    async fn execute(
        &self,
        _calls: Vec<FinalizedToolCall>,
        ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult> {
        ctx.child_token.cancel();
        futures::future::pending::<()>().await;
        Vec::new()
    }
}

fn agent_spec(tools: Option<Vec<ToolSpec>>, max_iterations: u32) -> TurnSpec {
    let mut spec = TurnSpec::agent();
    spec.max_iterations = max_iterations;
    spec.limits.max_iterations = max_iterations;
    spec.tools = tools;
    spec
}

// ---------------------------------------------------------------------------
// 事件读取与断言工具
// ---------------------------------------------------------------------------

fn node_events(sink: &RecordingSink) -> Vec<AgentNodeEvent> {
    sink.events()
        .into_iter()
        .filter_map(|event| match event {
            TurnEvent::Node(event) => Some(event),
            _ => None,
        })
        .collect()
}

fn is_terminal(state: NodeState) -> bool {
    matches!(
        state,
        NodeState::Finished | NodeState::Error | NodeState::Skipped | NodeState::Cancelled
    )
}

/// 按 `(node, iteration)` 聚合（`AgentNode` 未实现 `Hash`，故用线性表）。
fn slot_counts(events: &[AgentNodeEvent]) -> Vec<(AgentNode, u32, usize, usize, usize)> {
    let mut slots: Vec<(AgentNode, u32, usize, usize, usize)> = Vec::new();
    for event in events {
        let index = match slots
            .iter()
            .position(|(node, iteration, ..)| *node == event.node && *iteration == event.iteration)
        {
            Some(index) => index,
            None => {
                slots.push((event.node, event.iteration, 0, 0, 0));
                slots.len() - 1
            }
        };
        let (_, _, running, terminal, facts) = &mut slots[index];
        if event.transition && event.state == NodeState::Running {
            *running += 1;
        } else if event.transition && is_terminal(event.state) {
            *terminal += 1;
        } else if !event.transition {
            *facts += 1;
        }
    }
    slots
}

/// O2 写侧：每个 `(node, iteration)` 恰好一条终态，且不得有悬空 `running`。
///
/// 唯一例外是 `persist-assistant`：一行 `finalize` 后只能新建行，同一迭代可能落库多次，
/// 因此按每次落库发一条 `finished`、**不重发 `running`**（I55 §4.6 第 4 条）。
fn assert_terminal_uniqueness(events: &[AgentNodeEvent]) {
    for (node, iteration, running, terminal, _) in slot_counts(events) {
        if node == AgentNode::PersistAssistant {
            assert!(
                terminal >= 1,
                "persist-assistant/{iteration} must persist at least one row"
            );
            continue;
        }
        assert_eq!(
            terminal, 1,
            "O2: {node:?}/{iteration} must have exactly one terminal, got {terminal}"
        );
        assert_eq!(
            running, 1,
            "I55 §4.6: {node:?}/{iteration} must emit exactly one transition:true running"
        );
    }
}

/// 逐槽位「终态之后不得再回 `running`」（`persist-assistant` 也不得例外）。
fn assert_no_running_after_terminal(events: &[AgentNodeEvent]) {
    let mut seen_terminal: Vec<(AgentNode, u32)> = Vec::new();
    for event in events {
        if !event.transition {
            continue;
        }
        if is_terminal(event.state) {
            if !seen_terminal
                .iter()
                .any(|(node, iteration)| *node == event.node && *iteration == event.iteration)
            {
                seen_terminal.push((event.node, event.iteration));
            }
            continue;
        }
        if event.state == NodeState::Running {
            assert!(
                !seen_terminal
                    .iter()
                    .any(|(node, iteration)| *node == event.node && *iteration == event.iteration),
                "O2: {:?}/{} re-entered running after a terminal state",
                event.node,
                event.iteration
            );
        }
    }
}

fn has_event(events: &[AgentNodeEvent], node: AgentNode, state: NodeState) -> bool {
    events
        .iter()
        .any(|event| event.node == node && event.state == state)
}

// ---------------------------------------------------------------------------
// G1：12 节点全覆盖
// ---------------------------------------------------------------------------

#[tokio::test]
async fn g1_all_twelve_nodes_finish_in_a_tool_turn() {
    let h = agent_harness(
        vec![
            tool_call_script("call-1", "echo"),
            ScriptedResponse::text(&["done"]),
        ],
        scripted_tool_exec(vec![ok_result("call-1", "echo", "ok")]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 2);

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);

    let events = node_events(&h.sink);
    let nodes = [
        AgentNode::Admit,
        AgentNode::Prepare,
        AgentNode::Brake,
        AgentNode::Ladder,
        AgentNode::Model,
        AgentNode::PersistAssistant,
        AgentNode::Branch,
        AgentNode::BudgetSplit,
        AgentNode::Pipeline,
        AgentNode::AfterTool,
        AgentNode::PersistTools,
        AgentNode::AfterIteration,
    ];
    for node in nodes {
        assert!(
            has_event(&events, node, NodeState::Finished),
            "G1: node {node:?} must emit at least one finished event; got {events:?}"
        );
    }
}

#[tokio::test]
async fn g1_no_tool_turn_never_reaches_the_tool_nodes() {
    let h = agent_harness(
        vec![ScriptedResponse::text(&["hello"])],
        scripted_tool_exec(Vec::new()),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 1);

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);

    let events = node_events(&h.sink);
    assert!(has_event(&events, AgentNode::Branch, NodeState::Finished));
    for node in [
        AgentNode::BudgetSplit,
        AgentNode::Pipeline,
        AgentNode::AfterTool,
        AgentNode::PersistTools,
        AgentNode::AfterIteration,
    ] {
        assert!(
            !events.iter().any(|event| event.node == node),
            "G1: {node:?} must stay silent when the branch has no tool calls \
             (skipped is derived by the frontend, I55 §4.6 第 2 条)"
        );
    }
}

// ---------------------------------------------------------------------------
// G3：seq 与 Delta 共用同一单调序列
// ---------------------------------------------------------------------------

fn seq_of(event: &TurnEvent) -> Option<u32> {
    match event {
        TurnEvent::Delta { seq, .. } => Some(*seq),
        TurnEvent::Node(AgentNodeEvent { seq, .. }) => Some(*seq),
        _ => None,
    }
}

#[tokio::test]
async fn g3_seq_is_strictly_monotonic_across_delta_and_node() {
    let h = agent_harness(
        vec![
            tool_call_script("call-1", "echo"),
            ScriptedResponse::text(&["done"]),
        ],
        scripted_tool_exec(vec![ok_result("call-1", "echo", "ok")]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 2);

    h.runner.run(&ctx, &spec).await;

    let events = h.sink.events();
    let seqs: Vec<u32> = events.iter().filter_map(seq_of).collect();
    assert!(
        seqs.len() > 20,
        "expected a dense event stream, got {seqs:?}"
    );
    assert_eq!(seqs[0], 1, "turn seq must start at 1");
    for pair in seqs.windows(2) {
        assert!(
            pair[1] > pair[0],
            "G3: seq must strictly increase with no duplicate and no rollback: {pair:?}"
        );
    }
    assert!(
        events
            .iter()
            .any(|event| matches!(event, TurnEvent::Delta { .. })),
        "the mixed stream must actually contain deltas"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, TurnEvent::Node(_))),
        "the mixed stream must actually contain node events"
    );
}

// ---------------------------------------------------------------------------
// G5：sink 失败不影响 turn 完成
// ---------------------------------------------------------------------------

#[tokio::test]
async fn g5_sink_failure_does_not_fail_the_turn() {
    let sink = Arc::new(RecordingSink::failing_after(1));
    let h = agent_harness_with_sink(
        vec![
            tool_call_script("call-1", "echo"),
            ScriptedResponse::text(&["done"]),
        ],
        scripted_tool_exec(vec![ok_result("call-1", "echo", "ok")]),
        sink.clone(),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 2);

    let outcome = h.runner.run(&ctx, &spec).await;

    assert_eq!(
        outcome.status,
        TurnStatus::Completed,
        "G5: a failing EventSink must never fail the turn"
    );
    assert!(
        sink.dropped_count() > 0,
        "the simulated failure must actually drop events, otherwise the case proves nothing"
    );
}

// ---------------------------------------------------------------------------
// G12：过程事实与状态迁移分离（写侧）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn g12_pipeline_running_is_single_per_iteration_even_with_a_concurrent_batch() {
    let h = agent_harness(
        vec![
            ScriptedResponse::Deltas(vec![
                delta_tool_call_at(0, "call-1", "echo", "{}"),
                delta_tool_call_at(1, "call-2", "echo", "{}"),
                delta_finish("tool_calls"),
            ]),
            ScriptedResponse::text(&["done"]),
        ],
        scripted_tool_exec(vec![
            ok_result("call-1", "echo", "a"),
            ok_result("call-2", "echo", "b"),
        ]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 2);

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);

    let events = node_events(&h.sink);
    let pipeline: Vec<&AgentNodeEvent> = events
        .iter()
        .filter(|event| event.node == AgentNode::Pipeline)
        .collect();
    let running: Vec<&&AgentNodeEvent> = pipeline
        .iter()
        .filter(|event| event.transition && event.state == NodeState::Running)
        .collect();
    assert_eq!(
        running.len(),
        1,
        "G12: exactly one transition:true pipeline running per (node, iteration), \
         even when the batch runs 2 calls concurrently: {pipeline:?}"
    );
    assert_eq!(running[0].iteration, 1);
    for event in &pipeline {
        assert!(
            event.transition,
            "G12: pipeline 的三态事件必须全部是状态迁移；过程事实走 transition:false: {event:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// O2：逐槽位终态唯一性
// ---------------------------------------------------------------------------

#[tokio::test]
async fn o2_every_slot_has_one_terminal_and_no_dangling_running() {
    let h = agent_harness(
        vec![
            tool_call_script("call-1", "echo"),
            tool_call_script("call-2", "echo"),
            ScriptedResponse::text(&["done"]),
        ],
        scripted_tool_exec(vec![
            ok_result("call-1", "echo", "a"),
            ok_result("call-2", "echo", "b"),
        ]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 3);

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);

    let events = node_events(&h.sink);
    assert_terminal_uniqueness(&events);
    assert_no_running_after_terminal(&events);
}

// ---------------------------------------------------------------------------
// 三条异常路径：收尾 / 取消 / 预算中止
// ---------------------------------------------------------------------------

/// `wrap_up` 路径的节点顺序是 ⑪→⑤→⑥（与模板顺序不同），但每个槽位仍须恰好一条终态。
#[tokio::test]
async fn wrap_up_path_terminates_model_persist_pipeline_and_tools() {
    let h = agent_harness(
        vec![
            tool_call_script("call-1", "echo"),
            ScriptedResponse::text(&["summary"]),
        ],
        scripted_tool_exec(vec![ok_result("call-1", "echo", "ok")]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    // 单迭代 + wrap_up：最后一轮仍发起工具调用 → 未执行的调用写结果行，再追加收尾调用。
    let mut spec = agent_spec(Some(vec![echo_tool()]), 1);
    spec.wrap_up = true;

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);
    assert!(outcome.wrap_up_injected);

    let events = node_events(&h.sink);
    for node in [
        AgentNode::Model,
        AgentNode::PersistAssistant,
        AgentNode::PersistTools,
    ] {
        assert!(
            events
                .iter()
                .any(|event| event.node == node && is_terminal(event.state)),
            "wrap_up: {node:?} must reach a terminal state"
        );
    }
    assert_no_running_after_terminal(&events);
    assert_terminal_uniqueness(&events);
}

#[tokio::test]
async fn cancelled_tool_exec_terminates_pipeline_and_persist_tools() {
    let h = agent_harness(
        vec![tool_call_script("call-1", "echo")],
        Arc::new(CancelOnExecToolExec),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    let spec = agent_spec(Some(vec![echo_tool()]), 3);

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Cancelled);

    let events = node_events(&h.sink);
    assert!(
        has_event(&events, AgentNode::Pipeline, NodeState::Cancelled),
        "cancelled_tool_exec 必须给 ⑨ 收口（否则留下悬空 running，违 O2）: {events:?}"
    );
    for node in [
        AgentNode::Model,
        AgentNode::PersistAssistant,
        AgentNode::PersistTools,
    ] {
        assert!(
            events
                .iter()
                .any(|event| event.node == node && is_terminal(event.state)),
            "cancel: {node:?} must reach a terminal state"
        );
    }
    assert_no_running_after_terminal(&events);
}

#[tokio::test]
async fn budget_halt_path_terminates_every_reached_node() {
    let h = agent_harness(
        vec![
            tool_call_script("call-1", "echo"),
            ScriptedResponse::text(&["summary"]),
        ],
        scripted_tool_exec(vec![ok_result("call-1", "echo", "ok")]),
    );
    let ctx = started_ctx(&h.tasks, CONVERSATION_ID);
    // 工具调用上限 1：第 1 迭代用完额度，第 2 迭代开头的 ③ 刹车命中 `tool-call-limit`。
    let mut spec = agent_spec(Some(vec![echo_tool()]), 3);
    spec.limits.max_tool_calls = 1;

    let outcome = h.runner.run(&ctx, &spec).await;
    assert_eq!(outcome.status, TurnStatus::Completed);

    let events = node_events(&h.sink);
    let brake_stop = events.iter().any(|event| {
        event.node == AgentNode::Brake
            && event.state == NodeState::Finished
            && matches!(
                &event.detail,
                Some(assistant_agent::agent::node_event::AgentNodeDetail::Brake(detail))
                    if detail.hook.as_deref() == Some("tool-call-limit")
            )
    });
    assert!(
        brake_stop,
        "预算中止必须由 ③ 的 finished + verdict:stop 表达（不是 error）: {events:?}"
    );
    for node in [
        AgentNode::Model,
        AgentNode::PersistAssistant,
        AgentNode::PersistTools,
        AgentNode::Pipeline,
    ] {
        assert!(
            events
                .iter()
                .any(|event| event.node == node && is_terminal(event.state)),
            "budget halt: {node:?} must reach a terminal state"
        );
    }
    assert_no_running_after_terminal(&events);
    assert_terminal_uniqueness(&events);
}
