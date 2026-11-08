//! 行级落库：建行 / 正文 / usage / tool_calls / 终态 / tool 结果行（I38b spec §5.1）。
//!
//! 落库只有两条路径：assistant 行走 `persist_row` / `persist_row_as_new`（`persist_final_text`
//! 只做择行），tool 行走 `append_tool_results`。代际过期时一律跳过并留日志，不报错。

use assistant_providers::types::{ChatMessage, MessageRole};

use crate::agent::round_executor::{ToolExecResult, TurnRunCtx};
use crate::agent::tool_call::FinalizedToolCall;
use crate::kernel::{AppendToolMessageReq, CreateAssistantReq, MessageStatus, StoreError};

use super::budget::tool_calls_json;
use super::RoundState;

fn is_current(ctx: &TurnRunCtx<'_>) -> bool {
    ctx.generation.is_current()
}

pub(super) async fn ensure_row(
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
    state.last_row_id = msg_id.clone();
    state.row_created = true;
    Ok(msg_id)
}

pub(super) async fn persist_row(
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

pub(super) async fn persist_row_as_new(
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
    state.last_row_id = msg_id.clone();
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
pub(super) async fn persist_final_text(
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

pub(super) async fn append_tool_results(
    state: &mut RoundState,
    ctx: &TurnRunCtx<'_>,
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
            parent_id: state.last_row_id.clone(),
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
                return Err(err);
            }
        }
    }
    Ok(())
}
