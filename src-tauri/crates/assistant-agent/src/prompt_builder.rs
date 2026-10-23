use assistant_providers::base::{self, ChatMessage, ChatOptions};
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::conversations::ConversationDetail;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentKind {
    #[default]
    Default,
    Quick,
    Deep,
    Custom,
}

impl AgentKind {
    pub fn from_mode_kind(s: &str) -> Self {
        match s {
            "default" => AgentKind::Default,
            "quick" => AgentKind::Quick,
            "deep" => AgentKind::Deep,
            "custom" => AgentKind::Custom,
            _ => AgentKind::Default,
        }
    }

    pub fn supports_tool_call(&self) -> bool {
        matches!(self, AgentKind::Custom)
    }

    pub fn persists_reasoning(&self) -> bool {
        matches!(self, AgentKind::Deep)
    }
}

pub fn prepare_messages(
    detail: &ConversationDetail,
    system_prompt: &str,
    append_user_prompt: Option<&str>,
) -> Vec<ChatMessage> {
    let mut chat_messages: Vec<ChatMessage> = detail
        .messages
        .iter()
        .filter(|m| m.role != "compact_summary")
        .map(|m| ChatMessage {
            role: base::message_role_from_str(&m.role),
            content: m.content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        })
        .collect();

    if !system_prompt.is_empty() {
        chat_messages.insert(
            0,
            ChatMessage {
                role: base::message_role_from_str("system"),
                content: system_prompt.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
        );
    }

    if let Some(extra) = append_user_prompt {
        if !extra.is_empty() {
            chat_messages.push(ChatMessage {
                role: base::message_role_from_str("user"),
                content: extra.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            });
        }
    }

    chat_messages
}

pub fn build_chat_options(
    kind: &AgentKind,
    mode: &AgentModeRow,
    context_window_tokens: u32,
) -> ChatOptions {
    let temperature = match kind {
        AgentKind::Deep => mode.temperature.unwrap_or(0.3),
        _ => mode.temperature.unwrap_or(0.7),
    };

    let max_tokens = mode.max_tokens.unwrap_or(4096);

    ChatOptions {
        temperature,
        max_tokens,
        stream: true,
        context_window_tokens,
        reasoning_enabled: mode.reasoning_enabled,
        reasoning_effort: mode.reasoning_effort.clone(),
    }
}
