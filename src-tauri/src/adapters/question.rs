use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use assistant_tools::{
    QuestionAnswerItem, QuestionOutcome, QuestionPrompter, QuestionRequest, QuestionSpec,
};
use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    AgentQuestionDto, AgentQuestionRequestDto, AgentToolCallDto, AgentToolCallPayloadDto,
};

/// 提问工具在事件里的模型可见名：与 `AgentAskUserTool::id()` 保持一致。
const ASK_USER_TOOL_NAME: &str = "ask_user";

/// 待作答登记表：`call_id -> (conversation_id, 题目数量, 答案投递端)`。
///
/// 与审批登记表同构：键用 `call_id`（一次 turn 内唯一），并记录 `conversation_id`
/// 做跨会话校验，避免 A 会话的窗口提交掉 B 会话的卡片。
/// 题目数量在登记时固化，回填时据此校验「逐题作答」。
#[derive(Debug, Clone, Default)]
pub struct PendingQuestionRegistry {
    pending: Arc<Mutex<HashMap<String, PendingQuestion>>>,
}

#[derive(Debug)]
struct PendingQuestion {
    conversation_id: String,
    question_count: usize,
    sender: oneshot::Sender<Vec<QuestionAnswerItem>>,
}

impl PendingQuestionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个挂起点。已有同 `call_id` 的挂起点时返回 `None`（调用方 fail-closed）。
    fn register(
        &self,
        call_id: &str,
        conversation_id: &str,
        question_count: usize,
    ) -> Option<oneshot::Receiver<Vec<QuestionAnswerItem>>> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.contains_key(call_id) {
            return None;
        }
        pending.insert(
            call_id.to_string(),
            PendingQuestion {
                conversation_id: conversation_id.to_string(),
                question_count,
                sender: tx,
            },
        );
        Some(rx)
    }

    /// 投递答案。返回错误说明这次提交无效，前端据此提示而不是假装成功。
    ///
    /// 三种无效：挂起点不存在（已超时 / 已提交 / 未知）、会话不匹配、
    /// 答案条数与题目数量不一致（前端应已拦住，这里是后端兜底）。
    pub fn resolve(
        &self,
        call_id: &str,
        conversation_id: &str,
        answers: Vec<QuestionAnswerItem>,
    ) -> Result<(), String> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let entry = match pending.get(call_id) {
            Some(entry) if entry.conversation_id == conversation_id => entry,
            Some(_) => {
                return Err(format!(
                    "question for call '{}' belongs to another conversation",
                    call_id
                ))
            }
            None => {
                return Err(format!(
                    "no pending question for call '{}' (already answered or timed out)",
                    call_id
                ))
            }
        };
        if answers.len() != entry.question_count {
            return Err(format!(
                "expected {} answers for call '{}', got {}",
                entry.question_count,
                call_id,
                answers.len()
            ));
        }
        match pending.remove(call_id) {
            Some(entry) => entry
                .sender
                .send(answers)
                .map_err(|_| format!("question channel for call '{}' is already closed", call_id)),
            None => Err(format!("no pending question for call '{}'", call_id)),
        }
    }

    fn discard(&self, call_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(call_id);
    }

    pub fn len(&self) -> usize {
        self.pending
            .lock()
            .map(|pending| pending.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

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
/// 缺路由信息（无 `conversationId` / `turnId`）时**不发事件直接返回 `Cancelled`**：
/// 卡片无法路由到任何窗口，等下去只会把 turn 挂死到超时。
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

        let receiver = match self.pending.register(
            &request.tool_call_id,
            &conversation_id,
            request.questions.len(),
        ) {
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
        emit_broadcast(
            &self.app,
            event_name::AGENT_TOOL_CALL,
            AgentToolCallDto {
                conversation_id,
                turn_id,
                iteration: request.iteration.unwrap_or_default(),
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

fn question_dto(question: &QuestionSpec) -> AgentQuestionDto {
    AgentQuestionDto {
        question: question.question.clone(),
        header: question.header.clone(),
        options: question
            .options
            .iter()
            .map(|option| crate::protocol::AgentQuestionOptionDto {
                label: option.label.clone(),
                description: option.description.clone(),
            })
            .collect(),
        multi_select: question.multi_select,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(label: &str) -> QuestionAnswerItem {
        QuestionAnswerItem::new(vec![label.to_string()], None)
    }

    #[test]
    fn resolve_requires_matching_conversation() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register("call-1", "conv-a", 1)
            .expect("首次登记必须成功");

        let error = registry
            .resolve("call-1", "conv-b", vec![answer("A")])
            .expect_err("跨会话提交必须失败");
        assert!(error.contains("another conversation"));
        assert_eq!(registry.len(), 1, "误投不得移除挂起点");

        assert!(registry
            .resolve("call-1", "conv-a", vec![answer("A")])
            .is_ok());
        assert!(registry.is_empty(), "提交后挂起点立即移除");
    }

    #[test]
    fn resolve_rejects_answer_count_mismatch() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register("call-1", "conv-a", 2)
            .expect("首次登记必须成功");

        let error = registry
            .resolve("call-1", "conv-a", vec![answer("A")])
            .expect_err("答案条数必须与题目数一致");
        assert!(error.contains("expected 2 answers"));
        assert_eq!(registry.len(), 1, "无效提交不得移除挂起点");
    }

    #[test]
    fn duplicate_call_id_is_refused_and_keeps_original() {
        let registry = PendingQuestionRegistry::new();
        let _first = registry
            .register("call-1", "conv-a", 1)
            .expect("首次登记必须成功");

        assert!(
            registry.register("call-1", "conv-a", 1).is_none(),
            "重复 call_id 必须 fail-closed 拒绝"
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn resolve_unknown_call_id_reports_error() {
        let registry = PendingQuestionRegistry::new();
        let error = registry
            .resolve("missing", "conv-a", vec![answer("A")])
            .expect_err("未知 call_id 必须报错");
        assert!(error.contains("no pending question"));
    }

    #[test]
    fn resolve_is_not_repeatable() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register("call-1", "conv-a", 1)
            .expect("首次登记必须成功");

        assert!(registry
            .resolve("call-1", "conv-a", vec![answer("A")])
            .is_ok());
        assert!(
            registry
                .resolve("call-1", "conv-a", vec![answer("B")])
                .is_err(),
            "同一 call_id 只能作答一次"
        );
    }

    #[test]
    fn discard_removes_pending_entry() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register("call-1", "conv-a", 1)
            .expect("首次登记必须成功");

        registry.discard("call-1");
        assert!(
            registry.is_empty(),
            "超时 / 取消路径必须清理登记项，否则登记表无界增长"
        );
    }

    #[test]
    fn discard_does_not_touch_other_calls() {
        let registry = PendingQuestionRegistry::new();
        let _first = registry
            .register("call-1", "conv-a", 1)
            .expect("首次登记必须成功");
        let _second = registry
            .register("call-2", "conv-a", 1)
            .expect("首次登记必须成功");

        registry.discard("call-1");
        assert_eq!(registry.len(), 1, "清理必须按 call_id 精确移除");
        assert!(registry
            .resolve("call-2", "conv-a", vec![answer("A")])
            .is_ok());
    }

    #[test]
    fn question_dto_maps_options_and_multi_select() {
        let spec = QuestionSpec {
            question: "用哪个库？".to_string(),
            header: "依赖".to_string(),
            options: vec![
                assistant_tools::QuestionOption {
                    label: "A".to_string(),
                    description: "更稳".to_string(),
                },
                assistant_tools::QuestionOption {
                    label: "B".to_string(),
                    description: "更快".to_string(),
                },
            ],
            multi_select: true,
        };

        let dto = question_dto(&spec);
        assert_eq!(dto.header, "依赖");
        assert!(dto.multi_select);
        assert_eq!(dto.options.len(), 2);
        assert_eq!(dto.options[1].label, "B");
        assert_eq!(dto.options[1].description, "更快");
    }
}
