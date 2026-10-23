use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::types::tool::ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(skip)]
    pub content_blocks: Option<Vec<ContentBlock>>,
}

pub fn message_role_from_str(role: &str) -> MessageRole {
    match role {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::User,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ContentBlockType {
    Text,
    Image,
    File,
    Thinking,
    ToolUse,
    ToolResult,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        attachment_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing)]
        cached_data: Option<String>,
    },
    File {
        attachment_id: String,
        name: String,
        bytes: u64,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    ToolUse {
        #[serde(rename = "tool_use_id")]
        id: String,
        name: String,
        input: String,
    },
    ToolResult {
        #[serde(rename = "tool_use_id")]
        id: String,
        output: String,
    },
}

pub fn parse_content(content: &str) -> Option<Vec<ContentBlock>> {
    serde_json::from_str::<Vec<ContentBlock>>(content)
        .ok()
        .filter(|b| !b.is_empty())
}