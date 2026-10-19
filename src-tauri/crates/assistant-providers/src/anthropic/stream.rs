use crate::anthropic::types::{
    AnthropicUsage, ContentBlock, ContentBlockDelta, ContentBlockDeltaType, ContentBlockStart,
    MessageDelta,
};
use crate::types::unified::{
    ReasoningContentKind, ToolCallCompletion, UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall,
};

pub(crate) fn map_anthropic_stop_reason(reason: &str) -> ToolCallCompletion {
    match reason.trim().to_ascii_lowercase().as_str() {
        "tool_use" => ToolCallCompletion::NormalToolUse,
        "max_tokens" => ToolCallCompletion::OutputLimit,
        "refusal" => ToolCallCompletion::Failed,
        "end_turn" | "stop_sequence" | "pause_turn" => ToolCallCompletion::NormalNoToolUse,
        _ => ToolCallCompletion::Unknown,
    }
}

impl From<AnthropicUsage> for UnifiedTokenUsage {
    fn from(value: AnthropicUsage) -> Self {
        let cache_read = value.cache_read_input_tokens;
        let cache_creation = value.cache_creation_input_tokens;
        let prompt_token_count =
            value.input_tokens.unwrap_or(0) + cache_read.unwrap_or(0) + cache_creation.unwrap_or(0);
        let candidates_token_count = value.output_tokens.unwrap_or(0);

        Self {
            prompt_token_count,
            candidates_token_count,
            total_token_count: prompt_token_count + candidates_token_count,
            reasoning_token_count: None,
            cached_content_token_count: cache_read,
            cache_creation_token_count: cache_creation,
        }
    }
}

pub(crate) fn content_block_start_to_unified(start: ContentBlockStart) -> UnifiedResponse {
    match start.content_block {
        ContentBlock::ToolUse { id, name } => UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: start.index,
                id: Some(id),
                name: Some(name),
                arguments: None,
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        },
        ContentBlock::Thinking {
            thinking,
            signature,
        } => UnifiedResponse {
            reasoning_content: thinking,
            reasoning_content_kind: Some(ReasoningContentKind::Thinking),
            thinking_signature: signature,
            ..Default::default()
        },
        _ => UnifiedResponse::default(),
    }
}

pub(crate) fn content_block_delta_to_unified(
    delta: ContentBlockDelta,
) -> Result<UnifiedResponse, String> {
    match delta.delta {
        ContentBlockDeltaType::Thinking { thinking } => Ok(UnifiedResponse {
            reasoning_content: Some(thinking),
            reasoning_content_kind: Some(ReasoningContentKind::Thinking),
            ..Default::default()
        }),
        ContentBlockDeltaType::Text { text } => Ok(UnifiedResponse {
            text: Some(text),
            ..Default::default()
        }),
        ContentBlockDeltaType::InputJson { partial_json } => Ok(UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: delta.index,
                id: None,
                name: None,
                arguments: Some(partial_json),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        }),
        ContentBlockDeltaType::Signature { signature } => Ok(UnifiedResponse {
            thinking_signature: Some(signature),
            ..Default::default()
        }),
        ContentBlockDeltaType::Unknown => Err("Unsupported anthropic delta type".to_string()),
    }
}

