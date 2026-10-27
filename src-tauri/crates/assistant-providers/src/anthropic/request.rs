use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, warn};

use crate::anthropic::stream::{
    content_block_delta_to_unified, content_block_start_to_unified, message_delta_to_unified,
};
use crate::anthropic::types::{
    AnthropicSSEError, AnthropicUsage, ContentBlockDelta, ContentBlockStart, MessageDelta,
    MessageStart, MessagesRequest,
};
use crate::base::{ChatDelta, ChatMessage, ChatOptions, ChatStream, ProviderError};
use crate::client::quirks::{normalize_generic_reasoning_effort, wants_bearer_auth};
use crate::sse_log::SseLogCollector;
use crate::stream::stats::StreamStats;
use crate::stream::timeout::{StreamTimeoutController, StreamTimeoutStage};
use crate::types::chat_delta::unified_responses_into_chat_deltas;
use tokio_util::sync::CancellationToken;

use super::message_converter;
use super::AnthropicAdapter;
use crate::client::error::{classify_http_error, classify_request_error};

pub(crate) fn chat_completion_stream(
    adapter: &AnthropicAdapter,
    api_key: &str,
    messages: &[ChatMessage],
    model: &str,
    options: &ChatOptions,
    cancel_token: CancellationToken,
) -> Result<ChatStream, ProviderError> {
    let api_key = api_key.to_string();
    let base_url = adapter.base_url().to_string();
    let (system, anthropic_messages) = message_converter::convert_messages(messages);

    let max_tokens = if options.max_tokens > 0 {
        options.max_tokens
    } else {
        4096
    };

    let req_body = MessagesRequest {
        model: model.to_string(),
        messages: anthropic_messages,
        max_tokens,
        stream: true,
        temperature: if options.temperature > 0.0 {
            Some(options.temperature)
        } else {
            None
        },
        system,
    };

    let client = adapter.client.clone();
    let retry_config = adapter.retry_config.clone();
    let timeout_config = adapter.timeout_config.clone();
    let sse_log_config = adapter.sse_log_config.clone();
    let context_window_tokens = options.context_window_tokens;
    let reasoning_enabled = options.reasoning_enabled;
    let reasoning_effort = options.reasoning_effort.clone();
    let _model_name = model.to_string();

    let stream = async_stream::stream! {
        let mut attempt: u32 = 0;
        'retry: loop {
            let url = format!("{}/messages", base_url);

            let mut body_value = match serde_json::to_value(&req_body) {
                Ok(v) => v,
                Err(e) => {
                    yield Err(ProviderError::ParseError(format!("Serialize request: {}", e)));
                    return;
                }
            };

            {
                let enabled = reasoning_enabled.unwrap_or(false);
                if reasoning_enabled.is_some() {
                    if enabled {
                        body_value["thinking"] = serde_json::json!({
                            "type": "enabled",
                            "budget_tokens": (max_tokens * 3 / 4).clamp(1, 10000)
                        });
                    } else {
                        body_value["thinking"] = serde_json::json!({ "type": "disabled" });
                    }
                }

                if let Some(ref effort) = reasoning_effort {
                    if let Some(normalized) = normalize_generic_reasoning_effort(effort) {
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

            let mut req_builder = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body);

            if wants_bearer_auth(&url) {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
                if !url.contains("bigmodel.cn") {
                    req_builder = req_builder.header("anthropic-version", "2023-06-01");
                }
            } else {
                req_builder = req_builder
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", "2023-06-01");
            }

            let resp = match req_builder.send().await {
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

            let mut usage = AnthropicUsage::default();
            let mut sse_error_count: u32 = 0;
            let mut sse_log = SseLogCollector::new(sse_log_config.clone());
            let mut timeout_ctrl = StreamTimeoutController::new(
                timeout_config.first_token_timeout,
                timeout_config.idle_timeout,
            );
            let mut stats = StreamStats::new("anthropic");
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

                let event_type = event.event;
                let data = event.data;

                sse_log.push(data.clone());
                sse_chunk_count += 1;
                stats.record_sse_event(&event_type);

                if event_type == "error" {
                    match serde_json::from_str::<AnthropicSSEError>(&data) {
                        Ok(sse_error) => {
                            let err_msg = format!("{}: {}", sse_error.error.error_type, sse_error.error.message);
                            warn!("Anthropic SSE error: {}", err_msg);
                            sse_log.flush_on_error(&err_msg);
                            yield Err(ProviderError::StreamError(err_msg));
                            return;
                        }
                        Err(e) => {
                            let err_msg = format!("SSE error parse failed: {}, data: {}", e, data);
                            warn!("{}", err_msg);
                            sse_log.flush_on_error(&err_msg);
                            yield Err(ProviderError::StreamError(err_msg));
                            return;
                        }
                    }
                }

                match event_type.as_str() {
                    "message_start" => {
                        match serde_json::from_str::<MessageStart>(&data) {
                            Ok(message_start) => {
                                if let Some(message_usage) = message_start.message.usage {
                                    usage.update(&message_usage);
                                }
                            }
                            Err(e) => {
                                sse_error_count += 1;
                                warn!("message_start parse error: {}, data: {}", e, data);
                                if sse_error_count > 3 {
                                    yield Err(ProviderError::StreamError(format!(
                                        "SSE parse failed {} times", sse_error_count
                                    )));
                                    return;
                                }
                                continue;
                            }
                        }
                    }
                    "content_block_start" => {
                        match serde_json::from_str::<ContentBlockStart>(&data) {
                            Ok(content_block_start) => {
                                let unified = content_block_start_to_unified(content_block_start);
                                let has_content = unified.text.is_some()
                                    || unified.reasoning_content.is_some()
                                    || unified.tool_call.is_some()
                                    || unified.thinking_signature.is_some();
                                if has_content {
                                    stats.record_unified_response(&unified);
                                    let deltas = unified_responses_into_chat_deltas(
                                        vec![unified],
                                        context_window_tokens,
                                    );
                                    for delta in deltas {
                                        if delta.delta_text.is_some() || delta.delta_reasoning.is_some() {
                                            timeout_ctrl.mark_first_token();
                                        }
                                        yield Ok(delta);
                                    }
                                }
                            }
                            Err(e) => {
                                sse_error_count += 1;
                                warn!("content_block_start parse error: {}, data: {}", e, data);
                                if sse_error_count > 3 {
                                    yield Err(ProviderError::StreamError(format!(
                                        "SSE parse failed {} times", sse_error_count
                                    )));
                                    return;
                                }
                                continue;
                            }
                        }
                    }
                    "content_block_delta" => {
                        match serde_json::from_str::<ContentBlockDelta>(&data) {
                            Ok(content_block_delta) => {
                                match content_block_delta_to_unified(content_block_delta) {
                                    Ok(unified) => {
                                        stats.record_unified_response(&unified);
                                        let deltas = unified_responses_into_chat_deltas(
                                            vec![unified],
                                            context_window_tokens,
                                        );
                                        for delta in deltas {
                                            if delta.delta_text.is_some() || delta.delta_reasoning.is_some() {
                                                timeout_ctrl.mark_first_token();
                                            }
                                            yield Ok(delta);
                                        }
                                    }
                                    Err(e) => {
                                        debug!("Skipping invalid content_block_delta: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                sse_error_count += 1;
                                warn!("content_block_delta parse error: {}, data: {}", e, data);
                                if sse_error_count > 3 {
                                    yield Err(ProviderError::StreamError(format!(
                                        "SSE parse failed {} times", sse_error_count
                                    )));
                                    return;
                                }
                                continue;
                            }
                        }
                    }
                    "message_delta" => {
                        match serde_json::from_str::<MessageDelta>(&data) {
                            Ok(mut message_delta) => {
                                if let Some(delta_usage) = message_delta.usage.as_ref() {
                                    usage.update(delta_usage);
                                }
                                message_delta.usage = if usage.is_empty() {
                                    None
                                } else {
                                    Some(usage.clone())
                                };
                                let unified = message_delta_to_unified(message_delta);
                                if unified.finish_reason.is_some() {
                                    received_finish_reason = true;
                                }
                                stats.record_unified_response(&unified);
                                let deltas = unified_responses_into_chat_deltas(
                                    vec![unified],
                                    context_window_tokens,
                                );
                                for delta in deltas {
                                    yield Ok(delta);
                                }
                            }
                            Err(e) => {
                                sse_error_count += 1;
                                warn!("message_delta parse error: {}, data: {}", e, data);
                                if sse_error_count > 3 {
                                    yield Err(ProviderError::StreamError(format!(
                                        "SSE parse failed {} times", sse_error_count
                                    )));
                                    return;
                                }
                                continue;
                            }
                        }
                    }
                    "message_stop" => {
                        debug!("[SSE] Anthropic stream done after {} chunks", sse_chunk_count);
                        return;
                    }
                    "content_block_stop" => {}
                    _ => {
                        debug!("[SSE] Unknown Anthropic event type: {}", event_type);
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