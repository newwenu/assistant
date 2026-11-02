use assistant_providers::types;
use assistant_storage::{
    agent_modes::AgentModeRow, attachment_store::AttachmentStore,
    conversations::ConversationDetail, providers::ProviderRow,
};

use assistant_providers::base::{self, ProviderKind};

use crate::chat_options::{build_chat_options, ReasoningOverride, DEFAULT_MAX_TOKENS};
use crate::multimodal::{self, ProjectionPurpose};
use crate::prompt_builder;
use crate::provider_config::build_provider_config;

pub struct ChatRequest {
    pub chat_messages: Vec<types::ChatMessage>,
    pub adapter: Box<dyn base::BaseProviderAdapter>,
    pub api_key: String,
    pub model: String,
    pub options: types::ChatOptions,
    pub context_window_tokens: Option<u32>,
    pub kind: prompt_builder::AgentKind,
}

pub async fn build_chat_request(
    detail: &ConversationDetail,
    mode: Option<&AgentModeRow>,
    provider_row: &ProviderRow,
    model_id: &str,
    attachment_store: &AttachmentStore,
    api_key: String,
    reasoning: &ReasoningOverride,
) -> Result<ChatRequest, String> {
    let system_prompt = mode.map(|m| m.prompt.as_str()).unwrap_or("");

    let kind = mode
        .map(|m| prompt_builder::AgentKind::from_mode_kind(&m.kind))
        .unwrap_or_default();
    let context_window_tokens =
        types::resolve_context_window_from_stored(model_id, &provider_row.models_json);
    let options = build_chat_options(
        &kind,
        mode.unwrap_or(&AgentModeRow::default()),
        context_window_tokens,
        reasoning,
    );

    // offload 预算 = 窗口扣除输出预留；窗口未知时预算不可判定，不做裁剪（不得按完整窗口裁剪）
    let offload_budget = context_window_tokens
        .filter(|&w| w > 0)
        .map(|w| w.saturating_sub(options.max_tokens) as usize);

    let chat_messages = project_chat_messages(
        detail,
        system_prompt,
        provider_row,
        model_id,
        attachment_store,
        offload_budget,
        ProjectionPurpose::Send,
    )
    .await;

    let config = build_provider_config(provider_row);
    let adapter = base::create_provider_adapter(config);

    Ok(ChatRequest {
        chat_messages,
        adapter,
        api_key,
        model: model_id.to_string(),
        options,
        context_window_tokens,
        kind,
    })
}

/// 出站消息投影：system prompt + 历史消息 + 附件投影 + 超预算降级。
///
/// 发送路径与「占用展示兜底」共用本函数，保证两者看到的是同一份消息。
#[allow(clippy::too_many_arguments)]
pub async fn project_chat_messages(
    detail: &ConversationDetail,
    system_prompt: &str,
    provider_row: &ProviderRow,
    model_id: &str,
    attachment_store: &AttachmentStore,
    offload_budget: Option<usize>,
    purpose: ProjectionPurpose,
) -> Vec<types::ChatMessage> {
    let mut chat_messages = prompt_builder::prepare_messages(detail, system_prompt, None);

    let support_vision =
        types::resolve_support_vision_from_stored(model_id, &provider_row.models_json);
    let support_pdf = resolve_native_pdf_support(provider_row, model_id);

    if support_vision {
        multimodal::rehydrate_images(&mut chat_messages, attachment_store).await;
    } else {
        multimodal::project_images_for_text_model(&mut chat_messages);
    }

    // 文件预算 = 输入预算 - 非文件内容（system / 文本 / 图片 / 工具结果）。
    // 之后再按新消息优先分配文件配额，避免文件与文本重复占用同一份预算。
    let non_file_tokens = crate::token_estimate::estimate_non_file_tokens(&chat_messages);
    let file_budget = offload_budget.map(|b| b.saturating_sub(non_file_tokens));

    multimodal::project_files(
        &mut chat_messages,
        attachment_store,
        support_pdf,
        file_budget,
        purpose,
    )
    .await;

    crate::request_projection::offload_if_over_budget(&mut chat_messages, offload_budget, purpose);
    chat_messages
}

/// 原生 PDF 能力 = 模型声明支持 PDF + 目标端点确实能发送 PDF。
///
/// OpenAI / 兼容协议的 `/chat/completions` 没有原生 PDF 字段，不得先读盘、
/// base64 再由转换器丢弃；Anthropic / Gemini / Responses 可发送原生附件。
fn resolve_native_pdf_support(provider_row: &ProviderRow, model_id: &str) -> bool {
    if !types::resolve_support_pdf_from_stored(model_id, &provider_row.models_json) {
        return false;
    }
    match base::provider_kind_from_str(&provider_row.type_, provider_row.base_url.clone()) {
        ProviderKind::Anthropic
        | ProviderKind::Gemini
        | ProviderKind::GeminiCodeAssist
        | ProviderKind::OpenAIResponses => true,
        ProviderKind::OpenAI | ProviderKind::Custom { .. } => false,
    }
}

/// 分支零记录时的占用投影：只读，不构造 adapter、不发起请求、不落库。
///
/// 查询期拿不到本次发送所选的 mode，因此 system prompt 取会话持久化的
/// `system_prompt`（与压缩路径一致）；该值缺失时按空串处理。
/// 输出预留按默认 `max_tokens` 计算，保证展示与发送使用同一档预算。
pub async fn estimate_branch_tokens(
    detail: &ConversationDetail,
    provider_row: &ProviderRow,
    attachment_store: &AttachmentStore,
) -> usize {
    let system_prompt = detail
        .conversation
        .system_prompt
        .clone()
        .unwrap_or_default();
    let model_id = detail.conversation.model_id.clone();
    let context_window_tokens =
        types::resolve_context_window_from_stored(&model_id, &provider_row.models_json);
    let offload_budget = context_window_tokens
        .filter(|&w| w > 0)
        .map(|w| w.saturating_sub(DEFAULT_MAX_TOKENS) as usize);
    let messages = project_chat_messages(
        detail,
        &system_prompt,
        provider_row,
        &model_id,
        attachment_store,
        offload_budget,
        ProjectionPurpose::Estimate,
    )
    .await;
    crate::request_projection::estimate_total_tokens(&messages)
}
