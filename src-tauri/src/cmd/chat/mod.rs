mod insert;
mod stream;

use std::sync::atomic::{AtomicU32, Ordering};

use tauri::{command, AppHandle, Manager};

use crate::protocol::events::{emit_orb_only, event_name};
use crate::protocol::{ChatStreamStartPayloadDto, ChatUserMessageConfirmedPayloadDto};
use assistant_agent::chat_request::{self, ChatRequest};
use assistant_agent::chat_runtime::{self, resolve_final_tokens, StreamError};
use assistant_agent::chat_tasks::ChatTasks;
use assistant_agent::compaction;
use assistant_agent::multimodal;
use assistant_config::constant::WINDOW_ORB;
use assistant_providers::base::{ProviderError, ProviderErrorPayload};
use assistant_storage::attachment_store::AttachmentStore;
use serde_json;

use insert::InsertedMessages;
use stream::{
    emit_delta, emit_stream_error, save_partial_on_error, spawn_compaction_listener,
    spawn_db_writer, spawn_finish_listener, DbWriteMsg, StreamEmitCtx,
};

#[command]
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    mode_id: Option<String>,
    branch_point_message_id: Option<String>,
    provider_id: String,
    model_id: String,
) -> Result<(), String> {
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
            &conversation_id,
            &content,
            mode_id.as_deref(),
            branch_point_message_id.as_deref(),
            &provider_id,
            &model_id,
        )?
    };

    let attachment_store = app.state::<AttachmentStore>();
    let req = chat_request::build_chat_request(
        &detail,
        mode.as_ref(),
        &provider_row,
        &model_id,
        &attachment_store,
    )
    .await?;

    spawn_chat_stream(
        &app,
        &conversation_id,
        &assistant_msg_id,
        user_msg_id.as_deref(),
        &content,
        req,
        detail,
        provider_row,
    )
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
        .map_err(|e| e.to_string())?;

    Ok(content)
}

fn spawn_chat_stream(
    app: &AppHandle,
    conversation_id: &str,
    assistant_msg_id: &str,
    user_msg_id: Option<&str>,
    content: &str,
    request: ChatRequest,
    detail: assistant_storage::conversations::ConversationDetail,
    provider_row: assistant_storage::providers::ProviderRow,
) -> Result<(), String> {
    let ChatRequest {
        chat_messages,
        adapter,
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

    let (auto_compact, compact_threshold_percent) = assistant_config::app_conf::AppConf::load()
        .map(|c| (c.auto_compact, c.compact_threshold_percent))
        .unwrap_or((
            true,
            assistant_config::constant::COMPACTION_THRESHOLD_PERCENT,
        ));

    let db_tx = spawn_db_writer(app);
    let finish_tx = spawn_finish_listener(app);
    let compact_tx = spawn_compaction_listener(app);

    let conv_id = conversation_id.to_string();
    let msg_id = assistant_msg_id.to_string();
    let attempt_id_clone = attempt_id.clone();

    let stream_future = async move {
        let stream = match adapter.chat_completion_stream(
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
                let tokens = resolve_final_tokens(&outcome);
                let reasoning = if kind.persists_reasoning() && !outcome.full_reasoning.is_empty() {
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
