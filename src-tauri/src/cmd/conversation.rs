use chrono::Utc;
use serde_json::json;
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use assistant_providers::base;
use assistant_storage::{
    agent_modes,
    conversations::{self, ConversationRow},
    providers::{self as provider_store},
};

#[command]
pub fn create_conversation(
    app: AppHandle,
    provider_id: String,
    model_id: String,
    mode_id: Option<String>,
) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let row = ConversationRow {
        id: id.clone(),
        title: assistant_i18n::t("chat.new_conversation_title"),
        title_source: "placeholder".to_string(),
        provider_id: Some(provider_id),
        model_id,
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: now,
        updated_at: now,
        is_full: false,
    };
    conversations::insert_conversation(&conn, &row)?;

    if let Some(mid) = mode_id {
        let _ = conn.execute(
            "UPDATE conversations SET mode_id = ?1 WHERE id = ?2",
            rusqlite::params![mid, id],
        );
    }

    Ok(id)
}

#[command]
pub fn list_conversations(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    serde_json::to_value(&rows).map_err(|e| e.to_string())
}

#[command]
pub fn get_conversation(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let detail = conversations::get_conversation_detail(&conn, &id)?;
    serde_json::to_value(&detail).map_err(|e| e.to_string())
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)
}

#[command]
pub fn rename_conversation(
    app: AppHandle,
    id: String,
    title: String,
    title_source: Option<String>,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let source = title_source.unwrap_or_else(|| "user".to_string());
    conversations::rename_conversation(&conn, &id, &title, &source)
}

#[command]
pub fn update_conversation_model(
    app: AppHandle,
    id: String,
    model_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut conv = conversations::get_conversation_by_id(&conn, &id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    conv.model_id = model_id;
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)
}

#[command]
pub fn update_conversation_provider(
    app: AppHandle,
    id: String,
    provider_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let now = Utc::now().timestamp_millis();
    let mut conv = conversations::get_conversation_by_id(&conn, &id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    conv.provider_id = Some(provider_id);
    conv.updated_at = now;
    conversations::update_conversation(&conn, &conv)
}

#[command]
pub fn get_conversation_paginated(
    app: AppHandle,
    id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    match conversations::get_conversation_by_id(&conn, &id)? {
        Some(conv) => {
            let page_limit = limit.unwrap_or(50);
            let messages =
                conversations::get_messages_paginated(&conn, &id, page_limit, before_timestamp)?;
            let total_count = conversations::get_message_count(&conn, &id)?;
            Ok(json!({
                "conversation": conv,
                "messages": messages,
                "total_count": total_count,
            }))
        }
        None => Ok(json!(null)),
    }
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_message(&conn, &message_id)
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    serde_json::to_value(&rows).map_err(|e| e.to_string())
}

#[command]
pub fn export_conversation(app: AppHandle, id: String, format: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    match format.as_str() {
        "json" => conversations::export_conversation_json(&conn, &id),
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id),
        _ => conversations::export_conversation_markdown(&conn, &id),
    }
}

#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::toggle_pin(&conn, &id)
}

#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::duplicate_conversation(&conn, &id)
}

#[command]
pub fn batch_delete_conversations(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::batch_delete(&conn, &ids)
}

#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::purge_all(&conn)
}

#[command]
pub fn edit_message(app: AppHandle, message_id: String, new_content: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::edit_message(&conn, &message_id, &new_content)
}

#[command]
pub fn regenerate_message(
    app: AppHandle,
    conversation_id: String,
    message_id: String,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_message_and_after(&conn, &message_id)?;
    let _ = app.emit(
        "chat-regenerate",
        json!({
            "conversation_id": conversation_id,
            "deleted_message_id": message_id,
        }),
    );
    Ok(())
}

#[command]
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let ctx = if let Some(c) = context_window_tokens {
        c
    } else {
        resolve_context_window_for_conversation(&conn, &conversation_id)
    };
    let summary = agent_modes::get_conversation_token_usage(&conn, &conversation_id, ctx)?;
    serde_json::to_value(&summary).map_err(|e| e.to_string())
}

