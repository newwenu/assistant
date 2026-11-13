use std::sync::Arc;

use assistant_providers::base;
use assistant_providers::capability::{CapabilityFact, Support};
use assistant_providers::types;
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::conversations::ConversationDetail;
use assistant_storage::providers::ProviderRow;

use crate::chat_options::{build_chat_options, DEFAULT_MAX_TOKENS};
use crate::multimodal::{self, ProjectionPurpose};
use crate::prompt_builder::{self, AgentKind, PromptFacts};
use crate::request_projection;

use super::ports::TurnInput;

/// provider 中立流类型，仅在此模块内用于收敛 `stream()` 的返回签名。
type ProviderStream = std::pin::Pin<
    Box<dyn futures::Stream<Item = Result<base::ChatDelta, base::ProviderError>> + Send>,
>;

/// 一次模型调用所需的全部输入；构建后不再触碰 DB。
#[derive(Clone)]
pub struct ChatRequest {
    pub chat_messages: Vec<types::ChatMessage>,
    /// 由输入端口提供，`build()` 只消费不构造（H1）。
    pub adapter: Arc<dyn base::BaseProviderAdapter>,
    pub api_key: String,
    pub model: String,
    pub options: types::ChatOptions,
    pub context_window_tokens: Option<u32>,
    pub kind: AgentKind,
}

impl ChatRequest {
    pub fn stream(
        &self,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Result<ProviderStream, base::ProviderError> {
        self.adapter.chat_completion_stream(
            &self.api_key,
            &self.chat_messages,
            &self.model,
            &self.options,
            cancel_token,
        )
    }
}

/// 顺序固定：投影 → 多模态 → 提示词 → tools 注入。
///
/// tools 只在 `AgentKind::supports_tool_call()` 为真且清单非空时注入；
/// 空清单视为无工具，循环退化为单次调用。
pub async fn build(
    input: &TurnInput,
    iteration: u32,
    tools: Option<&[types::ToolSpec]>,
) -> Result<ChatRequest, String> {
    let _ = iteration;

    let detail = &input.detail;
    let provider_row = &input.provider.row;
    let model_id = detail.conversation.model_id.clone();
    let mode: Option<&AgentModeRow> = input.mode.as_ref();

    let kind = mode
        .map(|m| AgentKind::from_mode_kind(&m.kind))
        .unwrap_or_default();
    // 环境事实只对 agent 链路可见（I38d §4.9.1）：chat 会话没有工具，路径事实无用且徒增常驻 token。
    let facts = if kind.supports_tool_call() {
        &input.prompt_facts
    } else {
        &prompt_builder::NO_FACTS
    };
    let system_prompt =
        prompt_builder::compose_system_prompt(mode.map(|m| m.prompt.as_str()).unwrap_or(""), facts);
    let context_window_tokens =
        types::resolve_context_window_from_stored(&model_id, &provider_row.models_json);
    let tools = match tools {
        Some(tools) if kind.supports_tool_call() => resolve_tools(tools),
        _ => None,
    };
    let options = build_chat_options(
        mode.unwrap_or(&AgentModeRow::default()),
        context_window_tokens,
        &input.reasoning,
        tools,
    );

    let offload_budget = offload_budget_of(context_window_tokens, options.max_tokens);
    let chat_messages = project_chat_messages(
        detail,
        &system_prompt,
        provider_row,
        &model_id,
        &input.attachments,
        offload_budget,
        ProjectionPurpose::Send,
        None,
    )
    .await;

    let adapter = input.adapter.clone();

    Ok(ChatRequest {
        chat_messages,
        adapter,
        api_key: input.provider.api_key.clone(),
        model: model_id,
        options,
        context_window_tokens,
        kind,
    })
}

/// 空清单视为无工具：`tools = None`，循环退化为单次调用。
fn resolve_tools(tools: &[types::ToolSpec]) -> Option<Vec<types::ToolSpec>> {
    if tools.is_empty() {
        None
    } else {
        Some(tools.to_vec())
    }
}

/// offload 预算 = 窗口扣除输出预留；窗口未知时预算不可判定，不做裁剪。
pub fn offload_budget_of(context_window_tokens: Option<u32>, max_tokens: u32) -> Option<usize> {
    context_window_tokens
        .filter(|&w| w > 0)
        .map(|w| w.saturating_sub(max_tokens) as usize)
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
    append_user_prompt: Option<&str>,
) -> Vec<types::ChatMessage> {
    let mut chat_messages =
        prompt_builder::prepare_messages(detail, system_prompt, append_user_prompt);

    let vision = types::resolve_vision_from_stored(model_id, &provider_row.models_json);
    let pdf = types::resolve_pdf_from_stored(model_id, &provider_row.models_json);

    let (image_projection, vision_warning) = image_projection(&vision, model_id);
    if let Some(message) = vision_warning {
        log::warn!("{message}");
    }
    match image_projection {
        ImageProjection::Rehydrate => {
            multimodal::rehydrate_images(&mut chat_messages, attachment_store).await;
        }
        ImageProjection::ProjectToText => {
            multimodal::project_images_for_text_model(&mut chat_messages);
        }
        // 未知不放行也不拦死：保留图片，由服务端判定；上面已记一条降级日志。
        ImageProjection::Keep => {}
    }

    let (support_pdf, pdf_warning) = native_pdf_support(provider_row, &pdf, model_id);
    if let Some(message) = pdf_warning {
        log::warn!("{message}");
    }

    // 文件预算 = 输入预算 - 非文件内容（system / 文本 / 图片 / 工具结果）。
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

    request_projection::offload_if_over_budget(&mut chat_messages, offload_budget, purpose);
    chat_messages
}

/// 图片投影分支：三态必须分开，`Unsupported` 与 `Unknown` 合并会把三态退化成布尔。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageProjection {
    /// 支持视觉：把附件引用还原成图片块
    Rehydrate,
    /// 确定不支持：图片块降级为文本说明
    ProjectToText,
    /// 不知道：保留图片交由服务端判定，同时给出一条降级日志
    Keep,
}

