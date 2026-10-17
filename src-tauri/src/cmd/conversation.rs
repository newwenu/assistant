use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use chrono::Utc;
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::protocol::{
    ChatDeltaPayloadDto, ChatRegeneratePayloadDto, ChatStreamStartPayloadDto,
    ConversationCompactFailedPayloadDto, ConversationDetailDto, ConversationDto,
    ConversationTerminatedPayloadDto, ConversationUnlockedPayloadDto, ProviderErrorPayloadDto,
    TokenUsageSummaryDto,
};
use assistant_agent::chat_runtime::{self, PartialOutcome, StreamError};
use assistant_agent::chat_tasks::ChatTasks;
use assistant_agent::prompt_builder;
use assistant_providers::base::{self, ProviderErrorPayload};
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
) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::get_conversations(&conn, provider_id.as_deref())?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[command]
pub fn get_conversation(
    app: AppHandle,
    id: String,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let detail = conversations::get_conversation_detail(&conn, &id)?;
    Ok(detail.map(|d| {
        let total_count = d.messages.len() as i64;
        ConversationDetailDto {
            conversation: d.conversation.into(),
            messages: d.messages.into_iter().map(Into::into).collect(),
            total_count,
        }
    }))
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
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    match conversations::get_conversation_by_id(&conn, &id)? {
        Some(conv) => {
            let page_limit = limit.unwrap_or(50);
            let messages =
                conversations::get_messages_paginated(&conn, &id, page_limit, before_timestamp)?;
            let total_count = conversations::get_message_count(&conn, &id)?;
            Ok(Some(ConversationDetailDto {
                conversation: conv.into(),
                messages: messages.into_iter().map(Into::into).collect(),
                total_count,
            }))
        }
        None => Ok(None),
    }
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conversations::delete_message(&conn, &message_id)
}

#[command]
pub fn search_conversations(app: AppHandle, query: String) -> Result<Vec<ConversationDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = conversations::search_conversations(&conn, &query)?;
    Ok(rows.into_iter().map(Into::into).collect())
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
        ChatRegeneratePayloadDto {
            conversation_id: conversation_id.clone(),
            deleted_message_id: message_id,
        },
    );
    Ok(())
}

#[command]
pub fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<TokenUsageSummaryDto, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let ctx = if let Some(c) = context_window_tokens {
        c
    } else {
        resolve_context_window_for_conversation(&conn, &conversation_id)
    };
    let summary = agent_modes::get_conversation_token_usage(&conn, &conversation_id, ctx)?;
    Ok(summary.into())
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

    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
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
                ConversationCompactFailedPayloadDto {
                    conversation_id: conv_id.clone(),
                    error: e.clone(),
                },
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
        ConversationUnlockedPayloadDto {
            conversation_id: conversation_id.clone(),
        },
    );

    Ok(())
}

