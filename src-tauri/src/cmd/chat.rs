use chrono::Utc;
use log::{error, info};
use serde_json::json;
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::chat_tasks::ChatTasks;
use assistant_config::constant::WINDOW_ORB;
use assistant_providers::base::{
    self, ChatMessage, ChatOptions, ProviderError, ProviderErrorPayload,
};
use assistant_storage::{
    agent_modes::{self, AgentModeRow},
    conversations::{self, MessageRow},
    providers::{self as provider_store},
};

#[command]
pub fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    mode_id: Option<String>,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("消息内容不能为空".to_string());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();

    let mode: Option<AgentModeRow> = mode_id
        .as_ref()
        .and_then(|id| agent_modes::get_by_id(&conn, id).ok().flatten());

    let user_msg = MessageRow {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        role: "user".to_string(),
        content: content.clone(),
        tokens: None,
        timestamp: now,
        parent_id: None,
        is_pinned: false,
    };
    conversations::insert_message(&conn, &user_msg)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    let provider_id = detail
        .conversation
        .provider_id
        .ok_or_else(|| "此对话的 Provider 已被删除，请重新选择 Provider 后再发送".to_string())?;
    let provider_row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let assistant_msg_id = Uuid::new_v4().to_string();
    let assistant_msg = MessageRow {
        id: assistant_msg_id.clone(),
        conversation_id: conversation_id.clone(),
        role: "assistant".to_string(),
        content: String::new(),
        tokens: None,
        timestamp: now + 1,
        parent_id: None,
        is_pinned: false,
    };
    conversations::insert_message(&conn, &assistant_msg)?;

    let mut chat_messages: Vec<ChatMessage> = detail
        .messages
        .iter()
        .map(|m| ChatMessage {
            role: base::message_role_from_str(&m.role),
            content: m.content.clone(),
        })
        .collect();

    let system_prompt = mode.as_ref().map(|m| m.prompt.as_str()).unwrap_or("");
    if !system_prompt.is_empty() {
        chat_messages.insert(
            0,
            ChatMessage {
                role: base::message_role_from_str("system"),
                content: system_prompt.to_string(),
            },
        );
    }

    let config = super::provider::build_provider_config(&provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();

    let context_window_tokens = {
        let mut ctx = base::get_context_window_tokens(&model);
        if let Ok(model_list) =
            serde_json::from_str::<Vec<base::ModelConfig>>(&provider_row.models_json)
        {
            if let Some(mc) = model_list.iter().find(|m| m.id == model) {
                if mc.context_window > 0 {
                    ctx = mc.context_window;
                }
            }
        }
        ctx
    };

    let temperature = mode.as_ref().and_then(|m| m.temperature).unwrap_or(0.7);
    let max_tokens = mode.as_ref().and_then(|m| m.max_tokens).unwrap_or(4096);

    let options = ChatOptions {
        temperature,
        max_tokens,
        stream: true,
        context_window_tokens,
    };

    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    let cancel_token = chat_tasks.start(conversation_id.clone());

    let orb_window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;
    let conv_id = conversation_id.clone();
    let msg_id = assistant_msg_id.clone();

    let (db_tx, mut db_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, String, Option<i64>)>();
    let db_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some((msg_id, content, tokens)) = db_rx.recv().await {
            let state = db_app.state::<crate::setup::DbState>();
            match super::get_db_conn(&state) {
                Ok(conn) => {
                    if let Err(db_err) =
                        conversations::update_message_content(&conn, &msg_id, &content, tokens)
                    {
                        error!("Failed to save content: {}", db_err);
                    }
                }
                Err(_) => {
                    error!(
                        "DB lock failed, content not saved ({} bytes)",
                        content.len()
                    );
                }
            };
        }
    });

    let (finish_tx, mut finish_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let finish_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(conv_id) = finish_rx.recv().await {
            let tasks = finish_app.state::<ChatTasks>();
            tasks.finish(&conv_id);
        }
    });

    let stream_future = async move {
        let stream = match adapter.chat_completion_stream(&chat_messages, &model, &options) {
            Ok(s) => s,
            Err(e) => {
                let payload = ProviderErrorPayload::from(&e);
                if let Err(emit_err) = orb_window.emit(
                    "chat-delta",
                    json!({
                        "conversation_id": conv_id,
                        "message_id": msg_id,
                        "error": payload,
                    }),
                ) {
                    error!("Failed to emit stream error event: {}", emit_err);
                }
                let _ = finish_tx.send(conv_id);
                return;
            }
        };
        use futures::StreamExt;
        let mut stream = stream;
        let mut full_content = String::new();
        let mut seq: u32 = 0;
        let mut last_tokens: Option<i64> = None;
        while let Some(result) = stream.next().await {
            if cancel_token.is_cancelled() {
                let cancel_tokens = last_tokens.or_else(|| {
                    if full_content.is_empty() {
                        return None;
                    }
                    Some((full_content.len() as i64 + 3) / 4 + 4)
                });
                let _ = db_tx.send((msg_id.clone(), full_content.clone(), cancel_tokens));
                if let Err(emit_err) = orb_window.emit(
                    "chat-delta",
                    json!({
                        "conversation_id": conv_id,
                        "message_id": msg_id,
                        "error": ProviderErrorPayload::from(&ProviderError::Cancelled),
                    }),
                ) {
                    error!("Failed to emit cancel event: {}", emit_err);
                }
                let _ = finish_tx.send(conv_id);
                break;
            }
            match result {
                Ok(delta) => {
                    if let Some(text) = &delta.delta_text {
                        full_content.push_str(text);
                    }
                    if let Some(ref usage) = delta.usage {
                        let total = usage.prompt_tokens as i64 + usage.completion_tokens as i64;
                        if total > 0 {
                            last_tokens = Some(total);
                        }
                        info!(
                            "[chat-delta] seq={} usage: prompt={} completion={} total={} ctx_window={}",
                            seq, usage.prompt_tokens, usage.completion_tokens, usage.total_tokens, usage.context_window_tokens
                        );
                    }
                    seq += 1;
                    if let Err(emit_err) = orb_window.emit(
                        "chat-delta",
                        json!({
                            "conversation_id": conv_id,
                            "message_id": msg_id,
                            "seq": seq,
                            "delta_text": delta.delta_text,
                            "delta_reasoning": delta.delta_reasoning,
                            "delta_tool_call_id": delta.delta_tool_call_id,
                            "delta_tool_call_name": delta.delta_tool_call_name,
                            "delta_tool_call_arguments": delta.delta_tool_call_arguments,
                            "finish_reason": delta.finish_reason,
                            "usage": delta.usage,
                        }),
                    ) {
                        error!("Failed to emit chat-delta: {}", emit_err);
                    }
                    if delta.finish_reason.is_some() {
                        let final_tokens = if last_tokens.is_none() && !full_content.is_empty() {
                            let estimated = (full_content.len() as i64 + 3) / 4 + 4;
                            info!(
                                "[chat-delta] stream finished: reason={:?} no_usage=>estimated={} (content_len={})",
                                delta.finish_reason, estimated, full_content.len()
                            );
                            Some(estimated)
                        } else {
                            info!(
                                "[chat-delta] stream finished: reason={:?} last_tokens={:?} has_usage={}",
                                delta.finish_reason, last_tokens, delta.usage.is_some()
                            );
                            last_tokens
                        };
                        let _ = db_tx.send((msg_id.clone(), full_content.clone(), final_tokens));
                        let _ = finish_tx.send(conv_id);
                        break;
                    }
                }
                Err(e) => {
                    let err_tokens = last_tokens.or_else(|| {
                        if full_content.is_empty() {
                            return None;
                        }
                        Some((full_content.len() as i64 + 3) / 4 + 4)
                    });
                    let _ = db_tx.send((msg_id.clone(), full_content.clone(), err_tokens));
                    let payload = ProviderErrorPayload::from(&e);
                    if let Err(emit_err) = orb_window.emit(
                        "chat-delta",
                        json!({
                            "conversation_id": conv_id,
                            "message_id": msg_id,
                            "error": payload,
                        }),
                    ) {
                        error!("Failed to emit error event: {}", emit_err);
                    }
                    let _ = finish_tx.send(conv_id);
                    break;
                }
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