/// 返回分支与「至多一条」的降级日志文案：调用方只在此处 `warn!` 一次，
/// 避免多分支各记一条导致重复日志。
pub(crate) fn image_projection(
    fact: &CapabilityFact,
    model_id: &str,
) -> (ImageProjection, Option<String>) {
    match fact.support {
        Support::Supported => (ImageProjection::Rehydrate, None),
        Support::Unsupported => (ImageProjection::ProjectToText, None),
        Support::Unknown => (
            ImageProjection::Keep,
            Some(format!(
                "vision capability unknown for model {model_id} (source {:?}): keep image blocks as-is",
                fact.provenance
            )),
        ),
    }
}

/// 原生 PDF = 模型声明支持 PDF 且目标端点确实能发送 PDF；
/// 端点可表达性不进能力合并链，只在这里与事实合取。
fn native_pdf_support(
    provider_row: &ProviderRow,
    fact: &CapabilityFact,
    model_id: &str,
) -> (bool, Option<String>) {
    let endpoint_can_send = endpoint_can_send_pdf(&base::provider_kind_from_str(
        &provider_row.type_,
        provider_row.base_url.clone(),
    ));
    match fact.support {
        Support::Supported => (endpoint_can_send, None),
        Support::Unsupported => (false, None),
        Support::Unknown => (
            false,
            Some(format!(
                "pdf capability unknown for model {model_id} (source {:?}): project pdf to text",
                fact.provenance
            )),
        ),
    }
}

fn endpoint_can_send_pdf(kind: &base::ProviderKind) -> bool {
    match kind {
        base::ProviderKind::Anthropic
        | base::ProviderKind::Gemini
        | base::ProviderKind::GeminiCodeAssist
        | base::ProviderKind::OpenAIResponses => true,
        base::ProviderKind::OpenAI | base::ProviderKind::Custom { .. } => false,
    }
}

