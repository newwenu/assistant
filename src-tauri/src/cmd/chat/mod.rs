pub(crate) mod insert;

use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use crate::protocol::{CommandErrorDto, SendChatMessageArgsDto};
use assistant_agent::kernel::{TurnSpec, TurnTasks};

#[command]
pub async fn send_chat_message(
    app: AppHandle,
    conversation_id: String,
    args: SendChatMessageArgsDto,
) -> Result<(), CommandErrorDto> {
    // 聊天链路不挂工具，因此不派生策略执行器，由 runner 回落默认。
    super::agent::start_turn(&app, &conversation_id, args, TurnSpec::chat(), None).await
}

#[command]
pub fn cancel_chat(app: AppHandle, conversation_id: String) -> Result<(), CommandErrorDto> {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    tasks.cancel(&conversation_id);
    Ok(())
}
