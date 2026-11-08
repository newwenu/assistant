use std::time::Duration;

use async_trait::async_trait;

/// 提问选项：`label` 是点选文本，`description` 说明这个选择的代价与后果。
///
/// 选项必须是 label + description 两段，而不是把「选项 + 说明」塞进一段自然语言：
/// 前端要把它渲染成可点选的输入框（I38c §9）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// 单个问题。`header` 是 ≤20 字符的短标签，前端当 chip 展示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionSpec {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
}

/// 一次提问的完整请求。内部类型，不承载 UI 状态；wire DTO 由适配层映射。
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionRequest {
    /// 事件路由信息：缺任一项时卡片无法定位到窗口，prompter 必须直接收口。
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub iteration: Option<u32>,
    pub tool_call_id: String,
    pub questions: Vec<QuestionSpec>,
    /// 等待上限。`None` = 不设 deadline（配置 `user_question_timeout_secs = 0`），
    /// 只由取消终止；`Some(d)` 由工具自身套 deadline。
    pub timeout: Option<Duration>,
}

/// 单题回答：单选也走数组（恒 0–1 项）；自由输入写 `other`，不塞进 `selected`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionAnswerItem {
    pub selected: Vec<String>,
    pub other: Option<String>,
}

impl QuestionAnswerItem {
    pub fn new(selected: Vec<String>, other: Option<String>) -> Self {
        Self { selected, other }
    }

    /// 回填模型的答案文本：已选项按顺序拼接，自由输入附在末尾。
    pub fn as_text(&self) -> String {
        let mut parts = self.selected.clone();
        if let Some(other) = self.other.as_deref() {
            let other = other.trim();
            if !other.is_empty() {
                parts.push(other.to_string());
            }
        }
        parts.join(", ")
    }
}

/// 一次提问等待的结局。三种结局都必须收口：**不得**默认继续执行。
#[derive(Debug, Clone, PartialEq)]
pub enum QuestionOutcome {
    Answered {
        answers: Vec<QuestionAnswerItem>,
    },
    /// 用户取消（turn 停止 / 链路切换）。
    Cancelled,
    /// 用户未在超时前回答。按「未回答」处理，turn 继续但模型必须知道这一点。
    TimedOut,
}

/// 提问通道：把 `ask_user` 的等待转成前端卡片，并把答案投递回来。
///
/// 与 [`crate::ApprovalPrompter`] 同构：工具只认这个 trait，真实实现（Tauri 事件 +
/// IPC 回填）在适配层；没有 UI 的场合由实现直接返回 `Cancelled`（fail-closed）。
#[async_trait]
pub trait QuestionPrompter: Send + Sync {
    async fn ask(&self, request: QuestionRequest) -> QuestionOutcome;
}

/// 输入校验的唯一来源（`validate_input` 与 `execute` 共用，避免两处口径漂移）。
///
/// 约束来自 I38c §9：1–4 个问题、每题 2–10 个选项、`header` ≤20 字符、
/// `label` / `description` 非空。「其他」选项由前端自动追加，**不在**此校验内。
pub fn validate_questions(questions: &[QuestionSpec]) -> Result<(), String> {
    if questions.is_empty() {
        return Err("at least one question is required".to_string());
    }
    if questions.len() > 4 {
        return Err("at most 4 questions are allowed".to_string());
    }

    for (index, question) in questions.iter().enumerate() {
        let number = index + 1;
        if question.question.trim().is_empty() {
            return Err(format!("question {} text is required", number));
        }
        if question.header.trim().is_empty() {
            return Err(format!("question {} header is required", number));
        }
        if question.header.chars().count() > 20 {
            return Err(format!(
                "question {} header must be at most 20 characters",
                number
            ));
        }
        if question.options.len() < 2 || question.options.len() > 10 {
            return Err(format!("question {} must have 2-10 options", number));
        }
        for (option_index, option) in question.options.iter().enumerate() {
            if option.label.trim().is_empty() {
                return Err(format!(
                    "question {} option {} label is required",
                    number,
                    option_index + 1
                ));
            }
            if option.description.trim().is_empty() {
                return Err(format!(
                    "question {} option {} description is required",
                    number,
                    option_index + 1
                ));
            }
        }
    }

    Ok(())
}

