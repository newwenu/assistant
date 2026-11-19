use assistant_agent::kernel::{EventRoute, EventSink, SessionState, TurnEvent, TurnStatus};
use serde::Serialize;
use tauri::Manager;

use crate::protocol::events::{emit_broadcast, emit_orb_only, event_name};
use crate::protocol::{
    AgentNodeEventDto, AgentToolCallDto, ChatDeltaPayloadDto, ChatMessagesChangedPayloadDto,
    ChatStreamStartPayloadDto, ChatUserMessageConfirmedPayloadDto, ConversationListChangeKindDto,
    ConversationListChangedPayloadDto, MessageChangeKindDto, ProviderErrorPayloadDto,
    SessionStateChangedPayloadDto,
};

pub struct TauriEventSink {
    app: tauri::AppHandle,
    route: EventRoute,
    conversation_id: String,
    message_id: String,
    user_message_content: Option<String>,
    /// 本轮 turn 序号（I46 §2.6），由宿主 `with_turn_index` 注入；`None` = 无坐标。
    turn_index: Option<u32>,
    /// 本次 assistant 占位行的迭代号（I46 §2.6）：分支重生成时可能 `> 1`，
    /// 因此不能写死 `1`，否则前端占位消息与落库行坐标不一致。
    iteration: Option<u32>,
    /// 终态刷新的 `change_kind`：chat / agent 为 `StreamFinished`，压缩为 `MessagesCompacted`。
    finished_change_kind: MessageChangeKindDto,
}

impl TauriEventSink {
    pub fn new(
        app: tauri::AppHandle,
        route: EventRoute,
        conversation_id: impl Into<String>,
        message_id: impl Into<String>,
    ) -> Self {
        Self {
            app,
            route,
            conversation_id: conversation_id.into(),
            message_id: message_id.into(),
            user_message_content: None,
            turn_index: None,
            iteration: None,
            finished_change_kind: MessageChangeKindDto::StreamFinished,
        }
    }

    pub fn with_user_message_content(mut self, content: impl Into<String>) -> Self {
        self.user_message_content = Some(content.into());
        self
    }

    /// 注入本轮坐标（I46 §2.6）：`chat-stream-start` 与 `chat-user-message-confirmed`
    /// 共用同一份值，避免两处各自推导。`iteration` 是 assistant 占位行的实际迭代号。
    pub fn with_turn_coords(mut self, turn_index: Option<u32>, iteration: Option<u32>) -> Self {
        self.turn_index = turn_index;
        self.iteration = iteration;
        self
    }

    /// 覆盖终态刷新的 `change_kind`。压缩成功必须用 `MessagesCompacted`：
    /// 前端对 `StreamFinished` 跳过 token 用量刷新，而压缩后占用需要按新分支重算。
    pub fn with_finished_change_kind(mut self, kind: MessageChangeKindDto) -> Self {
        self.finished_change_kind = kind;
        self
    }

    fn emit_wire(&self, name: &str, payload: impl Serialize + Clone) {
        match self.route {
            EventRoute::Orb => emit_orb_only(&self.app, name, payload),
            EventRoute::Broadcast => emit_broadcast(&self.app, name, payload),
        }
    }

    fn emit_session_state(&self, turn_id: &str, state: SessionState) {
        self.emit_wire(
            event_name::SESSION_STATE_CHANGED,
            SessionStateChangedPayloadDto {
                conversation_id: self.conversation_id.clone(),
                turn_id: turn_id.to_string(),
                state: state.into(),
            },
        );
    }

