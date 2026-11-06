//! P4 多迭代闭环执行器（I38b spec §5 / impl-guide 附录 A）。
//!
//! 一次 `run_iterations` 走完一个 turn 的全部迭代：投影一次 → 迭代循环
//! （三维刹车 → before_model → 模型调用（空响应内层重试）→ 行级落库 → 无工具结束
//! → 收尾 / 预算裁剪 → 工具执行 → after_tool / after_iteration → nudge 传递）→ 循环兜底。
//!
//! 落库只有两条路径：assistant 行走 `persist_row` / `persist_row_as_new`（`persist_final_text`
//! 只做择行），tool 行走 `append_tool_results`（§5.1）。
//!
//! 长 await 一律 `cancellable`：与 `TurnRunner` 的取消语义一致（`biased` 优先响应取消）。

use std::sync::Arc;

use assistant_providers::base::{ChatDelta, ProviderError, ProviderErrorPayload};
use assistant_providers::types::{ChatMessage, MessageRole, ToolCall};
use async_trait::async_trait;

use crate::agent::middleware::{
    MiddlewareChain, MiddlewareOutcome, ModelRequestView, ToolResultView, TurnCtx,
};
use crate::agent::round_executor::{
    RoundExecutor, RoundOutcome, ToolErrorClass, ToolExecCtx, ToolExecResult, ToolExecutionPort,
    ToolStage, TurnRunCtx,
};
use crate::agent::stop::stop_hooks_all_continue;
use crate::agent::tool_call::{FinalizedToolCall, ToolCallAccumulator, ToolCallBoundary};
use crate::kernel::request::{self, ChatRequest};
use crate::kernel::stream::{self, PartialOutcome, StreamError, StreamOutcome, UsageSnapshot};
use crate::kernel::{
    AppendToolMessageReq, CreateAssistantReq, MessageStatus, StopHook, StoreError, TurnError,
    TurnPhase, TurnState, TurnStatus,
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

fn merge_usage(
    prompt: &mut Option<u32>,
    completion: &mut Option<u32>,
    cache_read: &mut Option<u32>,
    total: &mut Option<u32>,
    usage: &UsageSnapshot,
) {
    if usage.prompt_tokens.is_some() {
        *prompt = usage.prompt_tokens;
    }
    if usage.completion_tokens.is_some() {
        *completion = usage.completion_tokens;
    }
    if usage.cache_read_tokens.is_some() {
        *cache_read = usage.cache_read_tokens;
    }
    if usage.total_tokens.is_some() {
        *total = usage.total_tokens;
    }
}

/// 仅存在于内存请求的指令消息（nudge / 收尾指令）：不写库、不带 `tool_call_id`。
fn nudge_message(text: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::User,
        content: text.to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: false,
        thinking_signature: None,
        content_blocks: None,
    }
}

/// 钩子名 → 用户可读的根因摘要；文本必须非空（§5.2）。
fn budget_summary_reason(hook_name: &str) -> String {
    match hook_name {
        "iteration-limit" => "已达迭代上限，本轮已停止。".to_string(),
        "tool-call-limit" => "已达工具调用上限，本轮已停止。".to_string(),
        "turn-wall-clock" => "本轮已超时，已停止。".to_string(),
        "token-budget" => "本轮 token 预算已用尽，已停止。".to_string(),
        _ => "本轮已停止：预算已用尽。".to_string(),
    }
}

/// 无进展出口的用户可见文本：`reason` 已是中间件给出的中文完整句，原样使用。
fn progress_summary_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        "本轮已停止：工具调用未取得进展。".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 第 0 步投影阶段（`RoundState` 尚未建立）的唯一出口：只有 `status` / `error` /
/// `provider_error` 有效，其余计数恒为 0。它是 `RoundState::finish` 之外的唯一前置例外（C3.26）。
fn projection_exit(
    status: TurnStatus,
    error: Option<TurnError>,
    provider_error: Option<ProviderErrorPayload>,
) -> RoundOutcome {
    RoundOutcome {
        status,
        error,
        provider_error,
        ..RoundOutcome::default()
    }
}

fn not_executed(calls: &[FinalizedToolCall], content: &str) -> Vec<ToolExecResult> {
    calls
        .iter()
        .map(|call| ToolExecResult {
            call_id: call.tool_id.clone(),
            tool_name: call.tool_name.clone(),
            content: content.to_string(),
            is_error: true,
            stage: ToolStage::Gateway,
            error_kind: Some(ToolErrorClass::Denied),
            confirmation_wait_ms: None,
        })
        .collect()
}

