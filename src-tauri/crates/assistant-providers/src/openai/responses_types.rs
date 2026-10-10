use crate::types::unified::{
    ModelResponseReplayCapture, ModelResponseReplayItem, ReasoningSummaryPart, UnifiedResponse,
    UnifiedTokenUsage, UnifiedToolCall,
};
use serde::Deserialize;
use serde_json::Value;

pub const OPENAI_RESPONSES_REPLAY_PROTOCOL: &str = "openai_responses";

#[derive(Debug, Deserialize)]
pub struct ResponsesStreamEvent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub output_index: Option<usize>,
    #[serde(default)]
    pub content_index: Option<usize>,
    #[serde(default)]
    pub summary_index: Option<usize>,
    #[serde(default)]
    pub response: Option<Value>,
    #[serde(default)]
    pub item: Option<Value>,
    #[serde(default)]
    pub delta: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesCompleted {
    pub id: String,
    #[serde(default)]
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesDone {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesUsage {
    pub input_tokens: u32,
    #[serde(default)]
    pub input_tokens_details: Option<ResponsesInputTokensDetails>,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesInputTokensDetails {
    pub cached_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: Option<u32>,
}

impl From<ResponsesUsage> for UnifiedTokenUsage {
    fn from(usage: ResponsesUsage) -> Self {
        let (cached_content_token_count, cache_creation_token_count) = usage
            .input_tokens_details
            .map(|details| (Some(details.cached_tokens), details.cache_write_tokens))
            .unwrap_or((None, None));
        Self {
            prompt_token_count: usage.input_tokens,
            candidates_token_count: usage.output_tokens,
            total_token_count: usage.total_tokens,
            reasoning_token_count: None,
            cached_content_token_count,
            cache_creation_token_count,
        }
    }
}

pub fn parse_responses_output_item(
    item_value: Value,
    tool_call_index: Option<usize>,
) -> Option<UnifiedResponse> {
    let item_type = item_value.get("type")?.as_str()?;

    match item_type {
        "function_call" => Some(UnifiedResponse {
            text: None,
            reasoning_content: None,
            reasoning_content_kind: None,
            thinking_signature: None,
            tool_call: Some(UnifiedToolCall {
                tool_call_index,
                id: item_value
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                name: item_value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                arguments: item_value
                    .get("arguments")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                arguments_is_snapshot: false,
            }),
            usage: None,
            tool_call_completion: None,
            finish_reason: None,
            provider_metadata: None,
            model_response_replay: None,
        }),
        "message" => {
            let text = item_value
                .get("content")
                .and_then(Value::as_array)
                .map(|content| {
                    content
                        .iter()
                        .filter(|item| {
                            item.get("type").and_then(Value::as_str) == Some("output_text")
                        })
                        .filter_map(|item| item.get("text").and_then(Value::as_str))
                        .collect::<String>()
                })
                .filter(|text| !text.is_empty());

            text.map(|text| UnifiedResponse {
                text: Some(text),
                reasoning_content: None,
                reasoning_content_kind: None,
                thinking_signature: None,
                tool_call: None,
                usage: None,
                tool_call_completion: None,
                finish_reason: None,
                provider_metadata: None,
                model_response_replay: None,
            })
        }
        _ => None,
    }
}

pub fn parse_responses_replay_capture(output: &[Value]) -> Option<ModelResponseReplayCapture> {
    if output.is_empty() {
        return None;
    }

    let mut contains_opaque_reasoning = false;
    let mut items = Vec::with_capacity(output.len());
    for item in output {
        let replay_item = parse_responses_replay_item(item)?;
        contains_opaque_reasoning |=
            matches!(replay_item, ModelResponseReplayItem::OpaqueReasoning { .. });
        items.push(replay_item);
    }

    contains_opaque_reasoning.then(|| ModelResponseReplayCapture {
        protocol: OPENAI_RESPONSES_REPLAY_PROTOCOL.to_string(),
        items,
    })
}

fn parse_responses_replay_item(item: &Value) -> Option<ModelResponseReplayItem> {
    match item.get("type")?.as_str()? {
        "reasoning" => {
            let opaque_state = item
                .get("encrypted_content")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())?
                .to_string();
            let item_id = item
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            let summary = match item.get("summary").filter(|summary| !summary.is_null()) {
                Some(summary) => parse_reasoning_summary(summary)?,
                None => Vec::new(),
            };
            Some(ModelResponseReplayItem::OpaqueReasoning {
                item_id,
                summary,
                opaque_state,
            })
        }
        "message"
            if item
                .get("role")
                .and_then(Value::as_str)
                .is_none_or(|role| role == "assistant") =>
        {
            Some(ModelResponseReplayItem::AssistantMessage)
        }
        "function_call" => item
            .get("call_id")
            .and_then(Value::as_str)
            .filter(|call_id| !call_id.is_empty())
            .map(|call_id| ModelResponseReplayItem::FunctionCall {
                call_id: call_id.to_string(),
            }),
        _ => None,
    }
}

fn parse_reasoning_summary(value: &Value) -> Option<Vec<ReasoningSummaryPart>> {
    value
        .as_array()?
        .iter()
        .map(|part| {
            (part.get("type").and_then(Value::as_str) == Some("summary_text"))
                .then(|| part.get("text").and_then(Value::as_str))
                .flatten()
                .map(|text| ReasoningSummaryPart {
                    text: text.to_string(),
                })
        })
        .collect()
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn responses_usage_mapping() {
        let with_cache: ResponsesUsage = serde_json::from_str(
            r#"{"input_tokens":200,"input_tokens_details":{"cached_tokens":80,"cache_write_tokens":64},"output_tokens":40,"total_tokens":240}"#,
        ).unwrap();
        let u: UnifiedTokenUsage = with_cache.into();
        assert_eq!(u.cached_content_token_count, Some(80));
        assert_eq!(u.cache_creation_token_count, Some(64));

        let no_cache: ResponsesUsage =
            serde_json::from_str(r#"{"input_tokens":200,"output_tokens":40,"total_tokens":240}"#)
                .unwrap();
        let u: UnifiedTokenUsage = no_cache.into();
        assert_eq!(u.cached_content_token_count, None);
        assert_eq!(u.cache_creation_token_count, None);

        let zero_write: ResponsesUsage = serde_json::from_str(
            r#"{"input_tokens":200,"input_tokens_details":{"cached_tokens":80,"cache_write_tokens":0},"output_tokens":40,"total_tokens":240}"#,
        ).unwrap();
        let u: UnifiedTokenUsage = zero_write.into();
        assert_eq!(u.cache_creation_token_count, Some(0));
    }

    #[test]
    fn parses_output_text_message_item() {
        let response = parse_responses_output_item(
            json!({
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "output_text",
                        "text": "hello"
                    }
                ]
            }),
            None,
        )
        .expect("message item");

        assert_eq!(response.text.as_deref(), Some("hello"));
    }

    #[test]
    fn parses_function_call_item() {
        let response = parse_responses_output_item(
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "get_weather",
                "arguments": "{\"city\":\"Beijing\"}"
            }),
            Some(3),
        )
        .expect("function_call item");

        let tc = response.tool_call.expect("tool_call");
        assert_eq!(tc.tool_call_index, Some(3));
        assert_eq!(tc.id.as_deref(), Some("call_1"));
        assert_eq!(tc.name.as_deref(), Some("get_weather"));
        assert_eq!(tc.arguments.as_deref(), Some("{\"city\":\"Beijing\"}"));
    }

    #[test]
    fn parses_replay_capture_with_opaque_reasoning() {
        let output = vec![json!({
            "type": "reasoning",
            "id": "rs_1",
            "encrypted_content": "opaque-blob",
            "summary": [{ "type": "summary_text", "text": "I thought about it" }]
        })];

        let capture = parse_responses_replay_capture(&output).expect("replay capture");
        assert_eq!(capture.protocol, OPENAI_RESPONSES_REPLAY_PROTOCOL);
        assert_eq!(capture.items.len(), 1);
        match &capture.items[0] {
            ModelResponseReplayItem::OpaqueReasoning {
                item_id,
                summary,
                opaque_state,
            } => {
                assert_eq!(item_id.as_deref(), Some("rs_1"));
                assert_eq!(opaque_state, "opaque-blob");
                assert_eq!(summary.len(), 1);
                assert_eq!(summary[0].text, "I thought about it");
            }
            other => panic!("expected OpaqueReasoning, got {:?}", other),
        }
    }

    #[test]
    fn replay_capture_returns_none_without_opaque_reasoning() {
        let output = vec![json!({
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": "hi" }]
        })];
        assert!(parse_responses_replay_capture(&output).is_none());
    }

    #[test]
    fn parses_replay_function_call_item() {
        let output = vec![
            json!({
                "type": "reasoning",
                "encrypted_content": "blob"
            }),
            json!({
                "type": "function_call",
                "call_id": "call_abc"
            }),
        ];
        let capture = parse_responses_replay_capture(&output).expect("replay capture");
        assert_eq!(capture.items.len(), 2);
        assert!(matches!(
            &capture.items[1],
            ModelResponseReplayItem::FunctionCall { call_id } if call_id == "call_abc"
        ));
    }
}
