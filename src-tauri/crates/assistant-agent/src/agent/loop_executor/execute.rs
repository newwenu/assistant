//! 工具批次执行与 `after_tool` 阶梯（I38b spec §5 第 6.5–7 步 / §5.3）。
//!
//! 只负责「预算裁剪后的批次执行」：调用 `ToolExecutionPort`、汇总确认等待、
//! 逐结果过 `after_tool` 中间件。结果行落库由编排层调用 `persist::append_tool_results`。

use std::sync::Arc;

use crate::agent::middleware::{MiddlewareOutcome, ToolResultView, TurnCtx};
use crate::agent::node_event::{
    AfterToolDetail, AgentNode, AgentNodeDetail, BudgetSplitDetail, NodeState,
};
use crate::agent::round_executor::{
    EscalationSink, RoundOutcome, ToolExecCtx, ToolExecResult, ToolExecutionPort, TurnRunCtx,
};
use crate::agent::tool_call::FinalizedToolCall;
use crate::kernel::TurnPhase;

use super::budget::{argument_fingerprint, cancellable, not_executed, split_by_budget, tool_scope};
use super::terminal::cancelled_tool_exec;
use super::RoundState;

/// 单迭代的升级收集器（I58 §3.8 ②）：批次结束后由编排层 `drain` 并尾部追加。
#[derive(Default)]
pub(super) struct RoundEscalationSink {
    pending: std::sync::Mutex<Vec<assistant_providers::types::ToolSpec>>,
}

impl RoundEscalationSink {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<assistant_providers::types::ToolSpec>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl EscalationSink for RoundEscalationSink {
    fn push(&self, tool: assistant_providers::types::ToolSpec) {
        self.lock().push(tool);
    }

