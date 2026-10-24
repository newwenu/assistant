use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentSummary {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationAttachmentSummary {
    pub conversation_id: String,
    pub title: String,
    pub attachments: Vec<AttachmentSummary>,
    pub total_bytes: u64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageBreakdownDto {
    pub total_bytes: u64,
    pub total_count: u64,
    pub orphans: Vec<AttachmentSummary>,
    pub conversations: Vec<ConversationAttachmentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentDeleteResultDto {
    pub removed_count: u64,
    pub removed_bytes: u64,
}
