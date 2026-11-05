use crate::gemini::types::{GeminiStreamResponse, GeminiUsageMetadata};
use crate::types::unified::{
    ReasoningContentKind, ToolCallCompletion, UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall,
};
use serde_json::{json, Value};

pub(crate) fn map_gemini_finish_reason(reason: &str) -> ToolCallCompletion {
    match reason.trim().to_ascii_uppercase().as_str() {
        "STOP" => ToolCallCompletion::NormalNoToolUse,
        "MAX_TOKENS" => ToolCallCompletion::OutputLimit,
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => {
            ToolCallCompletion::ContentFiltered
        }
        _ => ToolCallCompletion::Unknown,
    }
}

impl From<GeminiUsageMetadata> for UnifiedTokenUsage {
    fn from(usage: GeminiUsageMetadata) -> Self {
        let reasoning_token_count = usage.thoughts_token_count;
        let candidates_token_count = usage
            .candidates_token_count
            .saturating_add(reasoning_token_count.unwrap_or(0));
        Self {
            prompt_token_count: usage.prompt_token_count,
            candidates_token_count,
            total_token_count: usage.total_token_count,
            reasoning_token_count,
            cached_content_token_count: usage.cached_content_token_count,
            cache_creation_token_count: None,
        }
    }
}

pub(crate) fn gemini_response_to_unified(response: GeminiStreamResponse) -> Vec<UnifiedResponse> {
    let mut usage = response.usage_metadata.map(Into::into);
    let prompt_feedback = response.prompt_feedback;

    let Some(candidate) = response.candidates.into_iter().next() else {
        return usage
            .take()
            .map(|u| {
                vec![UnifiedResponse {
                    usage: Some(u),
                    ..Default::default()
                }]
            })
            .unwrap_or_default();
    };

    let mut responses = Vec::new();
    let finish_reason = candidate.finish_reason;
    let grounding_metadata = candidate.grounding_metadata;
    let safety_ratings = candidate.safety_ratings;

    if let Some(content) = candidate.content {
        for (part_index, part) in content.parts.into_iter().enumerate() {
            let has_function_call = part.function_call.is_some();
            let text = part.text.filter(|t| !t.is_empty());
            let is_thought = part.thought.unwrap_or(false);
            let thinking_signature = part.thought_signature.filter(|s| !s.is_empty());

            if let Some(function_call) = part.function_call {
                let arguments = function_call.args.unwrap_or_else(|| json!({}));
                responses.push(UnifiedResponse {
                    text: None,
                    reasoning_content: None,
                    reasoning_content_kind: None,
                    thinking_signature,
                    tool_call: Some(UnifiedToolCall {
                        tool_call_index: Some(part_index),
                        id: None,
                        name: function_call.name,
                        arguments: serde_json::to_string(&arguments).ok(),
                        arguments_is_snapshot: true,
                    }),
                    usage: usage.take(),
                    tool_call_completion: None,
                    finish_reason: None,
                    provider_metadata: None,
                    model_response_replay: None,
                });
                continue;
            }

            if let Some(ref executable_code) = part.executable_code {
                if let Some(reasoning_content) = render_executable_code(executable_code) {
                    responses.push(UnifiedResponse {
                        text: None,
                        reasoning_content: Some(reasoning_content),
                        reasoning_content_kind: None,
                        thinking_signature,
                        tool_call: None,
                        usage: usage.take(),
                        tool_call_completion: None,
                        finish_reason: None,
                        provider_metadata: None,
                        model_response_replay: None,
                    });
                    continue;
                }
            }

            if let Some(ref code_execution_result) = part.code_execution_result {
                if let Some(reasoning_content) = render_code_execution_result(code_execution_result)
                {
                    responses.push(UnifiedResponse {
                        text: None,
                        reasoning_content: Some(reasoning_content),
                        reasoning_content_kind: None,
                        thinking_signature,
                        tool_call: None,
                        usage: usage.take(),
                        tool_call_completion: None,
                        finish_reason: None,
                        provider_metadata: None,
                        model_response_replay: None,
                    });
                    continue;
                }
            }

            if let Some(text) = text {
                responses.push(UnifiedResponse {
                    text: if is_thought { None } else { Some(text.clone()) },
                    reasoning_content: if is_thought { Some(text) } else { None },
                    reasoning_content_kind: if is_thought {
                        Some(ReasoningContentKind::Thinking)
                    } else {
                        None
                    },
                    thinking_signature,
                    tool_call: None,
                    usage: usage.take(),
                    tool_call_completion: None,
                    finish_reason: None,
                    provider_metadata: None,
                    model_response_replay: None,
                });
                continue;
            }

            if thinking_signature.is_some() && !has_function_call {
                responses.push(UnifiedResponse {
                    text: None,
                    reasoning_content: None,
                    reasoning_content_kind: None,
                    thinking_signature,
                    tool_call: None,
                    usage: usage.take(),
                    tool_call_completion: None,
                    finish_reason: None,
                    provider_metadata: None,
                    model_response_replay: None,
                });
            }
        }
    }

    let provider_metadata =
        build_provider_metadata(prompt_feedback, grounding_metadata, safety_ratings);
    if let Some(provider_metadata) = provider_metadata {
        let summary = provider_metadata_summary(&provider_metadata);
        responses.push(UnifiedResponse {
            text: summary,
            reasoning_content: None,
            reasoning_content_kind: None,
            thinking_signature: None,
            tool_call: None,
            usage: usage.take(),
            tool_call_completion: None,
            finish_reason: None,
            provider_metadata: Some(provider_metadata),
            model_response_replay: None,
        });
    }

    if let Some(finish_reason) = finish_reason {
        if let Some(last_response) = responses.last_mut() {
            last_response.tool_call_completion = Some(map_gemini_finish_reason(&finish_reason));
            last_response.finish_reason = Some(finish_reason);
            return responses;
        }

        responses.push(UnifiedResponse {
            usage,
            tool_call_completion: Some(map_gemini_finish_reason(&finish_reason)),
            finish_reason: Some(finish_reason),
            ..Default::default()
        });
        return responses;
    }

    if responses.is_empty() {
        responses.push(UnifiedResponse {
            usage,
            finish_reason,
            ..Default::default()
        });
    }

    responses
}

