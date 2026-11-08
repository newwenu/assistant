//! 终态出口与收尾调用（I38b spec §5.2 / §5.3）。
//!
//! 每个出口都走 `RoundState::finish`（唯一 `RoundOutcome` 构造点）并统一处理落库失败；
//! `projection_exit` 是第 0 步投影阶段的前置例外（`RoundState` 尚未建立）。

use assistant_providers::base::{ProviderError, ProviderErrorPayload};

use crate::agent::middleware::{MiddlewareOutcome, ModelRequestView, TurnCtx};
use crate::agent::round_executor::{
    RoundOutcome, ToolErrorClass, ToolExecResult, ToolStage, TurnRunCtx,
};
use crate::agent::tool_call::FinalizedToolCall;
use crate::kernel::stream::{self, StreamError};
use crate::kernel::{MessageStatus, StoreError, TurnError, TurnStatus};

use super::budget::{
    apply_partial, effective_model_call_budget, merge_usage, not_executed, nudge_message,
};
use super::persist::{append_tool_results, persist_final_text, persist_row};
use super::{RoundState, WRAP_UP_INSTRUCTION};

/// 取消批次的结果形状（§4.4）：`error_kind = Cancelled`、content 写「已取消」，不当作失败重试。
pub(super) fn cancelled_results(calls: &[FinalizedToolCall]) -> Vec<ToolExecResult> {
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

/// 钩子名 → 用户可读的根因摘要；文本必须非空（§5.2）。
pub(super) fn budget_summary_reason(hook_name: &str) -> String {
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
pub(super) fn projection_exit(
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

pub(super) enum StreamFailure {
    Cancelled,
    Timeout,
    Provider(String, ProviderErrorPayload),
}

pub(super) async fn stream_failure(
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

pub(super) async fn cancelled_before_call(
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

pub(super) async fn cancelled_tool_exec(
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

pub(super) async fn stopped(
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

pub(super) async fn halted_with_summary(
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

pub(super) async fn halted_by_budget(
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

/// 最后一轮仍发起工具调用时的收尾出口（仅 `spec.wrap_up`）：
/// 未执行的调用写「已达迭代上限」结果行，再追加一次不带工具的收尾调用。
pub(super) async fn wrap_up_on_last_iteration(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
    turn_ctx: &TurnCtx<'_>,
    calls: Vec<FinalizedToolCall>,
    start_iteration: u32,
) -> RoundOutcome {
    state.tool_calls += calls.len() as u32;
    let skipped = not_executed(&calls, "未执行：已达迭代上限");
    let msg_id = state.msg_id.clone();
    let append = append_tool_results(state, ctx, msg_id, skipped).await;
    let wrapped = call_model_without_tools(state, ctx, turn_ctx).await;
    let (base, persist) = match wrapped {
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
            (outcome, persist)
        }
        Err(err) => {
            let summary = budget_summary_reason("iteration-limit");
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
            (outcome, persist)
        }
    };
    let persist = match (persist, append) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        _ => Ok(()),
    };
    state.finish_with_persist_error(base, persist)
}

pub(super) struct WrapUpOutcome {
    pub(super) text: String,
    pub(super) reasoning: String,
}

/// 收尾调用：撤 tools + 追加收尾指令 + `is_wrap_up_call = true`（触发 ④ 还原）。
pub(super) async fn call_model_without_tools(
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
