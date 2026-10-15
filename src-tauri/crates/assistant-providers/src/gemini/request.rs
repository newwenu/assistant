use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, warn};

use crate::base::{ChatDelta, ChatMessage, ChatOptions, ChatStream, ProviderError};
use crate::client::quirks::normalize_generic_reasoning_effort;
use crate::gemini::stream::{extract_sse_api_error, gemini_response_to_unified};
use crate::gemini::types::{GeminiStreamResponse, GenerateContentRequest};
use crate::sse_log::SseLogCollector;
use crate::stream::stats::StreamStats;
use crate::stream::timeout::{StreamTimeoutController, StreamTimeoutStage};
use crate::tool_call_accumulator::{ToolCallAccumulator, ToolCallBoundary, ToolCallStreamKey};
use crate::trace;
use crate::types::chat_delta::unified_responses_into_chat_deltas;

use super::message_converter;
use super::GeminiAdapter;
use crate::client::error::{classify_http_error, classify_request_error};

pub(crate) fn chat_completion_stream(
    adapter: &GeminiAdapter,
    messages: &[ChatMessage],
    model: &str,
    options: &ChatOptions,
) -> Result<ChatStream, ProviderError> {
    let api_key = adapter.decrypt_api_key()?;
    let base_url = adapter.base_url().to_string();
    let (system_instruction, contents) = message_converter::convert_messages(messages);

    let url = resolve_request_url(&base_url, model);

    let req_body = GenerateContentRequest {
        contents,
        system_instruction,
        generation_config: None,
        tools: None,
        tool_config: None,
    };

    let reasoning_enabled = options.reasoning_enabled;
    let reasoning_effort = options.reasoning_effort.clone();
    let max_tokens = options.max_tokens;
    let temperature = options.temperature;

    let client = adapter.client.clone();
    let retry_config = adapter.retry_config.clone();
    let timeout_config = adapter.timeout_config.clone();
    let sse_log_config = adapter.sse_log_config.clone();
    let context_window_tokens = options.context_window_tokens;

    let stream = async_stream::stream! {
        let mut attempt: u32 = 0;
        'retry: loop {
            let mut body_value = match serde_json::to_value(&req_body) {
                Ok(v) => v,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request: {}", e)));
                    return;
                }
            };

            {
                let mut generation_config = serde_json::Map::new();
                if max_tokens > 0 {
                    generation_config.insert(
                        "maxOutputTokens".to_string(),
                        serde_json::json!(max_tokens),
                    );
                }
                if temperature > 0.0 {
                    generation_config.insert(
                        "temperature".to_string(),
                        serde_json::json!(temperature),
                    );
                }

                let enabled = reasoning_enabled.unwrap_or(false);
                if reasoning_enabled.is_some() {
                    if enabled {
                        let mut thinking_config = serde_json::Map::new();
                        thinking_config.insert("includeThoughts".to_string(), serde_json::json!(true));

                        if let Some(ref effort) = reasoning_effort {
                            if let Some(normalized) = normalize_generic_reasoning_effort(effort) {
                                thinking_config.insert(
                                    "thinkingLevel".to_string(),
                                    serde_json::json!(normalized.to_ascii_uppercase()),
                                );
                            }
                        }

                        generation_config.insert(
                            "thinkingConfig".to_string(),
                            serde_json::Value::Object(thinking_config),
                        );
                    } else {
                        let mut thinking_config = serde_json::Map::new();
                        thinking_config.insert("includeThoughts".to_string(), serde_json::json!(false));
                        thinking_config.insert("thinkingBudget".to_string(), serde_json::json!(0));
                        generation_config.insert(
                            "thinkingConfig".to_string(),
                            serde_json::Value::Object(thinking_config),
                        );
                    }
                }

                if !generation_config.is_empty() {
                    body_value["generationConfig"] = serde_json::Value::Object(generation_config);
                }
            }

            let body = match serde_json::to_string(&body_value) {
                Ok(b) => b,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request: {}", e)));
                    return;
                }
            };

            trace::log_request(&url, &body_value, attempt);

            let req_builder = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("x-goog-api-key", &api_key)
                .body(body);

            let resp = match req_builder.send().await {
                Ok(r) => r,
                Err(e) => {
                    let err = classify_request_error(e);
                    if err.is_retryable() && attempt < retry_config.max_attempts {
                        let delay = retry_config.delay_ms(attempt, &err);
                        warn!("Attempt {} failed, retrying in {}ms: {}", attempt + 1, delay, err);
                        yield Ok(ChatDelta::retry_notification(attempt, retry_config.max_attempts, delay, &err));
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt += 1;
                        continue 'retry;
                    }
                    yield Err(err);
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let err = classify_http_error(status, &text);
                trace::log_error(attempt, &err.to_string());
                if err.is_retryable() && attempt < retry_config.max_attempts {
                    let delay = retry_config.delay_ms(attempt, &err);
                    warn!("Attempt {} failed (HTTP {}), retrying in {}ms", attempt + 1, status, delay);
                    yield Ok(ChatDelta::retry_http_notification(attempt, retry_config.max_attempts, delay, status));
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    attempt += 1;
                    continue 'retry;
                }
                yield Err(err);
                return;
            }

            let mut event_source = resp.bytes_stream().eventsource();

            let mut sse_error_count: u32 = 0;
            let mut sse_log = SseLogCollector::new(sse_log_config.clone());
            let mut timeout_ctrl = StreamTimeoutController::new(
                timeout_config.first_token_timeout,
                timeout_config.idle_timeout,
            );
            let mut stats = StreamStats::new("gemini");
            let mut _sse_chunk_count: u32 = 0;
            let mut received_finish_reason = false;
            let mut tool_call_accumulator = ToolCallAccumulator::new();

            while let Some(event_result) = event_source.next().await {
                if let Some(stage) = timeout_ctrl.check() {
                    match stage {
                        StreamTimeoutStage::FirstToken => {
                            let err = ProviderError::Timeout(format!(
                                "First token not received within {}s",
                                timeout_config.first_token_timeout.as_secs()
                            ));
                            sse_log.flush_on_error("first-token timeout");
                            if attempt < retry_config.max_attempts {
                                let delay = retry_config.delay_ms(attempt, &err);
                                warn!("First token timeout, retrying in {}ms", delay);
                                yield Ok(ChatDelta::retry_notification(attempt, retry_config.max_attempts, delay, &err));
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                                attempt += 1;
                                continue 'retry;
                            }
                            yield Err(err);
                            return;
                        }
                        StreamTimeoutStage::Idle => {
                            let err = ProviderError::Timeout(format!(
                                "No chunk received for {}s (stream idle)",
                                timeout_config.idle_timeout.as_secs()
                            ));
                            sse_log.flush_on_error("stream idle timeout");
                            yield Err(err);
                            return;
                        }
                    }
                }

                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        let err = ProviderError::StreamError(format!("SSE stream error: {}", e));
                        sse_log.flush_on_error(&err.to_string());
                        if !timeout_ctrl.first_token_received() && err.is_retryable() && attempt < retry_config.max_attempts {
                            let delay = retry_config.delay_ms(attempt, &err);
                            warn!("SSE stream error, retrying in {}ms: {}", delay, err);
                            yield Ok(ChatDelta::retry_notification(attempt, retry_config.max_attempts, delay, &err));
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                            attempt += 1;
                            continue 'retry;
                        }
                        yield Err(err);
                        return;
                    }
                };

                timeout_ctrl.touch();

                let data = event.data;
                sse_log.push(data.clone());
                _sse_chunk_count += 1;
                trace::log_response_chunk(attempt, _sse_chunk_count - 1, &data);
                stats.record_sse_event("data");

                if data == "[DONE]" {
                    stats.record_sse_event("done");
                    if !received_finish_reason {
                        let finalized = tool_call_accumulator.finalize_all(ToolCallBoundary::StreamEnd);
                        for tc in finalized {
                            let deltas = unified_responses_into_chat_deltas(
                                vec![crate::types::unified::UnifiedResponse {
                                    tool_call: Some(crate::types::unified::UnifiedToolCall {
                                        tool_call_index: None,
                                        id: Some(tc.tool_id.clone()),
                                        name: Some(tc.tool_name.clone()),
                                        arguments: Some(tc.raw_arguments.clone()),
                                        arguments_is_snapshot: false,
                                    }),
                                    ..Default::default()
                                }],
                                context_window_tokens,
                            );
                            for delta in deltas {
                                yield Ok(delta);
                            }
                        }
                        yield Ok(ChatDelta::finish("stop"));
                    }
                    return;
                }

                let event_json: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("Gemini SSE parse error: {}, data: {}", e, data);
                        if sse_error_count > 3 {
                            yield Err(ProviderError::StreamError(format!(
                                "SSE parse failed {} times", sse_error_count
                            )));
                            return;
                        }
                        continue;
                    }
                };

                if let Some(error_msg) = extract_sse_api_error(&event_json) {
                    let err_msg = format!("Gemini SSE API error: {}, data: {}", error_msg, data);
                    warn!("{}", err_msg);
                    sse_log.flush_on_error(&err_msg);
                    yield Err(ProviderError::StreamError(err_msg));
                    return;
                }

                let gemini_response: GeminiStreamResponse = match serde_json::from_value(event_json) {
                    Ok(r) => r,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("Gemini SSE schema error: {}, data: {}", e, data);
                        if sse_error_count > 3 {
                            yield Err(ProviderError::StreamError(format!(
                                "SSE schema failed {} times", sse_error_count
                            )));
                            return;
                        }
                        continue;
                    }
                };

                let mut unified_responses = gemini_response_to_unified(gemini_response);

                for unified in &mut unified_responses {
                    if let Some(ref tc) = unified.tool_call {
                        let key = ToolCallStreamKey::from(tc.tool_call_index);
                        let outcome = tool_call_accumulator.apply_delta(
                            key,
                            tc.id.clone(),
                            tc.name.clone(),
                            tc.arguments.clone(),
                            true,
                        );

                        if let Some(finalized) = outcome.finalized_previous {
                            let deltas = unified_responses_into_chat_deltas(
                                vec![crate::types::unified::UnifiedResponse {
                                    tool_call: Some(crate::types::unified::UnifiedToolCall {
                                        tool_call_index: None,
                                        id: Some(finalized.tool_id.clone()),
                                        name: Some(finalized.tool_name.clone()),
                                        arguments: Some(finalized.raw_arguments.clone()),
                                        arguments_is_snapshot: false,
                                    }),
                                    ..Default::default()
                                }],
                                context_window_tokens,
                            );
                            for delta in deltas {
                                yield Ok(delta);
                            }
                        }

                        if let Some(ref params) = outcome.params_partial {
                            unified.tool_call = Some(crate::types::unified::UnifiedToolCall {
                                tool_call_index: tc.tool_call_index,
                                id: Some(params.tool_id.clone()),
                                name: Some(params.tool_name.clone()),
                                arguments: Some(params.params_chunk.clone()),
                                arguments_is_snapshot: false,
                            });
                        } else {
                            unified.tool_call = None;
                        }
                    }

                    if unified.finish_reason.is_some() {
                        received_finish_reason = true;
                    }
                }

                for unified in unified_responses {
                    let has_content = unified.text.is_some()
                        || unified.reasoning_content.is_some()
                        || unified.tool_call.is_some()
                        || unified.thinking_signature.is_some()
                        || unified.finish_reason.is_some()
                        || unified.usage.is_some();

                    if has_content {
                        stats.record_unified_response(&unified);
                        let deltas = unified_responses_into_chat_deltas(
                            vec![unified],
                            context_window_tokens,
                        );
                        for delta in deltas {
                            if delta.delta_text.is_some()
                                || delta.delta_reasoning.is_some()
                            {
                                timeout_ctrl.mark_first_token();
                            }
                            yield Ok(delta);
                        }
                    }
                }
            }

            if !received_finish_reason {
                debug!("Gemini stream ended without finish_reason, flushing tool calls");
                stats.log_summary("no_finish_reason");
                let finalized = tool_call_accumulator.finalize_all(ToolCallBoundary::StreamEnd);
                for tc in finalized {
                    let deltas = unified_responses_into_chat_deltas(
                        vec![crate::types::unified::UnifiedResponse {
                            tool_call: Some(crate::types::unified::UnifiedToolCall {
                                tool_call_index: None,
                                id: Some(tc.tool_id.clone()),
                                name: Some(tc.tool_name.clone()),
                                arguments: Some(tc.raw_arguments.clone()),
                                arguments_is_snapshot: false,
                            }),
                            ..Default::default()
                        }],
                        context_window_tokens,
                    );
                    for delta in deltas {
                        yield Ok(delta);
                    }
                }
            }
            return;
        }
    };

    Ok(Box::pin(stream))
}

