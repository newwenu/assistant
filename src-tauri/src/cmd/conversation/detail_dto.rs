use super::to_dto;
use crate::protocol::{ConversationDetailDto, MessageDto, MessageRecordDto};
use assistant_storage::conversations::{ConversationRow, MessageRow};

pub(super) fn build_detail_dto(
    conn: &rusqlite::Connection,
    conv_id: &str,
    conv: ConversationRow,
    messages: Vec<MessageRow>,
    total_count: i64,
    include_records: bool,
) -> Result<ConversationDetailDto, String> {
    let message_dtos: Vec<MessageDto> =
        to_dto::messages_with_sibling_counts(conn, conv_id, &messages)?;
    let records = include_records.then(|| {
        messages
            .iter()
            .cloned()
            .map(MessageRecordDto::from)
            .collect()
    });
    Ok(ConversationDetailDto {
        conversation: conv.into(),
        messages: message_dtos,
        total_count,
        records,
    })
}
