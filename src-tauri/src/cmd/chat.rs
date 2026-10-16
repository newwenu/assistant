use std::sync::atomic::{AtomicU32, Ordering};

use chrono::Utc;
use log::{error, info};
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use assistant_agent::chat_runtime::{self, PartialOutcome, StreamError, StreamOutcome};
use assistant_agent::chat_tasks::ChatTasks;
use assistant_agent::compaction;
use assistant_agent::prompt_builder;
use assistant_config::constant::WINDOW_ORB;
use assistant_providers::base::{self, ProviderError, ProviderErrorPayload};
use assistant_storage::{
    agent_modes::{self, AgentModeRow},
    conversations::{self, MessageRow},
    providers::{self as provider_store},
};
use serde_json;
use crate::protocol::{ChatDeltaPayload, ChatStreamStartPayload, ConversationCompactFailedPayload};

struct StreamEmitCtx<'a> {
    orb_window: &'a tauri::WebviewWindow,
    conv_id: &'a str,
    attempt_id: &'a str,
    msg_id: &'a str,
}

fn resolve_context_window(model: &str, provider_row: &provider_store::ProviderRow) -> u32 {
    let mut ctx = base::get_context_window_tokens(model);
    if let Ok(model_list) = serde_json::from_str::<Vec<base::ModelConfig>>(&provider_row.models_json)
    {
        if let Some(mc) = model_list.iter().find(|m| m.id == model) {
            if mc.context_window > 0 {
                ctx = mc.context_window;
            }
        }
    }
    ctx
}

fn estimate_tokens(content: &str) -> Option<i64> {
    if content.is_empty() {
        return None;
    }
    Some((content.len() as i64 + 3) / 4 + 4)
}

fn resolve_final_tokens(outcome: &StreamOutcome) -> Option<i64> {
    if outcome.total_tokens.is_none() && !outcome.full_content.is_empty() {
        let estimated = estimate_tokens(&outcome.full_content).unwrap();
        info!(
            "[chat-delta] stream finished: reason={:?} no_usage=>estimated={} (content_len={})",
            outcome.finish_reason, estimated, outcome.full_content.len()
        );
        Some(estimated)
    } else {
        info!(
            "[chat-delta] stream finished: reason={:?} total_tokens={:?}",
            outcome.finish_reason, outcome.total_tokens
        );
        outcome.total_tokens.map(|t| t as i64)
    }
}

fn resolve_partial_tokens(partial: &PartialOutcome) -> Option<i64> {
    partial
        .total_tokens
        .map(|t| t as i64)
        .or_else(|| estimate_tokens(&partial.full_content))
}

fn emit_delta(ectx: &StreamEmitCtx, seq: u32, delta: &base::ChatDelta) {
    if let Some(ref retry) = delta.delta_retry {
        info!(
            "[chat-delta] retry notification: attempt={}/{} delay={}ms message={}",
            retry.attempt, retry.max_attempts, retry.delay_ms, retry.message
        );
    }
    if let Some(ref usage) = delta.usage {
        info!(
            "[chat-delta] seq={} usage: prompt={} completion={} total={} ctx_window={}",
            seq, usage.prompt_tokens, usage.completion_tokens, usage.total_tokens, usage.context_window_tokens
        );
    }
    let payload = ChatDeltaPayload {
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
        usage: delta.usage.clone(),
        delta_retry: delta.delta_retry.clone(),
        ..Default::default()
    };
    if let Err(emit_err) = ectx.orb_window.emit("chat-delta", payload) {
        error!("Failed to emit chat-delta: {}", emit_err);
    }
}

fn emit_stream_error(ectx: &StreamEmitCtx, last_seq: u32, error: &ProviderErrorPayload) {
    let payload = ChatDeltaPayload {
        conversation_id: ectx.conv_id.to_string(),
        attempt_id: ectx.attempt_id.to_string(),
        message_id: ectx.msg_id.to_string(),
        seq: last_seq,
        error: Some(error.clone()),
        ..Default::default()
    };
    if let Err(emit_err) = ectx.orb_window.emit("chat-delta", payload) {
        error!("Failed to emit stream error event: {}", emit_err);
    }
}

