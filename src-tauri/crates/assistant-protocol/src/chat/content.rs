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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileAttachmentRefDto {
    pub attachment_id: String,
    pub name: String,
    pub bytes: u64,
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