fn render_executable_code(code: &crate::gemini::types::GeminiExecutableCode) -> Option<String> {
    let code_text = code.code.as_deref()?.trim();
    if code_text.is_empty() {
        return None;
    }
    let language = code
        .language
        .as_deref()
        .map(|l| l.to_ascii_lowercase())
        .unwrap_or_else(|| "text".to_string());
    Some(format!(
        "Gemini code execution generated code:\n```{}\n{}\n```",
        language, code_text
    ))
}

fn render_code_execution_result(
    result: &crate::gemini::types::GeminiCodeExecutionResult,
) -> Option<String> {
    let output = result.output.as_deref()?.trim();
    if output.is_empty() {
        return None;
    }
    let outcome = result.outcome.as_deref().unwrap_or("OUTCOME_UNKNOWN");
    Some(format!(
        "Gemini code execution result ({}):\n{}",
        outcome, output
    ))
}

fn build_provider_metadata(
    prompt_feedback: Option<Value>,
    grounding_metadata: Option<Value>,
    safety_ratings: Option<Value>,
) -> Option<Value> {
    let mut metadata = serde_json::Map::new();
    if let Some(pf) = prompt_feedback {
        metadata.insert("promptFeedback".to_string(), pf);
    }
    if let Some(gm) = grounding_metadata {
        metadata.insert("groundingMetadata".to_string(), gm);
    }
    if let Some(sr) = safety_ratings {
        metadata.insert("safetyRatings".to_string(), sr);
    }
    if metadata.is_empty() {
        None
    } else {
        Some(Value::Object(metadata))
    }
}

