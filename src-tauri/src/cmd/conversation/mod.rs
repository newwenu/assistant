use tauri::AppHandle;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{ConversationListChangeKindDto, ConversationListChangedPayloadDto};

mod branch;
mod detail;
mod detail_dto;
mod export;
mod list;
mod mutate;
pub(crate) mod to_dto;

pub use branch::*;
pub use detail::*;
pub use export::*;
pub use list::*;
pub use mutate::*;

fn emit_list_changed(
    app: &AppHandle,
    kind: ConversationListChangeKindDto,
    id: Option<String>,
    affected: Option<Vec<String>>,
) {
    emit_broadcast(
        app,
        event_name::CONVERSATION_LIST_CHANGED,
        ConversationListChangedPayloadDto {
            change_kind: kind,
            conversation_id: id,
            affected_ids: affected,
        },
    );
}