/// 取消批次的结果形状（§4.4）：`error_kind = Cancelled`、content 写「已取消」，不当作失败重试。
fn cancelled_results(calls: &[FinalizedToolCall]) -> Vec<ToolExecResult> {
    calls
        .iter()
        .map(|call| ToolExecResult {
            call_id: call.tool_id.clone(),
            tool_name: call.tool_name.clone(),
            content: "已取消".to_string(),
            is_error: true,
            stage: ToolStage::Gateway,
            error_kind: Some(ToolErrorClass::Cancelled),
            confirmation_wait_ms: None,
        })
        .collect()
}

fn skipped_tool_limit(max_tool_calls: u32) -> String {
    format!(
        "未执行：本轮工具调用预算已用尽（max_tool_calls = {}）",
        max_tool_calls
    )
}

const SKIPPED_WALL_CLOCK: &str = "未执行：本轮已超时";

/// 按原始顺序切分：前 `remaining_calls` 个执行，其余回填未执行结果（§5.3）。
fn split_by_budget(
    calls: Vec<FinalizedToolCall>,
    remaining_calls: u32,
    remaining_clock: Option<std::time::Duration>,
    max_tool_calls: u32,
) -> (Vec<FinalizedToolCall>, Vec<ToolExecResult>) {
    if remaining_clock.is_some_and(|d| d.is_zero()) {
        let skipped = not_executed(&calls, SKIPPED_WALL_CLOCK);
        return (Vec::new(), skipped);
    }
    let allowed = remaining_calls as usize;
    if calls.len() <= allowed {
        return (calls, Vec::new());
    }
    let mut keep = calls;
    let tail = keep.split_off(allowed);
    let skipped = not_executed(&tail, &skipped_tool_limit(max_tool_calls));
    (keep, skipped)
}

/// 参数指纹：原始 arguments 文本；解析失败按 `{}` 归一（D19 同参判定）。
fn argument_fingerprint(call: &FinalizedToolCall) -> String {
    if call.is_error {
        "{}".to_string()
    } else {
        call.raw_arguments.clone()
    }
}

fn tool_calls_json(calls: &[FinalizedToolCall]) -> String {
    let payload: Vec<ToolCall> = calls
        .iter()
        .map(|call| ToolCall {
            id: call.tool_id.clone(),
            name: call.tool_name.clone(),
            arguments: call.raw_arguments.clone(),
        })
        .collect();
    serde_json::to_string(&payload).unwrap_or_else(|err| {
        log::error!("serialize tool_calls failed: {err}");
        "[]".to_string()
    })
}

fn effective_model_call_budget(ctx: &TurnRunCtx<'_>) -> Option<std::time::Duration> {
    let model_call = ctx.limits.model_call.filter(|d| !d.is_zero());
    let remaining = ctx
        .limits
        .turn_wall_clock
        .filter(|d| !d.is_zero())
        .map(|limit| {
            limit.saturating_sub(
                ctx.clock
                    .now()
                    .saturating_duration_since(ctx.turn_started_at),
            )
        });
    match (model_call, remaining) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

async fn cancellable<F, T>(token: &tokio_util::sync::CancellationToken, fut: F) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => None,
        value = fut => Some(value),
    }
}

struct WrapUpOutcome {
    text: String,
    reasoning: String,
}

