use std::sync::atomic::{AtomicU32, Ordering};

use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::events::{emit_orb_only, event_name};
use crate::protocol::ChatStreamStartPayloadDto;
use assistant_agent::chat_runtime::{self, StreamError};
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

use super::chat_stream::{
    emit_delta, emit_stream_error, save_partial_on_error, spawn_compaction_listener,
    spawn_db_writer, spawn_finish_listener, DbWriteMsg, StreamEmitCtx,
};
use super::chat_utils::{resolve_context_window, resolve_final_tokens};

#[command]
pub fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    content: String,
    mode_id: Option<String>,
    branch_point_message_id: Option<String>,
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

    let detail_before = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;

    let msg_provider_id = detail_before.conversation.provider_id.clone();
    let msg_model_id = Some(detail_before.conversation.model_id.clone());

    let (_user_msg_id, assistant_msg_id) = if let Some(bp_id) = &branch_point_message_id {
        let branch_point = conversations::get_message_by_id(&conn, bp_id)?
            .ok_or_else(|| format!("Branch point message {} not found", bp_id))?;

        let bp_parent_id = branch_point.parent_id.clone();

        if branch_point.role == "user" {
            let user_parent_id = bp_parent_id.clone();
            let user_sibling_index = conversations::get_sibling_count(
                &conn,
                &conversation_id,
                user_parent_id.as_deref(),
            )?
            .max(0);

            let uid = Uuid::new_v4().to_string();
            conversations::insert_message(
                &conn,
                &MessageRow {
                    id: uid.clone(),
                    conversation_id: conversation_id.clone(),
                    role: "user".to_string(),
                    content: content.clone(),
                    tokens: None,
                    timestamp: now,
                    parent_id: user_parent_id,
                    is_pinned: false,
                    reasoning: None,
                    tool_calls_json: None,
                    sibling_index: user_sibling_index,
                    provider_id: None,
                    model_id: None,
                },
            )?;

            let aid = Uuid::new_v4().to_string();
            conversations::insert_message(
                &conn,
                &MessageRow {
                    id: aid.clone(),
                    conversation_id: conversation_id.clone(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    tokens: None,
                    timestamp: now + 1,
                    parent_id: Some(uid.clone()),
                    is_pinned: false,
                    reasoning: None,
                    tool_calls_json: None,
                    sibling_index: 0,
                    provider_id: msg_provider_id.clone(),
                    model_id: msg_model_id.clone(),
                },
            )?;

            (uid, aid)
        } else {
            let assistant_parent_id = bp_parent_id.clone();
            let assistant_sibling_index = conversations::get_sibling_count(
                &conn,
                &conversation_id,
                assistant_parent_id.as_deref(),
            )?
            .max(0);

            let aid = Uuid::new_v4().to_string();
            conversations::insert_message(
                &conn,
                &MessageRow {
                    id: aid.clone(),
                    conversation_id: conversation_id.clone(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    tokens: None,
                    timestamp: now,
                    parent_id: assistant_parent_id,
                    is_pinned: false,
                    reasoning: None,
                    tool_calls_json: None,
                    sibling_index: assistant_sibling_index,
                    provider_id: msg_provider_id.clone(),
                    model_id: msg_model_id.clone(),
                },
            )?;

            (String::new(), aid)
        }
    } else {
        let user_parent_id = detail_before.messages.last().map(|m| m.id.clone());
        let user_sibling_index =
            conversations::get_sibling_count(&conn, &conversation_id, user_parent_id.as_deref())?
                .max(0);

        let uid = Uuid::new_v4().to_string();
        conversations::insert_message(
            &conn,
            &MessageRow {
                id: uid.clone(),
                conversation_id: conversation_id.clone(),
                role: "user".to_string(),
                content: content.clone(),
                tokens: None,
                timestamp: now,
                parent_id: user_parent_id,
                is_pinned: false,
                reasoning: None,
                tool_calls_json: None,
                sibling_index: user_sibling_index,
                provider_id: None,
                model_id: None,
            },
        )?;

        let aid = Uuid::new_v4().to_string();
        conversations::insert_message(
            &conn,
            &MessageRow {
                id: aid.clone(),
                conversation_id: conversation_id.clone(),
                role: "assistant".to_string(),
                content: String::new(),
                tokens: None,
                timestamp: now + 1,
                parent_id: Some(uid.clone()),
                is_pinned: false,
                reasoning: None,
                tool_calls_json: None,
                sibling_index: 0,
                provider_id: msg_provider_id.clone(),
                model_id: msg_model_id.clone(),
            },
        )?;

        (uid, aid)
    };

    conversations::update_active_branch_id(&conn, &conversation_id, &assistant_msg_id)?;

    let detail = conversations::get_conversation_detail(&conn, &conversation_id)?
        .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
    let provider_id = detail
        .conversation
        .provider_id
        .clone()
        .ok_or_else(|| assistant_i18n::t("chat.provider_deleted"))?;
    let provider_row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| assistant_i18n::t("error.provider_not_found"))?;

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
    emit_orb_only(
        &app,
        event_name::CHAT_STREAM_START,
        ChatStreamStartPayloadDto {
            conversation_id: conversation_id.clone(),
            attempt_id: attempt_id.clone(),
            message_id: assistant_msg_id.clone(),
        },
    );

    let (auto_compact, compact_threshold_percent) = assistant_config::app_conf::AppConf::load()
        .map(|c| (c.auto_compact, c.compact_threshold_percent))
        .unwrap_or((
            true,
            assistant_config::constant::COMPACTION_THRESHOLD_PERCENT,
        ));

    let db_tx = spawn_db_writer(&app);
    let finish_tx = spawn_finish_listener(&app);
    let compact_tx = spawn_compaction_listener(&app);

    let conv_id = conversation_id.clone();
    let msg_id = assistant_msg_id.clone();
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