    /// 终态后的消息列表刷新：广播给所有窗口，等价于原 `finish` 监听器。
    ///
    /// `change_kind` 由调用方给出：chat / agent 用 `StreamFinished`，
    /// 压缩成功用 `MessagesCompacted`（前端据此决定是否刷新 token 用量）。
    pub(crate) fn emit_stream_finished(&self, change_kind: MessageChangeKindDto) {
        let state = self.app.state::<crate::setup::DbState>();
        let conn = match crate::cmd::get_db_conn(&state) {
            Ok(conn) => conn,
            Err(err) => {
                log::error!("{err}, skip emit chat-messages-changed");
                return;
            }
        };
        let detail = match assistant_storage::conversations::get_conversation_detail(
            &conn,
            &self.conversation_id,
        ) {
            Ok(Some(detail)) => detail,
            Ok(None) => return,
            Err(err) => {
                log::error!("Failed to reload conversation detail: {err}");
                return;
            }
        };
        let messages = match crate::cmd::conversation_utils::messages_with_sibling_counts(
            &conn,
            &self.conversation_id,
            &detail.messages,
        ) {
            Ok(messages) => messages,
            Err(err) => {
                log::error!("Failed to get sibling counts: {err}");
                detail
                    .messages
                    .iter()
                    .map(|m| crate::cmd::conversation_utils::wire_message_dto(m.clone()))
                    .collect()
            }
        };
        emit_broadcast(
            &self.app,
            event_name::CHAT_MESSAGES_CHANGED,
            ChatMessagesChangedPayloadDto {
                conversation_id: self.conversation_id.clone(),
                change_kind,
                messages,
                active_branch_id: detail.conversation.active_branch_id.clone(),
                seq: None,
            },
        );
        emit_broadcast(
            &self.app,
            event_name::CONVERSATION_LIST_CHANGED,
            ConversationListChangedPayloadDto {
                change_kind: ConversationListChangeKindDto::StreamFinished,
                conversation_id: Some(self.conversation_id.clone()),
                affected_ids: None,
            },
        );
    }
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: TurnEvent) {
        match event {
            TurnEvent::StreamStart { turn_id, phase } => {
                log::debug!("chat-stream-start turn={} phase={:?}", turn_id, phase);
                self.emit_wire(
                    event_name::CHAT_STREAM_START,
                    ChatStreamStartPayloadDto {
                        conversation_id: self.conversation_id.clone(),
                        attempt_id: turn_id.clone(),
                        message_id: self.message_id.clone(),
                        phase: phase.into(),
                        turn_index: self.turn_index,
                        iteration: self.iteration,
                    },
                );
                self.emit_session_state(
                    &turn_id,
                    SessionState::Processing {
                        turn_id: turn_id.clone(),
                        phase,
                    },
                );
            }
            TurnEvent::Delta {
                turn_id,
                seq,
                iteration,
                delta,
            } => {
                let payload = ChatDeltaPayloadDto {
                    conversation_id: self.conversation_id.clone(),
                    attempt_id: turn_id,
                    message_id: self.message_id.clone(),
                    seq,
                    iteration,
                    delta_text: delta.delta_text.clone(),
                    delta_reasoning: delta.delta_reasoning.clone(),
                    finish_reason: delta.finish_reason.clone(),
                    usage: delta.usage.clone().map(Into::into),
                    delta_retry: delta.delta_retry.clone().map(Into::into),
                    ..Default::default()
                };
                self.emit_wire(event_name::CHAT_DELTA, payload);
            }
            TurnEvent::PhaseChanged { turn_id, phase } => {
                self.emit_session_state(
                    &turn_id,
                    SessionState::Processing {
                        turn_id: turn_id.clone(),
                        phase,
                    },
                );
            }
            TurnEvent::Tool(event) => {
                log::debug!(
                    "tool event turn={} call={} name={}",
                    event.turn_id,
                    event.call_id,
                    event.name
                );
                let payload = AgentToolCallDto {
                    conversation_id: self.conversation_id.clone(),
                    turn_id: event.turn_id.clone(),
                    iteration: event.iteration,
                    call_id: event.call_id.clone(),
                    name: event.name.clone(),
                    payload: event.payload.into(),
                };
                self.emit_wire(event_name::AGENT_TOOL_CALL, payload);
            }
            TurnEvent::Node(event) => {
                log::debug!(
                    "node event turn={} node={:?} state={:?} iteration={} transition={}",
                    event.turn_id,
                    event.node,
                    event.state,
                    event.iteration,
                    event.transition
                );
                let payload: AgentNodeEventDto = event.into();
                self.emit_wire(event_name::AGENT_NODE_EVENT, payload);
            }
            TurnEvent::CompactionStarted { turn_id } => {
                log::debug!("compaction started turn={}", turn_id);
            }
            TurnEvent::CompactionFinished { turn_id, ok } => {
                log::debug!("compaction finished turn={} ok={}", turn_id, ok);
            }
            TurnEvent::Terminated {
                turn_id,
                status,
                last_seq,
                error,
            } => {
                let error_summary = error.as_ref().map(|err| {
                    let message: String = err.message.chars().take(200).collect();
                    format!("{:?}: {}", err.code, message)
                });
                log::info!(
                    "chat-stream-end turn={} status={} last_seq={} error={}",
                    turn_id,
                    status.as_str(),
                    last_seq,
                    error_summary.as_deref().unwrap_or("none")
                );
                let error_message = error.as_ref().map(|err| err.message.clone());
                if let Some(err) = error {
                    let payload = ChatDeltaPayloadDto {
                        conversation_id: self.conversation_id.clone(),
                        attempt_id: turn_id.clone(),
                        message_id: self.message_id.clone(),
                        seq: last_seq,
                        error: Some(ProviderErrorPayloadDto::from(err)),
                        ..Default::default()
                    };
                    self.emit_wire(event_name::CHAT_DELTA, payload);
                }
                self.emit_stream_finished(self.finished_change_kind);
                let state = match status {
                    TurnStatus::Completed | TurnStatus::Cancelled | TurnStatus::Interrupted => {
                        SessionState::Idle
                    }
                    TurnStatus::Failed => SessionState::Error {
                        message: error_message.unwrap_or_else(|| "turn failed".to_string()),
                        recoverable: false,
                    },
                };
                self.emit_session_state(&turn_id, state);
            }
            TurnEvent::UserMessageConfirmed {
                conversation_id,
                user_msg_id,
            } => {
                self.emit_wire(
                    event_name::CHAT_USER_MESSAGE_CONFIRMED,
                    ChatUserMessageConfirmedPayloadDto {
                        conversation_id,
                        user_message_id: user_msg_id,
                        user_message_content: self.user_message_content.clone().unwrap_or_default(),
                        turn_index: self.turn_index,
                    },
                );
            }
        }
    }
}