struct DbWriteMsg {
    msg_id: String,
    content: String,
    tokens: Option<i64>,
    reasoning: Option<String>,
    tool_calls_json: Option<String>,
}

type DbSender = tokio::sync::mpsc::UnboundedSender<DbWriteMsg>;

fn save_partial_on_error(db_tx: &DbSender, msg_id: &str, partial: &PartialOutcome) {
    let tokens = resolve_partial_tokens(partial);
    let _ = db_tx.send(DbWriteMsg {
        msg_id: msg_id.to_string(),
        content: partial.full_content.clone(),
        tokens,
        reasoning: None,
        tool_calls_json: None,
    });
}

fn spawn_db_writer(app: &AppHandle) -> DbSender {
    let (db_tx, mut db_rx) = tokio::sync::mpsc::unbounded_channel::<DbWriteMsg>();
    let db_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = db_rx.recv().await {
            let state = db_app.state::<crate::setup::DbState>();
            match super::get_db_conn(&state) {
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
                    error!("DB lock failed, content not saved ({} bytes)", msg.content.len());
                }
            };
        }
    });
    db_tx
}

fn spawn_finish_listener(app: &AppHandle) -> tokio::sync::mpsc::UnboundedSender<String> {
    let (finish_tx, mut finish_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let finish_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(conv_id) = finish_rx.recv().await {
            let tasks = finish_app.state::<ChatTasks>();
            tasks.finish(&conv_id);
        }
    });
    finish_tx
}

