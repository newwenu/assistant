use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, warn};

use crate::base::{ChatDelta, ChatMessage, ChatOptions, ChatStream, ProviderError};
use crate::gemini::message_converter;
use crate::gemini::stream::{extract_sse_api_error, gemini_response_to_unified};
use crate::gemini::types::GenerateContentRequest;
use crate::sse_log::SseLogCollector;
use crate::stream::stats::StreamStats;
use crate::stream::timeout::{StreamTimeoutController, StreamTimeoutStage};
use crate::types::chat_delta::unified_responses_into_chat_deltas;
use tokio_util::sync::CancellationToken;

use super::GeminiAdapter;
use crate::client::error::{classify_http_error, classify_request_error};

const CODE_ASSIST_BASE: &str = "https://cloudcode-pa.googleapis.com";
const STREAM_ENDPOINT: &str = "/v1internal:streamGenerateContent?alt=sse";

const DEFAULT_CODE_ASSIST_MODELS: &[(&str, &str)] = &[
    ("gemini-2.5-pro", "Gemini 2.5 Pro"),
    ("gemini-2.5-flash", "Gemini 2.5 Flash"),
    ("gemini-2.5-flash-lite", "Gemini 2.5 Flash-Lite"),
    ("gemini-2.0-flash", "Gemini 2.0 Flash"),
];

