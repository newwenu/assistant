//! 单次模型调用与内层重试（I38b spec §5 第 3–4 步 / D22）。
//!
//! 内层 loop 只处理「空响应重试」与「provider 失败重试」，不递增 `iteration`；
//! 任一终态（取消 / 超时 / provider 失败 / 请求构造失败）都以 `CallOutcome::Exit`
//! 直接把 `RoundOutcome` 交回编排层，避免在编排层重复终态判断。

use assistant_providers::base::{ChatDelta, ProviderErrorPayload};

use crate::agent::node_event::{AgentNode, AgentNodeDetail, ModelDetail, NodeState};
use crate::agent::round_executor::{RoundOutcome, ToolErrorClass, TurnRunCtx};
use crate::agent::tool_call::{DiscardedToolCall, ToolCallAccumulator};
use crate::agent::tool_event::{ToolEvent, ToolEventPayload};
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

/// 重试前为被丢弃的调用补发终态。
///
/// `ArgsDelta` 已向外部声明过身份，重试只是丢弃本次 attempt 的参数累积，不会回滚这些声明；
/// 不补终态会让前端留下永远停在 `preparing` 的卡片（I60 N21 / B15）。
fn emit_discarded_calls(ctx: &TurnRunCtx<'_>, it: u32, discarded: Vec<DiscardedToolCall>) {
    for call in discarded {
        ctx.observer.emit_tool_event(ToolEvent {
            turn_id: ctx.turn_id.to_string(),
            iteration: it,
            call_id: call.tool_id,
            name: call.tool_name,
            payload: ToolEventPayload::Failed {
                error: "已取消".to_string(),
                error_kind: ToolErrorClass::Cancelled,
                confirmation_wait_ms: None,
                compact: None,
            },
        });
    }
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
    // ⑤ `model` 的 `running` 每迭代只发一次（I55 §4.6 第 3 条）；重试走 `waiting`。
    ctx.observer.node(
        AgentNode::Model,
        it,
        NodeState::Running,
        true,
        Some(AgentNodeDetail::Model(ModelDetail {
            attempt: retries_used + 1,
            retry_reason: None,
            prompt_tokens: state.prompt_tokens,
            completion_tokens: state.completion_tokens,
            finish_reason: None,
            tool_call_count: 0,
        })),
    );

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
                ctx.observer.node(
                    AgentNode::Model,
                    it,
                    NodeState::Error,
                    true,
                    Some(super::model_detail(state, Some(message.clone()), None, 0)),
                );
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
                let outcome = accumulator.apply_chat_delta(delta);
                for chunk in [outcome.opened, outcome.params_partial]
                    .into_iter()
                    .flatten()
                {
                    ctx.observer.emit_tool_event(ToolEvent {
                        turn_id: ctx.turn_id.to_string(),
                        iteration: it,
                        call_id: chunk.tool_id,
                        name: chunk.tool_name,
                        payload: ToolEventPayload::ArgsDelta {
                            delta: chunk.params_chunk,
                        },
                    });
                }
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
                    state.model_retries_used = retries_used;
                    emit_discarded_calls(ctx, it, accumulator.discard_pending());
                    log::warn!(
                        "[agent-loop] provider 失败重试 {}/{}：turn={} iteration={} err={}",
                        retries_used,
                        ctx.limits.empty_response_retries,
                        ctx.turn_id,
                        it,
                        message
                    );
                    // 重试等待：`waiting` → 回到 `running`（I55 §5.2 第 3 条，禁新增事件）。
                    ctx.observer.node(
                        AgentNode::Model,
                        it,
                        NodeState::Waiting,
                        true,
                        Some(super::model_detail(state, Some(message.clone()), None, 0)),
                    );
                    ctx.observer.node(
                        AgentNode::Model,
                        it,
                        NodeState::Running,
                        true,
                        Some(super::model_detail(state, None, None, 0)),
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
        state.finish_reason = finish_reason.clone();
        // 4) 空响应重试：纯工具调用是有效输出；输出上限截断不重发。
        if !outcome.has_effective_output
            && !accumulator.has_pending()
            && !stream::is_output_limit_reached(finish_reason.as_deref())
            && retries_used < ctx.limits.empty_response_retries
        {
            retries_used += 1;
            state.model_retries_used = retries_used;
            emit_discarded_calls(ctx, it, accumulator.discard_pending());
            log::info!(
                "[agent-loop] 空响应重试 {}/{}：turn={} iteration={}",
                retries_used,
                ctx.limits.empty_response_retries,
                ctx.turn_id,
                it
            );
            ctx.observer.node(
                AgentNode::Model,
                it,
                NodeState::Waiting,
                true,
                Some(super::model_detail(
                    state,
                    Some("empty-response".to_string()),
                    None,
                    0,
                )),
            );
            ctx.observer.node(
                AgentNode::Model,
                it,
                NodeState::Running,
                true,
                Some(super::model_detail(state, None, None, 0)),
            );
            continue;
        }
        return CallOutcome::Done;
    }
}
