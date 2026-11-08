use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

use crate::context::ToolUseContext;
use crate::question::{
    format_answers_for_assistant, validate_questions, QuestionOption, QuestionOutcome,
    QuestionPrompter, QuestionRequest, QuestionSpec,
};
use crate::r#trait::{BehaviorHints, Tool, ToolAudience, ToolTier, ValidationResult};
use crate::result::{ToolErrorKind, ToolResult};

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "ask_user";

/// 等待上限的取值来源：每次执行现读全局配置，改设置后下一次提问立即生效。
/// `None` → 不设 deadline（配置 `user_question_timeout_secs = 0`）。
pub type QuestionTimeoutSource = Arc<dyn Fn() -> Option<Duration> + Send + Sync>;

/// `ask_user`：暂停 Agent Loop 向用户提问，等前端作答后再继续（I38c §9）。
///
/// 它是 `manages_own_execution_timeout() = true` 的唯一工具：等待长度由用户决定，
/// 30s 统一 deadline 会把「用户还在想」误判成失败；超时值来自配置，不写进 schema。
pub struct AgentAskUserTool {
    prompter: Arc<dyn QuestionPrompter>,
    timeout: QuestionTimeoutSource,
}

impl AgentAskUserTool {
    pub fn new(prompter: Arc<dyn QuestionPrompter>, timeout: QuestionTimeoutSource) -> Self {
        Self { prompter, timeout }
    }
}

#[async_trait]
impl Tool for AgentAskUserTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Ask the user focused follow-up questions during execution and wait for the answers. \
             Use it when the request is ambiguous, several valid approaches exist with different \
             trade-offs, or the change has significant impact. Do not use it when the request is \
             clear, when following an already-approved plan, or for trivial changes. Put every \
             question you need into a single call. State your recommendation in the option \
             description and make the recommended option the first one; append \"(Recommended)\" \
             to its label. The user can always select \"Other\" to type a custom answer, so never \
             add an \"Other\" option yourself."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 4,
                    "description": "Questions to ask the user (1-4 questions)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask the user. Be clear, \
                                 specific and end with a question mark."
                            },
                            "header": {
                                "type": "string",
                                "maxLength": 20,
                                "description": "Very short label shown as a chip (max 20 characters)."
                            },
                            "options": {
                                "type": "array",
                                "minItems": 2,
                                "maxItems": 10,
                                "description": "The available choices (2-10). Do not add an \
                                 'Other' option, it is provided automatically.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Display text of this choice, concise \
                                             (1-5 words)."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "What this choice means or implies, \
                                             including trade-offs."
                                        }
                                    },
                                    "required": ["label", "description"],
                                    "additionalProperties": false
                                }
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "default": false,
                                "description": "Set to true to allow selecting multiple options."
                            }
                        },
                        "required": ["question", "header", "options"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["questions"],
            "additionalProperties": false
        })
    }

    /// 提问不触碰 OS 也不访问外部世界，放在最内层，永远不会被层级上限拒绝。
    fn tier(&self) -> ToolTier {
        ToolTier::Sandbox
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::AGENT_ONLY
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: true,
            destructive: false,
            open_world: false,
        }
    }

    /// 等待用户作答，长度不可预估，必须自管超时（I38c §11 OQ5）。
    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    fn validate_input(&self, input: &Value, _context: &ToolUseContext) -> ValidationResult {
        match AskUserInput::parse(input) {
            Ok(request) => match validate_questions(&request.questions) {
                Ok(()) => ValidationResult::ok(),
                Err(error) => ValidationResult::invalid(error),
            },
            Err(error) => ValidationResult::invalid(error),
        }
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let request = match AskUserInput::parse(input) {
            Ok(request) => request,
            Err(error) => return Ok(ToolResult::err(error)),
        };
        if let Err(error) = validate_questions(&request.questions) {
            return Ok(ToolResult::err(error));
        }

        let questions = request.questions;
        let timeout = (self.timeout)();
        let prompt = QuestionRequest {
            conversation_id: context.session_id.clone(),
            turn_id: context.turn_id.clone(),
            iteration: context.iteration,
            tool_call_id: context
                .tool_call_id
                .clone()
                .unwrap_or_else(|| TOOL_ID.to_string()),
            questions: questions.clone(),
            timeout,
        };

        let outcome = match timeout {
            Some(duration) => match tokio::time::timeout(duration, self.prompter.ask(prompt)).await
            {
                Ok(outcome) => outcome,
                Err(_) => QuestionOutcome::TimedOut,
            },
            None => self.prompter.ask(prompt).await,
        };

        Ok(match outcome {
            QuestionOutcome::Answered { answers } => {
                let assistant_text = format_answers_for_assistant(&questions, &answers);
                let summary: Vec<Value> = questions
                    .iter()
                    .map(|question| {
                        json!({
                            "question": question.question,
                            "header": question.header
                        })
                    })
                    .collect();
                let answers_json: Vec<Value> = answers
                    .iter()
                    .map(|answer| {
                        json!({
                            "selected": answer.selected,
                            "other": answer.other
                        })
                    })
                    .collect();
                ToolResult::ok_with_assistant(
                    json!({
                        "questions": summary,
                        "answers": answers_json,
                        "status": "answered"
                    }),
                    assistant_text,
                )
            }
            QuestionOutcome::TimedOut => ToolResult::err_with_kind(
                ToolErrorKind::Cancelled,
                "the user did not answer before the timeout; the questions were skipped, \
                 continue without their answers",
            ),
            QuestionOutcome::Cancelled => ToolResult::err_with_kind(
                ToolErrorKind::Cancelled,
                "the question was cancelled before the user answered",
            ),
        })
    }
}