/// 回答与问题逐条对齐：条数不等即视为无效提交（前端应已拦住，这里兜底）。
pub fn validate_answers(
    questions: &[QuestionSpec],
    answers: &[QuestionAnswerItem],
) -> Result<(), String> {
    if answers.len() != questions.len() {
        return Err(format!(
            "expected {} answers, got {}",
            questions.len(),
            answers.len()
        ));
    }
    Ok(())
}

/// 回填模型的文本（`result_for_assistant`）：逐题列出「问 → 答」，末尾给出继续指令。
pub fn format_answers_for_assistant(
    questions: &[QuestionSpec],
    answers: &[QuestionAnswerItem],
) -> String {
    let mut lines = vec!["User has answered your questions:".to_string()];
    for (index, question) in questions.iter().enumerate() {
        let answer = answers
            .get(index)
            .map(QuestionAnswerItem::as_text)
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "N/A".to_string());
        lines.push(format!(
            "- {} ({}): \"{}\"",
            question.question, question.header, answer
        ));
    }
    lines.push(String::new());
    lines.push("You can now continue with the user's answers in mind.".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(label: &str) -> QuestionOption {
        QuestionOption {
            label: label.to_string(),
            description: format!("{} 的含义", label),
        }
    }

    fn question(header: &str, option_count: usize) -> QuestionSpec {
        QuestionSpec {
            question: "选哪个？".to_string(),
            header: header.to_string(),
            options: (0..option_count)
                .map(|index| option(&format!("选项{}", index)))
                .collect(),
            multi_select: false,
        }
    }

    #[test]
    fn rejects_empty_question_list() {
        assert!(validate_questions(&[]).is_err());
    }

    #[test]
    fn rejects_more_than_four_questions() {
        let questions: Vec<QuestionSpec> = (0..5).map(|_| question("标签", 2)).collect();
        assert!(validate_questions(&questions).is_err());
    }

    #[test]
    fn rejects_blank_text_and_header() {
        let mut blank_text = question("标签", 2);
        blank_text.question = "   ".to_string();
        assert!(validate_questions(&[blank_text]).is_err());

        let mut blank_header = question("标签", 2);
        blank_header.header = " ".to_string();
        assert!(validate_questions(&[blank_header]).is_err());
    }

    #[test]
    fn rejects_header_longer_than_twenty_chars() {
        let long_header = "一二三四五六七八九十一二三四五六七八九十一";
        assert_eq!(long_header.chars().count(), 21);
        assert!(validate_questions(&[question(long_header, 2)]).is_err());
        assert!(
            validate_questions(&[question("一二三四五六七八九十一二三四五六七八九十", 2)]).is_ok()
        );
    }

    #[test]
    fn rejects_option_count_out_of_range() {
        assert!(validate_questions(&[question("标签", 1)]).is_err());
        assert!(validate_questions(&[question("标签", 2)]).is_ok());
        assert!(validate_questions(&[question("标签", 10)]).is_ok());
        assert!(validate_questions(&[question("标签", 11)]).is_err());
    }

    #[test]
    fn rejects_blank_option_fields() {
        let mut spec = question("标签", 2);
        spec.options[0].label = " ".to_string();
        assert!(validate_questions(&[spec]).is_err());

        let mut spec = question("标签", 2);
        spec.options[1].description = String::new();
        assert!(validate_questions(&[spec]).is_err());
    }

    #[test]
    fn rejects_answer_count_mismatch() {
        let questions = vec![question("标签", 2), question("标签", 2)];
        let answers = vec![QuestionAnswerItem::new(vec!["选项0".to_string()], None)];
        assert!(validate_answers(&questions, &answers).is_err());
    }

    #[test]
    fn formats_selected_and_other_into_assistant_text() {
        let questions = vec![question("风格", 2), question("范围", 2)];
        let answers = vec![
            QuestionAnswerItem::new(vec!["选项0".to_string()], None),
            QuestionAnswerItem::new(vec![], Some("  自定义描述  ".to_string())),
        ];
        let text = format_answers_for_assistant(&questions, &answers);

        assert!(text.contains("选哪个？ (风格): \"选项0\""));
        assert!(text.contains("选哪个？ (范围): \"自定义描述\""));
        assert!(text.contains("You can now continue with the user's answers in mind."));
    }

    #[test]
    fn empty_answer_renders_as_not_available() {
        let questions = vec![question("标签", 2)];
        let answers = vec![QuestionAnswerItem::new(vec![], None)];
        let text = format_answers_for_assistant(&questions, &answers);
        assert!(text.contains("(标签): \"N/A\""));
    }
}
