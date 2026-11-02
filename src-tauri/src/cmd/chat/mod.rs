mod insert;
mod stream;

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{command, AppHandle, Manager};

use crate::protocol::events::{emit_orb_only, event_name};
use crate::protocol::{
    ChatStreamStartPayloadDto, ChatUserMessageConfirmedPayloadDto, SendChatMessageArgsDto,
};
use assistant_agent::chat_options;
use assistant_agent::chat_request::{self, ChatRequest};
use assistant_agent::chat_runtime::{self, resolve_final_tokens, StreamError};
use assistant_agent::chat_tasks::ChatTasks;
use assistant_agent::compaction;
use assistant_agent::multimodal;
use assistant_config::constant::WINDOW_ORB;
use assistant_crypto::api_key as crypto;
use assistant_providers::base::{ProviderError, ProviderErrorPayload};
use assistant_storage::attachment_store::AttachmentStore;

use crate::cmd::token_usage_record::{build_usage_records, cache_capable_of, UsageRecordCtx};
use insert::InsertedMessages;
use stream::{
    emit_delta, emit_stream_error, save_partial_on_error, spawn_compaction_listener,
    spawn_db_writer, spawn_finish_listener, DbWriteMsg, StreamEmitCtx,
};

#[command]
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), String> {
    let SendChatMessageArgsDto {
        content,
        mode_id,
        branch_point_message_id,
        provider_id,
        model_id,
        reasoning_level,
    } = args;

    let content = validate_and_preprocess(&app, &content, &conversation_id).await?;

    let InsertedMessages {
        mode,
        assistant_msg_id,
        user_msg_id,
        detail,
        provider_row,
    } = {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        insert::insert_chat_messages(
            &conn,
            insert::InsertChatInput {
                conversation_id: &conversation_id,
                content: &content,
                mode_id: mode_id.as_deref(),
                branch_point_message_id: branch_point_message_id.as_deref(),
                provider_id: &provider_id,
                model_id: &model_id,
            },
        )?
    };

    let attachment_store = app.state::<AttachmentStore>();
    let machine_id = crypto::get_machine_id().map_err(|e| e.to_string())?;
    let api_key = crypto::decrypt_api_key_b64(&provider_row.api_key_encrypted, &machine_id)
        .map_err(|e| e.to_string())?;
    let reasoning = chat_options::ReasoningOverride {
        level: reasoning_level,
    };
    let req = chat_request::build_chat_request(
        &detail,
        mode.as_ref(),
        &provider_row,
        &model_id,
        &attachment_store,
        api_key,
        &reasoning,
    )
    .await?;

    spawn_chat_stream(ChatStreamSpawn {
        app: &app,
        conversation_id: &conversation_id,
        assistant_msg_id: &assistant_msg_id,
        user_msg_id: user_msg_id.as_deref(),
        content: &content,
        request: req,
        detail,
        provider_row,
    })
}

async fn validate_and_preprocess(
    app: &AppHandle,
    content: &str,
    conversation_id: &str,
) -> Result<String, String> {
    if insert::is_content_empty(content) {
        return Err(assistant_i18n::t("chat.empty_content"));
    }
    if compaction::is_terminating(conversation_id) {
        return Err("Conversation is being terminated".to_string());
    }

    let attachment_store = app.state::<AttachmentStore>();
    let config = multimodal::MultimodalConfig::trusted_channel_input();
    let content = multimodal::admit_attachments(content, &attachment_store, &config)
        .await
        .map_err(super::translate_agent_error)?;

    Ok(content)
}

struct ChatStreamSpawn<'a> {
    app: &'a AppHandle,
    conversation_id: &'a str,
    assistant_msg_id: &'a str,
    user_msg_id: Option<&'a str>,
    content: &'a str,
    request: ChatRequest,
    detail: assistant_storage::conversations::ConversationDetail,
    provider_row: assistant_storage::providers::ProviderRow,
}