/// 入参的解析结果。解析与校验分开：解析只管类型与必填，语义约束交给
/// [`validate_questions`]，两处口径同源。
struct AskUserInput {
    questions: Vec<QuestionSpec>,
}

impl AskUserInput {
    fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;
        let questions = object
            .get("questions")
            .ok_or_else(|| "'questions' is required".to_string())?
            .as_array()
            .ok_or_else(|| "'questions' must be an array".to_string())?;

        let mut parsed = Vec::with_capacity(questions.len());
        for (index, question) in questions.iter().enumerate() {
            parsed.push(parse_question(question, index + 1)?);
        }
        Ok(Self { questions: parsed })
    }
}

fn parse_question(value: &Value, number: usize) -> Result<QuestionSpec, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("question {} must be an object", number))?;

    let question = required_str(object, "question", number)?;
    let header = required_str(object, "header", number)?;

    let options = object
        .get("options")
        .ok_or_else(|| format!("question {} 'options' is required", number))?
        .as_array()
        .ok_or_else(|| format!("question {} 'options' must be an array", number))?
        .iter()
        .enumerate()
        .map(|(index, option)| parse_option(option, number, index + 1))
        .collect::<Result<Vec<_>, _>>()?;

    let multi_select = match object.get("multiSelect") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(format!(
                "question {} 'multiSelect' must be a boolean",
                number
            ))
        }
    };

    Ok(QuestionSpec {
        question,
        header,
        options,
        multi_select,
    })
}

fn parse_option(value: &Value, question: usize, number: usize) -> Result<QuestionOption, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("question {} option {} must be an object", question, number))?;
    let label = object.get("label").and_then(Value::as_str).ok_or_else(|| {
        format!(
            "question {} option {} 'label' is required",
            question, number
        )
    })?;
    let description = object
        .get("description")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "question {} option {} 'description' is required",
                question, number
            )
        })?;
    Ok(QuestionOption {
        label: label.to_string(),
        description: description.to_string(),
    })
}

