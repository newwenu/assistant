use serde::{Deserialize, Serialize};

use super::unified::{UnifiedResponse, UnifiedTokenUsage};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RetryNotification {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<crate::base::ProviderErrorCode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatDelta {
    pub seq: Option<u32>,
    pub delta_text: Option<String>,
    pub delta_reasoning: Option<String>,
    pub delta_tool_call_id: Option<String>,
    pub delta_tool_call_name: Option<String>,
    pub delta_tool_call_arguments: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_retry: Option<RetryNotification>,
}

impl ChatDelta {
    pub fn retry_notification(
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        message: impl std::fmt::Display,
    ) -> Self {
        Self {
            delta_retry: Some(RetryNotification {
                attempt,
                max_attempts,
                delay_ms,
                message: message.to_string(),
                error_code: None,
            }),
            ..Default::default()
        }
    }

    pub fn retry_notification_with_code(
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        message: impl std::fmt::Display,
        error_code: crate::base::ProviderErrorCode,
    ) -> Self {
        Self {
            delta_retry: Some(RetryNotification {
                attempt,
                max_attempts,
                delay_ms,
                message: message.to_string(),
                error_code: Some(error_code),
            }),
            ..Default::default()
        }
    }

    pub fn retry_http_notification(
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        status: reqwest::StatusCode,
    ) -> Self {
        Self {
            delta_retry: Some(RetryNotification {
                attempt,
                max_attempts,
                delay_ms,
                message: format!("HTTP {}", status),
                error_code: None,
            }),
            ..Default::default()
        }
    }

    pub fn retry_http_notification_with_code(
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        status: reqwest::StatusCode,
        error_code: crate::base::ProviderErrorCode,
    ) -> Self {
        Self {
            delta_retry: Some(RetryNotification {
                attempt,
                max_attempts,
                delay_ms,
                message: format!("HTTP {}", status),
                error_code: Some(error_code),
            }),
            ..Default::default()
        }
    }

    pub fn finish(reason: &str) -> Self {
        Self {
            finish_reason: Some(reason.to_string()),
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
    #[serde(default)]
    pub context_window_tokens: u32,
}

pub fn unified_responses_into_chat_deltas(
    responses: Vec<UnifiedResponse>,
    context_window_tokens: u32,
) -> Vec<ChatDelta> {
    responses
        .into_iter()
        .flat_map(|r| unified_response_into_chat_deltas(r, context_window_tokens))
        .collect()
}

fn unified_response_into_chat_deltas(
    response: UnifiedResponse,
    context_window_tokens: u32,
) -> Vec<ChatDelta> {
    let mut deltas = Vec::new();
    let mut usage_consumed = false;

    let has_text = response.text.is_some();
    let has_reasoning = response.reasoning_content.is_some();

    if has_text || has_reasoning {
        let usage = response
            .usage
            .as_ref()
            .map(|u| unified_usage_to_token_usage(u, context_window_tokens));
        usage_consumed = true;
        deltas.push(ChatDelta {
            seq: None,
            delta_text: response.text,
            delta_reasoning: response.reasoning_content,
            delta_tool_call_id: None,
            delta_tool_call_name: None,
            delta_tool_call_arguments: None,
            finish_reason: None,
            usage,
            delta_retry: None,
        });
    }

    if let Some(tc) = response.tool_call {
        let usage = if !usage_consumed {
            response
                .usage
                .as_ref()
                .map(|u| unified_usage_to_token_usage(u, context_window_tokens))
        } else {
            None
        };
        deltas.push(ChatDelta {
            seq: None,
            delta_text: None,
            delta_reasoning: None,
            delta_tool_call_id: tc.id,
            delta_tool_call_name: tc.name,
            delta_tool_call_arguments: tc.arguments,
            finish_reason: None,
            usage,
            delta_retry: None,
        });
    }

    if response.finish_reason.is_some() || (response.usage.is_some() && deltas.is_empty()) {
        if let Some(last) = deltas.last_mut() {
            if response.finish_reason.is_some() {
                last.finish_reason = response.finish_reason;
            }
            if response.usage.is_some() && last.usage.is_none() {
                last.usage = response
                    .usage
                    .as_ref()
                    .map(|u| unified_usage_to_token_usage(u, context_window_tokens));
            }
        } else {
            deltas.push(ChatDelta {
                seq: None,
                delta_text: None,
                delta_reasoning: None,
                delta_tool_call_id: None,
                delta_tool_call_name: None,
                delta_tool_call_arguments: None,
                finish_reason: response.finish_reason,
                usage: response
                    .usage
                    .as_ref()
                    .map(|u| unified_usage_to_token_usage(u, context_window_tokens)),
                delta_retry: None,
            });
        }
    }

    deltas
}

fn unified_usage_to_token_usage(
    usage: &UnifiedTokenUsage,
    context_window_tokens: u32,
) -> TokenUsage {
    TokenUsage {
        prompt_tokens: usage.prompt_token_count,
        completion_tokens: usage.candidates_token_count,
        total_tokens: usage.total_token_count,
        cache_read_tokens: usage.cached_content_token_count.unwrap_or(0),
        cache_write_tokens: usage.cache_creation_token_count.unwrap_or(0),
        context_window_tokens,
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::types::unified::{ReasoningContentKind, ToolCallCompletion, UnifiedToolCall};

    #[test]
    fn text_and_reasoning_in_same_chunk() {
        let response = UnifiedResponse {
            text: Some("hello".to_string()),
            reasoning_content: Some("thinking...".to_string()),
            ..Default::default()
        };
        let deltas = unified_response_into_chat_deltas(response, 0);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].delta_text.as_deref(), Some("hello"));
        assert_eq!(deltas[0].delta_reasoning.as_deref(), Some("thinking..."));
    }

    #[test]
    fn tool_call_produces_separate_delta() {
        let response = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: Some(0),
                id: Some("tc_1".to_string()),
                name: Some("search".to_string()),
                arguments: Some("{\"q\":\"test\"}".to_string()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };
        let deltas = unified_response_into_chat_deltas(response, 0);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].delta_tool_call_id.as_deref(), Some("tc_1"));
        assert_eq!(deltas[0].delta_tool_call_name.as_deref(), Some("search"));
    }

    #[test]
    fn finish_reason_merges_to_last_delta() {
        let response = UnifiedResponse {
            text: Some("done".to_string()),
            finish_reason: Some("stop".to_string()),
            tool_call_completion: Some(ToolCallCompletion::NormalNoToolUse),
            ..Default::default()
        };
        let deltas = unified_response_into_chat_deltas(response, 0);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn usage_only_produces_delta() {
        let response = UnifiedResponse {
            usage: Some(UnifiedTokenUsage {
                prompt_token_count: 10,
                candidates_token_count: 20,
                total_token_count: 30,
                reasoning_token_count: None,
                cached_content_token_count: Some(5),
                cache_creation_token_count: Some(3),
            }),
            ..Default::default()
        };
        let deltas = unified_response_into_chat_deltas(response, 128000);
        assert_eq!(deltas.len(), 1);
        let usage = deltas[0].usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.cache_read_tokens, 5);
        assert_eq!(usage.cache_write_tokens, 3);
        assert_eq!(usage.context_window_tokens, 128000);
    }

    #[test]
    fn text_plus_tool_call_plus_finish() {
        let response = UnifiedResponse {
            text: Some("result".to_string()),
            tool_call: Some(UnifiedToolCall {
                tool_call_index: Some(0),
                id: Some("tc_1".to_string()),
                name: Some("tool".to_string()),
                arguments: None,
                arguments_is_snapshot: false,
            }),
            finish_reason: Some("tool_calls".to_string()),
            ..Default::default()
        };
        let deltas = unified_response_into_chat_deltas(response, 0);
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].delta_text.as_deref(), Some("result"));
        assert_eq!(deltas[1].delta_tool_call_id.as_deref(), Some("tc_1"));
        assert_eq!(deltas[1].finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn thinking_signature_passthrough() {
        let response = UnifiedResponse {
            reasoning_content: Some("thinking step".to_string()),
            reasoning_content_kind: Some(ReasoningContentKind::Thinking),
            thinking_signature: Some("sig_abc123".to_string()),
            ..Default::default()
        };
        assert_eq!(response.thinking_signature.as_deref(), Some("sig_abc123"));
        assert_eq!(
            response.reasoning_content_kind,
            Some(ReasoningContentKind::Thinking)
        );
    }
}
