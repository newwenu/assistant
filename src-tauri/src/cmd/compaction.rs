use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::protocol::events::{emit_broadcast, emit_orb_only, event_name};
use crate::protocol::{
    ChatDeltaPayloadDto, ChatMessagesChangedPayloadDto, ChatStreamStartPayloadDto,
    ConversationCompactFailedPayloadDto, ConversationTerminatedPayloadDto,
    ConversationUnlockedPayloadDto, MessageChangeKindDto, ProviderErrorPayloadDto, TurnPhaseDto,
};
use assistant_agent::chat_runtime::{
    self, build_usage_records, PartialOutcome, StreamError, UsageRecordCtx,
};
use assistant_agent::kernel::{OnBusy, TurnTasks};
use assistant_agent::prompt_builder;
use assistant_agent::provider_config::build_provider_config;
use assistant_crypto::api_key as crypto;
use assistant_providers::base::{self, ProviderErrorPayload};
use assistant_storage::{
    conversations,
    providers::{self as provider_store},
};

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

    let tasks = app.state::<std::sync::Arc<TurnTasks>>().inner().clone();
    if tasks.current(&conversation_id).is_some() {
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
            emit_broadcast(
                &app_clone,
                event_name::CONVERSATION_COMPACT_FAILED,
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

    emit_broadcast(
        &app,
        event_name::CONVERSATION_UNLOCKED,
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

    let config = build_provider_config(provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();

    let machine_id = crypto::get_machine_id().map_err(|e| e.to_string())?;
    let api_key = crypto::decrypt_api_key_b64(&provider_row.api_key_encrypted, &machine_id)
        .map_err(|e| e.to_string())?;

    let max_tokens = app_conf.compact_summary_max_tokens as u32;

    let options = ChatOptions {
        temperature: 0.3,
        max_tokens,
        stream: true,
        context_window_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
    };

    let tasks = app.state::<std::sync::Arc<TurnTasks>>().inner().clone();
    let (attempt_id, cancel_token) = tasks
        .begin(conversation_id, OnBusy::Replace)
        .map_err(|_| "Conversation is busy".to_string())?;

    let stream = adapter
        .chat_completion_stream(
            &api_key,
            &chat_messages,
            &model,
            &options,
            cancel_token.clone(),
        )
        .map_err(|e| e.to_string())?;

    let summary_msg_id = Uuid::new_v4().to_string();
    let summary_parent_id = detail.messages.last().map(|m| m.id.clone());
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
                parent_id: summary_parent_id,
                is_pinned: false,
                reasoning: None,
                tool_calls_json: None,
                sibling_index: 0,
                provider_id: None,
                model_id: None,
            },
        )?;
    }

    let orb_win = app
        .get_webview_window(assistant_config::constant::WINDOW_ORB)
        .ok_or_else(|| "Orb window not found".to_string())?;

    emit_orb_only(
        app,
        event_name::CHAT_STREAM_START,
        ChatStreamStartPayloadDto {
            conversation_id: conversation_id.to_string(),
            attempt_id: attempt_id.to_string(),
            message_id: summary_msg_id.to_string(),
            phase: TurnPhaseDto::Streaming,
        },
    );

    emit_orb_only(
        app,
        event_name::CHAT_DELTA,
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
            let _ = orb_win.emit(event_name::CHAT_DELTA, payload);
        },
        Some(Duration::from_secs(30)),
    )
    .await;

    let seq_val = last_seq.load(Ordering::Relaxed);

    tasks.finish(conversation_id, &attempt_id);

    let usage_ctx = UsageRecordCtx {
        conv_id: conversation_id,
        msg_id: &summary_msg_id,
        attempt_id: &attempt_id,
        branch_leaf_id: detail.conversation.active_branch_id.as_deref(),
        kind: "compaction",
        provider_id: Some(provider_row.id.as_str()),
        model_id: Some(model.as_str()),
        cache_capable: crate::cmd::token_usage_record::cache_capable_of(provider_row),
    };

    match result {
        Ok(outcome) => {
            let mut summary = outcome.full_content;

            log::info!(
                "[compaction-usage] conversation={} prompt={:?} completion={:?} total={:?} estimated={} attempts={}",
                conversation_id,
                outcome.prompt_tokens,
                outcome.completion_tokens,
                outcome.total_tokens,
                outcome.estimated,
                outcome.attempt_usages.len()
            );
            let usage_records = build_usage_records(&outcome.attempt_usages, &usage_ctx);

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

                conversations::update_message_content(&tx, &summary_msg_id, &summary, None, None)?;
                conversations::set_conversation_is_full(&tx, conversation_id, true)?;
                conversations::update_active_branch_id(&tx, conversation_id, &summary_msg_id)?;

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

                for record in &usage_records {
                    assistant_storage::token_usage::insert_record(&tx, record)?;
                }

                tx.commit().map_err(|e| e.to_string())?;
            }

            set_terminating(conversation_id, false);

            emit_broadcast(
                app,
                event_name::CONVERSATION_TERMINATED,
                ConversationTerminatedPayloadDto {
                    conversation_id: conversation_id.to_string(),
                    summary: summary.clone(),
                    is_full: true,
                },
            );

            {
                let state = app.state::<crate::setup::DbState>();
                let conn = super::get_db_conn(&state)?;
                if let Some(detail) =
                    conversations::get_conversation_detail(&conn, conversation_id)?
                {
                    let messages = super::conversation_utils::messages_with_sibling_counts(
                        &conn,
                        conversation_id,
                        &detail.messages,
                    )?;
                    emit_broadcast(
                        app,
                        event_name::CHAT_MESSAGES_CHANGED,
                        ChatMessagesChangedPayloadDto {
                            conversation_id: conversation_id.to_string(),
                            change_kind: MessageChangeKindDto::MessagesCompacted,
                            messages,
                            active_branch_id: detail.conversation.active_branch_id.clone(),
                            seq: None,
                        },
                    );
                }
            }

            log::info!(
                "Conversation {} terminated with summary ({} chars)",
                conversation_id,
                summary.len()
            );

            Ok(())
        }
        Err(StreamError::Cancelled(partial)) => {
            let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            set_terminating(conversation_id, false);
            emit_orb_only(
                app,
                event_name::CHAT_DELTA,
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
            let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            set_terminating(conversation_id, false);
            emit_orb_only(
                app,
                event_name::CHAT_DELTA,
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
            let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            set_terminating(conversation_id, false);
            emit_orb_only(
                app,
                event_name::CHAT_DELTA,
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

fn save_compaction_partial(
    app: &AppHandle,
    msg_id: &str,
    partial: &PartialOutcome,
    usage_records: &[assistant_storage::token_usage::TokenUsageRecord],
) {
    let state = app.state::<crate::setup::DbState>();
    let Ok(conn) = super::get_db_conn(&state) else {
        return;
    };
    // 失败也落 usage：累计口径包含中断请求
    for record in usage_records {
        if let Err(e) = assistant_storage::token_usage::insert_record(&conn, record) {
            log::error!("Failed to save compaction token usage: {}", e);
        }
    }
    if !partial.has_effective_output {
        return;
    }
    // 摘要消息的 tokens 保持 NULL：压缩产出的 usage 只进累计口径，不写估算值
    let _ = conversations::update_message_content(&conn, msg_id, &partial.full_content, None, None);
}
