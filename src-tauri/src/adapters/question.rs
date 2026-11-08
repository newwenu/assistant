use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// 待作答登记表：`call_id -> (会话 / 轮次 / 迭代 / 卡片 DTO / 答案投递端)`。
///
/// 与审批登记表同构：键用 `call_id`（一次 turn 内唯一），并记录 `conversation_id`
/// 做跨会话校验，避免 A 会话的窗口提交掉 B 会话的卡片。
/// 卡片 DTO 一并固化：前端漏收事件后可按会话补拉重放（事件是广播、不重放）。
#[derive(Debug, Clone, Default)]
pub struct PendingQuestionRegistry {
    pending: Arc<Mutex<HashMap<String, PendingQuestion>>>,
}

#[derive(Debug)]
struct PendingQuestion {
    conversation_id: String,
    turn_id: String,
    iteration: u32,
    /// 卡片事实来源：补拉快照直接克隆它，题目数量也从这里派生。
    question: AgentQuestionRequestDto,
    /// 原始等待上限；`None` 表示不设 deadline（配置为 `0`）。
    timeout: Option<Duration>,
    /// 登记时刻：补拉时据此把 `timeoutMs` 改写成剩余时长。
    started_at: Instant,
    sender: oneshot::Sender<Vec<QuestionAnswerItem>>,
}

/// 答案与题目规格的语义校验（IPC 兜底，前端已拦住正常路径）。
///
/// 只拒绝**确定无效**的提交：越界 `label`、同一 `label` 重复、单选多答。
/// 不校验「是否已作答」——那属于交互约束，误判会把用户卡在卡片上重试不出来。
fn validate_answer_semantics(
    questions: &[AgentQuestionDto],
    answers: &[QuestionAnswerItem],
) -> Result<(), String> {
    for (index, (spec, answer)) in questions.iter().zip(answers.iter()).enumerate() {
        let number = index + 1;
        let mut seen = HashSet::new();
        for label in &answer.selected {
            if !spec.options.iter().any(|option| &option.label == label) {
                return Err(format!(
                    "question {} answer '{}' is not one of its options",
                    number, label
                ));
            }
            if !seen.insert(label.as_str()) {
                return Err(format!(
                    "question {} answer repeats option '{}'",
                    number, label
                ));
            }
        }
        if !spec.multi_select && answer.selected.len() > 1 {
            return Err(format!(
                "question {} is single-select but got {} selected options",
                number,
                answer.selected.len()
            ));
        }
    }
    Ok(())
}

