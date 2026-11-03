use super::chat::MessageDto;
use assistant_storage::conversations::SiblingInfo;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `chat-messages-changed` 事件的变更原因（封闭集合）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageChangeKindDto {
    BranchCreated,
    BranchSwitched,
    BranchForked,
    MessageDeleted,
    MessagesCompacted,
    StreamFinished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SiblingInfoDto {
    pub id: String,
    pub sibling_index: i64,
    pub content_preview: String,
}

impl From<SiblingInfo> for SiblingInfoDto {
    fn from(s: SiblingInfo) -> Self {
        Self {
            id: s.id,
            sibling_index: s.sibling_index,
            content_preview: s.content_preview,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatMessagesChangedPayloadDto {
    pub conversation_id: String,
    pub change_kind: MessageChangeKindDto,
    pub messages: Vec<MessageDto>,
    #[serde(default)]
    pub active_branch_id: Option<String>,
    #[serde(default)]
    pub seq: Option<u32>,
}
