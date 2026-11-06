//! P4 多迭代闭环执行器（I38b spec §5 / impl-guide 附录 A）。
//!
//! 一次 `run_iterations` 走完一个 turn 的全部迭代：投影一次 → 迭代循环
//! （三维刹车 → before_model → 模型调用（空响应内层重试）→ 行级落库 → 无工具结束
//! → 收尾 / 预算裁剪 → 工具执行 → after_tool / after_iteration → nudge 传递）→ 循环兜底。
//!
//! 按职责分文件（拆分判据见 docs/store/agent-code-inventory.md §4.3）：
//!
//! - 本文件：`RoundState`（单 turn 可变状态与 `RoundOutcome` 构造点）、`run()` 阶段编排、执行器入口；
//! - `call`：单次模型调用与内层重试（空响应 / provider 失败，不递增 iteration）；
//! - `execute`：预算裁剪后的工具批次执行与 `after_tool` 阶梯；
//! - `budget`：无状态辅助（usage 合并、预算切分、结果 / 指令消息构造、指纹与 JSON）；
//! - `terminal`：终态出口（失败 / 取消 / 停止 / 预算终止 / 最后一轮收尾）与收尾调用；
//! - `persist`：行级落库（建行 / 正文 / usage / tool_calls / 终态 / tool 结果行）。
//!
//! 落库只有两条路径：assistant 行走 `persist_row` / `persist_row_as_new`（`persist_final_text`
//! 只做择行），tool 行走 `append_tool_results`（§5.1）。
//!
//! 长 await 一律 `cancellable`：与 `TurnRunner` 的取消语义一致（`biased` 优先响应取消）。

mod budget;
mod call;
mod execute;
mod persist;
mod terminal;

use std::sync::Arc;

use assistant_providers::base::{ProviderError, ProviderErrorPayload};
use async_trait::async_trait;

use crate::agent::middleware::{MiddlewareChain, MiddlewareOutcome, ModelRequestView, TurnCtx};
use crate::agent::round_executor::{RoundExecutor, RoundOutcome, TurnRunCtx};
use crate::agent::stop::stop_hooks_all_continue;
use crate::agent::tool_call::{ToolCallAccumulator, ToolCallBoundary};
use crate::kernel::request::{self, ChatRequest};
use crate::kernel::stream::UsageSnapshot;
use crate::kernel::{
    MessageStatus, StopHook, StoreError, TurnError, TurnPhase, TurnState, TurnStatus,
};

use self::budget::{cancellable, decide_stop_phase, nudge_message};
use self::call::{call_model_with_retries, CallOutcome};
use self::execute::{execute_tools, ToolRunOutcome};
use self::persist::{append_tool_results, persist_final_text, persist_row};
use self::terminal::{
    budget_summary_reason, halted_by_budget, halted_with_summary, projection_exit, stopped,
    wrap_up_on_last_iteration,
};

/// 收尾指令：只追加到本次请求，不落库（D19 同口径）。
pub const WRAP_UP_INSTRUCTION: &str = "预算已用尽，请基于已有结果给出结论，不要再调用工具。";

/// 本 turn 的全部可变量。三个终态出口与 `call_model_without_tools` 都要读写它，
/// 因此收敛成一个结构体而非散落的局部变量（impl-guide 附录 A 末注）。
struct RoundState {
    request: ChatRequest,
    last_request: ChatRequest,
    msg_id: String,
    row_created: bool,
    row_finalized: bool,
    last_row_id: String,
    chain: MiddlewareChain,
    pending_nudges: Vec<String>,
    previous_content: String,
    model_calls: u32,
    tool_calls: u32,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    cache_read_tokens: Option<u32>,
    total_tokens: Option<u32>,
    attempt_usages: Vec<UsageSnapshot>,
    persisted_usage_count: usize,
    ttft_ms: Option<u64>,
    confirmation_wait_ms: u64,
    final_message_id: Option<String>,
    iteration: u32,
    content: String,
    reasoning: String,
    provider_error: Option<ProviderErrorPayload>,
}

