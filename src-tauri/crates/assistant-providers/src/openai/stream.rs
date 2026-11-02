use crate::openai::types::{ChatChoice, ChatResponseChunk, ChatUsage};
use crate::types::unified::{
    ReasoningContentKind, ToolCallCompletion, UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall,
};

pub(crate) fn map_openai_finish_reason(reason: &str) -> ToolCallCompletion {
    match reason.trim().to_ascii_lowercase().as_str() {
        "tool_calls" | "function_call" => ToolCallCompletion::NormalToolUse,
        "length" => ToolCallCompletion::OutputLimit,
        "content_filter" => ToolCallCompletion::ContentFiltered,
        "stop" => ToolCallCompletion::NormalNoToolUse,
        _ => ToolCallCompletion::Unknown,
    }
}

impl From<ChatUsage> for UnifiedTokenUsage {
    fn from(usage: ChatUsage) -> Self {
        let standard_cached = usage.prompt_tokens_details.and_then(|d| d.cached_tokens);
        let cache_read = usage.prompt_cache_hit_tokens.or(standard_cached);
        let reasoning = usage
            .completion_tokens_details
            .and_then(|d| d.reasoning_tokens);

        Self {
            prompt_token_count: usage.prompt_tokens.unwrap_or(0),
            candidates_token_count: usage.completion_tokens.unwrap_or(0),
            total_token_count: usage.total_tokens.unwrap_or(0),
            reasoning_token_count: reasoning,
            cached_content_token_count: cache_read,
            cache_creation_token_count: None,
        }
    }
}

