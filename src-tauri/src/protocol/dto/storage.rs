use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentSummary {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationAttachmentSummary {
    pub conversation_id: String,
    pub title: String,
    pub attachments: Vec<AttachmentSummary>,
    pub total_bytes: u64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StorageBreakdownDto {
    /// 目录内附件总字节数。未找到目录配额常量，无法论证更窄上限，故保留 `u64`。
    pub total_bytes: u64,
    pub total_count: u32,
    pub orphans: Vec<AttachmentSummary>,
    pub conversations: Vec<ConversationAttachmentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentImportDto {
    pub attachment_id: String,
    pub media_type: String,
    pub bytes: u64,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetDirsDto {
    pub attachments_dir: String,
    pub mascot_assets_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentDeleteResultDto {
    pub removed_count: u32,
    /// 本次删除释放的字节数。未找到目录配额常量，无法论证更窄上限，故保留 `u64`。
    pub removed_bytes: u64,
}
