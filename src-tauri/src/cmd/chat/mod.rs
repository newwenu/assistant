pub(crate) mod insert;

use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use crate::protocol::SendChatMessageArgsDto;
use assistant_agent::kernel::{TurnSpec, TurnTasks};

#[command]
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), String> {
    super::agent::start_turn(&app, &conversation_id, args, TurnSpec::chat()).await
}

#[command]
pub fn cancel_chat(app: AppHandle, conversation_id: String) -> Result<(), String> {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    tasks.cancel(&conversation_id);
    Ok(())
}