fn resolve_context_window_for_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> u32 {
    let conv = match conversations::get_conversation_by_id(conn, conversation_id) {
        Ok(Some(c)) => c,
        _ => return 128_000,
    };
    let model_id = &conv.model_id;
    let mut ctx = assistant_providers::base::get_context_window_tokens(model_id);
    if let Some(ref provider_id) = conv.provider_id {
        if let Ok(Some(provider_row)) = provider_store::get_provider_by_id(conn, provider_id) {
            if let Ok(model_list) = serde_json::from_str::<
                Vec<assistant_providers::base::ModelConfig>,
            >(&provider_row.models_json)
            {
                if let Some(mc) = model_list.iter().find(|m| m.id == *model_id) {
                    if mc.context_window > 0 {
                        ctx = mc.context_window;
                    }
                }
            }
        }
    }
    ctx
}

#[command]
pub fn manual_compact_conversation(app: AppHandle, conversation_id: String) -> Result<(), String> {
    use assistant_agent::compaction::{is_terminating, set_terminating};

    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    if detail.conversation.is_full {
        return Err("Conversation is already full".to_string());
    }
    if is_terminating(&conversation_id) {
        return Err("Conversation is already being terminated".to_string());
    }

    let provider_id = detail
        .conversation
        .provider_id
        .clone()
        .ok_or_else(|| "Provider not set for conversation".to_string())?;
    let provider_row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let chat_tasks: tauri::State<'_, crate::chat_tasks::ChatTasks> =
        app.state::<crate::chat_tasks::ChatTasks>();
    if chat_tasks.is_running(&conversation_id) {
        return Err("Cannot compact while stream is running".to_string());
    }

    set_terminating(&conversation_id, true);

    let app_clone = app.clone();
    let conv_id = conversation_id.clone();
    let compact_detail = detail.clone();
    let compact_provider_row = provider_row.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            execute_compaction(&app_clone, &conv_id, &compact_detail, &compact_provider_row).await
        {
            use assistant_agent::compaction::set_terminating;
            set_terminating(&conv_id, false);
            let _ = app_clone.emit(
                "conversation_compact_failed",
                json!({
                    "conversation_id": conv_id,
                    "error": e,
                }),
            );
            log::error!("Manual compaction failed for {}: {}", conv_id, e);
        }
    });

    Ok(())
}

#[command]
pub fn unlock_conversation(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    conversations::set_conversation_is_full(&conn, &conversation_id, false)?;

    let _ = app.emit(
        "conversation_unlocked",
        json!({
            "conversation_id": conversation_id,
        }),
    );

    Ok(())
}