impl PendingQuestionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一个挂起点。已有同 `call_id` 的挂起点时返回 `None`（调用方 fail-closed）。
    fn register(
        &self,
        iteration: u32,
        question: AgentQuestionRequestDto,
        timeout: Option<Duration>,
    ) -> Option<oneshot::Receiver<Vec<QuestionAnswerItem>>> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.contains_key(&question.call_id) {
            return None;
        }
        pending.insert(
            question.call_id.clone(),
            PendingQuestion {
                conversation_id: question.conversation_id.clone(),
                turn_id: question.turn_id.clone(),
                iteration,
                question,
                timeout,
                started_at: Instant::now(),
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
        let expected = entry.question.questions.len();
        if answers.len() != expected {
            return Err(format!(
                "expected {} answers for call '{}', got {}",
                expected,
                call_id,
                answers.len()
            ));
        }
        if let Err(error) = validate_answer_semantics(&entry.question.questions, &answers) {
            return Err(error);
        }
        match pending.remove(call_id) {
            Some(entry) => entry
                .sender
                .send(answers)
                .map_err(|_| format!("question channel for call '{}' is already closed", call_id)),
            None => Err(format!("no pending question for call '{}'", call_id)),
        }
    }

    /// 挂起提问的补拉快照：窗口重连 / 切回会话时据此重建卡片。
    ///
    /// 提问事件是广播、不重放，前端一旦漏收就无从得知，只能等后端超时。
    /// 这里按会话给出全部挂起点，`timeoutMs` 改写为**剩余**毫秒（`0` 仍表示
    /// 不限时）：前端倒计时从事件到达时刻起算，重放必须扣掉已经等过的时间。
    /// 已耗尽上限的挂起点跳过，收口交给等待守卫，避免下发一张必过期卡片。
    pub fn snapshot(&self, conversation_id: &str) -> Vec<AgentToolCallDto> {
        let pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for entry in pending.values() {
            if entry.conversation_id != conversation_id {
                continue;
            }
            let remaining_ms = match entry.timeout {
                None => 0,
                Some(timeout) => {
                    let remaining = timeout.saturating_sub(entry.started_at.elapsed());
                    if remaining.is_zero() {
                        continue;
                    }
                    remaining.as_millis() as u64
                }
            };
            let mut question = entry.question.clone();
            question.timeout_ms = remaining_ms;
            out.push(AgentToolCallDto {
                conversation_id: entry.conversation_id.clone(),
                turn_id: entry.turn_id.clone(),
                iteration: entry.iteration,
                call_id: question.call_id.clone(),
                name: ASK_USER_TOOL_NAME.to_string(),
                payload: AgentToolCallPayloadDto::AwaitingAnswer { question },
            });
        }
        out
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
    use crate::protocol::AgentQuestionOptionDto;

    fn answer(label: &str) -> QuestionAnswerItem {
        QuestionAnswerItem::new(vec![label.to_string()], None)
    }

    fn request(call_id: &str, conversation_id: &str, count: usize) -> AgentQuestionRequestDto {
        AgentQuestionRequestDto {
            conversation_id: conversation_id.to_string(),
            turn_id: format!("{conversation_id}:1"),
            call_id: call_id.to_string(),
            questions: (0..count)
                .map(|index| AgentQuestionDto {
                    question: format!("q{index}"),
                    header: "h".to_string(),
                    // 题目恒带 2 个选项：`validate_questions` 也要求 2-10 个，
                    // 空选项在真实链路里不存在，用它做夹具会掩盖答案校验。
                    options: vec![
                        AgentQuestionOptionDto {
                            label: "A".to_string(),
                            description: "a".to_string(),
                        },
                        AgentQuestionOptionDto {
                            label: "B".to_string(),
                            description: "b".to_string(),
                        },
                    ],
                    multi_select: false,
                })
                .collect(),
            timeout_ms: 0,
        }
    }

    fn register_one(
        registry: &PendingQuestionRegistry,
        call_id: &str,
        conversation_id: &str,
        count: usize,
    ) -> Option<oneshot::Receiver<Vec<QuestionAnswerItem>>> {
        registry.register(1, request(call_id, conversation_id, count), None)
    }

    #[test]
    fn resolve_requires_matching_conversation() {
        let registry = PendingQuestionRegistry::new();
        let _rx = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

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
        let _rx = register_one(&registry, "call-1", "conv-a", 2).expect("首次登记必须成功");

        let error = registry
            .resolve("call-1", "conv-a", vec![answer("A")])
            .expect_err("答案条数必须与题目数一致");
        assert!(error.contains("expected 2 answers"));
        assert_eq!(registry.len(), 1, "无效提交不得移除挂起点");
    }

    #[test]
    fn resolve_rejects_label_outside_options() {
        let registry = PendingQuestionRegistry::new();
        let _rx = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

        let error = registry
            .resolve("call-1", "conv-a", vec![answer("C")])
            .expect_err("越界 label 必须拒绝");
        assert!(error.contains("is not one of its options"));
        assert_eq!(registry.len(), 1, "无效提交不得移除挂起点");
    }

    #[test]
    fn resolve_rejects_repeated_label() {
        let registry = PendingQuestionRegistry::new();
        let mut spec = request("call-1", "conv-a", 1);
        spec.questions[0].multi_select = true;
        let _rx = registry.register(1, spec, None).expect("首次登记必须成功");

        let error = registry
            .resolve(
                "call-1",
                "conv-a",
                vec![QuestionAnswerItem::new(
                    vec!["A".to_string(), "A".to_string()],
                    None,
                )],
            )
            .expect_err("重复 label 必须拒绝");
        assert!(error.contains("repeats option"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn resolve_rejects_multiple_labels_for_single_select() {
        let registry = PendingQuestionRegistry::new();
        let _rx = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

        let error = registry
            .resolve(
                "call-1",
                "conv-a",
                vec![QuestionAnswerItem::new(
                    vec!["A".to_string(), "B".to_string()],
                    None,
                )],
            )
            .expect_err("单选题多答必须拒绝");
        assert!(error.contains("single-select"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn resolve_accepts_multiple_labels_for_multi_select() {
        let registry = PendingQuestionRegistry::new();
        let mut spec = request("call-1", "conv-a", 1);
        spec.questions[0].multi_select = true;
        let _rx = registry.register(1, spec, None).expect("首次登记必须成功");

        assert!(
            registry
                .resolve(
                    "call-1",
                    "conv-a",
                    vec![QuestionAnswerItem::new(
                        vec!["A".to_string(), "B".to_string()],
                        None,
                    )],
                )
                .is_ok(),
            "多选题必须接受多个已选 label"
        );
    }

    #[test]
    fn duplicate_call_id_is_refused_and_keeps_original() {
        let registry = PendingQuestionRegistry::new();
        let _first = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

        assert!(
            registry
                .register(1, request("call-1", "conv-a", 1), None)
                .is_none(),
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
        let _rx = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

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
        let _rx = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");

        registry.discard("call-1");
        assert!(
            registry.is_empty(),
            "超时 / 取消路径必须清理登记项，否则登记表无界增长"
        );
    }

    #[test]
    fn discard_does_not_touch_other_calls() {
        let registry = PendingQuestionRegistry::new();
        let _first = register_one(&registry, "call-1", "conv-a", 1).expect("首次登记必须成功");
        let _second = register_one(&registry, "call-2", "conv-a", 1).expect("首次登记必须成功");

        registry.discard("call-1");
        assert_eq!(registry.len(), 1, "清理必须按 call_id 精确移除");
        assert!(registry
            .resolve("call-2", "conv-a", vec![answer("A")])
            .is_ok());
    }

    #[test]
    fn snapshot_filters_by_conversation_and_reports_remaining_timeout() {
        let registry = PendingQuestionRegistry::new();
        let _a = registry
            .register(
                2,
                request("call-a", "conv-a", 1),
                Some(Duration::from_secs(180)),
            )
            .expect("首次登记必须成功");
        let _b = registry
            .register(1, request("call-b", "conv-b", 1), None)
            .expect("首次登记必须成功");

        let shots = registry.snapshot("conv-a");
        assert_eq!(shots.len(), 1, "只返回本会话的挂起点");
        let shot = &shots[0];
        assert_eq!(shot.call_id, "call-a");
        assert_eq!(shot.iteration, 2);
        assert_eq!(shot.turn_id, "conv-a:1");
        assert_eq!(shot.name, ASK_USER_TOOL_NAME);

        let AgentToolCallPayloadDto::AwaitingAnswer { question } = &shot.payload else {
            panic!("补拉必须重放 AwaitingAnswer 卡片");
        };
        assert_eq!(question.call_id, "call-a");
        assert!(
            question.timeout_ms <= 180_000 && question.timeout_ms >= 179_000,
            "剩余时长必须扣掉已等待时间，实际 {}",
            question.timeout_ms
        );
    }

    #[test]
    fn snapshot_keeps_zero_timeout_as_unlimited() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register(1, request("call-1", "conv-a", 1), None)
            .expect("首次登记必须成功");

        let shots = registry.snapshot("conv-a");
        assert_eq!(shots.len(), 1);
        let AgentToolCallPayloadDto::AwaitingAnswer { question } = &shots[0].payload else {
            panic!("补拉必须重放 AwaitingAnswer 卡片");
        };
        assert_eq!(question.timeout_ms, 0, "0 表示不限时，重放不得改写");
    }

    #[test]
    fn snapshot_skips_expired_entry() {
        let registry = PendingQuestionRegistry::new();
        let _rx = registry
            .register(1, request("call-1", "conv-a", 1), Some(Duration::ZERO))
            .expect("首次登记必须成功");

        assert!(
            registry.snapshot("conv-a").is_empty(),
            "已耗尽上限的挂起点不得下发一张必过期卡片"
        );
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
