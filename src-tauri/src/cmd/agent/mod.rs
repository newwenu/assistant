use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use assistant_agent::chat_options::ReasoningOverride;
use assistant_agent::kernel::{
    EventSink, TurnContext, TurnEvent, TurnInputSource, TurnSpec, TurnTasks,
};
use assistant_agent::multimodal;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::ToolRegistry;

use crate::adapters::{build_turn_runner, RegistryToolCatalog, TauriEventSink, TauriInputSource};
use crate::protocol::SendChatMessageArgsDto;

#[command]
pub async fn send_agent_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), String> {
    let registry = app.state::<Arc<ToolRegistry>>().inner().clone();
    let mut spec = TurnSpec::agent();
    spec.tools = Some(Arc::new(RegistryToolCatalog::new(registry)));
    start_turn(&app, &conversation_id, args, spec).await
}

#[command]
pub fn cancel_agent(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    tasks.cancel(&conversation_id);
    Ok(())
}

/// 宿主对一次发送只做四件事：入参校验、建消息、`begin`、`spawn`。
/// 其余编排全部在内核里，`spec` 是唯一的链路差异来源。
pub(crate) async fn start_turn(
    app: &AppHandle,
    conversation_id: &str,
    args: SendChatMessageArgsDto,
    spec: TurnSpec,
) -> Result<(), String> {
    let SendChatMessageArgsDto {
        content,
        mode_id,
        branch_point_message_id,
        provider_id,
        model_id,
        reasoning_level,
    } = args;

    let content = validate_and_preprocess(app, &content).await?;

    // `begin` 先于建消息：`Reject` 链路被拒时不得在库里留下孤儿消息。
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    let (turn_id, _token) = tasks
        .begin(conversation_id, spec.on_busy)
        .map_err(|_| assistant_i18n::t("chat.session_busy"))?;

    let inserted = {
        let state = app.state::<crate::setup::DbState>();
        super::get_db_conn(&state).and_then(|conn| {
            super::chat::insert::insert_chat_messages(
                &conn,
                super::chat::insert::InsertChatInput {
                    conversation_id,
                    content: &content,
                    branch_point_message_id: branch_point_message_id.as_deref(),
                    provider_id: &provider_id,
                    model_id: &model_id,
                },
            )
        })
    };
    let inserted = match inserted {
        Ok(inserted) => inserted,
        Err(err) => {
            tasks.finish(conversation_id, &turn_id);
            return Err(err);
        }
    };

    let sink: Arc<dyn EventSink> = Arc::new(
        TauriEventSink::new(
            app.clone(),
            spec.route,
            conversation_id,
            inserted.assistant_msg_id.clone(),
        )
        .with_user_message_content(&content),
    );

    if let Some(user_msg_id) = inserted.user_msg_id.clone() {
        sink.emit(TurnEvent::UserMessageConfirmed {
            conversation_id: conversation_id.to_string(),
            user_msg_id,
        });
    }

    let input: Arc<dyn TurnInputSource> = Arc::new(TauriInputSource::new(
        app.clone(),
        mode_id,
        ReasoningOverride {
            level: reasoning_level,
        },
    ));
    let runner = build_turn_runner(app, sink, input);

    let ctx = TurnContext::new(conversation_id, turn_id, inserted.assistant_msg_id);

    tauri::async_runtime::spawn(async move {
        let outcome = runner.run(&ctx, &spec).await;
        log::debug!(
            "turn {} finished: status={:?} iterations={} model_calls={}",
            ctx.turn_id,
            outcome.status,
            outcome.iterations,
            outcome.model_calls
        );
    });

    Ok(())
}

async fn validate_and_preprocess(app: &AppHandle, content: &str) -> Result<String, String> {
    if super::chat::insert::is_content_empty(content) {
        return Err(assistant_i18n::t("chat.empty_content"));
    }

    let attachment_store = app.state::<AttachmentStore>();
    let config = multimodal::MultimodalConfig::trusted_channel_input();
    let content = multimodal::admit_attachments(content, &attachment_store, &config)
        .await
        .map_err(super::translate_agent_error)?;

    Ok(content)
}