pub(crate) fn into_unified_responses(chunk: ChatResponseChunk) -> Vec<UnifiedResponse> {
    let mut usage = chunk.usage.map(|u| u.into());

    let Some(choices) = chunk.choices else {
        return usage
            .map(|u| {
                vec![UnifiedResponse {
                    usage: Some(u),
                    ..Default::default()
                }]
            })
            .unwrap_or_default();
    };

    let Some(first_choice) = choices.into_iter().next() else {
        return usage
            .map(|u| {
                vec![UnifiedResponse {
                    usage: Some(u),
                    ..Default::default()
                }]
            })
            .unwrap_or_default();
    };

    let ChatChoice {
        delta,
        message,
        finish_reason,
    } = first_choice;

    let effective_delta = delta.or(message);

    let (content, reasoning_content, tool_calls) = match effective_delta {
        Some(d) => {
            let content = d.content.filter(|s| !s.is_empty());

            let reasoning_content = d.reasoning_content.or(d.reasoning).or_else(|| {
                d.reasoning_details.and_then(|details| {
                    let text: String = details
                        .into_iter()
                        .filter(|d| d.detail_type.as_deref() == Some("reasoning.text"))
                        .filter_map(|d| d.text)
                        .collect();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                })
            });

            (content, reasoning_content, d.tool_calls)
        }
        None => (None, None, None),
    };

    let mut responses = Vec::new();

    if content.is_some() || reasoning_content.is_some() {
        let kind = if reasoning_content.is_some() {
            Some(ReasoningContentKind::Reasoning)
        } else {
            None
        };
        responses.push(UnifiedResponse {
            text: content,
            reasoning_content,
            reasoning_content_kind: kind,
            usage: usage.take(),
            ..Default::default()
        });
    }

    if let Some(tool_calls) = tool_calls {
        for tc in tool_calls {
            let is_first_event = responses.is_empty();
            responses.push(UnifiedResponse {
                tool_call: Some(UnifiedToolCall {
                    tool_call_index: tc.index,
                    id: tc.id,
                    name: tc.function.as_ref().and_then(|f| f.name.clone()),
                    arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                    arguments_is_snapshot: false,
                }),
                usage: if is_first_event { usage.take() } else { None },
                ..Default::default()
            });
        }
    }

    if let Some(finish_reason) = finish_reason {
        if let Some(last) = responses.last_mut() {
            last.tool_call_completion = Some(map_openai_finish_reason(&finish_reason));
            last.finish_reason = Some(finish_reason);
            return responses;
        }

        responses.push(UnifiedResponse {
            tool_call_completion: Some(map_openai_finish_reason(&finish_reason)),
            finish_reason: Some(finish_reason),
            usage,
            ..Default::default()
        });
        return responses;
    }

    if responses.is_empty() {
        responses.push(UnifiedResponse {
            usage,
            ..Default::default()
        });
    }

    responses
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::openai::types::*;

    fn make_chunk_with_delta(delta: ChatDeltaContent) -> ChatResponseChunk {
        ChatResponseChunk {
            object: Some("chat.completion.chunk".to_string()),
            choices: Some(vec![ChatChoice {
                delta: Some(delta),
                message: None,
                finish_reason: None,
            }]),
            usage: None,
        }
    }

    #[test]
    fn text_and_reasoning_in_same_chunk() {
        let chunk = make_chunk_with_delta(ChatDeltaContent {
            content: Some("hello".to_string()),
            reasoning_content: Some("thinking...".to_string()),
            reasoning: None,
            reasoning_details: None,
            tool_calls: None,
        });
        let responses = into_unified_responses(chunk);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].text.as_deref(), Some("hello"));
        assert_eq!(
            responses[0].reasoning_content.as_deref(),
            Some("thinking...")
        );
        assert_eq!(
            responses[0].reasoning_content_kind,
            Some(ReasoningContentKind::Reasoning)
        );
    }

    #[test]
    fn reasoning_field_fallback() {
        let chunk = make_chunk_with_delta(ChatDeltaContent {
            content: None,
            reasoning_content: None,
            reasoning: Some("qwen reasoning".to_string()),
            reasoning_details: None,
            tool_calls: None,
        });
        let responses = into_unified_responses(chunk);
        assert_eq!(
            responses[0].reasoning_content.as_deref(),
            Some("qwen reasoning")
        );
    }

    #[test]
    fn reasoning_details_fallback() {
        let chunk = make_chunk_with_delta(ChatDeltaContent {
            content: None,
            reasoning_content: None,
            reasoning: None,
            reasoning_details: Some(vec![ReasoningDetail {
                detail_type: Some("reasoning.text".to_string()),
                text: Some("minimax thinking".to_string()),
            }]),
            tool_calls: None,
        });
        let responses = into_unified_responses(chunk);
        assert_eq!(
            responses[0].reasoning_content.as_deref(),
            Some("minimax thinking")
        );
    }

    #[test]
    fn multiple_tool_calls_split() {
        let chunk = make_chunk_with_delta(ChatDeltaContent {
            content: None,
            reasoning_content: None,
            reasoning: None,
            reasoning_details: None,
            tool_calls: Some(vec![
                ChatToolCallDelta {
                    index: Some(0),
                    id: Some("tc_1".to_string()),
                    function: Some(ChatToolCallFunctionDelta {
                        name: Some("search".to_string()),
                        arguments: None,
                    }),
                },
                ChatToolCallDelta {
                    index: Some(1),
                    id: Some("tc_2".to_string()),
                    function: Some(ChatToolCallFunctionDelta {
                        name: Some("read".to_string()),
                        arguments: None,
                    }),
                },
            ]),
        });
        let responses = into_unified_responses(chunk);
        assert_eq!(responses.len(), 2);
        assert_eq!(
            responses[0].tool_call.as_ref().unwrap().id.as_deref(),
            Some("tc_1")
        );
        assert_eq!(
            responses[1].tool_call.as_ref().unwrap().id.as_deref(),
            Some("tc_2")
        );
    }

    #[test]
    fn finish_reason_with_tool_call_completion() {
        let chunk = ChatResponseChunk {
            object: Some("chat.completion.chunk".to_string()),
            choices: Some(vec![ChatChoice {
                delta: None,
                message: None,
                finish_reason: Some("tool_calls".to_string()),
            }]),
            usage: None,
        };
        let responses = into_unified_responses(chunk);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(
            responses[0].tool_call_completion,
            Some(ToolCallCompletion::NormalToolUse)
        );
    }

    #[test]
    fn usage_cache_mapping() {
        let chunk_standard = ChatResponseChunk {
            object: Some("chat.completion.chunk".to_string()),
            choices: None,
            usage: Some(ChatUsage {
                prompt_tokens: Some(100),
                completion_tokens: Some(50),
                total_tokens: Some(150),
                prompt_tokens_details: Some(ChatPromptTokensDetails {
                    cached_tokens: Some(30),
                }),
                completion_tokens_details: None,
                prompt_cache_hit_tokens: None,
                prompt_cache_miss_tokens: None,
            }),
        };
        let resp = into_unified_responses(chunk_standard);
        let u = resp[0].usage.as_ref().unwrap();
        assert_eq!(u.prompt_token_count, 100);
        assert_eq!(u.cached_content_token_count, Some(30));
        assert_eq!(u.reasoning_token_count, None);

        let chunk_deepseek = ChatResponseChunk {
            object: Some("chat.completion.chunk".to_string()),
            choices: None,
            usage: Some(ChatUsage {
                prompt_tokens: Some(100),
                completion_tokens: Some(50),
                total_tokens: Some(150),
                prompt_tokens_details: Some(ChatPromptTokensDetails {
                    cached_tokens: Some(20),
                }),
                completion_tokens_details: None,
                prompt_cache_hit_tokens: Some(40),
                prompt_cache_miss_tokens: None,
            }),
        };
        let resp = into_unified_responses(chunk_deepseek);
        let u = resp[0].usage.as_ref().unwrap();
        assert_eq!(u.cached_content_token_count, Some(40));
    }

    #[test]
    fn usage_reasoning_tokens_mapping() {
        let chunk = ChatResponseChunk {
            object: Some("chat.completion.chunk".to_string()),
            choices: None,
            usage: Some(ChatUsage {
                prompt_tokens: Some(100),
                completion_tokens: Some(80),
                total_tokens: Some(180),
                prompt_tokens_details: None,
                completion_tokens_details: Some(ChatCompletionTokensDetails {
                    reasoning_tokens: Some(60),
                }),
                prompt_cache_hit_tokens: None,
                prompt_cache_miss_tokens: None,
            }),
        };
        let resp = into_unified_responses(chunk);
        let u = resp[0].usage.as_ref().unwrap();
        assert_eq!(u.reasoning_token_count, Some(60));
    }

    #[test]
    fn empty_content_treated_as_none() {
        let chunk = make_chunk_with_delta(ChatDeltaContent {
            content: Some("".to_string()),
            reasoning_content: None,
            reasoning: None,
            reasoning_details: None,
            tool_calls: None,
        });
        let responses = into_unified_responses(chunk);
        assert!(responses[0].text.is_none());
    }

    #[test]
    fn minimax_final_chunk_message_field() {
        let chunk = ChatResponseChunk {
            object: Some("chat.completion".to_string()),
            choices: Some(vec![ChatChoice {
                delta: None,
                message: Some(ChatDeltaContent {
                    content: Some("final answer".to_string()),
                    reasoning_content: None,
                    reasoning: None,
                    reasoning_details: None,
                    tool_calls: None,
                }),
                finish_reason: Some("stop".to_string()),
            }]),
            usage: Some(ChatUsage {
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                prompt_tokens_details: None,
                completion_tokens_details: None,
                prompt_cache_hit_tokens: None,
                prompt_cache_miss_tokens: None,
            }),
        };
        let responses = into_unified_responses(chunk);
        assert_eq!(responses[0].text.as_deref(), Some("final answer"));
        assert_eq!(responses[0].finish_reason.as_deref(), Some("stop"));
    }
}
