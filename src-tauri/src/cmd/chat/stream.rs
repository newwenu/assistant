use log::{error, info};

use tauri::{AppHandle, Emitter, Manager};

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ChatDeltaPayloadDto, ConversationCompactFailedPayloadDto, ProviderErrorPayloadDto,
};
use assistant_agent::chat_tasks::ChatTasks;
use assistant_agent::compaction;
use assistant_providers::base::{self, ProviderErrorPayload};
use assistant_storage::{
    conversations::{self},
    providers::{self as provider_store},
};

pub(crate) struct StreamEmitCtx<'a> {
    pub orb_window: &'a tauri::WebviewWindow,
    pub conv_id: &'a str,
    pub attempt_id: &'a str,
    pub msg_id: &'a str,
}

pub(crate) fn emit_delta(ectx: &StreamEmitCtx, seq: u32, delta: &base::ChatDelta) {
    if let Some(ref retry) = delta.delta_retry {
        info!(
            "[chat-delta] retry notification: attempt={}/{} delay={}ms message={}",
            retry.attempt, retry.max_attempts, retry.delay_ms, retry.message
        );
    }
    if let Some(ref usage) = delta.usage {
        info!(
            "[chat-delta] seq={} usage: prompt={} completion={} total={} ctx_window={}",
            seq,
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            usage.context_window_tokens
        );
    }
    let payload = ChatDeltaPayloadDto {
        conversation_id: ectx.conv_id.to_string(),
        attempt_id: ectx.attempt_id.to_string(),
        message_id: ectx.msg_id.to_string(),
        seq,
        delta_text: delta.delta_text.clone(),
        delta_reasoning: delta.delta_reasoning.clone(),
        delta_tool_call_id: delta.delta_tool_call_id.clone(),
        delta_tool_call_name: delta.delta_tool_call_name.clone(),
        delta_tool_call_arguments: delta.delta_tool_call_arguments.clone(),
        finish_reason: delta.finish_reason.clone(),
        usage: delta.usage.clone().map(Into::into),
        delta_retry: delta.delta_retry.clone().map(Into::into),
        ..Default::default()
    };
    if let Err(emit_err) = ectx.orb_window.emit(event_name::CHAT_DELTA, payload) {
        error!("Failed to emit chat-delta: {}", emit_err);
    }
}

pub(crate) fn emit_stream_error(ectx: &StreamEmitCtx, last_seq: u32, error: &ProviderErrorPayload) {
    let payload = ChatDeltaPayloadDto {
        conversation_id: ectx.conv_id.to_string(),
        attempt_id: ectx.attempt_id.to_string(),
        message_id: ectx.msg_id.to_string(),
        seq: last_seq,
        error: Some(ProviderErrorPayloadDto::from(error.clone())),
        ..Default::default()
    };
    if let Err(emit_err) = ectx.orb_window.emit(event_name::CHAT_DELTA, payload) {
        error!("Failed to emit stream error event: {}", emit_err);
    }
}

pub(crate) struct DbWriteMsg {
    pub msg_id: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub reasoning: Option<String>,
    pub tool_calls_json: Option<String>,
}

pub(crate) type DbSender = tokio::sync::mpsc::UnboundedSender<DbWriteMsg>;

pub(crate) fn save_partial_on_error(
    db_tx: &DbSender,
    msg_id: &str,
    partial: &assistant_agent::chat_runtime::PartialOutcome,
) {
    let tokens = assistant_agent::chat_runtime::resolve_partial_tokens(partial);
    let _ = db_tx.send(DbWriteMsg {
        msg_id: msg_id.to_string(),
        content: partial.full_content.clone(),
        tokens,
        reasoning: None,
        tool_calls_json: None,
    });
}

pub(crate) fn spawn_db_writer(app: &AppHandle) -> DbSender {
    let (db_tx, mut db_rx) = tokio::sync::mpsc::unbounded_channel::<DbWriteMsg>();
    let db_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = db_rx.recv().await {
            let state = db_app.state::<crate::setup::DbState>();
            match super::super::get_db_conn(&state) {
                Ok(conn) => {
                    if let Err(db_err) = conversations::update_message_content(
                        &conn,
                        &msg.msg_id,
                        &msg.content,
                        msg.tokens,
                        msg.reasoning.as_deref(),
                        msg.tool_calls_json.as_deref(),
                    ) {
                        error!("Failed to save content: {}", db_err);
                    }
                }
                Err(_) => {
                    error!(
                        "DB lock failed, content not saved ({} bytes)",
                        msg.content.len()
                    );
                }
            };
        }
    });
    db_tx
}

pub(crate) fn spawn_finish_listener(app: &AppHandle) -> tokio::sync::mpsc::UnboundedSender<String> {
    let (finish_tx, mut finish_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let finish_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(conv_id) = finish_rx.recv().await {
            let tasks = finish_app.state::<ChatTasks>();
            tasks.finish(&conv_id);

            {
                let state = finish_app.state::<crate::setup::DbState>();
                match super::super::get_db_conn(&state) {
                    Ok(conn) => {
                        if let Ok(Some(detail)) =
                            conversations::get_conversation_detail(&conn, &conv_id)
                        {
                            let messages =
                                match super::super::conversation_utils::messages_with_sibling_counts(
                                    &conn,
                                    &conv_id,
                                    &detail.messages,
                                ) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        log::error!("Failed to get sibling counts: {}", e);
                                        detail.messages.iter().map(|m| m.clone().into()).collect()
                                    }
                                };
                            emit_broadcast(
                                &finish_app,
                                event_name::CHAT_MESSAGES_CHANGED,
                                crate::protocol::ChatMessagesChangedPayloadDto {
                                    conversation_id: conv_id.clone(),
                                    change_kind: "stream_finished".to_string(),
                                    messages,
                                    active_branch_id: detail.conversation.active_branch_id.clone(),
                                    _seq: None,
                                },
                            );
                            emit_broadcast(
                                &finish_app,
                                event_name::CONVERSATION_LIST_CHANGED,
                                crate::protocol::ConversationListChangedPayloadDto {
                                    change_kind: "stream_finished".to_string(),
                                    conversation_id: Some(conv_id.clone()),
                                    affected_ids: None,
                                },
                            );
                        }
                    }
                    Err(_) => {
                        log::error!("DB lock failed, skip emit chat-messages-changed");
                    }
                };
            }
        }
    });
    finish_tx
}

pub(crate) fn spawn_compaction_listener(
    app: &AppHandle,
) -> tokio::sync::mpsc::UnboundedSender<(
    String,
    conversations::ConversationDetail,
    provider_store::ProviderRow,
)> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
        String,
        conversations::ConversationDetail,
        provider_store::ProviderRow,
    )>();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some((conv_id, detail, provider_row)) = rx.recv().await {
            compaction::set_terminating(&conv_id, true);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = super::super::compaction::execute_compaction(
                    &app,
                    &conv_id,
                    &detail,
                    &provider_row,
                )
                .await
                {
                    compaction::set_terminating(&conv_id, false);
                    emit_broadcast(
                        &app,
                        event_name::CONVERSATION_COMPACT_FAILED,
                        ConversationCompactFailedPayloadDto {
                            conversation_id: conv_id.clone(),
                            error: e.clone(),
                        },
                    );
                    log::error!("Auto compaction failed: {}", e);
                }
            });
        }
    });
    tx
}
