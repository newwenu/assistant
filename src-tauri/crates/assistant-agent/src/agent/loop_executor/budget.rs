//! 循环内的无状态辅助函数：usage 合并、预算切分、结果与指令消息构造、指纹与 JSON。
//!
//! 这里只放「不触发 IO、不改变编排顺序」的纯函数；有状态的编排在 `super::run`。

use assistant_providers::types::{ChatMessage, MessageRole, ToolCall};

use crate::agent::round_executor::{ToolErrorClass, ToolExecResult, ToolStage, TurnRunCtx};
use crate::agent::tool_call::FinalizedToolCall;
use crate::kernel::stream::{PartialOutcome, UsageSnapshot};

use super::RoundState;

pub(super) fn merge_usage(
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

/// 把流式中断时的部分结果并入 `RoundState`（终态出口与收尾调用共用）。
pub(super) fn apply_partial(state: &mut RoundState, partial: &PartialOutcome) {
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

/// 仅存在于内存请求的指令消息（nudge / 收尾指令）：不写库、不带 `tool_call_id`。
pub(super) fn nudge_message(text: &str) -> ChatMessage {
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

pub(super) fn not_executed(calls: &[FinalizedToolCall], content: &str) -> Vec<ToolExecResult> {
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

fn skipped_tool_limit(max_tool_calls: u32) -> String {
    format!(
        "未执行：本轮工具调用预算已用尽（max_tool_calls = {}）",
        max_tool_calls
    )
}

const SKIPPED_WALL_CLOCK: &str = "未执行：本轮已超时";

/// 按原始顺序切分：前 `remaining_calls` 个执行，其余回填未执行结果（§5.3）。
pub(super) fn split_by_budget(
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
pub(super) fn argument_fingerprint(call: &FinalizedToolCall) -> String {
    if call.is_error {
        "{}".to_string()
    } else {
        call.raw_arguments.clone()
    }
}

pub(super) fn tool_calls_json(calls: &[FinalizedToolCall]) -> String {
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

pub(super) fn effective_model_call_budget(ctx: &TurnRunCtx<'_>) -> Option<std::time::Duration> {
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

pub(super) async fn cancellable<F, T>(
    token: &tokio_util::sync::CancellationToken,
    fut: F,
) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => None,
        value = fut => Some(value),
    }
}

pub(super) fn decide_stop_phase(reason: &Option<(&'static str, String)>, phase: &str) {
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
