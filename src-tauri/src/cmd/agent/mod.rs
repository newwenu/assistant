use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use assistant_agent::chat_options::ReasoningOverride;
use assistant_agent::kernel::{
    EventSink, TurnContext, TurnEvent, TurnInputSource, TurnSpec, TurnTasks,
};
use assistant_agent::multimodal;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::{ApprovalDecision, QuestionAnswerItem, ToolRegistry};

use crate::adapters::{
    build_turn_runner, policy_for_tool_config, resolve_available_tools, PendingApprovalRegistry,
    PendingQuestionRegistry, PipelineToolExecution, TauriEventSink, TauriInputSource,
};
use crate::protocol::{
    AgentQuestionAnswerDto, AgentToolCallDto, ConversationToolConfigDto, SendChatMessageArgsDto,
    ToolApprovalDecisionDto, ToolApprovalKindDto, ToolDescriptorDto,
};

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

    // 权限策略随本次发送的配置逐轮现算（I38d §4.6）：审批级别 → 主规则集，
    // 层级上限 → 约束层，项目级 `.assistant/permissions.json` → 额外约束层。
    let tool_exec = policy_executor(&app, &conversation_id, &tool_config);

    let mut spec = TurnSpec::agent();
    spec.tools = Some(tools);
    start_turn(&app, &conversation_id, args, spec, Some(tool_exec)).await
}

/// 从宿主托管的基底执行器派生「本轮策略」执行器；共享 `pipeline` 与审批等待表，
/// 不把策略写回全局单例，因此改配置后下一轮即生效。
fn policy_executor(
    app: &AppHandle,
    conversation_id: &str,
    tool_config: &ConversationToolConfigDto,
) -> Arc<dyn assistant_agent::agent::round_executor::ToolExecutionPort> {
    let base = app.state::<Arc<PipelineToolExecution>>().inner().clone();
    let policy =
        policy_for_tool_config(tool_config, workspace_root(app, conversation_id).as_deref());
    Arc::new(base.with_policy(policy))
}

/// 工作区根目录：与执行期同源（`tool_setup::resolve_workspace`），
/// 保证项目级 `.assistant/permissions.json` 与工具边界锚在同一目录（I38h §4 #4）。
fn workspace_root(app: &AppHandle, conversation_id: &str) -> Option<String> {
    let state = app.state::<crate::setup::DbState>();
    let resolved = match state.0.lock() {
        Ok(conn) => crate::tool_setup::resolve_workspace(&conn, conversation_id),
        Err(e) => {
            log::warn!("Failed to lock db for workspace resolve: {e}");
            None
        }
    };
    resolved
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

/// 提问回填：把前端卡片上的答案投递给正在等待的 `ask_user`。
///
/// 与 `resolve_agent_tool_call` 对称：只做「找到挂起点 → 投递」，
/// 收口（超时 / 取消 / 继续执行）全部在工具与 turn 侧完成。
/// 提交无效（已超时 / 已作答 / 会话不匹配 / 答案条数不符）返回错误，
/// 让前端提示这次提交无效，而不是假装成功。
#[command]
pub fn resolve_agent_question(app: AppHandle, args: AgentQuestionAnswerDto) -> Result<(), String> {
    let pending = app.state::<PendingQuestionRegistry>();
    let answers: Vec<QuestionAnswerItem> = args
        .answers
        .into_iter()
        .map(|item| QuestionAnswerItem::new(item.selected, item.other))
        .collect();
    pending.resolve(&args.call_id, &args.conversation_id, answers)
}

/// 挂起提问的补拉：窗口重连 / 切回运行中的会话时，用快照重建 `ask_user` 卡片。
///
/// 提问事件是广播、不重放：前端漏收（事件早于 `setActiveAttemptId`、窗口重建、
/// 会话切换回来）就再也无从得知，后端只能等到超时。补拉是唯一恢复途径，
/// 因此 `timeoutMs` 由后端改写为剩余时长，前端倒计时与真实 deadline 对齐。
#[command]
pub fn list_pending_questions(
    app: AppHandle,
    conversation_id: String,
) -> Result<Vec<AgentToolCallDto>, String> {
    let pending = app.state::<PendingQuestionRegistry>();
    Ok(pending.snapshot(&conversation_id))
}

/// 工具装配清单：返回全部 `agent` 受众工具（含 `tier`），供 `ToolConfigPanel` 分组多选。
///
/// 不按层级上限过滤：面板要能显示「已选但超出上限、将被忽略」的工具。
/// 白名单仍由 `resolve_available_tools` 在发送时按 `max_tool_tier` 过滤，二者口径不同。
#[command]
pub async fn list_available_tools(app: AppHandle) -> Result<Vec<ToolDescriptorDto>, String> {
    let registry = app.state::<Arc<ToolRegistry>>().inner().clone();
    Ok(registry
        .agent_tool_descriptors()
        .await
        .into_iter()
        .map(ToolDescriptorDto::from)
        .collect())
}

/// 宿主对一次发送只做四件事：入参校验、建消息、`begin`、`spawn`。
/// 其余编排全部在内核里，`spec` 是唯一的链路差异来源。
pub(crate) async fn start_turn(
    app: &AppHandle,
    conversation_id: &str,
    args: SendChatMessageArgsDto,
    spec: TurnSpec,
    tool_exec: Option<Arc<dyn assistant_agent::agent::round_executor::ToolExecutionPort>>,
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
                    mode_id: mode_id.as_deref(),
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
    let runner = build_turn_runner(app, sink, input, tool_exec);

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
