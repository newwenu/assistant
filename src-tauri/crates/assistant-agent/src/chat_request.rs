use assistant_providers::types;
use assistant_storage::{
    agent_modes::AgentModeRow, attachment_store::AttachmentStore,
    conversations::ConversationDetail, providers::ProviderRow,
};

use assistant_providers::base;

use crate::multimodal;
use crate::prompt_builder;
use crate::provider_config::build_provider_config;

pub struct ChatRequest {
    pub chat_messages: Vec<types::ChatMessage>,
    pub adapter: Box<dyn base::BaseProviderAdapter>,
    pub model: String,
    pub options: types::ChatOptions,
    pub context_window_tokens: u32,
    pub kind: prompt_builder::AgentKind,
}

pub async fn build_chat_request(
    detail: &ConversationDetail,
    mode: Option<&AgentModeRow>,
    provider_row: &ProviderRow,
    model_id: &str,
    attachment_store: &AttachmentStore,
) -> Result<ChatRequest, String> {
    let system_prompt = mode.map(|m| m.prompt.as_str()).unwrap_or("");
    let mut chat_messages = prompt_builder::prepare_messages(detail, system_prompt, None);

    {
        let support_vision = resolve_support_vision(provider_row, model_id);
        if support_vision {
            multimodal::rehydrate_images(&mut chat_messages, attachment_store).await;
        } else {
            multimodal::project_images_for_text_model(&mut chat_messages);
        }
    }

    let config = build_provider_config(provider_row);
    let adapter = base::create_provider_adapter(config);
    let model = model_id.to_string();
    let context_window_tokens =
        types::resolve_context_window_from_stored(model_id, &provider_row.models_json);

    let kind = mode
        .map(|m| prompt_builder::AgentKind::from_mode_kind(&m.kind))
        .unwrap_or_default();
    let options = prompt_builder::build_chat_options(
        &kind,
        mode.unwrap_or(&AgentModeRow::default()),
        context_window_tokens,
    );

    Ok(ChatRequest {
        chat_messages,
        adapter,
        model,
        options,
        context_window_tokens,
        kind,
    })
}

fn resolve_support_vision(provider_row: &ProviderRow, model_id: &str) -> bool {
    types::resolve_support_vision_from_stored(model_id, &provider_row.models_json)
}
