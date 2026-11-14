//! 工具批次执行与 `after_tool` 阶梯（I38b spec §5 第 6.5–7 步 / §5.3）。
//!
//! 只负责「预算裁剪后的批次执行」：调用 `ToolExecutionPort`、汇总确认等待、
//! 逐结果过 `after_tool` 中间件。结果行落库由编排层调用 `persist::append_tool_results`。

use crate::agent::middleware::{MiddlewareOutcome, ToolResultView, TurnCtx};
use crate::agent::round_executor::{
    RoundOutcome, ToolExecCtx, ToolExecResult, ToolExecutionPort, TurnRunCtx,
};
use crate::agent::tool_call::FinalizedToolCall;
use crate::kernel::TurnPhase;

use super::budget::{argument_fingerprint, cancellable, not_executed, split_by_budget};
use super::terminal::cancelled_tool_exec;
use super::RoundState;

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
    let (to_run, skipped) = split_by_budget(
        calls,
        remaining_tool_calls,
        remaining_wall_clock,
        ctx.limits.max_tool_calls,
    );

    // 7) 工具执行 + after_tool 阶梯。
    ctx.observer.set_phase(TurnPhase::ToolCalling);
    let mut halt_reason_from_tools: Option<String> = None;
    let tool_exec: Option<&dyn ToolExecutionPort> = ctx.tool_exec;
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
                observer: ctx.observer,
            };
            let meta: Vec<(String, String, String)> = to_run
                .iter()
                .map(|call| {
                    (
                        call.tool_id.clone(),
                        argument_fingerprint(call),
                        call.tool_name.clone(),
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
            for result in results.iter_mut() {
                let Some((_, fingerprint, tool_name)) =
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
                    scope: tool_name.as_str(),
                };
                if let MiddlewareOutcome::StopTurn { middleware, reason } =
                    state.chain.after_tool(&mut view, turn_ctx).await
                {
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
            results
        }
        Some(_) => Vec::new(),
        None => {
            log::warn!(
                "turn {} requested {} tool calls but no tool execution port is wired",
                ctx.turn_id,
                to_run.len()
            );
            not_executed(&to_run, "未执行：本链路没有工具执行端口")
        }
    };

    let mut all_results = executed;
    all_results.extend(skipped);
    ToolRunOutcome::Done {
        all_results,
        halt_reason: halt_reason_from_tools,
    }
}