fn spawn_compaction_listener(
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
                if let Err(e) =
                    super::conversation::execute_compaction(&app, &conv_id, &detail, &provider_row)
                        .await
                {
                    compaction::set_terminating(&conv_id, false);
                    let _ = app.emit(
                        "conversation_compact_failed",
                        ConversationCompactFailedPayload {
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

#[command]
pub fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    mode_id: Option<String>,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err(assistant_i18n::t("chat.empty_content"));
    }
    if compaction::is_terminating(&conversation_id) {
        return Err("Conversation is being terminated".to_string());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    if conversations::is_conversation_full(&conn, &conversation_id)? {
        return Err("Conversation is full, cannot send".to_string());
    }

    let now = Utc::now().timestamp_millis();
    let mode: Option<AgentModeRow> = mode_id
        .as_ref()
        .and_then(|id| agent_modes::get_by_id(&conn, id).ok().flatten());

    conversations::insert_message(
        &conn,
        &MessageRow {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.clone(),
            role: "user".to_string(),
            content: content.clone(),
            tokens: None,
            timestamp: now,
            parent_id: None,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
        },
    )?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    let provider_id = detail
        .conversation
        .provider_id
        .clone()
        .ok_or_else(|| assistant_i18n::t("chat.provider_deleted"))?;
    let provider_row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

    let assistant_msg_id = Uuid::new_v4().to_string();
    conversations::insert_message(
        &conn,
        &MessageRow {
            id: assistant_msg_id.clone(),
            conversation_id: conversation_id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            tokens: None,
            timestamp: now + 1,
            parent_id: None,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
        },
    )?;

    let system_prompt = mode.as_ref().map(|m| m.prompt.as_str()).unwrap_or("");
    let chat_messages = prompt_builder::prepare_messages(&detail, system_prompt, None);

    let config = super::provider::build_provider_config(&provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();
    let context_window_tokens = resolve_context_window(&model, &provider_row);

    let kind = mode
        .as_ref()
        .map(|m| prompt_builder::AgentKind::from_mode_kind(&m.kind))
        .unwrap_or_default();
    let options = prompt_builder::build_chat_options(
        &kind,
        mode.as_ref().unwrap_or(&AgentModeRow::default()),
        context_window_tokens,
    );

    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    let (cancel_token, attempt_id) = chat_tasks.start(conversation_id.clone());

    let orb_window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| assistant_i18n::t("error.orb_window_not_found"))?;
    if let Err(emit_err) = orb_window.emit(
        "chat-stream-start",
        ChatStreamStartPayload {
            conversation_id: conversation_id.clone(),
            attempt_id: attempt_id.clone(),
            message_id: assistant_msg_id.clone(),
        },
    ) {
        error!("Failed to emit chat-stream-start: {}", emit_err);
    }

    let (auto_compact, compact_threshold_percent) =
        assistant_config::app_conf::AppConf::load()
            .map(|c| (c.auto_compact, c.compact_threshold_percent))
            .unwrap_or((true, assistant_config::constant::COMPACTION_THRESHOLD_PERCENT));

    let db_tx = spawn_db_writer(&app);
    let finish_tx = spawn_finish_listener(&app);
    let compact_tx = spawn_compaction_listener(&app);

    let conv_id = conversation_id.clone();
    let msg_id = assistant_msg_id.clone();
    let attempt_id_clone = attempt_id.clone();

    let stream_future = async move {
        let stream = match adapter.chat_completion_stream(&chat_messages, &model, &options) {
            Ok(s) => s,
            Err(e) => {
                let ectx = StreamEmitCtx {
                    orb_window: &orb_window,
                    conv_id: &conv_id,
                    attempt_id: &attempt_id_clone,
                    msg_id: &msg_id,
                };
                emit_stream_error(&ectx, 0, &ProviderErrorPayload::from(&e));
                let _ = finish_tx.send(conv_id);
                return;
            }
        };

        let ectx = StreamEmitCtx {
            orb_window: &orb_window,
            conv_id: &conv_id,
            attempt_id: &attempt_id_clone,
            msg_id: &msg_id,
        };

        let last_seq = AtomicU32::new(0);

        let result = chat_runtime::consume_stream(
            stream,
            cancel_token,
            |seq, delta| {
                last_seq.store(seq, Ordering::Relaxed);
                emit_delta(&ectx, seq, delta);
            },
            None,
        )
        .await;

        let seq_val = last_seq.load(Ordering::Relaxed);

        match result {
            Ok(outcome) => {
                let tokens = resolve_final_tokens(&outcome);
                let reasoning =
                    if kind.persists_reasoning() && !outcome.full_reasoning.is_empty() {
                        Some(outcome.full_reasoning)
                    } else {
                        None
                    };
                let tool_calls_json = outcome
                    .tool_calls
                    .as_ref()
                    .filter(|tc| !tc.is_empty())
                    .and_then(|tc| serde_json::to_string(tc).ok());
                let _ = db_tx.send(DbWriteMsg {
                    msg_id: msg_id.clone(),
                    content: outcome.full_content.clone(),
                    tokens,
                    reasoning,
                    tool_calls_json,
                });
                if let Some(pt) = outcome.prompt_tokens {
                    if compaction::should_terminate(
                        pt,
                        context_window_tokens,
                        auto_compact,
                        compact_threshold_percent,
                    ) {
                        let _ = compact_tx.send((conv_id.clone(), detail, provider_row));
                    }
                }
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Cancelled(partial)) => {
                save_partial_on_error(&db_tx, &msg_id, &partial);
                emit_stream_error(
                    &ectx,
                    seq_val,
                    &ProviderErrorPayload::from(&ProviderError::Cancelled),
                );
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Provider(e, partial)) => {
                save_partial_on_error(&db_tx, &msg_id, &partial);
                emit_stream_error(&ectx, seq_val, &ProviderErrorPayload::from(&e));
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Timeout(partial)) => {
                save_partial_on_error(&db_tx, &msg_id, &partial);
                emit_stream_error(
                    &ectx,
                    seq_val,
                    &ProviderErrorPayload::from(&ProviderError::Timeout(
                        "Stream timed out".to_string(),
                    )),
                );
                let _ = finish_tx.send(conv_id);
            }
        }
    };

    tauri::async_runtime::spawn(Box::pin(stream_future));

    Ok(())
}

#[command]
pub fn cancel_chat(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    chat_tasks.cancel(&conversation_id);
    Ok(())
}