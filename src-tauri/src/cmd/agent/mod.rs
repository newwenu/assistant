use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use assistant_agent::chat_options::ReasoningOverride;
use assistant_agent::kernel::{
    EventSink, TurnContext, TurnEvent, TurnInputSource, TurnSpec, TurnTasks,
};
use assistant_agent::multimodal;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::{ApprovalDecision, ToolRegistry};

use crate::adapters::{
    build_turn_runner, resolve_available_tools, PendingApprovalRegistry, TauriEventSink,
    TauriInputSource,
};
use crate::protocol::{SendChatMessageArgsDto, ToolApprovalDecisionDto, ToolApprovalKindDto};

#[command]
pub async fn send_agent_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), String> {
    let registry = app.state::<Arc<ToolRegistry>>().inner().clone();

    // 缺省配置即 deny-by-default：空白名单 → 空工具集，模型看不到任何工具。
    let tool_config = args.tool_config.clone().unwrap_or_default();
    let tools = resolve_available_tools(
        &registry,
        &tool_config.enabled_tools,
        &tool_config.disabled_tools,
        tool_config.max_tool_tier.into(),
    )
    .await;

    let mut spec = TurnSpec::agent();
    spec.tools = Some(tools);
    start_turn(&app, &conversation_id, args, spec).await
}

#[command]
pub fn cancel_agent(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    tasks.cancel(&conversation_id);
    Ok(())
}

/// 审批回填：把前端卡片上的决策投递给正在等待的网关。
///
/// 只做「找到挂起点 → 投递」，不在这里写 `permission_grants`：
/// `AllowSession` / `AllowAlways` 落授权由 `ToolGateway` 统一处理，命令层不重复。
/// 找不到挂起点（已超时 / 已决策 / `conversationId` 不匹配）返回错误，
/// 让前端知道这次点击无效，而不是假装成功。
#[command]
pub fn resolve_agent_tool_call(
    app: AppHandle,
    args: ToolApprovalDecisionDto,
) -> Result<(), String> {
    let pending = app.state::<PendingApprovalRegistry>();
    let decision = match args.decision {
        ToolApprovalKindDto::Approve => ApprovalDecision::Approve,
        ToolApprovalKindDto::Deny => ApprovalDecision::Deny {
            reason: args.reason.clone(),
        },
        ToolApprovalKindDto::AllowSession => ApprovalDecision::AllowSession,
        ToolApprovalKindDto::AllowAlways => ApprovalDecision::AllowAlways,
    };
    if pending.resolve(&args.call_id, &args.conversation_id, decision) {
        Ok(())
    } else {
        Err(format!(
            "no pending approval for call '{}' (already decided or timed out)",
            args.call_id
        ))
    }
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
        tool_config: _,
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
