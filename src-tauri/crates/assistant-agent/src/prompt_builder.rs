use assistant_providers::base::{self, ChatMessage};
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