fn decide_stop_phase(reason: &Option<(&'static str, String)>, phase: &str) {
    if let Some((hook, why)) = reason {
        log::warn!(
            target: "agent::loop",
            "三维刹车在{}触发：{} ({})",
            phase,
            hook,
            why
        );
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
    let tool_exec: Option<&dyn ToolExecutionPort> = ctx.tool_exec;

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
        let mut retries_used = 0u32;
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

        // 3) 单次模型调用；空响应重试用内层 loop（不递增 iteration，D22）。
        ctx.observer.set_phase(TurnPhase::Streaming);
        state.content.clear();
        state.reasoning.clear();

        loop {
            if ctx.child_token.is_cancelled() {
                return cancelled_before_call(&mut state, &ctx, start_iteration).await;
            }
            state.last_request = request_with_tools.clone();
            let request_started = ctx.clock.now();
            let model_call_budget = effective_model_call_budget(&ctx);
            let child = crate::kernel::TurnTasks::child_token(ctx.child_token);
            let stream = match request_with_tools.stream(child.clone()) {
                Ok(stream) => stream,
                Err(err) => {
                    let message = err.to_string();
                    state.provider_error = Some(ProviderErrorPayload::from(&err));
                    state.model_calls += 1;
                    let content = state.content.clone();
                    let reasoning = state.reasoning.clone();
                    let persist = persist_row(
                        &mut state,
                        &ctx,
                        content,
                        reasoning,
                        &[],
                        MessageStatus::Failed,
                        start_iteration,
                    )
                    .await;
                    let outcome = state.finish(
                        TurnStatus::Failed,
                        state.content.clone(),
                        state.reasoning.clone(),
                        false,
                        false,
                        Some(TurnError::Provider(message)),
                    );
                    return state.finish_with_persist_error(outcome, persist);
                }
            };
            let result = stream::consume(
                stream,
                child,
                model_call_budget,
                ctx.clock,
                request_started,
                |_seq, delta: &ChatDelta| {
                    accumulator.apply_chat_delta(delta);
                    ctx.observer.delta(delta);
                },
            )
            .await;
            state.model_calls += 1;

            match &result {
                Ok(outcome) => {
                    merge_usage(
                        &mut state.prompt_tokens,
                        &mut state.completion_tokens,
                        &mut state.cache_read_tokens,
                        &mut state.total_tokens,
                        &outcome.last_usage(),
                    );
                    state.attempt_usages.extend(outcome.attempt_usages.clone());
                    if state.ttft_ms.is_none() {
                        state.ttft_ms = outcome.ttft_ms;
                    }
                }
                Err(StreamError::Cancelled(partial)) => {
                    apply_partial(&mut state, partial);
                    return stream_failure(
                        &mut state,
                        &ctx,
                        StreamFailure::Cancelled,
                        start_iteration,
                    )
                    .await;
                }
                Err(StreamError::Timeout(partial)) => {
                    apply_partial(&mut state, partial);
                    return stream_failure(
                        &mut state,
                        &ctx,
                        StreamFailure::Timeout,
                        start_iteration,
                    )
                    .await;
                }
                Err(StreamError::Provider(err, partial)) => {
                    apply_partial(&mut state, partial);
                    let message = err.to_string();
                    if retries_used < limits.empty_response_retries {
                        retries_used += 1;
                        accumulator.reset();
                        log::warn!(
                            "[agent-loop] provider 失败重试 {}/{}：turn={} iteration={} err={}",
                            retries_used,
                            limits.empty_response_retries,
                            ctx.turn_id,
                            it,
                            message
                        );
                        continue;
                    }
                    return stream_failure(
                        &mut state,
                        &ctx,
                        StreamFailure::Provider(message, ProviderErrorPayload::from(err)),
                        start_iteration,
                    )
                    .await;
                }
            }
            let outcome: StreamOutcome = result.unwrap_or_default();
            state.content = outcome.full_content.clone();
            state.reasoning = outcome.full_reasoning.clone();
            let finish_reason = outcome.finish_reason.clone();
            // 4) 空响应重试：纯工具调用是有效输出；输出上限截断不重发。
            if !outcome.has_effective_output
                && !accumulator.has_pending()
                && !stream::is_output_limit_reached(finish_reason.as_deref())
                && retries_used < limits.empty_response_retries
            {
                retries_used += 1;
                accumulator.reset();
                log::info!(
                    "[agent-loop] 空响应重试 {}/{}：turn={} iteration={}",
                    retries_used,
                    limits.empty_response_retries,
                    ctx.turn_id,
                    it
                );
                continue;
            }
            break;
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
            state.tool_calls += n;
            let skipped = not_executed(&calls, "未执行：已达迭代上限");
            let msg_id = state.msg_id.clone();
            let append = append_tool_results(&mut state, &ctx, msg_id, skipped).await;
            let wrapped = call_model_without_tools(&mut state, &ctx, &turn_ctx).await;
            let (base, persist) = match wrapped {
                Ok(wrapped) => {
                    let persist = persist_final_text(
                        &mut state,
                        &ctx,
                        wrapped.text.clone(),
                        MessageStatus::Completed,
                        start_iteration,
                    )
                    .await;
                    let outcome = state.finish(
                        TurnStatus::Completed,
                        wrapped.text,
                        wrapped.reasoning,
                        true,
                        true,
                        None,
                    );
                    (outcome, persist)
                }
                Err(err) => {
                    let summary = budget_summary_reason("iteration-limit");
                    let persist = persist_final_text(
                        &mut state,
                        &ctx,
                        summary.clone(),
                        MessageStatus::Failed,
                        start_iteration,
                    )
                    .await;
                    let outcome = state.finish(
                        TurnStatus::Failed,
                        summary,
                        String::new(),
                        true,
                        true,
                        Some(err),
                    );
                    (outcome, persist)
                }
            };
            let persist = match (persist, append) {
                (Err(err), _) => Err(err),
                (Ok(()), Err(err)) => Err(err),
                _ => Ok(()),
            };
            return state.finish_with_persist_error(base, persist);
        }

        // 6.5) 预算内裁剪：超额/超时的调用不执行，但仍写配对结果行（§5.3）。
        let remaining_tool_calls = limits.max_tool_calls.saturating_sub(state.tool_calls);
        let remaining_wall_clock = ctx
            .remaining_turn_budget
            .turn_deadline
            .map(|deadline| deadline.saturating_duration_since(ctx.clock.now()));
        let (to_run, skipped) = split_by_budget(
            calls,
            remaining_tool_calls,
            remaining_wall_clock,
            limits.max_tool_calls,
        );

        // 7) 工具执行 + after_tool 阶梯。
        ctx.observer.set_phase(TurnPhase::ToolCalling);
        let mut halt_reason_from_tools: Option<String> = None;
        let executed: Vec<ToolExecResult> = match tool_exec {
            Some(port) if !to_run.is_empty() => {
                let exec_ctx = ToolExecCtx {
                    turn_id: ctx.turn_id,
                    conversation_id: ctx.conversation_id,
                    iteration: it,
                    child_token: ctx.child_token,
                    tools: ctx.tools,
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
                    return cancelled_tool_exec(&mut state, &ctx, to_run, skipped, start_iteration)
                        .await;
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
                        state.chain.after_tool(&mut view, &turn_ctx).await
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

fn apply_partial(state: &mut RoundState, partial: &PartialOutcome) {
    merge_usage(
        &mut state.prompt_tokens,
        &mut state.completion_tokens,
        &mut state.cache_read_tokens,
        &mut state.total_tokens,
        &partial.last_usage(),
    );
    state.attempt_usages.extend(partial.attempt_usages.clone());
    if state.ttft_ms.is_none() {
        state.ttft_ms = partial.ttft_ms;
    }
    state.content = partial.full_content.clone();
    state.reasoning = partial.full_reasoning.clone();
}

enum StreamFailure {
    Cancelled,
    Timeout,
    Provider(String, ProviderErrorPayload),
}

async fn stream_failure(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    failure: StreamFailure,
    start_iteration: u32,
) -> RoundOutcome {
    let (status, message_status, error, provider_error) = match failure {
        StreamFailure::Cancelled => (
            TurnStatus::Cancelled,
            MessageStatus::Interrupted,
            None,
            ProviderErrorPayload::from(&ProviderError::Cancelled),
        ),
        StreamFailure::Timeout => (
            TurnStatus::Failed,
            MessageStatus::Failed,
            Some(TurnError::Timeout),
            ProviderErrorPayload::from(&ProviderError::Timeout("Stream timed out".to_string())),
        ),
        StreamFailure::Provider(message, payload) => (
            TurnStatus::Failed,
            MessageStatus::Failed,
            Some(TurnError::Provider(message)),
            payload,
        ),
    };
    state.provider_error = Some(provider_error);
    let persist = persist_row(
        state,
        ctx,
        state.content.clone(),
        state.reasoning.clone(),
        &[],
        message_status,
        start_iteration,
    )
    .await;
    let outcome = state.finish(
        status,
        state.content.clone(),
        state.reasoning.clone(),
        false,
        false,
        error,
    );
    state.finish_with_persist_error(outcome, persist)
}

async fn cancelled_before_call(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    start_iteration: u32,
) -> RoundOutcome {
    let text = state.content.clone();
    let reasoning = state.reasoning.clone();
    let persist = persist_final_text(
        state,
        ctx,
        text.clone(),
        MessageStatus::Interrupted,
        start_iteration,
    )
    .await;
    let outcome = state.finish(TurnStatus::Cancelled, text, reasoning, false, false, None);
    state.finish_with_persist_error(outcome, persist)
}

async fn cancelled_tool_exec(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    to_run: Vec<FinalizedToolCall>,
    skipped: Vec<ToolExecResult>,
    start_iteration: u32,
) -> RoundOutcome {
    let n = (to_run.len() + skipped.len()) as u32;
    let mut all_results = cancelled_results(&to_run);
    all_results.extend(skipped);
    let msg_id = state.msg_id.clone();
    let append = append_tool_results(state, ctx, msg_id, all_results).await;
    state.tool_calls += n;
    let persist = match append {
        Err(err) => Err(err),
        Ok(()) => {
            persist_row(
                state,
                ctx,
                state.content.clone(),
                state.reasoning.clone(),
                &[],
                MessageStatus::Interrupted,
                start_iteration,
            )
            .await
        }
    };
    let outcome = state.finish(
        TurnStatus::Cancelled,
        state.content.clone(),
        state.reasoning.clone(),
        false,
        false,
        None,
    );
    state.finish_with_persist_error(outcome, persist)
}

async fn stopped(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    reason: String,
    start_iteration: u32,
) -> RoundOutcome {
    let text = if state.previous_content.is_empty() {
        progress_summary_reason(&reason)
    } else {
        state.previous_content.clone()
    };
    let persist = persist_final_text(
        state,
        ctx,
        text.clone(),
        MessageStatus::Completed,
        start_iteration,
    )
    .await;
    let outcome = state.finish(
        TurnStatus::Completed,
        text.clone(),
        String::new(),
        false,
        false,
        Some(TurnError::MiddlewareStop {
            name: "before_model".to_string(),
            reason,
        }),
    );
    state.finish_with_persist_error(outcome, persist)
}

async fn halted_with_summary(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    reason: String,
    prior: Result<(), StoreError>,
    start_iteration: u32,
) -> RoundOutcome {
    let summary = progress_summary_reason(&reason);
    let persist = persist_final_text(
        state,
        ctx,
        summary.clone(),
        MessageStatus::Failed,
        start_iteration,
    )
    .await;
    let persist = match (persist, prior) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        _ => Ok(()),
    };
    let outcome = state.finish(
        TurnStatus::Failed,
        summary,
        String::new(),
        false,
        false,
        Some(TurnError::NoProgress { reason }),
    );
    state.finish_with_persist_error(outcome, persist)
}

async fn halted_by_budget(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    hook_name: &str,
    _reason: &str,
    start_iteration: u32,
) -> RoundOutcome {
    if ctx.spec.wrap_up {
        let turn_ctx = TurnCtx {
            turn_id: ctx.turn_id,
            conversation_id: ctx.conversation_id,
            iteration: state.iteration,
            max_iterations: ctx.spec.max_iterations,
            model: ctx.model,
        };
        let wrapped = call_model_without_tools(state, ctx, &turn_ctx).await;
        return match wrapped {
            Ok(wrapped) => {
                let persist = persist_final_text(
                    state,
                    ctx,
                    wrapped.text.clone(),
                    MessageStatus::Completed,
                    start_iteration,
                )
                .await;
                let outcome = state.finish(
                    TurnStatus::Completed,
                    wrapped.text,
                    wrapped.reasoning,
                    true,
                    true,
                    None,
                );
                state.finish_with_persist_error(outcome, persist)
            }
            Err(err) => {
                let summary = budget_summary_reason(hook_name);
                let persist = persist_final_text(
                    state,
                    ctx,
                    summary.clone(),
                    MessageStatus::Failed,
                    start_iteration,
                )
                .await;
                let outcome = state.finish(
                    TurnStatus::Failed,
                    summary,
                    String::new(),
                    true,
                    true,
                    Some(err),
                );
                state.finish_with_persist_error(outcome, persist)
            }
        };
    }
    let summary = budget_summary_reason(hook_name);
    let persist = persist_final_text(
        state,
        ctx,
        summary.clone(),
        MessageStatus::Completed,
        start_iteration,
    )
    .await;
    let outcome = state.finish(
        TurnStatus::Completed,
        summary,
        String::new(),
        true,
        false,
        None,
    );
    state.finish_with_persist_error(outcome, persist)
}

/// 收尾调用：撤 tools + 追加收尾指令 + `is_wrap_up_call = true`（触发 ④ 还原）。
async fn call_model_without_tools(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    turn_ctx: &TurnCtx<'_>,
) -> Result<WrapUpOutcome, TurnError> {
    let mut request = state.request.clone();
    request.options.tools = None;
    request
        .chat_messages
        .push(nudge_message(WRAP_UP_INSTRUCTION));
    let (window, max_tokens) = {
        let mut view = ModelRequestView {
            messages: &mut request.chat_messages,
            context_window_tokens: request.context_window_tokens,
            max_tokens: request.options.max_tokens,
            is_wrap_up_call: true,
        };
        if let MiddlewareOutcome::StopTurn { middleware, reason } =
            state.chain.before_model(&mut view, turn_ctx).await
        {
            log::warn!(
                target: "agent::loop",
                "中间件 {} 在收尾调用前终止：{}",
                middleware,
                reason
            );
            return Err(TurnError::MiddlewareStop {
                name: middleware.to_string(),
                reason,
            });
        }
        (view.context_window_tokens, view.max_tokens)
    };
    request.context_window_tokens = window;
    request.options.max_tokens = max_tokens;
    state.last_request = request.clone();

    let request_started = ctx.clock.now();
    let model_call_budget = effective_model_call_budget(ctx);
    let child = crate::kernel::TurnTasks::child_token(ctx.child_token);
    let stream = request.stream(child.clone()).map_err(|err| {
        state.provider_error = Some(ProviderErrorPayload::from(&err));
        TurnError::Provider(err.to_string())
    })?;
    let result = stream::consume(
        stream,
        child,
        model_call_budget,
        ctx.clock,
        request_started,
        |_seq, delta| {
            ctx.observer.delta(delta);
        },
    )
    .await;
    state.model_calls += 1;
    match result {
        Ok(outcome) => {
            merge_usage(
                &mut state.prompt_tokens,
                &mut state.completion_tokens,
                &mut state.cache_read_tokens,
                &mut state.total_tokens,
                &outcome.last_usage(),
            );
            state.attempt_usages.extend(outcome.attempt_usages.clone());
            if state.ttft_ms.is_none() {
                state.ttft_ms = outcome.ttft_ms;
            }
            Ok(WrapUpOutcome {
                text: outcome.full_content.clone(),
                reasoning: outcome.full_reasoning.clone(),
            })
        }
        Err(StreamError::Timeout(partial)) => {
            apply_partial(state, &partial);
            state.provider_error = Some(ProviderErrorPayload::from(&ProviderError::Timeout(
                "Stream timed out".to_string(),
            )));
            Err(TurnError::Timeout)
        }
        Err(StreamError::Cancelled(partial)) => {
            apply_partial(state, &partial);
            state.provider_error = Some(ProviderErrorPayload::from(&ProviderError::Cancelled));
            Err(TurnError::MiddlewareStop {
                name: "wrap-up".to_string(),
                reason: "本轮已取消".to_string(),
            })
        }
        Err(StreamError::Provider(err, partial)) => {
            apply_partial(state, &partial);
            state.provider_error = Some(ProviderErrorPayload::from(&err));
            Err(TurnError::Provider(err.to_string()))
        }
    }
}

fn is_current(ctx: &TurnRunCtx<'_>) -> bool {
    ctx.generation.is_current()
}

async fn ensure_row(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    start_iteration: u32,
) -> Result<String, StoreError> {
    if state.iteration <= start_iteration || state.row_created {
        return Ok(state.msg_id.clone());
    }
    let req = CreateAssistantReq {
        conversation_id: ctx.conversation_id.to_string(),
        parent_id: Some(state.last_row_id.clone()),
        provider_id: ctx.provider_id.map(str::to_string),
        model_id: ctx.model_id.map(str::to_string),
    };
    let msg_id = ctx.store.create_assistant_message(req).await?;
    state.msg_id = msg_id.clone();
    state.row_created = true;
    Ok(msg_id)
}

async fn persist_row(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    content: String,
    reasoning: String,
    calls: &[FinalizedToolCall],
    status: MessageStatus,
    start_iteration: u32,
) -> Result<(), StoreError> {
    if !is_current(ctx) {
        log::warn!(
            "persist_row skipped: turn={} is not the current generation",
            ctx.turn_id
        );
        return Ok(());
    }
    let msg_id = ensure_row(state, ctx, start_iteration).await?;
    let tokens = state.total_tokens;
    let reasoning = (!reasoning.is_empty()).then_some(reasoning.as_str());
    if let Err(err) = ctx
        .store
        .save_content(&msg_id, &content, tokens, reasoning)
        .await
    {
        log::error!("save assistant content failed: {err}");
        return Err(err);
    }

    let pending_usages = &state.attempt_usages[state.persisted_usage_count..];
    let usage = crate::kernel::PersistCtx {
        conversation_id: ctx.conversation_id.to_string(),
        turn_id: ctx.turn_id.to_string(),
        message_id: msg_id.clone(),
        branch_leaf_id: ctx.branch_leaf_id.map(str::to_string),
        kind: ctx.spec.kind.to_string(),
        provider_id: ctx.provider_id.map(str::to_string),
        model_id: ctx.model_id.map(str::to_string),
        cache_capable: ctx.cache_capable,
        created_at_ms: ctx.clock.now_millis(),
    }
    .usage_records(pending_usages);
    if !usage.is_empty() {
        if let Err(err) = ctx.store.record_usage(&usage).await {
            log::error!("record assistant usage failed: {err}");
            return Err(err);
        }
        state.persisted_usage_count = state.attempt_usages.len();
    }

    if !calls.is_empty() {
        let json = tool_calls_json(calls);
        if let Err(err) = ctx.store.save_tool_calls(&msg_id, &json).await {
            log::error!("save tool_calls failed: {err}");
            return Err(err);
        }
    }

    if let Err(err) = ctx.store.finalize(&msg_id, status).await {
        log::error!("finalize assistant row failed: {err}");
        return Err(err);
    }
    state.final_message_id = Some(msg_id);
    state.row_finalized = true;
    Ok(())
}

async fn persist_row_as_new(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    content: String,
    status: MessageStatus,
) -> Result<(), StoreError> {
    if !is_current(ctx) {
        log::warn!(
            "persist_row_as_new skipped: turn={} is not the current generation",
            ctx.turn_id
        );
        return Ok(());
    }
    let req = CreateAssistantReq {
        conversation_id: ctx.conversation_id.to_string(),
        parent_id: Some(state.last_row_id.clone()),
        provider_id: ctx.provider_id.map(str::to_string),
        model_id: ctx.model_id.map(str::to_string),
    };
    let msg_id = ctx.store.create_assistant_message(req).await?;
    state.msg_id = msg_id.clone();
    state.row_created = true;
    state.row_finalized = false;
    let reasoning = String::new();
    let calls: Vec<FinalizedToolCall> = Vec::new();
    persist_row(
        state,
        ctx,
        content,
        reasoning,
        &calls,
        status,
        state.iteration,
    )
    .await
}

/// 择行写入：本迭代行已 `finalize` 时只能新建，保证不覆盖也不悬空。
async fn persist_final_text(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    text: String,
    status: MessageStatus,
    start_iteration: u32,
) -> Result<(), StoreError> {
    if state.row_finalized {
        persist_row_as_new(state, ctx, text, status).await
    } else {
        let reasoning = state.reasoning.clone();
        let calls: Vec<FinalizedToolCall> = Vec::new();
        persist_row(state, ctx, text, reasoning, &calls, status, start_iteration).await
    }
}

async fn append_tool_results(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    parent_id: String,
    results: Vec<ToolExecResult>,
) -> Result<(), StoreError> {
    if results.is_empty() {
        return Ok(());
    }
    if !is_current(ctx) {
        log::warn!(
            "append_tool_results skipped: turn={} is not the current generation",
            ctx.turn_id
        );
        return Ok(());
    }
    for result in &results {
        state.request.chat_messages.push(ChatMessage {
            role: MessageRole::Tool,
            content: result.content.clone(),
            tool_calls: None,
            tool_call_id: Some(result.call_id.clone()),
            name: Some(result.tool_name.clone()),
            is_error: result.is_error,
            thinking_signature: None,
            content_blocks: None,
        });
        let req = AppendToolMessageReq {
            conversation_id: ctx.conversation_id.to_string(),
            parent_id: parent_id.clone(),
            tool_call_id: result.call_id.clone(),
            content: result.content.clone(),
            is_error: result.is_error,
            timestamp: ctx.clock.now_millis(),
        };
        let row_id = ctx.store.append_tool_message(req).await;
        match row_id {
            Ok(id) => state.last_row_id = id,
            Err(err) => {
                log::error!("append tool row failed: {err}");
                state.last_row_id = parent_id.clone();
                return Err(err);
            }
        }
    }
    Ok(())
}
