use crate::protocol::{AgentQuestionDto, AgentQuestionOptionDto};
use assistant_tools::QuestionSpec;

/// 提问工具在事件里的模型可见名：与 `AgentAskUserTool::id()` 保持一致。
pub(super) const ASK_USER_TOOL_NAME: &str = "ask_user";

pub(super) fn question_dto(question: &QuestionSpec) -> AgentQuestionDto {
    AgentQuestionDto {
        question: question.question.clone(),
        header: question.header.clone(),
        options: question
            .options
            .iter()
            .map(|option| AgentQuestionOptionDto {
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