impl RoundState {
    fn new(request: ChatRequest, initial_message_id: &str, chain: MiddlewareChain) -> Self {
        Self {
            last_request: request.clone(),
            msg_id: initial_message_id.to_string(),
            last_row_id: initial_message_id.to_string(),
            request,
            row_created: false,
            row_finalized: false,
            chain,
            pending_nudges: Vec::new(),
            previous_content: String::new(),
            model_calls: 0,
            tool_calls: 0,
            prompt_tokens: None,
            completion_tokens: None,
            cache_read_tokens: None,
            total_tokens: None,
            attempt_usages: Vec::new(),
            persisted_usage_count: 0,
            ttft_ms: None,
            confirmation_wait_ms: 0,
            final_message_id: None,
            iteration: 0,
            content: String::new(),
            reasoning: String::new(),
            provider_error: None,
        }
    }

    /// 每迭代重置（§5 / C4.40a）。
    fn reset_iteration_flags(&mut self) {
        self.row_created = false;
        self.row_finalized = false;
    }

    /// `None` 表示全部钩子 `Continue`；否则携带钩子名与原因。
    #[allow(clippy::too_many_arguments)]
    fn brake(
        &self,
        hooks: &[Arc<dyn StopHook>],
        max_iterations: u32,
        max_tool_calls: u32,
        elapsed: std::time::Duration,
        turn_deadline: Option<std::time::Duration>,
        model: &str,
    ) -> Option<(&'static str, String)> {
        stop_hooks_all_continue(
            hooks,
            &TurnState {
                iteration: self.iteration,
                max_iterations,
                tool_calls: self.tool_calls,
                max_tool_calls,
                prompt_tokens: self.prompt_tokens.unwrap_or(0),
                completion_tokens: self.completion_tokens.unwrap_or(0),
                elapsed,
                turn_deadline,
                model,
            },
        )
    }

    /// 统一的 `RoundOutcome` 构造点：禁止各出口各自拼字段（C3.26）。
    fn finish(
        &self,
        status: TurnStatus,
        text: String,
        reasoning: String,
        hit_cap: bool,
        wrap_up_injected: bool,
        error: Option<TurnError>,
    ) -> RoundOutcome {
        RoundOutcome {
            status,
            text,
            reasoning,
            iterations: self.iteration,
            model_calls: self.model_calls,
            tool_calls: self.tool_calls,
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            cache_read_tokens: self.cache_read_tokens,
            total_tokens: self.total_tokens,
            attempt_usages: self.attempt_usages.clone(),
            ttft_ms: self.ttft_ms,
            confirmation_wait_ms: self.confirmation_wait_ms,
            hit_cap,
            wrap_up_injected,
            final_message_id: self.final_message_id.clone(),
            context_window_tokens: self.last_request.context_window_tokens,
            reserved_output_tokens: self.last_request.options.max_tokens,
            error,
            provider_error: self.provider_error.clone(),
        }
    }

    /// 带落库失败标记的 `finish`：DB 写失败不得吞掉（C4.22c）。
    fn finish_with_persist_error(
        &self,
        base: RoundOutcome,
        persist: Result<(), StoreError>,
    ) -> RoundOutcome {
        match persist {
            Ok(()) => base,
            Err(err) => {
                log::error!("final persist failed: {err}");
                let mut outcome = base;
                outcome.error = Some(TurnError::Persist(err.to_string()));
                outcome
            }
        }
    }
}

#[async_trait]
impl RoundExecutor for LoopRoundExecutor {
    async fn run_iterations(&self, ctx: TurnRunCtx<'_>) -> RoundOutcome {
        run(ctx).await
    }
}

/// P4 的生产执行器；无自身状态，全部状态落在 `RoundState`。
pub struct LoopRoundExecutor;

