use assistant_storage::conversations::{ConversationRow, MessageRow};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversationDetailResponse {
    pub conversation: ConversationRow,
    pub messages: Vec<MessageRow>,
    pub total_count: i64,
}