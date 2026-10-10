use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, warn};

use crate::base::{ChatDelta, ChatMessage, ChatOptions, ChatStream, ProviderError};
use crate::client::quirks::{
    apply_reasoning_toggle, extract_sse_api_error, is_deepseek_reasoning_effort_model,
    is_glm_reasoning_effort_model, is_valid_chat_completion_chunk,
    normalize_deepseek_reasoning_effort, normalize_generic_reasoning_effort,
    normalize_glm_reasoning_effort, should_append_tool_stream,
};
use crate::sse_log::SseLogCollector;
use crate::stream::inline_think::InlineThinkParser;
use crate::stream::stats::StreamStats;
use crate::stream::timeout::{StreamTimeoutController, StreamTimeoutStage};
use crate::types::chat_delta::unified_responses_into_chat_deltas;

use super::message_converter;
use super::stream::into_unified_responses;
use super::types::{ChatRequest, StreamOptions};
use super::OpenAIAdapter;
use crate::client::error::{classify_http_error, classify_request_error};

pub(crate) fn chat_completion_stream(
    adapter: &OpenAIAdapter,
    messages: &[ChatMessage],
    model: &str,
    options: &ChatOptions,
) -> Result<ChatStream, ProviderError> {
    let api_key = adapter.decrypt_api_key()?;
    let base_url = adapter.base_url().to_string();
    let req_messages = message_converter::convert_messages(messages);
    let req_body = ChatRequest {
        model: model.to_string(),
        messages: req_messages,
        stream: true,
        temperature: if options.temperature > 0.0 {
            Some(options.temperature)
        } else {
            None
        },
        max_tokens: if options.max_tokens > 0 {
            Some(options.max_tokens)
        } else {
            None
        },
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
    };

    let client = adapter.client.clone();
    let retry_config = adapter.retry_config.clone();
    let timeout_config = adapter.timeout_config.clone();
    let sse_log_config = adapter.sse_log_config.clone();
    let context_window_tokens = options.context_window_tokens;
    let reasoning_enabled = options.reasoning_enabled;
    let reasoning_effort = options.reasoning_effort.clone();
    let model_name = model.to_string();
    let stream = async_stream::stream! {
        let mut attempt: u32 = 0;
        'retry: loop {
            let url = format!("{}/chat/completions", base_url);

            let mut body_value = match serde_json::to_value(&req_body) {
                Ok(v) => v,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request: {}", e)));
                    return;
                }
            };

            if should_append_tool_stream(&url, &model_name) {
                body_value["tool_stream"] = serde_json::Value::Bool(true);
            }

            {
                let enabled = reasoning_enabled.unwrap_or(true);
                let toggle_handled = apply_reasoning_toggle(&mut body_value, enabled, &url);

                if !toggle_handled && reasoning_enabled.is_some() {
                    body_value["thinking"] = serde_json::json!({
                        "type": if enabled { "enabled" } else { "disabled" }
                    });
                }

                if let Some(ref effort) = reasoning_effort {
                    if is_deepseek_reasoning_effort_model(&model_name) {
                        if let Some(normalized) = normalize_deepseek_reasoning_effort(&model_name, effort) {
                            body_value["thinking"] = serde_json::json!({ "type": "enabled" });
                            body_value["reasoning_effort"] = serde_json::json!(normalized);
                        }
                    } else if is_glm_reasoning_effort_model(&model_name) {
                        if let Some(normalized) = normalize_glm_reasoning_effort(effort) {
                            body_value["thinking"] = serde_json::json!({ "type": "enabled" });
                            body_value["reasoning_effort"] = serde_json::json!(normalized);
                        }
                    } else if let Some(normalized) = normalize_generic_reasoning_effort(effort) {
                        body_value["reasoning_effort"] = serde_json::json!(normalized);
                    }
                }

                if !enabled {
                    if let Some(obj) = body_value.as_object_mut() {
                        obj.remove("reasoning_effort");
                    }
                }
            }

            let body = match serde_json::to_string(&body_value) {
                Ok(b) => b,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request body: {}", e)));
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
                        yield Ok(ChatDelta::retry_notification(attempt, &err));
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
                if err.is_retryable() && attempt < retry_config.max_attempts {
                    let delay = retry_config.delay_ms(attempt, &err);
                    warn!("Attempt {} failed (HTTP {}), retrying in {}ms", attempt + 1, status, delay);
                    yield Ok(ChatDelta::retry_http_notification(attempt, status));
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    attempt += 1;
                    continue 'retry;
                }
                yield Err(err);
                return;
            }

            let mut event_source = resp
                .bytes_stream()
                .eventsource();

            let mut sse_error_count: u32 = 0;
            let mut sse_log = SseLogCollector::new(sse_log_config.clone());
            let mut timeout_ctrl = StreamTimeoutController::new(
                timeout_config.first_token_timeout,
                timeout_config.idle_timeout,
            );
            let mut stats = StreamStats::new("openai/chat");
            let mut inline_think_parser = InlineThinkParser::new(true);
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
                if data == "[DONE]" {
                    let flushed = inline_think_parser.flush();
                    if !flushed.is_empty() {
                        let deltas = unified_responses_into_chat_deltas(flushed, context_window_tokens);
                        for delta in deltas {
                            yield Ok(delta);
                        }
                    }
                    debug!("[SSE] stream done after {} chunks", sse_chunk_count);
                    stats.record_sse_event("done");
                    stats.log_summary("done");
                    yield Ok(ChatDelta::finish("stop"));
                    return;
                }

                stats.record_sse_event("data");
                sse_log.push(data.clone());
                sse_chunk_count += 1;
                if data.contains("\"usage\"") {
                    let preview = &data[..data.len().min(200)];
                    debug!("[SSE #{}] {}", sse_chunk_count, preview);
                }

                let event_json: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("SSE parse error #{}: {} for data: {}", sse_error_count, e, data);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "SSE parse failed {} times, last: {}, data: {}", sse_error_count, e, data
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                if let Some(sse_err) = extract_sse_api_error(&event_json) {
                    let diag_msg = format!("SSE API error: {}, data: {}", sse_err, data);
                    warn!("{}", diag_msg);
                    sse_log.flush_on_error(&diag_msg);
                    yield Err(ProviderError::StreamError(diag_msg));
                    return;
                }

                if !is_valid_chat_completion_chunk(&event_json) {
                    debug!(
                        "[SSE] skipping non-standard event; object={}",
                        event_json.get("object").and_then(|v| v.as_str()).unwrap_or("<missing>")
                    );
                    continue;
                }

                let parsed: super::types::ChatResponseChunk = match serde_json::from_value(event_json) {
                    Ok(c) => c,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("SSE schema error #{}: {} for data: {}", sse_error_count, e, data);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "SSE schema failed {} times, last: {}, data: {}", sse_error_count, e, data
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                let unified = into_unified_responses(parsed);
                let mut processed = Vec::with_capacity(unified.len());
                for ur in unified {
                    let normalized = inline_think_parser.normalize_response(ur);
                    for n in &normalized {
                        stats.record_unified_response(n);
                    }
                    processed.extend(normalized);
                }
                let deltas = unified_responses_into_chat_deltas(processed, context_window_tokens);

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
                debug!("[SSE] stream closed after finish_reason received");
                stats.log_summary("finish_reason");
            } else {
                warn!("[SSE] stream closed without finish_reason");
                stats.log_summary("no_finish_reason");
            }
            return;
        }
    };
    Ok(Box::pin(stream))
}