pub(crate) async fn execute_compaction(
    app: &AppHandle,
    conversation_id: &str,
    detail: &conversations::ConversationDetail,
    provider_row: &provider_store::ProviderRow,
) -> Result<(), String> {
    use assistant_agent::compaction::{default_compact_summary_prompt, set_terminating};
    use assistant_config::app_conf::AppConf;
    use assistant_providers::base::{ChatOptions, ProviderError};
    use std::time::Duration;

    let app_conf = AppConf::load().map_err(|e| e.to_string())?;
    let system_prompt = detail
        .conversation
        .system_prompt
        .clone()
        .unwrap_or_default();

    let default_prompt = default_compact_summary_prompt();
    let summary_prompt = app_conf
        .compact_summary_prompt
        .as_deref()
        .unwrap_or(&default_prompt);

    let chat_messages =
        prompt_builder::prepare_messages(detail, &system_prompt, Some(summary_prompt));

    let config = super::provider::build_provider_config(provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();

    let max_tokens = app_conf.compact_summary_max_tokens as u32;

    let options = ChatOptions {
        temperature: 0.3,
        max_tokens,
        stream: true,
        context_window_tokens: 0,
        reasoning_enabled: None,
        reasoning_effort: None,
    };

    let chat_tasks = app.state::<ChatTasks>();
    let (cancel_token, attempt_id) = chat_tasks.start(conversation_id.to_string());

    let stream = adapter
        .chat_completion_stream(&chat_messages, &model, &options, cancel_token.clone())
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
                reasoning: None,
                tool_calls_json: None,
            },
        )?;
    }

    let orb_window = app
        .get_webview_window(assistant_config::constant::WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;

    let _ = orb_window.emit(
        "chat-stream-start",
        ChatStreamStartPayloadDto {
            conversation_id: conversation_id.to_string(),
            attempt_id: attempt_id.to_string(),
            message_id: summary_msg_id.to_string(),
        },
    );

    let _ = orb_window.emit(
        "chat-delta",
        ChatDeltaPayloadDto {
            conversation_id: conversation_id.to_string(),
            attempt_id: attempt_id.to_string(),
            message_id: summary_msg_id.to_string(),
            seq: 0,
            delta_text: Some(String::new()),
            ..Default::default()
        },
    );

    let conv_id = conversation_id.to_string();
    let msg_id = summary_msg_id.clone();
    let aid = attempt_id.clone();
    let orb_win = orb_window.clone();

    let last_seq = Arc::new(AtomicU32::new(0));
    let last_seq_clone = last_seq.clone();

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        move |seq, delta| {
            last_seq_clone.store(seq, Ordering::Relaxed);
            let payload = ChatDeltaPayloadDto {
                conversation_id: conv_id.clone(),
                attempt_id: aid.clone(),
                message_id: msg_id.clone(),
                seq,
                delta_text: delta.delta_text.clone(),
                finish_reason: delta.finish_reason.clone(),
                ..Default::default()
            };
            let _ = orb_win.emit("chat-delta", payload);
        },
        Some(Duration::from_secs(30)),
    )
    .await;

    let seq_val = last_seq.load(Ordering::Relaxed);

    chat_tasks.finish(conversation_id);

    match result {
        Ok(outcome) => {
            let mut summary = outcome.full_content;

            let max_summary_chars =
                (assistant_config::constant::MAX_SUMMARY_OUTPUT_TOKENS as usize) * 4;
            if summary.len() > max_summary_chars {
                summary.truncate(max_summary_chars);
                summary.push_str(&format!(
                    "\n\n{}",
                    assistant_i18n::t("chat.compaction.summary_truncated")
                ));
            }

            {
                let state = app.state::<crate::setup::DbState>();
                let conn = super::get_db_conn(&state)?;

                let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

                conversations::update_message_content(
                    &tx,
                    &summary_msg_id,
                    &summary,
                    None,
                    None,
                    None,
                )?;
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
                ConversationTerminatedPayloadDto {
                    conversation_id: conversation_id.to_string(),
                    summary: summary.clone(),
                    is_full: true,
                },
            );

            log::info!(
                "Conversation {} terminated with summary ({} chars)",
                conversation_id,
                summary.len()
            );

            Ok(())
        }
        Err(StreamError::Cancelled(partial)) => {
            save_compaction_partial(app, &summary_msg_id, &partial);
            set_terminating(conversation_id, false);
            let _ = orb_window.emit(
                "chat-delta",
                ChatDeltaPayloadDto {
                    conversation_id: conversation_id.to_string(),
                    attempt_id: attempt_id.to_string(),
                    message_id: summary_msg_id.to_string(),
                    seq: seq_val,
                    error: Some(ProviderErrorPayloadDto::from(ProviderErrorPayload::from(
                        &ProviderError::Cancelled,
                    ))),
                    ..Default::default()
                },
            );
            Err("Compaction cancelled".to_string())
        }
        Err(StreamError::Provider(e, partial)) => {
            save_compaction_partial(app, &summary_msg_id, &partial);
            set_terminating(conversation_id, false);
            let _ = orb_window.emit(
                "chat-delta",
                ChatDeltaPayloadDto {
                    conversation_id: conversation_id.to_string(),
                    attempt_id: attempt_id.to_string(),
                    message_id: summary_msg_id.to_string(),
                    seq: seq_val,
                    error: Some(ProviderErrorPayloadDto::from(ProviderErrorPayload::from(
                        &e,
                    ))),
                    ..Default::default()
                },
            );
            Err(e.to_string())
        }
        Err(StreamError::Timeout(partial)) => {
            save_compaction_partial(app, &summary_msg_id, &partial);
            set_terminating(conversation_id, false);
            let _ = orb_window.emit(
                "chat-delta",
                ChatDeltaPayloadDto {
                    conversation_id: conversation_id.to_string(),
                    attempt_id: attempt_id.to_string(),
                    message_id: summary_msg_id.to_string(),
                    seq: seq_val,
                    error: Some(ProviderErrorPayloadDto::from(ProviderErrorPayload::from(
                        &ProviderError::Timeout("Summary generation timed out (30s)".to_string()),
                    ))),
                    ..Default::default()
                },
            );
            Err("Summary generation timed out (30s)".to_string())
        }
    }
}

fn save_compaction_partial(app: &AppHandle, msg_id: &str, partial: &PartialOutcome) {
    if !partial.has_effective_output {
        return;
    }
    let state = app.state::<crate::setup::DbState>();
    let Ok(conn) = super::get_db_conn(&state) else {
        return;
    };
    let tokens = partial
        .total_tokens
        .map(|t| t as i64)
        .or_else(|| estimate_compaction_tokens(&partial.full_content));
    let _ = conversations::update_message_content(
        &conn,
        msg_id,
        &partial.full_content,
        tokens,
        None,
        None,
    );
}

fn estimate_compaction_tokens(content: &str) -> Option<i64> {
    if content.is_empty() {
        return None;
    }
    Some((content.len() as i64 + 3) / 4 + 4)
}