/// 分支零记录时的占用投影：只读，不构造 adapter、不发起请求、不落库。
///
/// `system_prompt` / `facts` 必须与 `build()` 同源（`mode.prompt` + 同一环境事实），
/// 否则「占用展示」与实际出站请求看到的系统前缀不同（I38d §4.9.1 / A29）。
pub async fn estimate_branch_tokens(
    detail: &ConversationDetail,
    system_prompt: &str,
    facts: &PromptFacts,
    provider_row: &ProviderRow,
    attachment_store: &AttachmentStore,
) -> usize {
    let system_prompt = prompt_builder::compose_system_prompt(system_prompt, facts);
    let model_id = detail.conversation.model_id.clone();
    let context_window_tokens =
        types::resolve_context_window_from_stored(&model_id, &provider_row.models_json);
    let offload_budget = offload_budget_of(context_window_tokens, DEFAULT_MAX_TOKENS);
    let messages = project_chat_messages(
        detail,
        &system_prompt,
        provider_row,
        &model_id,
        attachment_store,
        offload_budget,
        ProjectionPurpose::Estimate,
        None,
    )
    .await;
    request_projection::estimate_total_tokens(&messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_providers::capability::Provenance;

    fn fact(support: Support) -> CapabilityFact {
        CapabilityFact::new(support, Provenance::Catalog)
    }

    /// 验收 A8：`Unknown` 与 `Unsupported` 在投影层必须走不同分支。
    #[test]
    fn unknown_and_unsupported_take_different_branches() {
        let (unknown_branch, unknown_log) = image_projection(&fact(Support::Unknown), "some-model");
        let (unsupported_branch, unsupported_log) =
            image_projection(&fact(Support::Unsupported), "some-model");
        assert_eq!(unknown_branch, ImageProjection::Keep);
        assert_eq!(unsupported_branch, ImageProjection::ProjectToText);
        assert_ne!(unknown_branch, unsupported_branch);
        assert!(unknown_log.is_some());
        assert!(unsupported_log.is_none());
    }

    /// `Unknown` 只有一处 `warn!` 站点，且文案只产生一条。
    #[test]
    fn unknown_produces_exactly_one_log_line() {
        let (_, warning) = image_projection(&fact(Support::Unknown), "some-model");
        let message = warning.expect("unknown must produce a degradation log");
        assert_eq!(message.lines().count(), 1);
        assert!(message.contains("some-model"));
    }

    #[test]
    fn supported_vision_rehydrates_without_log() {
        let (branch, warning) = image_projection(&fact(Support::Supported), "some-model");
        assert_eq!(branch, ImageProjection::Rehydrate);
        assert!(warning.is_none());
    }

    #[test]
    fn pdf_endpoint_expressiveness_is_not_part_of_merge_chain() {
        assert!(endpoint_can_send_pdf(&base::ProviderKind::Anthropic));
        assert!(endpoint_can_send_pdf(&base::ProviderKind::Gemini));
        assert!(!endpoint_can_send_pdf(&base::ProviderKind::OpenAI));
    }

    #[test]
    fn pdf_unknown_is_projected_to_text_even_on_capable_endpoint() {
        let row = ProviderRow {
            id: "p1".to_string(),
            name: "Anthropic".to_string(),
            type_: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key_encrypted: String::new(),
            models_json: "[]".to_string(),
            default_model: "claude".to_string(),
            updated_at: 0,
        };
        let (native, warning) = native_pdf_support(&row, &fact(Support::Unknown), "claude");
        assert!(!native);
        assert!(warning.is_some());

        let (native, warning) = native_pdf_support(&row, &fact(Support::Supported), "claude");
        assert!(native);
        assert!(warning.is_none());
    }
}
