use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use log::{debug, warn};

use crate::base::{ChatDelta, ChatMessage, ChatOptions, ChatStream, ProviderError};
use crate::openai::responses_types::{
    parse_responses_output_item, parse_responses_replay_capture, ResponsesDone,
    ResponsesStreamEvent,
};
use crate::sse_log::SseLogCollector;
use crate::stream::stats::StreamStats;
use crate::stream::timeout::{StreamTimeoutController, StreamTimeoutStage};
use crate::types::chat_delta::unified_responses_into_chat_deltas;
use crate::types::unified::{
    ReasoningContentKind, ToolCallCompletion, UnifiedResponse, UnifiedToolCall,
};
use tokio_util::sync::CancellationToken;

use super::message_converter;
use super::OpenAIAdapter;
use crate::client::error::{classify_http_error, classify_request_error};

#[derive(Debug, Default, Clone)]
struct InProgressToolCall {
    call_id: Option<String>,
    name: Option<String>,
    args_so_far: String,
    saw_any_delta: bool,
    sent_header: bool,
}

impl InProgressToolCall {
    fn from_item_value(item: &serde_json::Value) -> Option<Self> {
        if item.get("type").and_then(|v| v.as_str()) != Some("function_call") {
            return None;
        }
        Some(Self {
            call_id: item
                .get("call_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            name: item
                .get("name")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            args_so_far: String::new(),
            saw_any_delta: false,
            sent_header: false,
        })
    }
}

fn responses_completed_tool_call_completion(saw_function_call: bool) -> ToolCallCompletion {
    if saw_function_call {
        ToolCallCompletion::NormalToolUse
    } else {
        ToolCallCompletion::NormalNoToolUse
    }
}

fn completed_replay_capture(
    response: Option<&serde_json::Value>,
    done_items: &BTreeMap<usize, serde_json::Value>,
    done_items_complete: bool,
) -> Option<crate::types::unified::ModelResponseReplayCapture> {
    let terminal_output = response
        .and_then(|r| r.get("output"))
        .and_then(|v| v.as_array())
        .filter(|output| !output.is_empty());
    if let Some(capture) = terminal_output.and_then(|output| parse_responses_replay_capture(output))
    {
        return Some(capture);
    }

    if !done_items_complete || done_items.is_empty() {
        return None;
    }
    if terminal_output.is_some_and(|output| output.len() != done_items.len()) {
        return None;
    }
    if done_items
        .keys()
        .copied()
        .enumerate()
        .any(|(expected, actual)| expected != actual)
    {
        return None;
    }

    let output = done_items.values().cloned().collect::<Vec<_>>();
    parse_responses_replay_capture(&output)
}

fn extract_api_error_message(event_json: &serde_json::Value) -> Option<String> {
    let error = event_json
        .get("response")
        .and_then(|r| r.get("error"))
        .or_else(|| event_json.get("error"))?;

    if error.is_null() {
        return None;
    }

    if let Some(message) = error.get("message").and_then(|v| v.as_str()) {
        return Some(message.to_string());
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }

    Some("An error occurred during responses streaming".to_string())
}

fn separate_reasoning_summary_part(
    last_part: &mut Option<(Option<usize>, usize)>,
    output_index: Option<usize>,
    summary_index: Option<usize>,
    delta: String,
) -> String {
    let Some(summary_index) = summary_index else {
        return delta;
    };
    let current_part = (output_index, summary_index);
    let starts_new_part = last_part.is_some_and(|previous| previous != current_part);
    *last_part = Some(current_part);

    if starts_new_part {
        format!("\n\n{delta}")
    } else {
        delta
    }
}

pub(crate) fn responses_stream(
    adapter: &OpenAIAdapter,
    api_key: &str,
    messages: &[ChatMessage],
    model: &str,
    options: &ChatOptions,
    cancel_token: CancellationToken,
) -> Result<ChatStream, ProviderError> {
    let api_key = api_key.to_string();
    let base_url = adapter.base_url().to_string();

    let req_messages = message_converter::convert_messages(messages);

    let client = adapter.client.clone();
    let retry_config = adapter.retry_config.clone();
    let timeout_config = adapter.timeout_config.clone();
    let sse_log_config = adapter.sse_log_config.clone();
    let context_window_tokens = options.context_window_tokens;
    let reasoning_enabled = options.reasoning_enabled;
    let reasoning_effort = options.reasoning_effort.clone();
    let max_tokens = options.max_tokens;
    let model_name = model.to_string();

    let stream = async_stream::stream! {
        let mut attempt: u32 = 0;
        'retry: loop {
            let url = format!("{}/responses", base_url);

            let mut body_value = serde_json::json!({
                "model": model_name,
                "input": req_messages,
                "stream": true,
                "store": false
            });

            {
                if max_tokens > 0 {
                    body_value["max_output_tokens"] = serde_json::json!(max_tokens);
                }

                let enabled = reasoning_enabled.unwrap_or(true);

                if reasoning_enabled.is_some() {
                    body_value["reasoning"] = serde_json::json!({
                        "effort": if enabled {
                            reasoning_effort.as_deref().unwrap_or("medium")
                        } else {
                            "none"
                        },
                        "summary": "auto"
                    });
                } else if let Some(ref effort) = reasoning_effort {
                    if !effort.trim().is_empty() {
                        body_value["reasoning"] = serde_json::json!({
                            "effort": effort,
                            "summary": "auto"
                        });
                    }
                }
            }

            let body = match serde_json::to_string(&body_value) {
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
            let mut stats = StreamStats::new("openai/responses");
            let mut sse_chunk_count: u32 = 0;
            let mut received_finish_reason = false;

            let mut saw_function_call = false;
            let mut tool_calls_by_output_index: HashMap<usize, InProgressToolCall> = HashMap::new();
            let mut tool_call_index_by_id: HashMap<String, usize> = HashMap::new();
            let mut done_items_by_output_index: BTreeMap<usize, serde_json::Value> = BTreeMap::new();
            let mut done_items_complete = true;
            let mut last_reasoning_summary_part: Option<(Option<usize>, usize)> = None;
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
                    debug!("[Responses SSE] stream done after {} chunks", sse_chunk_count);
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
                        warn!("Responses SSE parse error #{}: {}", sse_error_count, e);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "Responses SSE parse failed {} times", sse_error_count
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                if let Some(sse_err) = extract_api_error_message(&event_json) {
                    let diag_msg = format!("Responses API error: {}", sse_err);
                    warn!("{}", diag_msg);
                    sse_log.flush_on_error(&diag_msg);
                    yield Err(ProviderError::StreamError(diag_msg));
                    return;
                }

                let resp_event: ResponsesStreamEvent = match serde_json::from_value(event_json) {
                    Ok(e) => e,
                    Err(e) => {
                        sse_error_count += 1;
                        warn!("Responses SSE schema error #{}: {}", sse_error_count, e);
                        if sse_error_count <= 3 {
                            continue;
                        }
                        let err = ProviderError::StreamError(format!(
                            "Responses SSE schema failed {} times", sse_error_count
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }
                };

                let mut unified_responses: Vec<UnifiedResponse> = Vec::new();

                match resp_event.kind.as_str() {
                    "response.created" => {}
                    "response.output_item.added" => {
                        if let Some(item) = resp_event.item.as_ref() {
                            if let Some(tc) = InProgressToolCall::from_item_value(item) {
                                let Some(output_index) = resp_event.output_index else {
                                    continue;
                                };
                                if let Some(ref call_id) = tc.call_id {
                                    tool_call_index_by_id.insert(call_id.clone(), output_index);
                                }
                                saw_function_call = true;
                                tool_calls_by_output_index.insert(output_index, tc);
                            }
                        }
                    }
                    "response.output_text.delta" => {
                        if let Some(delta) = resp_event.delta.filter(|d| !d.is_empty()) {
                            timeout_ctrl.mark_first_token();
                            unified_responses.push(UnifiedResponse {
                                text: Some(delta),
                                ..Default::default()
                            });
                        }
                    }
                    "response.reasoning_text.delta" => {
                        if let Some(delta) = resp_event.delta.filter(|d| !d.is_empty()) {
                            timeout_ctrl.mark_first_token();
                            unified_responses.push(UnifiedResponse {
                                reasoning_content: Some(delta),
                                reasoning_content_kind: Some(ReasoningContentKind::Reasoning),
                                ..Default::default()
                            });
                        }
                    }
                    "response.reasoning_summary_text.delta" => {
                        if let Some(delta) = resp_event.delta.filter(|d| !d.is_empty()) {
                            timeout_ctrl.mark_first_token();
                            let delta = separate_reasoning_summary_part(
                                &mut last_reasoning_summary_part,
                                resp_event.output_index,
                                resp_event.summary_index,
                                delta,
                            );
                            unified_responses.push(UnifiedResponse {
                                reasoning_content: Some(delta),
                                reasoning_content_kind: Some(ReasoningContentKind::Summary),
                                ..Default::default()
                            });
                        }
                    }
                    "response.function_call_arguments.delta" => {
                        let Some(delta) = resp_event.delta.filter(|d| !d.is_empty()) else {
                            continue;
                        };
                        let Some(output_index) = resp_event.output_index else {
                            continue;
                        };
                        let Some(tc) = tool_calls_by_output_index.get_mut(&output_index) else {
                            continue;
                        };

                        tc.saw_any_delta = true;
                        tc.args_so_far.push_str(&delta);

                        let (id, name) = if tc.sent_header {
                            (None, None)
                        } else {
                            tc.sent_header = true;
                            (tc.call_id.clone(), tc.name.clone())
                        };

                        unified_responses.push(UnifiedResponse {
                            tool_call: Some(UnifiedToolCall {
                                tool_call_index: Some(output_index),
                                id,
                                name,
                                arguments: Some(delta),
                                arguments_is_snapshot: false,
                            }),
                            ..Default::default()
                        });
                    }
                    "response.output_item.done" => {
                        let Some(item_value) = resp_event.item else {
                            continue;
                        };
                        if let Some(output_index) = resp_event.output_index {
                            done_items_by_output_index.insert(output_index, item_value.clone());
                        } else {
                            done_items_complete = false;
                        }

                        if item_value.get("type").and_then(|v| v.as_str()) == Some("function_call")
                        {
                            saw_function_call = true;
                            let event_output_index = resp_event.output_index;
                            let output_index = event_output_index.or_else(|| {
                                item_value
                                    .get("call_id")
                                    .and_then(|v| v.as_str())
                                    .and_then(|id| tool_call_index_by_id.get(id).copied())
                            });

                            if let Some(output_index) = output_index {
                                if let Some(tc) =
                                    tool_calls_by_output_index.get_mut(&output_index)
                                {
                                    let full_args = item_value
                                        .get("arguments")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or_default();
                                    let need_fallback_full = !tc.saw_any_delta;
                                    let need_tail = tc.saw_any_delta
                                        && tc.args_so_far.len() < full_args.len()
                                        && full_args.starts_with(&tc.args_so_far);

                                    if need_fallback_full || need_tail {
                                        let delta = if need_fallback_full {
                                            full_args.to_string()
                                        } else {
                                            full_args[tc.args_so_far.len()..].to_string()
                                        };
                                        if !delta.is_empty() {
                                            tc.args_so_far.push_str(&delta);
                                            let (id, name) = if tc.sent_header {
                                                (None, None)
                                            } else {
                                                tc.sent_header = true;
                                                (tc.call_id.clone(), tc.name.clone())
                                            };
                                            unified_responses.push(UnifiedResponse {
                                                tool_call: Some(UnifiedToolCall {
                                                    tool_call_index: Some(output_index),
                                                    id,
                                                    name,
                                                    arguments: Some(delta),
                                                    arguments_is_snapshot: false,
                                                }),
                                                ..Default::default()
                                            });
                                        }
                                    }

                                    if let Some(tc) =
                                        tool_calls_by_output_index.remove(&output_index)
                                    {
                                        if let Some(call_id) = tc.call_id {
                                            tool_call_index_by_id.remove(&call_id);
                                        }
                                    }
                                } else if let Some(ur) =
                                    parse_responses_output_item(item_value, event_output_index)
                                {
                                    if ur.tool_call.is_some() {
                                        unified_responses.push(ur);
                                    }
                                }
                            } else if let Some(ur) =
                                parse_responses_output_item(item_value, event_output_index)
                            {
                                if ur.tool_call.is_some() {
                                    unified_responses.push(ur);
                                }
                            }
                            continue;
                        }

                        if let Some(mut ur) =
                            parse_responses_output_item(item_value, resp_event.output_index)
                        {
                            if timeout_ctrl.first_token_received() && ur.text.is_some() {
                                ur.text = None;
                            }
                            if ur.text.is_some() || ur.tool_call.is_some() {
                                unified_responses.push(ur);
                            }
                        }
                    }
                    "response.completed" | "response.done" => {
                        if received_finish_reason {
                            continue;
                        }
                        let model_response_replay = completed_replay_capture(
                            resp_event.response.as_ref(),
                            &done_items_by_output_index,
                            done_items_complete,
                        );

                        if let Some(response_val) = resp_event.response.as_ref() {
                            if let Some(output) =
                                response_val.get("output").and_then(|v| v.as_array())
                            {
                                for (idx, item) in output.iter().enumerate() {
                                    if item.get("type").and_then(|v| v.as_str())
                                        != Some("function_call")
                                    {
                                        continue;
                                    }
                                    saw_function_call = true;
                                    let Some(tc) = tool_calls_by_output_index.get_mut(&idx)
                                    else {
                                        continue;
                                    };
                                    let full_args = item
                                        .get("arguments")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or_default();
                                    if tc.args_so_far.len() < full_args.len()
                                        && full_args.starts_with(&tc.args_so_far)
                                    {
                                        let delta = full_args[tc.args_so_far.len()..].to_string();
                                        if !delta.is_empty() {
                                            tc.args_so_far.push_str(&delta);
                                            let (id, name) = if tc.sent_header {
                                                (None, None)
                                            } else {
                                                tc.sent_header = true;
                                                (tc.call_id.clone(), tc.name.clone())
                                            };
                                            unified_responses.push(UnifiedResponse {
                                                tool_call: Some(UnifiedToolCall {
                                                    tool_call_index: Some(idx),
                                                    id,
                                                    name,
                                                    arguments: Some(delta),
                                                    arguments_is_snapshot: false,
                                                }),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        let usage = resp_event.response.clone().and_then(|v| {
                            serde_json::from_value::<ResponsesDone>(v)
                                .ok()
                                .and_then(|r| r.usage)
                                .map(Into::into)
                        });

                        received_finish_reason = true;
                        unified_responses.push(UnifiedResponse {
                            usage,
                            tool_call_completion: Some(
                                responses_completed_tool_call_completion(saw_function_call),
                            ),
                            finish_reason: Some("stop".to_string()),
                            model_response_replay,
                            ..Default::default()
                        });
                    }
                    "response.failed" => {
                        let error_msg = resp_event
                            .response
                            .as_ref()
                            .and_then(|r| r.get("error"))
                            .and_then(|e| e.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Responses API returned response.failed")
                            .to_string();
                        sse_log.flush_on_error(&error_msg);
                        yield Err(ProviderError::StreamError(error_msg));
                        return;
                    }
                    "response.incomplete" => {
                        if received_finish_reason {
                            continue;
                        }
                        let reason = resp_event
                            .response
                            .as_ref()
                            .and_then(|r| r.get("incomplete_details"))
                            .and_then(|d| d.get("reason"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let finish_reason = reason
                            .as_deref()
                            .map(|r| format!("incomplete:{r}"))
                            .unwrap_or_else(|| "incomplete".to_string());

                        let completion =
                            match reason.as_deref().map(str::to_ascii_lowercase).as_deref() {
                                Some("max_output_tokens") | Some("max_tokens") => {
                                    ToolCallCompletion::OutputLimit
                                }
                                Some("content_filter") => ToolCallCompletion::ContentFiltered,
                                _ => ToolCallCompletion::Unknown,
                            };

                        let usage = resp_event.response.clone().and_then(|v| {
                            serde_json::from_value::<ResponsesDone>(v)
                                .ok()
                                .and_then(|r| r.usage)
                                .map(Into::into)
                        });

                        received_finish_reason = true;
                        unified_responses.push(UnifiedResponse {
                            usage,
                            tool_call_completion: Some(completion),
                            finish_reason: Some(finish_reason),
                            ..Default::default()
                        });
                    }
                    _ => {}
                }

                for ur in &unified_responses {
                    stats.record_unified_response(ur);
                }
                let deltas = unified_responses_into_chat_deltas(unified_responses, context_window_tokens);

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
                        yield Ok(delta);
                    }
                }
            }

            if received_finish_reason {
                debug!("[Responses SSE] stream closed after finish_reason received");
                stats.log_summary("finish_reason");
            } else {
                warn!("[Responses SSE] stream closed without finish_reason");
                stats.log_summary("no_finish_reason");
            }
            return;
        }
    };
    Ok(Box::pin(stream))
}