pub(crate) async fn run(ctx: TurnRunCtx<'_>) -> RoundOutcome {
    let initial_message_id = ctx.initial_message_id;
    let start_iteration = ctx.start_iteration;
    let hooks: &[Arc<dyn StopHook>] = ctx.spec.stop_hooks.as_slice();
    let max_iterations = ctx.spec.max_iterations;
    let limits = ctx.limits;

    // 0) 请求历史只投影一次；读库同属长 await，取消优先（C4.23）。
    let request = match cancellable(
        ctx.child_token,
        request::build(ctx.input, start_iteration, ctx.tools),
    )
    .await
    {
        None => {
            return projection_exit(
                TurnStatus::Cancelled,
                None,
                Some(ProviderErrorPayload::from(&ProviderError::Cancelled)),
            )
        }
        Some(Ok(request)) => request,
        Some(Err(err)) => {
            return projection_exit(
                TurnStatus::Failed,
                Some(TurnError::Other(err.clone())),
                Some(ProviderErrorPayload::from(&ProviderError::RequestRejected(
                    err,
                ))),
            )
        }
    };

    let chain = if ctx.spec.ladder.is_projection_only() {
        MiddlewareChain::projection_only()
    } else {
        MiddlewareChain::p4(limits.microcompact_keep_recent)
    };
    let mut state = RoundState::new(request, initial_message_id, chain);

    for it in start_iteration..=max_iterations {
        state.iteration = it;
        let elapsed = ctx
            .clock
            .now()
            .saturating_duration_since(ctx.turn_started_at);
        let brake = state.brake(
            hooks,
            max_iterations,
            limits.max_tool_calls,
            elapsed,
            limits.turn_wall_clock,
            ctx.model,
        );
        decide_stop_phase(&brake, "迭代开头");
        if let Some((hook_name, reason)) = brake {
            return halted_by_budget(&mut state, &ctx, hook_name, &reason, start_iteration).await;
        }

        state.reset_iteration_flags();
        let mut accumulator = ToolCallAccumulator::new();
        let turn_ctx = TurnCtx {
            turn_id: ctx.turn_id,
            conversation_id: ctx.conversation_id,
            iteration: it,
            max_iterations,
            model: ctx.model,
        };

        // 2) before_model：阶梯与中间件只改本次请求的消息副本。
        ctx.observer.set_phase(TurnPhase::Thinking);
        let (window, max_tokens) = {
            let mut view = ModelRequestView {
                messages: &mut state.request.chat_messages,
                context_window_tokens: state.request.context_window_tokens,
                max_tokens: state.request.options.max_tokens,
                is_wrap_up_call: false,
            };
            if let MiddlewareOutcome::StopTurn { middleware, reason } =
                state.chain.before_model(&mut view, &turn_ctx).await
            {
                log::warn!(
                    target: "agent::loop",
                    "中间件 {} 在 before_model 结束本轮：{}",
                    middleware,
                    reason
                );
                return stopped(&mut state, &ctx, reason, start_iteration).await;
            }
            (view.context_window_tokens, view.max_tokens)
        };
        state.request.context_window_tokens = window;
        state.request.options.max_tokens = max_tokens;

        // 2.5) nudge 注入：只进本次请求，不落库（D19）。
        for nudge in state.pending_nudges.drain(..) {
            state.request.chat_messages.push(nudge_message(&nudge));
        }

        // 2.6) 本次请求快照（tools 已在第 0 步注入，此处只 clone）。
        let request_with_tools = state.request.clone();

        // 3+4) 单次模型调用与内层重试（不递增 iteration，D22）；终态由该函数直接交回。
        if let CallOutcome::Exit(outcome) = call_model_with_retries(
            &mut state,
            &ctx,
            &request_with_tools,
            &mut accumulator,
            it,
            start_iteration,
        )
        .await
        {
            return *outcome;
        }

        let calls = accumulator.finalize_all(ToolCallBoundary::StreamEnd);
        let has_effective_output = !state.content.is_empty() || !calls.is_empty();

        // 4.1) 重试用尽仍空 → Failed（首迭代占位行以空内容收尾，不加新行）。
        if !has_effective_output {
            let persist = if it == start_iteration {
                persist_row(
                    &mut state,
                    &ctx,
                    String::new(),
                    String::new(),
                    &[],
                    MessageStatus::Failed,
                    start_iteration,
                )
                .await
            } else {
                Ok(())
            };
            let error = TurnError::EmptyResponse;
            state.provider_error = Some(ProviderErrorPayload::from(&ProviderError::ParseError(
                "empty response".to_string(),
            )));
            let outcome = state.finish(
                TurnStatus::Failed,
                String::new(),
                String::new(),
                false,
                false,
                Some(error),
            );
            return state.finish_with_persist_error(outcome, persist);
        }

        // 4.5) 落库本次调用：建行 → 正文/推理 → usage → tool_calls → finalize。
        let content = state.content.clone();
        let reasoning = state.reasoning.clone();
        let persist = persist_row(
            &mut state,
            &ctx,
            content,
            reasoning,
            &calls,
            MessageStatus::Completed,
            start_iteration,
        )
        .await;
        if let Err(err) = &persist {
            log::error!("assistant row persist failed: {err}");
        }
        state.previous_content = state.content.clone();

        // 5) 无工具 → 结束。
        if calls.is_empty() {
            let outcome = state.finish(
                TurnStatus::Completed,
                state.content.clone(),
                state.reasoning.clone(),
                false,
                false,
                None,
            );
            return state.finish_with_persist_error(outcome, persist);
        }

        let n = calls.len() as u32;

        // 6) 最后一轮：收尾（仅 spec.wrap_up）。
        if ctx.spec.wrap_up && it == max_iterations {
            return wrap_up_on_last_iteration(&mut state, &ctx, &turn_ctx, calls, start_iteration)
                .await;
        }

        // 6.5 + 7) 预算内裁剪 → 工具执行 → after_tool 阶梯（取消路径直接交回终态）。
        let (all_results, halt_reason_from_tools) =
            match execute_tools(&mut state, &ctx, &turn_ctx, calls, it, start_iteration).await {
                ToolRunOutcome::Exit(outcome) => return *outcome,
                ToolRunOutcome::Done {
                    all_results,
                    halt_reason,
                } => (all_results, halt_reason),
            };

        let msg_id = state.msg_id.clone();
        let append = append_tool_results(&mut state, &ctx, msg_id, all_results).await;
        state.tool_calls += n;
        let _ = &append;

        // 8) 迭代后钩子：nudge 只进下一轮，不落库。
        state
            .pending_nudges
            .extend(state.chain.take_pending_nudges());
        let halt_reason = match halt_reason_from_tools {
            Some(reason) => Some(reason),
            None => match state.chain.after_iteration(&turn_ctx).await {
                MiddlewareOutcome::StopTurn { middleware, reason } => {
                    log::warn!(
                        target: "agent::loop",
                        "中间件 {} 在 after_iteration 请求终止本轮：{}",
                        middleware,
                        reason
                    );
                    Some(reason)
                }
                _ => None,
            },
        };
        if let Some(reason) = halt_reason {
            return halted_with_summary(&mut state, &ctx, reason, append, start_iteration).await;
        }
        let _ = append;
    }

    // 循环正常走完：wrap_up = false 且最后一轮仍发起了工具调用。
    let summary = if state.content.is_empty() {
        budget_summary_reason("iteration-limit")
    } else {
        state.content.clone()
    };
    let persist = persist_final_text(
        &mut state,
        &ctx,
        summary.clone(),
        MessageStatus::Completed,
        start_iteration,
    )
    .await;
    let outcome = state.finish(
        TurnStatus::Completed,
        summary,
        if state.content.is_empty() {
            String::new()
        } else {
            state.reasoning.clone()
        },
        true,
        false,
        None,
    );
    state.finish_with_persist_error(outcome, persist)
}
