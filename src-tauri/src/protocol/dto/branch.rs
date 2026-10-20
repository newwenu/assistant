use super::chat::MessageDto;
use assistant_storage::conversations::SiblingInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagesChangedPayloadDto {
    pub conversation_id: String,
    pub change_kind: String,
    pub messages: Vec<MessageDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub _seq: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MaxSiblingsExceededErrorDto {
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub current_count: i64,
    pub max_siblings: i64,
}
