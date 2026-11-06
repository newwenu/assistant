//! 单次模型调用与内层重试（I38b spec §5 第 3–4 步 / D22）。
//!
//! 内层 loop 只处理「空响应重试」与「provider 失败重试」，不递增 `iteration`；
//! 任一终态（取消 / 超时 / provider 失败 / 请求构造失败）都以 `CallOutcome::Exit`
//! 直接把 `RoundOutcome` 交回编排层，避免在编排层重复终态判断。

use assistant_providers::base::{ChatDelta, ProviderErrorPayload};

use crate::agent::round_executor::{RoundOutcome, TurnRunCtx};
use crate::agent::tool_call::ToolCallAccumulator;
use crate::kernel::request::ChatRequest;
use crate::kernel::stream::{self, StreamError, StreamOutcome};
use crate::kernel::{MessageStatus, TurnError, TurnPhase, TurnStatus};

use super::budget::{apply_partial, effective_model_call_budget, merge_usage};
use super::persist::persist_row;
use super::terminal::{cancelled_before_call, stream_failure, StreamFailure};
use super::RoundState;

pub(super) enum CallOutcome {
    Done,
    /// 终态只每 turn 返回一次，装箱避免与 `Done` 的尺寸差触发 lint。
    Exit(Box<RoundOutcome>),
}

pub(super) async fn call_model_with_retries(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    request_with_tools: &ChatRequest,
    accumulator: &mut ToolCallAccumulator,
    it: u32,
    start_iteration: u32,
) -> CallOutcome {
    ctx.observer.set_phase(TurnPhase::Streaming);
    state.content.clear();
    state.reasoning.clear();
    let mut retries_used = 0u32;

    loop {
        if ctx.child_token.is_cancelled() {
            return CallOutcome::Exit(Box::new(
                cancelled_before_call(state, ctx, start_iteration).await,
            ));
        }
        state.last_request = request_with_tools.clone();
        let request_started = ctx.clock.now();
        let model_call_budget = effective_model_call_budget(ctx);
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
                    state,
                    ctx,
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
                return CallOutcome::Exit(Box::new(
                    state.finish_with_persist_error(outcome, persist),
                ));
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
                apply_partial(state, partial);
                return CallOutcome::Exit(Box::new(
                    stream_failure(state, ctx, StreamFailure::Cancelled, start_iteration).await,
                ));
            }
            Err(StreamError::Timeout(partial)) => {
                apply_partial(state, partial);
                return CallOutcome::Exit(Box::new(
                    stream_failure(state, ctx, StreamFailure::Timeout, start_iteration).await,
                ));
            }
            Err(StreamError::Provider(err, partial)) => {
                apply_partial(state, partial);
                let message = err.to_string();
                if retries_used < ctx.limits.empty_response_retries {
                    retries_used += 1;
                    accumulator.reset();
                    log::warn!(
                        "[agent-loop] provider 失败重试 {}/{}：turn={} iteration={} err={}",
                        retries_used,
                        ctx.limits.empty_response_retries,
                        ctx.turn_id,
                        it,
                        message
                    );
                    continue;
                }
                return CallOutcome::Exit(Box::new(
                    stream_failure(
                        state,
                        ctx,
                        StreamFailure::Provider(message, ProviderErrorPayload::from(err)),
                        start_iteration,
                    )
                    .await,
                ));
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
            && retries_used < ctx.limits.empty_response_retries
        {
            retries_used += 1;
            accumulator.reset();
            log::info!(
                "[agent-loop] 空响应重试 {}/{}：turn={} iteration={}",
                retries_used,
                ctx.limits.empty_response_retries,
                ctx.turn_id,
                it
            );
            continue;
        }
        return CallOutcome::Done;
    }
}
