use assistant_tools::{QuestionOutcome, QuestionPrompter, QuestionRequest};
use async_trait::async_trait;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{AgentQuestionRequestDto, AgentToolCallDto, AgentToolCallPayloadDto};

use super::dto::{question_dto, ASK_USER_TOOL_NAME};
use super::registry::PendingQuestionRegistry;

/// 挂起点的清理守卫：超时或 turn 取消时 future 被 drop，登记项必须一并移除，
/// 否则登记表会随超时次数无界增长。正常作答走 `resolve` 已先移除，此处为幂等空操作。
///
/// 与审批不同，这里**不发**撤回事件：前端按 `timeoutMs` 自行倒计时置灰，
/// 真正收口由工具结果的 `failed(cancelled)` 事件完成。
struct PendingQuestionGuard<'a> {
    registry: &'a PendingQuestionRegistry,
    call_id: String,
}

impl Drop for PendingQuestionGuard<'_> {
    fn drop(&mut self) {
        self.registry.discard(&self.call_id);
    }
}

/// 真实提问通道：把 `ask_user` 的等待转成前端卡片，等 IPC 回填答案。
///
/// 缺路由信息（无 `conversationId` / `turnId` / `iteration`）时**不发事件直接返回
/// `Cancelled`**：卡片无法归属到任何一条消息，等下去只会把 turn 挂死到超时。
pub struct TauriQuestionPrompter {
    app: tauri::AppHandle,
    pending: PendingQuestionRegistry,
}

impl TauriQuestionPrompter {
    pub fn new(app: tauri::AppHandle, pending: PendingQuestionRegistry) -> Self {
        Self { app, pending }
    }

    pub fn pending(&self) -> &PendingQuestionRegistry {
        &self.pending
    }
}

#[async_trait]
impl QuestionPrompter for TauriQuestionPrompter {
    async fn ask(&self, request: QuestionRequest) -> QuestionOutcome {
        let (conversation_id, turn_id) =
            match (request.conversation_id.clone(), request.turn_id.clone()) {
                (Some(conversation_id), Some(turn_id)) => (conversation_id, turn_id),
                _ => {
                    log::error!(
                        "question for '{}' lacks conversation/turn id, cancelling",
                        request.tool_call_id
                    );
                    return QuestionOutcome::Cancelled;
                }
            };

        // 迭代号是卡片归属的唯一依据（I38d §3.1.1）：缺值时下发 `0` 只会被前端
        // 当作「落后于当前段」的迟到事件丢弃，不如在这里 fail-closed。
        let iteration = match request.iteration {
            Some(iteration) => iteration,
            None => {
                log::error!(
                    "question for '{}' lacks iteration, cancelling",
                    request.tool_call_id
                );
                return QuestionOutcome::Cancelled;
            }
        };

        let question = AgentQuestionRequestDto {
            conversation_id: conversation_id.clone(),
            turn_id: turn_id.clone(),
            call_id: request.tool_call_id.clone(),
            questions: request.questions.iter().map(question_dto).collect(),
            // `0` 表示不设超时：前端不显示倒计时（I38d §2.4）。
            timeout_ms: request
                .timeout
                .map(|timeout| timeout.as_millis() as u64)
                .unwrap_or(0),
        };

        let receiver = match self
            .pending
            .register(iteration, question.clone(), request.timeout)
        {
            Some(receiver) => receiver,
            None => {
                log::error!(
                    "duplicate pending question for call_id='{}', cancelling",
                    request.tool_call_id
                );
                return QuestionOutcome::Cancelled;
            }
        };
        let _guard = PendingQuestionGuard {
            registry: &self.pending,
            call_id: request.tool_call_id.clone(),
        };

        emit_broadcast(
            &self.app,
            event_name::AGENT_TOOL_CALL,
            AgentToolCallDto {
                conversation_id,
                turn_id,
                iteration,
                call_id: request.tool_call_id.clone(),
                name: ASK_USER_TOOL_NAME.to_string(),
                payload: AgentToolCallPayloadDto::AwaitingAnswer { question },
            },
        );

        match receiver.await {
            Ok(answers) => QuestionOutcome::Answered { answers },
            Err(_) => QuestionOutcome::Cancelled,
        }
    }
}
