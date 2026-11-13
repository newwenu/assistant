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
pub struct ImageAttachmentRef {
    #[serde(rename = "attachmentId")]
    pub attachment_id: String,
    #[serde(rename = "mediaType", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileAttachmentRef {
    #[serde(rename = "attachmentId")]
    pub attachment_id: String,
    pub name: String,
    pub bytes: u64,
}

impl FileAttachmentRef {
    /// 降级为纯文本时使用的占位描述（Provider 转换器与投影共用）。
    pub fn handle_text(&self) -> String {
        format!("[file: {} ({})]", self.name, format_size(self.bytes))
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        #[serde(rename = "attachment")]
        attachment: ImageAttachmentRef,
        #[serde(skip_serializing)]
        cached_data: Option<String>,
        #[serde(skip_serializing)]
        file_path: Option<String>,
    },
    File {
        #[serde(rename = "attachment")]
        attachment: FileAttachmentRef,
        #[serde(skip_serializing)]
        cached_data: Option<String>,
        #[serde(skip_serializing)]
        file_path: Option<String>,
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

/// 统一取数：优先内存缓存 `content_blocks`，否则解析 `content` 里的 JSON 块。
/// 返回 `None` 表示该消息是存量纯文本，调用方应原样使用 `content`。
pub fn blocks_of(m: &ChatMessage) -> Option<Vec<ContentBlock>> {
    m.content_blocks
        .clone()
        .or_else(|| parse_content(&m.content))
}

/// 拼接全部 `Text` 块，供「纯文本出站形状」使用；非文本块不参与。
pub fn plain_text_of(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
