use std::sync::Arc;

use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::adapters::{SystemClock, TauriEventSink};
use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ConversationTerminatedPayloadDto, ConversationUnlockedPayloadDto, MessageChangeKindDto,
};
use assistant_agent::kernel::{
    limits, request, stream, Clock, EventRoute, EventSink, OnBusy, PartialOutcome, PersistCtx,
    StreamError, TurnEmitter, TurnPhase, TurnStatus, TurnTasks,
};
use assistant_agent::multimodal::ProjectionPurpose;
use assistant_agent::provider_config::build_provider_config;
use assistant_crypto::api_key as crypto;
use assistant_providers::base::{self, ProviderErrorPayload};
use assistant_storage::{
    conversations,
    providers::{self as provider_store},
};

#[command]
pub fn manual_compact_conversation(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| "Conversation not found".to_string())?;
    if detail.conversation.is_full {
        return Err("Conversation is already full".to_string());
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

    let app_clone = app.clone();
    let conv_id = conversation_id.clone();
    let compact_detail = detail.clone();
    let compact_provider_row = provider_row.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            execute_compaction(&app_clone, &conv_id, &compact_detail, &compact_provider_row).await
        {
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
    use assistant_agent::compaction::default_compact_summary_prompt;
    use assistant_config::app_conf::AppConf;
    use assistant_providers::base::{ChatOptions, ProviderError};

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

    let config = build_provider_config(provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = detail.conversation.model_id.clone();

    let machine_id = crypto::get_machine_id().map_err(|e| e.to_string())?;
    let api_key = crypto::decrypt_api_key_b64(&provider_row.api_key_encrypted, &machine_id)
        .map_err(|e| e.to_string())?;

    let max_tokens = app_conf.compact_summary_max_tokens as u32;

    // 与 chat 共用投影：附件投影 + 超预算降级，避免超限请求直发 provider。
    let context_window_tokens = assistant_providers::types::resolve_context_window_from_stored(
        &model,
        &provider_row.models_json,
    );
    let offload_budget = request::offload_budget_of(context_window_tokens, max_tokens);
    let attachment_store = app.state::<assistant_storage::attachment_store::AttachmentStore>();
    let chat_messages = request::project_chat_messages(
        detail,
        &system_prompt,
        provider_row,
        &model,
        &attachment_store,
        offload_budget,
        ProjectionPurpose::Send,
        Some(summary_prompt),
    )
    .await;

    let options = ChatOptions {
        temperature: 0.3,
        max_tokens,
        stream: true,
        context_window_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
        tools: None,
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
                tool_call_id: None,
                is_error: false,
            },
        )?;
    }

    let sink: Arc<dyn EventSink> = Arc::new(
        TauriEventSink::new(
            app.clone(),
            EventRoute::Orb,
            conversation_id,
            summary_msg_id.clone(),
        )
        .with_finished_change_kind(MessageChangeKindDto::MessagesCompacted),
    );
    let emitter = TurnEmitter::new(sink, tasks.clone(), conversation_id, attempt_id.clone());
    emitter.stream_start(TurnPhase::Compacting);

    let clock = SystemClock::new();
    let request_started = clock.now();

    let result = stream::consume(
        stream,
        cancel_token,
        Some(limits::DEFAULT_COMPACTION_MODEL_CALL_TIMEOUT),
        &clock,
        request_started,
        |_seq, delta| {
            emitter.delta(delta);
        },
    )
    .await;

    let usage_ctx = PersistCtx {
        conversation_id: conversation_id.to_string(),
        turn_id: attempt_id.clone(),
        message_id: summary_msg_id.clone(),
        branch_leaf_id: detail.conversation.active_branch_id.clone(),
        kind: "compaction".to_string(),
        provider_id: Some(provider_row.id.clone()),
        model_id: Some(model.clone()),
        cache_capable: crate::cmd::token_usage_record::cache_capable_of(provider_row),
        created_at_ms: clock.now_millis(),
    };

    let (_, terminal_error) = match &result {
        Ok(_) => (TurnStatus::Completed, None),
        Err(StreamError::Cancelled(_)) => (
            TurnStatus::Cancelled,
            Some(ProviderErrorPayload::from(&ProviderError::Cancelled)),
        ),
        Err(StreamError::Timeout(_)) => (
            TurnStatus::Failed,
            Some(ProviderErrorPayload::from(&ProviderError::Timeout(
                "Summary generation timed out (30s)".to_string(),
            ))),
        ),
        Err(StreamError::Provider(e, _)) => {
            (TurnStatus::Failed, Some(ProviderErrorPayload::from(e)))
        }
    };

    let final_status = match result {
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
            let usage_records = usage_ctx.usage_records(&outcome.attempt_usages);

            let max_summary_chars =
                (assistant_config::constant::MAX_SUMMARY_OUTPUT_TOKENS as usize) * 4;
            if summary.len() > max_summary_chars {
                summary.truncate(max_summary_chars);
                summary.push_str(&format!(
                    "\n\n{}",
                    assistant_i18n::t("chat.compaction.summary_truncated")
                ));
            }

            let commit = commit_summary(
                app,
                conversation_id,
                &summary_msg_id,
                &summary,
                &usage_records,
            );
            match commit {
                Ok(()) => {
                    log::info!(
                        "Conversation {} terminated with summary ({} chars)",
                        conversation_id,
                        summary.len()
                    );
                    TurnStatus::Completed
                }
                Err(err) => {
                    log::error!("Compaction commit failed for {}: {}", conversation_id, err);
                    TurnStatus::Failed
                }
            }
        }
        Err(StreamError::Cancelled(partial)) => {
            let usage_records = usage_ctx.usage_records(&partial.attempt_usages);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            TurnStatus::Cancelled
        }
        Err(StreamError::Provider(_, partial)) => {
            let usage_records = usage_ctx.usage_records(&partial.attempt_usages);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            TurnStatus::Failed
        }
        Err(StreamError::Timeout(partial)) => {
            let usage_records = usage_ctx.usage_records(&partial.attempt_usages);
            save_compaction_partial(app, &summary_msg_id, &partial, &usage_records);
            TurnStatus::Failed
        }
    };

    let terminal_error = match final_status {
        TurnStatus::Completed | TurnStatus::Cancelled => terminal_error,
        TurnStatus::Failed if terminal_error.is_none() => Some(ProviderErrorPayload::from(
            &ProviderError::RequestFailed("compaction commit failed".to_string()),
        )),
        TurnStatus::Failed => terminal_error,
        TurnStatus::Interrupted => terminal_error,
    };

    // 顺序契约：事务提交 → 终态事件（含 chat-messages-changed）→ 会话级终止 → 释放互斥。
    emitter.terminated(final_status, terminal_error);

    if final_status == TurnStatus::Completed {
        emit_broadcast(
            app,
            event_name::CONVERSATION_TERMINATED,
            ConversationTerminatedPayloadDto {
                conversation_id: conversation_id.to_string(),
                is_full: true,
            },
        );
    }

    tasks.finish(conversation_id, &attempt_id);

    if final_status == TurnStatus::Completed {
        Ok(())
    } else if final_status == TurnStatus::Cancelled {
        Err("Compaction cancelled".to_string())
    } else {
        Err("Compaction failed".to_string())
    }
}

fn commit_summary(
    app: &AppHandle,
    conversation_id: &str,
    summary_msg_id: &str,
    summary: &str,
    usage_records: &[assistant_storage::token_usage::TokenUsageRecord],
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    conversations::update_message_content(&tx, summary_msg_id, summary, None, None)?;
    conversations::set_conversation_is_full(&tx, conversation_id, true)?;
    conversations::update_active_branch_id(&tx, conversation_id, summary_msg_id)?;

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

    for record in usage_records {
        assistant_storage::token_usage::insert_record(&tx, record)?;
    }

    tx.commit().map_err(|e| e.to_string())
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
