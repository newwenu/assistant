use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageAttachmentRefDto {
    pub attachment_id: String,
    pub media_type: Option<String>,
    pub bytes: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub name: Option<String>,
}

impl From<assistant_providers::types::ImageAttachmentRef> for ImageAttachmentRefDto {
    fn from(r: assistant_providers::types::ImageAttachmentRef) -> Self {
        Self {
            attachment_id: r.attachment_id,
            media_type: r.media_type,
            bytes: r.bytes,
            width: r.width,
            height: r.height,
            name: r.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileAttachmentRefDto {
    pub attachment_id: String,
    pub name: String,
    pub bytes: u64,
}

impl From<assistant_providers::types::FileAttachmentRef> for FileAttachmentRefDto {
    fn from(r: assistant_providers::types::FileAttachmentRef) -> Self {
        Self {
            attachment_id: r.attachment_id,
            name: r.name,
            bytes: r.bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDto {
    Text {
        text: String,
    },
    Image {
        attachment: ImageAttachmentRefDto,
    },
    File {
        attachment: FileAttachmentRefDto,
    },
    Thinking {
        thinking: String,
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

impl From<assistant_providers::types::ContentBlock> for ContentBlockDto {
    fn from(b: assistant_providers::types::ContentBlock) -> Self {
        match b {
            assistant_providers::types::ContentBlock::Text { text } => Self::Text { text },
            assistant_providers::types::ContentBlock::Image { attachment, .. } => Self::Image {
                attachment: ImageAttachmentRefDto::from(attachment),
            },
            assistant_providers::types::ContentBlock::File { attachment, .. } => Self::File {
                attachment: FileAttachmentRefDto::from(attachment),
            },
            assistant_providers::types::ContentBlock::Thinking {
                thinking,
                signature,
            } => Self::Thinking {
                thinking,
                signature,
            },
            assistant_providers::types::ContentBlock::ToolUse { id, name, input } => {
                Self::ToolUse { id, name, input }
            }
            assistant_providers::types::ContentBlock::ToolResult { id, output } => {
                Self::ToolResult { id, output }
            }
        }
    }
}