fn provider_metadata_summary(metadata: &Value) -> Option<String> {
    let mut lines = Vec::new();

    if let Some(pf) = metadata.get("promptFeedback") {
        if let Some(blocked_reason) = pf
            .get("blockReason")
            .and_then(Value::as_str)
            .filter(|r| !r.trim().is_empty())
        {
            lines.push(format!("Prompt blocked reason: {}", blocked_reason));
        }
    }

    if let Some(gm) = metadata.get("groundingMetadata") {
        if let Some(queries) = gm.get("webSearchQueries").and_then(Value::as_array) {
            let query_strs: Vec<&str> = queries
                .iter()
                .filter_map(Value::as_str)
                .filter(|q| !q.trim().is_empty())
                .collect();
            if !query_strs.is_empty() {
                lines.push(format!("Search queries: {}", query_strs.join(" | ")));
            }
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub(crate) fn extract_sse_api_error(event_json: &Value) -> Option<String> {
    let error = event_json.get("error")?;
    error
        .get("message")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| error.as_str().map(|s| s.to_string()))
        .or_else(|| Some("Gemini streaming request failed".to_string()))
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn map_finish_reason_table() {
        let cases = [
            ("STOP", ToolCallCompletion::NormalNoToolUse),
            ("MAX_TOKENS", ToolCallCompletion::OutputLimit),
            ("SAFETY", ToolCallCompletion::ContentFiltered),
            ("RECITATION", ToolCallCompletion::ContentFiltered),
            ("BLOCKLIST", ToolCallCompletion::ContentFiltered),
            ("other", ToolCallCompletion::Unknown),
        ];
        for (input, expected) in cases {
            assert_eq!(map_gemini_finish_reason(input), expected, "input={}", input);
        }
    }

    #[test]
    fn usage_mapping() {
        let usage = GeminiUsageMetadata {
            prompt_token_count: 100,
            candidates_token_count: 50,
            total_token_count: 150,
            thoughts_token_count: Some(10),
            cached_content_token_count: Some(30),
        };
        let u: UnifiedTokenUsage = usage.into();
        assert_eq!(u.prompt_token_count, 100);
        assert_eq!(u.candidates_token_count, 60);
        assert_eq!(u.total_token_count, 150);
        assert_eq!(u.reasoning_token_count, Some(10));
        assert_eq!(u.cached_content_token_count, Some(30));
    }

    #[test]
    fn text_and_thought_parts() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "thinking", "thought": true, "thoughtSignature": "sig_1" },
                        { "text": "answer" }
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 4,
                "thoughtsTokenCount": 2,
                "totalTokenCount": 14
            }
        });

        let data: GeminiStreamResponse = serde_json::from_value(payload).expect("parse");
        let responses = gemini_response_to_unified(data);

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].reasoning_content.as_deref(), Some("thinking"));
        assert_eq!(responses[0].thinking_signature.as_deref(), Some("sig_1"));
        assert_eq!(
            responses[0].reasoning_content_kind,
            Some(ReasoningContentKind::Thinking)
        );
        assert_eq!(responses[1].text.as_deref(), Some("answer"));
    }

    #[test]
    fn function_call_part() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {
                            "functionCall": {
                                "name": "get_weather",
                                "args": { "city": "Paris" }
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });

        let data: GeminiStreamResponse = serde_json::from_value(payload).expect("parse");
        let responses = gemini_response_to_unified(data);

        assert_eq!(responses.len(), 1);
        let tc = responses[0].tool_call.as_ref().unwrap();
        assert_eq!(tc.name.as_deref(), Some("get_weather"));
        assert!(tc.arguments.is_some());
    }

    #[test]
    fn extract_sse_error() {
        let json = serde_json::json!({
            "error": { "message": "API key expired", "code": 401 }
        });
        assert_eq!(
            extract_sse_api_error(&json),
            Some("API key expired".to_string())
        );
    }

    #[test]
    fn safety_ratings_in_provider_metadata() {
        let payload = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "answer" }]
                },
                "safetyRatings": [{ "category": "HARM_CATEGORY", "probability": "HIGH" }]
            }]
        });

        let data: GeminiStreamResponse = serde_json::from_value(payload).expect("parse");
        let responses = gemini_response_to_unified(data);

        let meta_response = responses.iter().find(|r| r.provider_metadata.is_some());
        assert!(meta_response.is_some());
        let meta = meta_response.unwrap().provider_metadata.as_ref().unwrap();
        assert!(meta.get("safetyRatings").is_some());
    }
}