fn spawn_chat_stream(spawn: ChatStreamSpawn<'_>) -> Result<(), String> {
    let ChatStreamSpawn {
        app,
        conversation_id,
        assistant_msg_id,
        user_msg_id,
        content,
        request,
        detail,
        provider_row,
    } = spawn;

    let ChatRequest {
        chat_messages,
        adapter,
        api_key,
        model,
        options,
        context_window_tokens,
        kind,
    } = request;

    let chat_tasks: tauri::State<'_, ChatTasks> = app.state::<ChatTasks>();
    let (cancel_token, attempt_id) = chat_tasks.start(conversation_id.to_string());

    let orb_window = app
        .get_webview_window(WINDOW_ORB)
        .ok_or_else(|| assistant_i18n::t("error.orb_window_not_found"))?;

    if let Some(uid) = user_msg_id {
        emit_orb_only(
            app,
            event_name::CHAT_USER_MESSAGE_CONFIRMED,
            ChatUserMessageConfirmedPayloadDto {
                conversation_id: conversation_id.to_string(),
                user_message_id: uid.to_string(),
                user_message_content: content.to_string(),
            },
        );
    }

    emit_orb_only(
        app,
        event_name::CHAT_STREAM_START,
        ChatStreamStartPayloadDto {
            conversation_id: conversation_id.to_string(),
            attempt_id: attempt_id.clone(),
            message_id: assistant_msg_id.to_string(),
        },
    );

    let (auto_compact, compact_threshold_percent_of_budget) =
        assistant_config::app_conf::AppConf::load()
            .map(|c| (c.auto_compact, c.compact_threshold_percent_of_budget))
            .unwrap_or((
                true,
                assistant_config::constant::COMPACTION_THRESHOLD_PERCENT,
            ));
    let reserved_output_tokens = options.max_tokens;

    let db_tx = spawn_db_writer(app);
    let finish_tx = spawn_finish_listener(app);
    let compact_tx = spawn_compaction_listener(app);

    let conv_id = conversation_id.to_string();
    let msg_id = assistant_msg_id.to_string();
    let attempt_id_clone = attempt_id.clone();
    let branch_leaf_id = detail.conversation.active_branch_id.clone();
    let cache_capable = cache_capable_of(&provider_row);

    let stream_future = async move {
        let stream = match adapter.chat_completion_stream(
            &api_key,
            &chat_messages,
            &model,
            &options,
            cancel_token.clone(),
        ) {
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
                let usage_ctx = UsageRecordCtx {
                    conv_id: &conv_id,
                    msg_id: &msg_id,
                    attempt_id: &attempt_id_clone,
                    branch_leaf_id: branch_leaf_id.as_deref(),
                    kind: "chat",
                    provider_id: Some(provider_row.id.as_str()),
                    model_id: Some(model.as_str()),
                    cache_capable,
                };
                let usage_records = build_usage_records(&outcome.attempt_usages, &usage_ctx);
                let tokens = resolve_final_tokens(&outcome);
                let reasoning = if kind.persists_reasoning() && !outcome.full_reasoning.is_empty() {
                    Some(outcome.full_reasoning)
                } else {
                    None
                };
                let _ = db_tx.send(DbWriteMsg {
                    msg_id: msg_id.clone(),
                    content: outcome.full_content.clone(),
                    tokens,
                    reasoning,
                    usage_records,
                });
                if let Some(pt) = outcome.prompt_tokens {
                    match compaction::evaluate_compaction(
                        pt,
                        context_window_tokens,
                        auto_compact,
                        compact_threshold_percent_of_budget,
                        reserved_output_tokens,
                    ) {
                        compaction::CompactDecision::Terminate => {
                            let _ = compact_tx.send((conv_id.clone(), detail, provider_row));
                        }
                        compaction::CompactDecision::Undetermined => {
                            log::warn!(
                                "[compact-decision] conversation={} 窗口未知，无法判定是否超限；不触发自动终止（P={}）",
                                conv_id,
                                pt
                            );
                        }
                        compaction::CompactDecision::NotNeeded => {}
                    }
                }
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Cancelled(partial)) => {
                let usage_ctx = UsageRecordCtx {
                    conv_id: &conv_id,
                    msg_id: &msg_id,
                    attempt_id: &attempt_id_clone,
                    branch_leaf_id: branch_leaf_id.as_deref(),
                    kind: "chat",
                    provider_id: Some(provider_row.id.as_str()),
                    model_id: Some(model.as_str()),
                    cache_capable,
                };
                let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
                save_partial_on_error(&db_tx, &msg_id, &partial, usage_records);
                emit_stream_error(
                    &ectx,
                    seq_val,
                    &ProviderErrorPayload::from(&ProviderError::Cancelled),
                );
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Provider(e, partial)) => {
                let usage_ctx = UsageRecordCtx {
                    conv_id: &conv_id,
                    msg_id: &msg_id,
                    attempt_id: &attempt_id_clone,
                    branch_leaf_id: branch_leaf_id.as_deref(),
                    kind: "chat",
                    provider_id: Some(provider_row.id.as_str()),
                    model_id: Some(model.as_str()),
                    cache_capable,
                };
                let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
                save_partial_on_error(&db_tx, &msg_id, &partial, usage_records);
                emit_stream_error(&ectx, seq_val, &ProviderErrorPayload::from(&e));
                let _ = finish_tx.send(conv_id);
            }
            Err(StreamError::Timeout(partial)) => {
                let usage_ctx = UsageRecordCtx {
                    conv_id: &conv_id,
                    msg_id: &msg_id,
                    attempt_id: &attempt_id_clone,
                    branch_leaf_id: branch_leaf_id.as_deref(),
                    kind: "chat",
                    provider_id: Some(provider_row.id.as_str()),
                    model_id: Some(model.as_str()),
                    cache_capable,
                };
                let usage_records = build_usage_records(&partial.attempt_usages, &usage_ctx);
                save_partial_on_error(&db_tx, &msg_id, &partial, usage_records);
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