pub(crate) async fn execute_compaction(
    app: &AppHandle,
    conversation_id: &str,
    detail: &conversations::ConversationDetail,
    provider_row: &provider_store::ProviderRow,
) -> Result<(), String> {
    use assistant_agent::compaction::{set_terminating, DEFAULT_COMPACT_SUMMARY_PROMPT};
    use assistant_config::app_conf::AppConf;
    use assistant_providers::base::{ChatMessage, ChatOptions};
    use std::time::Duration;

    let app_conf = AppConf::load().map_err(|e| e.to_string())?;
    let system_prompt = detail
        .conversation
        .system_prompt
        .clone()
        .unwrap_or_default();

    let summary_prompt = app_conf
        .compact_summary_prompt
        .as_deref()
        .unwrap_or(DEFAULT_COMPACT_SUMMARY_PROMPT);

    let config = super::provider::build_provider_config(&provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();

    let mut chat_messages: Vec<ChatMessage> = detail
        .messages
        .iter()
        .filter(|m| m.role != "compact_summary")
        .map(|m| ChatMessage {
            role: base::message_role_from_str(&m.role),
            content: m.content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
        })
        .collect();

    if !system_prompt.is_empty() {
        chat_messages.insert(
            0,
            ChatMessage {
                role: base::message_role_from_str("system"),
                content: system_prompt.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
            },
        );
    }

    chat_messages.push(ChatMessage {
        role: base::message_role_from_str("user"),
        content: summary_prompt.to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: false,
        thinking_signature: None,
    });

    let max_tokens = app_conf.compact_summary_max_tokens as u32;

    let options = ChatOptions {
        temperature: 0.3,
        max_tokens,
        stream: true,
        context_window_tokens: 0,
        reasoning_enabled: None,
        reasoning_effort: None,
    };

    let stream = adapter
        .chat_completion_stream(&chat_messages, &model, &options)
        .map_err(|e| e.to_string())?;

    let summary_msg_id = Uuid::new_v4().to_string();
    {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        let now = chrono::Utc::now().timestamp_millis();
        conversations::insert_message(
            &conn,
            &conversations::MessageRow {
                id: summary_msg_id.clone(),
                conversation_id: conversation_id.to_string(),
                role: "compact_summary".to_string(),
                content: String::new(),
                tokens: None,
                timestamp: now,
                parent_id: None,
                is_pinned: false,
            },
        )?;
    }

    let orb_window = app
        .get_webview_window(assistant_config::constant::WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;

    let _ = orb_window.emit(
        "chat-delta",
        json!({
            "conversation_id": conversation_id,
            "message_id": summary_msg_id,
            "seq": 0,
            "delta_text": "",
        }),
    );

    let mut summary = String::new();
    let stream_result: Result<(), String> = {
        use tokio_stream::StreamExt as TokioStreamExt;
        let mut stream = stream;
        let mut seq: u32 = 0;
        let timeout_res = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(result) = TokioStreamExt::next(&mut stream).await {
                match result {
                    Ok(delta) => {
                        if let Some(text) = &delta.delta_text {
                            summary.push_str(text);
                        }
                        seq += 1;
                        let _ = orb_window.emit(
                            "chat-delta",
                            json!({
                                "conversation_id": conversation_id,
                                "message_id": summary_msg_id,
                                "seq": seq,
                                "delta_text": delta.delta_text,
                                "finish_reason": delta.finish_reason,
                            }),
                        );
                        if delta.finish_reason.is_some() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = orb_window.emit(
                            "chat-delta",
                            json!({
                                "conversation_id": conversation_id,
                                "message_id": summary_msg_id,
                                "error": { "code": "provider_error", "message": e.to_string() },
                            }),
                        );
                        return Err(e.to_string());
                    }
                }
            }
            Ok(())
        })
        .await;
        timeout_res.map_err(|_| "Summary generation timed out (30s)".to_string())?
    };

    if let Err(e) = stream_result {
        {
            let state = app.state::<crate::setup::DbState>();
            let conn = super::get_db_conn(&state)?;
            conversations::delete_message(&conn, &summary_msg_id)?;
        }
        return Err(e);
    }

    let max_summary_chars = (assistant_config::constant::MAX_SUMMARY_OUTPUT_TOKENS as usize) * 4;
    if summary.len() > max_summary_chars {
        summary.truncate(max_summary_chars);
        summary.push_str("\n\n[摘要已截断，完整信息请通过 search_history 检索]");
    }

    {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        conversations::update_message_content(&tx, &summary_msg_id, &summary, None)?;
        conversations::set_conversation_is_full(&tx, conversation_id, true)?;

        let now = chrono::Utc::now().timestamp_millis();
        let existing = conversations::get_compression_state(&tx, conversation_id)?;
        let new_count = existing
            .as_ref()
            .map(|s| s.compression_count + 1)
            .unwrap_or(1);
        let state_row = conversations::CompressionStateRow {
            conversation_id: conversation_id.to_string(),
            last_compression_at: Some(now),
            compression_count: new_count,
        };
        conversations::upsert_compression_state(&tx, &state_row)?;

        tx.commit().map_err(|e| e.to_string())?;
    }

    set_terminating(conversation_id, false);

    let _ = app.emit(
        "conversation_terminated",
        json!({
            "conversation_id": conversation_id,
            "summary": summary,
            "is_full": true,
        }),
    );

    log::info!(
        "Conversation {} terminated with summary ({} chars)",
        conversation_id,
        summary.len()
    );

    Ok(())
}