    fn drain(&self) -> Vec<assistant_providers::types::ToolSpec> {
        std::mem::take(&mut *self.lock())
    }
}

/// 尾部追加已登记的升级项（I58 §3.8 ④）：只增不减、不改顺序、下一次模型调用可见。
fn append_escalations(state: &mut RoundState, added: Vec<assistant_providers::types::ToolSpec>) {
    if added.is_empty() {
        return;
    }
    let mut merged: Vec<assistant_providers::types::ToolSpec> = state
        .tools
        .as_deref()
        .map(|tools| tools.as_slice().to_vec())
        .unwrap_or_default();
    // 同批次重复申请（或申请已在集中的项）只保留一次：provider 请求里同名工具重复
    // 无意义，且端口可能在申请期间已把它追加过。
    for tool in added {
        if merged.iter().any(|existing| existing.name == tool.name) {
            continue;
        }
        merged.push(tool);
    }
    state.request.options.tools = Some(merged.clone());
    state.tools = Some(Arc::new(merged));
}

pub(super) enum ToolRunOutcome {
    Done {
        all_results: Vec<ToolExecResult>,
        halt_reason: Option<String>,
    },
    /// 终态只每 turn 返回一次，装箱避免与 `Done` 的尺寸差触发 lint。
    Exit(Box<RoundOutcome>),
}

pub(super) async fn execute_tools(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    turn_ctx: &TurnCtx<'_>,
    calls: Vec<FinalizedToolCall>,
    it: u32,
    start_iteration: u32,
) -> ToolRunOutcome {
    // 6.5) 预算内裁剪：超额/超时的调用不执行，但仍写配对结果行（§5.3）。
    let remaining_tool_calls = ctx.limits.max_tool_calls.saturating_sub(state.tool_calls);
    let remaining_wall_clock = ctx
        .remaining_turn_budget
        .turn_deadline
        .map(|deadline| deadline.saturating_duration_since(ctx.clock.now()));
    let requested = calls.len();
    // ⑧ `budget-split`：`requested` 取本迭代声明数，`executed` / `deferred` 取裁剪结果。
    ctx.observer
        .node(AgentNode::BudgetSplit, it, NodeState::Running, true, None);
    let (to_run, skipped) = split_by_budget(
        calls,
        remaining_tool_calls,
        remaining_wall_clock,
        ctx.limits.max_tool_calls,
    );
    ctx.observer.node(
        AgentNode::BudgetSplit,
        it,
        NodeState::Finished,
        true,
        Some(AgentNodeDetail::BudgetSplit(BudgetSplitDetail {
            requested: requested as u32,
            executed: to_run.len() as u32,
            deferred: skipped.len() as u32,
        })),
    );

    // 7) 工具执行 + after_tool 阶梯。
    ctx.observer.set_phase(TurnPhase::ToolCalling);
    // ⑨ `pipeline` 的 `running`：每迭代仅一条（`transition = true`），子阶段事实随 V1-c 追加。
    ctx.observer
        .node(AgentNode::Pipeline, it, NodeState::Running, true, None);
    let mut halt_reason_from_tools: Option<String> = None;
    let tool_exec: Option<&dyn ToolExecutionPort> = ctx.tool_exec;
    // 本迭代的升级收集器与工具集快照：`Arc` 让适配器拿到的 `tools` 与追加前的
    // `request.options.tools` 同源，追加时统一重建（I58 §3.8 ③④）。
    let tools = state.tools.clone();
    let escalation = Arc::new(RoundEscalationSink::default());
    // 进执行队列前先把占位行推到 `running`（D27）：只有中断落在「已声明 → 已进入队列」
    // 之间，才能断言「未开始执行」并允许重试；写失败退化为「结果未知」，不阻断执行。
    if tool_exec.is_some() && !to_run.is_empty() {
        let call_ids: Vec<String> = to_run.iter().map(|call| call.tool_id.clone()).collect();
        if let Err(err) = ctx
            .store
            .mark_tool_calls_running(ctx.conversation_id, &call_ids)
            .await
        {
            log::warn!("mark tool_calls running failed: {err}");
        }
    }
    let executed: Vec<ToolExecResult> = match tool_exec {
        Some(port) if !to_run.is_empty() => {
            let exec_ctx = ToolExecCtx {
                turn_id: ctx.turn_id,
                conversation_id: ctx.conversation_id,
                iteration: it,
                child_token: ctx.child_token,
                clock: ctx.clock,
                // 本轮工具集原样透传：`request_tool` 的清单以它为准（I58 §3.1）。
                tools: tools.as_deref().map(|tools| tools.as_slice()),
                observer: ctx.observer,
                escalations: Some(escalation.clone()),
            };
            let meta: Vec<(String, String, String)> = to_run
                .iter()
                .map(|call| {
                    (
                        call.tool_id.clone(),
                        argument_fingerprint(call),
                        tool_scope(call),
                    )
                })
                .collect();
            let executed =
                cancellable(ctx.child_token, port.execute(to_run.clone(), &exec_ctx)).await;
            let Some(mut results) = executed else {
                return ToolRunOutcome::Exit(Box::new(
                    cancelled_tool_exec(state, ctx, to_run, skipped, start_iteration).await,
                ));
            };
            state.confirmation_wait_ms += results
                .iter()
                .filter_map(|r| r.confirmation_wait_ms)
                .sum::<u64>();
            // ⑩ `after-tool`：只在真有结果可过中间件时才起 `running`（取消路径不留悬空）。
            ctx.observer
                .node(AgentNode::AfterTool, it, NodeState::Running, true, None);
            let mut verdicts = Vec::new();
            for result in results.iter_mut() {
                let Some((_, fingerprint, scope)) =
                    meta.iter().find(|(id, _, _)| id == &result.call_id)
                else {
                    log::warn!(
                        "tool result {} has no matching call in this iteration",
                        result.call_id
                    );
                    continue;
                };
                let mut view = ToolResultView {
                    result,
                    arguments: fingerprint.as_str(),
                    argument_fingerprint: fingerprint.as_str(),
                    scope: scope.as_str(),
                };
                let (outcome, mut per_result) = state.chain.after_tool(&mut view, turn_ctx).await;
                verdicts.append(&mut per_result);
                if let MiddlewareOutcome::StopTurn { middleware, reason } = outcome {
                    log::warn!(
                        target: "agent::loop",
                        "中间件 {} 在 after_tool 请求终止本轮：{}",
                        middleware,
                        reason
                    );
                    if halt_reason_from_tools.is_none() {
                        halt_reason_from_tools = Some(reason);
                    }
                }
            }
            ctx.observer.node(
                AgentNode::AfterTool,
                it,
                NodeState::Finished,
                true,
                Some(AgentNodeDetail::AfterTool(AfterToolDetail { verdicts })),
            );
            results
        }
        Some(_) => {
            // 全部调用被预算裁掉：⑩ 无结果可处理，仍发一对空事件，避免留悬空格子（O2）。
            empty_after_tool(ctx, it);
            Vec::new()
        }
        None => {
            log::warn!(
                "turn {} requested {} tool calls but no tool execution port is wired",
                ctx.turn_id,
                to_run.len()
            );
            empty_after_tool(ctx, it);
            not_executed(&to_run, "未执行：本链路没有工具执行端口")
        }
    };

    // 本批次执行完毕后取出已登记的升级项并尾部追加（I58 §3.8 ④）：
    // 空集不重建请求，保证「没有申请」的迭代与升级通道存在前逐字节一致。
    append_escalations(state, escalation.drain());

    let mut all_results = executed;
    all_results.extend(skipped);
    ctx.observer
        .node(AgentNode::Pipeline, it, NodeState::Finished, true, None);
    ToolRunOutcome::Done {
        all_results,
        halt_reason: halt_reason_from_tools,
    }
}

/// 无结果可过中间件时 ⑩ 的空事件对（`verdicts` 为空）。
fn empty_after_tool(ctx: &TurnRunCtx<'_>, it: u32) {
    ctx.observer
        .node(AgentNode::AfterTool, it, NodeState::Running, true, None);
    ctx.observer.node(
        AgentNode::AfterTool,
        it,
        NodeState::Finished,
        true,
        Some(AgentNodeDetail::AfterTool(AfterToolDetail {
            verdicts: Vec::new(),
        })),
    );
}