pub(crate) fn code_assist_stream(
    adapter: &GeminiAdapter,
    api_key: &str,
    messages: &[ChatMessage],
    model: &str,
    options: &ChatOptions,
    cancel_token: CancellationToken,
) -> Result<ChatStream, ProviderError> {
    let api_key = api_key.to_string();
    let base_url = adapter.base_url().to_string();
    let (system_instruction, contents) = message_converter::convert_messages(messages);

    let url = if adapter.config.base_url.as_deref().unwrap_or("").is_empty() {
        format!("{}{}", CODE_ASSIST_BASE, STREAM_ENDPOINT)
    } else {
        format!("{}{}", base_url, STREAM_ENDPOINT)
    };

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
    let model_name = model.to_string();

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
                    let mut thinking_config = serde_json::Map::new();
                    thinking_config.insert("includeThoughts".to_string(), serde_json::json!(enabled));

                    if enabled {
                        if let Some(ref effort) = reasoning_effort {
                            if !effort.trim().is_empty() {
                                thinking_config.insert(
                                    "thinkingLevel".to_string(),
                                    serde_json::json!(effort.to_ascii_uppercase()),
                                );
                            }
                        }
                    } else {
                        thinking_config.insert("thinkingBudget".to_string(), serde_json::json!(0));
                    }

                    generation_config.insert(
                        "thinkingConfig".to_string(),
                        serde_json::Value::Object(thinking_config),
                    );
                }

                if !generation_config.is_empty() {
                    body_value["generationConfig"] = serde_json::Value::Object(generation_config);
                }
            }

            let request_body = serde_json::json!({
                "model": model_name,
                "request": body_value,
            });

            let body = match serde_json::to_string(&request_body) {
                Ok(b) => b,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request: {}", e)));
                    return;
                }
            };

            let resp = match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err = classify_request_error(e);
                    if err.is_retryable() && attempt < retry_config.max_attempts {
                        let delay = retry_config.delay_ms(attempt, &err);
                        warn!("Attempt {} failed, retrying in {}ms: {}", attempt + 1, delay, err);
                        yield Ok(ChatDelta::retry_notification_with_code(attempt, retry_config.max_attempts, delay, &err, err.code()));
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
                            _ = cancel_token.cancelled() => {
                                yield Err(ProviderError::Cancelled);
                                return;
                            }
                        };
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
                let classified = classify_http_error(status, &text);
                if classified.error.is_retryable() && attempt < retry_config.max_attempts {
                    let delay = retry_config.delay_ms(attempt, &classified.error);
                    warn!("Attempt {} failed (HTTP {}), retrying in {}ms", attempt + 1, status, delay);
                    yield Ok(ChatDelta::retry_http_notification_with_code(attempt, retry_config.max_attempts, delay, status, classified.error.code()));
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
                        _ = cancel_token.cancelled() => {
                            yield Err(ProviderError::Cancelled);
                            return;
                        }
                    };
                    attempt += 1;
                    continue 'retry;
                }
                yield Err(classified.error);
                return;
            }

            let mut event_source = resp.bytes_stream().eventsource();
            let mut sse_error_count: u32 = 0;
            let mut sse_log = SseLogCollector::new(sse_log_config.clone());
            let mut timeout_ctrl = StreamTimeoutController::new(
                timeout_config.first_token_timeout,
                timeout_config.idle_timeout,
            );
            let mut stats = StreamStats::new("gemini/code_assist");
            let mut sse_chunk_count: u32 = 0;
            let mut received_finish_reason = false;
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
                                yield Ok(ChatDelta::retry_notification_with_code(attempt, retry_config.max_attempts, delay, &err, err.code()));
                                tokio::select! {
                                    _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
                                    _ = cancel_token.cancelled() => {
                                        yield Err(ProviderError::Cancelled);
                                        return;
                                    }
                                };
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
                            yield Ok(ChatDelta::retry_notification_with_code(attempt, retry_config.max_attempts, delay, &err, err.code()));
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_millis(delay)) => {},
                                _ = cancel_token.cancelled() => {
                                    yield Err(ProviderError::Cancelled);
                                    return;
                                }
                            };
                            attempt += 1;
                            continue 'retry;
                        }
                        yield Err(err);
                        return;
                    }
                };

                timeout_ctrl.touch();
                let data = event.data;

                if data == "[DONE]" {
                    debug!("[Code Assist SSE] stream done after {} chunks", sse_chunk_count);
                    stats.record_sse_event("done");
                    stats.log_summary("done");
                    if !received_finish_reason {
                        yield Ok(ChatDelta::finish("stop"));
                    }
                    return;
                }

                stats.record_sse_event("data");
                sse_log.push(data.clone());
                sse_chunk_count += 1;

                let event_json: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("Code Assist SSE parse error #{}: {}", sse_error_count, e);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "Code Assist SSE parse failed {} times", sse_error_count
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                if let Some(sse_err) = extract_sse_api_error(&event_json) {
                    let diag_msg = format!("Code Assist API error: {}", sse_err);
                    warn!("{}", diag_msg);
                    sse_log.flush_on_error(&diag_msg);
                    yield Err(ProviderError::StreamError(diag_msg));
                    return;
                }

                let parsed: crate::gemini::types::GeminiStreamResponse = match serde_json::from_value(event_json) {
                    Ok(c) => c,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("Code Assist SSE schema error #{}: {}", sse_error_count, e);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "Code Assist SSE schema failed {} times", sse_error_count
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                let unified = gemini_response_to_unified(parsed);
                for ur in &unified {
                    stats.record_unified_response(ur);
                }
                let deltas = unified_responses_into_chat_deltas(unified, context_window_tokens);

                for delta in deltas {
                    let has_content = delta.delta_text.is_some()
                        || delta.delta_reasoning.is_some()
                        || delta.delta_tool_call_id.is_some()
                        || delta.delta_tool_call_name.is_some()
                        || delta.delta_tool_call_arguments.is_some()
                        || delta.finish_reason.is_some()
                        || delta.usage.is_some();
                    if has_content {
                        if delta.finish_reason.is_some() {
                            received_finish_reason = true;
                        }
                        if delta.delta_text.is_some() || delta.delta_reasoning.is_some() {
                            timeout_ctrl.mark_first_token();
                        }
                        yield Ok(delta);
                    }
                }
            }

            if received_finish_reason {
                debug!("[Code Assist SSE] stream closed after finish_reason received");
                stats.log_summary("finish_reason");
            } else {
                warn!("[Code Assist SSE] stream closed without finish_reason");
                stats.log_summary("no_finish_reason");
            }
            return;
        }
    };
    Ok(Box::pin(stream))
}

pub(crate) fn list_code_assist_models() -> Vec<crate::base::ModelConfig> {
    DEFAULT_CODE_ASSIST_MODELS
        .iter()
        .map(|(id, name)| crate::base::ModelConfig {
            id: (*id).to_string(),
            name: (*name).to_string(),
            owned_by: "google".to_string(),
            context_window: Some(1_000_000),
            support_vision: true,
            input_modalities: vec!["text".to_string(), "image".to_string()],
            vision: Some(crate::capability::CapabilityFact::new(
                crate::capability::Support::Supported,
                crate::capability::Provenance::Fallback,
            )),
            pdf: None,
            attachment: None,
        })
        .collect()
}