fn resolve_request_url(base_url: &str, model_name: &str) -> String {
    let base = gemini_base_url(base_url);
    let encoded_model = urlencoding::encode(model_name.trim());
    format!(
        "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
        base, encoded_model
    )
}

pub(crate) fn gemini_base_url(url: &str) -> String {
    let mut value = url.trim().trim_end_matches('/').to_string();
    if let Some(pos) = value.find("/v1beta") {
        value = value[..pos].to_string();
    }
    if let Some(pos) = value.find("/models/") {
        value = value[..pos].to_string();
    }
    value.trim_end_matches('/').to_string()
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn resolve_request_url_cases() {
        let expected = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse";
        assert_eq!(
            resolve_request_url(
                "https://generativelanguage.googleapis.com",
                "gemini-2.0-flash"
            ),
            expected
        );
        assert_eq!(
            resolve_request_url(
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-2.0-flash"
            ),
            expected
        );
    }

    #[test]
    fn gemini_base_url_strips_paths() {
        assert_eq!(
            gemini_base_url("https://example.com/v1beta"),
            "https://example.com"
        );
        assert_eq!(
            gemini_base_url("https://example.com/models/gemini-pro"),
            "https://example.com"
        );
        assert_eq!(
            gemini_base_url("https://example.com"),
            "https://example.com"
        );
    }
}