pub(crate) fn message_delta_to_unified(delta: MessageDelta) -> UnifiedResponse {
    UnifiedResponse {
        text: None,
        reasoning_content: None,
        reasoning_content_kind: None,
        thinking_signature: None,
        tool_call: None,
        usage: delta.usage.map(UnifiedTokenUsage::from),
        tool_call_completion: delta
            .delta
            .stop_reason
            .as_deref()
            .map(map_anthropic_stop_reason),
        finish_reason: delta.delta.stop_reason,
        provider_metadata: None,
        model_response_replay: None,
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn map_stop_reason_table() {
        let cases = [
            ("tool_use", ToolCallCompletion::NormalToolUse),
            ("end_turn", ToolCallCompletion::NormalNoToolUse),
            ("stop_sequence", ToolCallCompletion::NormalNoToolUse),
            ("max_tokens", ToolCallCompletion::OutputLimit),
            ("refusal", ToolCallCompletion::Failed),
            ("unknown", ToolCallCompletion::Unknown),
        ];
        for (input, expected) in cases {
            assert_eq!(
                map_anthropic_stop_reason(input),
                expected,
                "input={}",
                input
            );
        }
    }

    #[test]
    fn usage_mapping() {
        let with_cache = AnthropicUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_read_input_tokens: Some(30),
            cache_creation_input_tokens: Some(20),
        };
        let u: UnifiedTokenUsage = with_cache.into();
        assert_eq!(u.prompt_token_count, 150);
        assert_eq!(u.candidates_token_count, 50);
        assert_eq!(u.total_token_count, 200);
        assert_eq!(u.cached_content_token_count, Some(30));
        assert_eq!(u.cache_creation_token_count, Some(20));

        let no_cache = AnthropicUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        };
        let u: UnifiedTokenUsage = no_cache.into();
        assert_eq!(u.prompt_token_count, 100);
        assert_eq!(u.cached_content_token_count, None);
        assert_eq!(u.cache_creation_token_count, None);
    }

    #[test]
    fn content_block_start_tool_use() {
        let start = ContentBlockStart {
            index: Some(0),
            content_block: ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "search".to_string(),
            },
        };
        let unified = content_block_start_to_unified(start);
        let tc = unified.tool_call.unwrap();
        assert_eq!(tc.id.as_deref(), Some("tool_1"));
        assert_eq!(tc.name.as_deref(), Some("search"));
        assert_eq!(tc.tool_call_index, Some(0));
    }

    #[test]
    fn content_block_start_thinking() {
        let start = ContentBlockStart {
            index: Some(0),
            content_block: ContentBlock::Thinking {
                thinking: Some("hmm".to_string()),
                signature: Some("sig_1".to_string()),
            },
        };
        let unified = content_block_start_to_unified(start);
        assert_eq!(unified.reasoning_content.as_deref(), Some("hmm"));
        assert_eq!(unified.thinking_signature.as_deref(), Some("sig_1"));
        assert_eq!(
            unified.reasoning_content_kind,
            Some(ReasoningContentKind::Thinking)
        );
    }

    #[test]
    fn content_block_delta_text() {
        let delta = ContentBlockDelta {
            index: Some(0),
            delta: ContentBlockDeltaType::Text {
                text: "hello".to_string(),
            },
        };
        let unified = content_block_delta_to_unified(delta).unwrap();
        assert_eq!(unified.text.as_deref(), Some("hello"));
    }

    #[test]
    fn content_block_delta_thinking() {
        let delta = ContentBlockDelta {
            index: Some(0),
            delta: ContentBlockDeltaType::Thinking {
                thinking: "deep thought".to_string(),
            },
        };
        let unified = content_block_delta_to_unified(delta).unwrap();
        assert_eq!(unified.reasoning_content.as_deref(), Some("deep thought"));
        assert_eq!(
            unified.reasoning_content_kind,
            Some(ReasoningContentKind::Thinking)
        );
    }

    #[test]
    fn content_block_delta_input_json() {
        let delta = ContentBlockDelta {
            index: Some(0),
            delta: ContentBlockDeltaType::InputJson {
                partial_json: "{\"q\":".to_string(),
            },
        };
        let unified = content_block_delta_to_unified(delta).unwrap();
        let tc = unified.tool_call.unwrap();
        assert_eq!(tc.arguments.as_deref(), Some("{\"q\":"));
        assert_eq!(tc.tool_call_index, Some(0));
    }

    #[test]
    fn content_block_delta_signature() {
        let delta = ContentBlockDelta {
            index: Some(0),
            delta: ContentBlockDeltaType::Signature {
                signature: "sig_abc".to_string(),
            },
        };
        let unified = content_block_delta_to_unified(delta).unwrap();
        assert_eq!(unified.thinking_signature.as_deref(), Some("sig_abc"));
    }
}