fn required_str(
    object: &serde_json::Map<String, Value>,
    key: &str,
    number: usize,
) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("question {} '{}' is required", number, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::QuestionAnswerItem;
    use std::sync::Mutex;

    /// 可控的提问通道替身：记录收到的请求，返回预设结局。
    struct StubPrompter {
        outcome: QuestionOutcome,
        seen: Mutex<Vec<QuestionRequest>>,
    }

    impl StubPrompter {
        fn new(outcome: QuestionOutcome) -> Arc<Self> {
            Arc::new(Self {
                outcome,
                seen: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl QuestionPrompter for StubPrompter {
        async fn ask(&self, request: QuestionRequest) -> QuestionOutcome {
            self.seen
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(request);
            self.outcome.clone()
        }
    }

    fn timeout_source(timeout: Option<Duration>) -> QuestionTimeoutSource {
        Arc::new(move || timeout)
    }

    fn make_tool(prompter: Arc<StubPrompter>, timeout: Option<Duration>) -> AgentAskUserTool {
        AgentAskUserTool::new(prompter, timeout_source(timeout))
    }

    fn context() -> ToolUseContext {
        ToolUseContext {
            workspace_root: None,
            session_id: Some("conv-1".to_string()),
            turn_id: Some("turn-1".to_string()),
            iteration: Some(2),
            tool_call_id: Some("call-1".to_string()),
        }
    }

    fn input(questions: Value) -> Value {
        json!({ "questions": questions })
    }

    fn single_question() -> Value {
        json!([{
            "question": "用哪个库？",
            "header": "依赖",
            "options": [
                { "label": "A", "description": "更稳" },
                { "label": "B", "description": "更快" }
            ]
        }])
    }

    #[test]
    fn schema_declares_question_bounds() {
        let schema = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None).input_schema();
        let questions = &schema["properties"]["questions"];
        assert_eq!(questions["minItems"], 1);
        assert_eq!(questions["maxItems"], 4);

        let item = &questions["items"];
        assert_eq!(item["properties"]["options"]["minItems"], 2);
        assert_eq!(item["properties"]["options"]["maxItems"], 10);
        assert_eq!(item["properties"]["header"]["maxLength"], 20);
        assert_eq!(item["required"], json!(["question", "header", "options"]));
    }

    #[test]
    fn declares_self_managed_timeout_and_agent_only() {
        let tool = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        assert!(tool.manages_own_execution_timeout());
        assert!(tool.audience().agent);
        assert!(!tool.audience().toolbox);
        assert!(tool.is_readonly());
        assert!(tool.is_concurrency_safe(None));
        assert_eq!(tool.tier(), ToolTier::Sandbox);
    }

    #[test]
    fn validate_rejects_missing_questions_key() {
        let tool = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        let result = tool.validate_input(&json!({}), &context());
        assert!(!result.ok);
        assert!(result.message.unwrap_or_default().contains("questions"));
    }

    #[test]
    fn validate_rejects_wrong_option_type() {
        let tool = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        let result = tool.validate_input(
            &input(json!([{
                "question": "问题",
                "header": "标签",
                "options": "not-an-array"
            }])),
            &context(),
        );
        assert!(!result.ok);
    }

    #[test]
    fn validate_rejects_missing_option_description() {
        let tool = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        let result = tool.validate_input(
            &input(json!([{
                "question": "问题",
                "header": "标签",
                "options": [{ "label": "A" }, { "label": "B", "description": "说明" }]
            }])),
            &context(),
        );
        assert!(!result.ok);
    }

    #[test]
    fn validate_accepts_multi_select_default_false() {
        let tool = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        assert!(
            tool.validate_input(&input(single_question()), &context())
                .ok
        );

        let with_multi = make_tool(StubPrompter::new(QuestionOutcome::Cancelled), None);
        let mut questions = single_question();
        questions[0]["multiSelect"] = json!(true);
        assert!(with_multi.validate_input(&input(questions), &context()).ok);
    }

    #[tokio::test]
    async fn answered_result_carries_summary_and_assistant_text() {
        let prompter = StubPrompter::new(QuestionOutcome::Answered {
            answers: vec![QuestionAnswerItem::new(vec!["A".to_string()], None)],
        });
        let tool = make_tool(prompter.clone(), Some(Duration::from_secs(180)));
        let result = tool
            .execute(&input(single_question()), &context())
            .await
            .expect("execute must not fail");

        assert!(result.ok);
        let data = result.data.expect("answered result carries data");
        assert_eq!(data["status"], "answered");
        assert_eq!(data["answers"][0]["selected"], json!(["A"]));
        let text = result.result_for_assistant.unwrap_or_default();
        assert!(text.contains("用哪个库？ (依赖): \"A\""));

        let seen = prompter
            .seen
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(seen[0].turn_id.as_deref(), Some("turn-1"));
        assert_eq!(seen[0].iteration, Some(2));
        assert_eq!(seen[0].tool_call_id, "call-1");
        assert_eq!(seen[0].timeout, Some(Duration::from_secs(180)));
        assert!(!seen[0].questions[0].multi_select);
    }

    #[tokio::test]
    async fn cancelled_outcome_is_reported_as_cancelled_kind() {
        let tool = make_tool(
            StubPrompter::new(QuestionOutcome::Cancelled),
            Some(Duration::from_secs(180)),
        );
        let result = tool
            .execute(&input(single_question()), &context())
            .await
            .expect("execute must not fail");

        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Cancelled));
        assert!(result.error.unwrap_or_default().contains("cancelled"));
    }

    #[tokio::test]
    async fn timeout_outcome_is_reported_as_cancelled_kind() {
        let tool = make_tool(
            StubPrompter::new(QuestionOutcome::TimedOut),
            Some(Duration::from_secs(180)),
        );
        let result = tool
            .execute(&input(single_question()), &context())
            .await
            .expect("execute must not fail");

        assert!(!result.ok);
        assert_eq!(
            result.error_kind,
            Some(ToolErrorKind::Cancelled),
            "超时与用户取消同形，turn 继续"
        );
        assert!(result.error.unwrap_or_default().contains("timeout"));
    }

    /// 超时由工具自身收口：配置 `0` → `None` → 不设 deadline，等待不会被 30s 之外打断。
    #[tokio::test]
    async fn zero_timeout_means_no_deadline() {
        let prompter = StubPrompter::new(QuestionOutcome::Cancelled);
        let tool = make_tool(prompter.clone(), None);
        let result = tool
            .execute(&input(single_question()), &context())
            .await
            .expect("execute must not fail");

        assert_eq!(result.error_kind, Some(ToolErrorKind::Cancelled));
        let seen = prompter
            .seen
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert_eq!(seen[0].timeout, None);
    }
}
