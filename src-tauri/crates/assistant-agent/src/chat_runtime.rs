use std::time::Duration;

use assistant_providers::base::{ChatDelta, ProviderError, ToolCall};
use futures::Stream;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct StreamOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub total_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub has_effective_output: bool,
    pub partial_recovery_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PartialOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub total_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub has_effective_output: bool,
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{0}")]
    Provider(ProviderError, PartialOutcome),
    #[error("Cancelled")]
    Cancelled(PartialOutcome),
    #[error("Timeout")]
    Timeout(PartialOutcome),
}

#[derive(Debug, Clone)]
pub struct ApplyDeltaOutcome {
    pub finalized_previous: Option<ToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Debug, Clone)]
pub struct ToolCallParamsChunk {
    pub index: usize,
    pub chunk: String,
}

#[derive(Debug, Clone)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments_buf: String,
}

#[derive(Debug, Clone)]
pub struct ToolCallAccumulator {
    pending: Vec<PartialToolCall>,
}

#[derive(Debug, Clone)]
pub struct ToolCallDeltaOutcome {
    pub finalized_previous: Option<ToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Error, Debug)]
pub enum ToolCallAccumulateError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Truncated tool call (max_tokens)")]
    Truncated,
}

pub struct StreamAccumulator {
    full_content: String,
    full_reasoning: String,
    total_tokens: Option<u32>,
    prompt_tokens: Option<u32>,
    finish_reason: Option<String>,
    tool_calls: ToolCallAccumulator,
    has_effective_output: bool,
}

enum TimedStreamItem<T> {
    Item(T),
    End,
    TimedOut,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            full_content: String::new(),
            full_reasoning: String::new(),
            total_tokens: None,
            prompt_tokens: None,
            finish_reason: None,
            tool_calls: ToolCallAccumulator::new(),
            has_effective_output: false,
        }
    }

    pub fn apply_delta(&mut self, delta: &ChatDelta) -> ApplyDeltaOutcome {
        if let Some(text) = &delta.delta_text {
            if !text.is_empty() {
                self.full_content.push_str(text);
                self.has_effective_output = true;
            }
        }

        if let Some(reasoning) = &delta.delta_reasoning {
            if !reasoning.is_empty() {
                self.full_reasoning.push_str(reasoning);
            }
        }

        if let Some(ref usage) = delta.usage {
            let total = usage.prompt_tokens + usage.completion_tokens;
            if total > 0 {
                self.total_tokens = Some(total);
            }
            if usage.prompt_tokens > 0 {
                self.prompt_tokens = Some(usage.prompt_tokens);
            }
        }

        if delta.finish_reason.is_some() {
            self.finish_reason = delta.finish_reason.clone();
        }

        let tc_outcome = self.tool_calls.apply_delta(delta);

        if tc_outcome.finalized_previous.is_some()
            || tc_outcome.params_partial.is_some()
            || delta
                .delta_tool_call_id
                .as_deref()
                .map_or(false, |id| !id.is_empty())
        {
            self.has_effective_output = true;
        }

        ApplyDeltaOutcome {
            finalized_previous: tc_outcome.finalized_previous,
            params_partial: tc_outcome.params_partial,
        }
    }

    pub fn into_outcome(self) -> StreamOutcome {
        let tool_calls_result = self.tool_calls.finish();
        let tool_calls = match tool_calls_result {
            Ok(calls) => {
                if calls.is_empty() {
                    None
                } else {
                    Some(calls)
                }
            }
            Err(ToolCallAccumulateError::Truncated) => None,
            Err(ToolCallAccumulateError::InvalidJson(_)) => None,
        };

        StreamOutcome {
            full_content: self.full_content,
            full_reasoning: self.full_reasoning,
            total_tokens: self.total_tokens,
            prompt_tokens: self.prompt_tokens,
            finish_reason: self.finish_reason,
            tool_calls,
            has_effective_output: self.has_effective_output,
            partial_recovery_reason: None,
        }
    }

    pub fn into_partial(self) -> PartialOutcome {
        PartialOutcome {
            full_content: self.full_content,
            full_reasoning: self.full_reasoning,
            total_tokens: self.total_tokens,
            prompt_tokens: self.prompt_tokens,
            has_effective_output: self.has_effective_output,
        }
    }

    pub fn has_effective_output(&self) -> bool {
        self.has_effective_output
    }

    pub fn full_content(&self) -> &str {
        &self.full_content
    }

    pub fn total_tokens(&self) -> Option<u32> {
        self.total_tokens
    }

    pub fn prompt_tokens(&self) -> Option<u32> {
        self.prompt_tokens
    }
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn apply_delta(&mut self, delta: &ChatDelta) -> ToolCallDeltaOutcome {
        let mut finalized_previous = None;
        let mut params_partial = None;

        if let Some(id) = &delta.delta_tool_call_id {
            if !id.is_empty() {
                if !self.pending.is_empty() {
                    let prev = self.pending.pop().unwrap();
                    finalized_previous = Some(ToolCall {
                        id: prev.id,
                        name: prev.name,
                        arguments: prev.arguments_buf,
                    });
                }
                self.pending.push(PartialToolCall {
                    id: id.clone(),
                    name: String::new(),
                    arguments_buf: String::new(),
                });
            }
        }

        if !self.pending.is_empty() {
            let last_idx = self.pending.len() - 1;
            let last = self.pending.last_mut().unwrap();
            if let Some(name) = &delta.delta_tool_call_name {
                last.name.push_str(name);
                self_mark_effective(&mut params_partial, last_idx, name);
            }
            if let Some(args) = &delta.delta_tool_call_arguments {
                last.arguments_buf.push_str(args);
                params_partial = Some(ToolCallParamsChunk {
                    index: last_idx,
                    chunk: args.clone(),
                });
            }
        }

        ToolCallDeltaOutcome {
            finalized_previous,
            params_partial,
        }
    }

    pub fn finish(self) -> Result<Vec<ToolCall>, ToolCallAccumulateError> {
        let mut result = Vec::with_capacity(self.pending.len());
        for partial in self.pending {
            let _validated: serde_json::Value = serde_json::from_str(&partial.arguments_buf)
                .map_err(|e| {
                    ToolCallAccumulateError::InvalidJson(format!(
                        "tool call '{}' arguments: {}",
                        partial.name, e
                    ))
                })?;
            result.push(ToolCall {
                id: partial.id,
                name: partial.name,
                arguments: partial.arguments_buf,
            });
        }
        Ok(result)
    }

    pub fn finish_with_truncation(
        self,
        was_max_tokens: bool,
    ) -> Result<Vec<ToolCall>, ToolCallAccumulateError> {
        if !was_max_tokens {
            return self.finish();
        }

        let total = self.pending.len();
        let mut result = Vec::new();
        for partial in self.pending {
            if serde_json::from_str::<serde_json::Value>(&partial.arguments_buf).is_ok() {
                result.push(ToolCall {
                    id: partial.id,
                    name: partial.name,
                    arguments: partial.arguments_buf,
                });
            }
        }
        if result.len() < total {
            return Err(ToolCallAccumulateError::Truncated);
        }
        Ok(result)
    }
}

fn self_mark_effective(
    params_partial: &mut Option<ToolCallParamsChunk>,
    index: usize,
    chunk: &str,
) {
    if params_partial.is_none() && !chunk.is_empty() {
        *params_partial = Some(ToolCallParamsChunk {
            index,
            chunk: chunk.to_string(),
        });
    }
}

pub async fn consume_stream<S, F>(
    stream: S,
    cancel_token: CancellationToken,
    on_delta: F,
    timeout: Option<Duration>,
) -> Result<StreamOutcome, StreamError>
where
    S: Stream<Item = Result<ChatDelta, ProviderError>> + Unpin,
    F: Fn(u32, &ChatDelta),
{
    let mut stream = stream;
    let mut accum = StreamAccumulator::new();
    let mut seq: u32 = 0;

    loop {
        let item = next_item(&mut stream, timeout).await;

        if cancel_token.is_cancelled() {
            return Err(StreamError::Cancelled(accum.into_partial()));
        }

        match item {
            TimedStreamItem::Item(Ok(delta)) => {
                let _outcome = accum.apply_delta(&delta);
                seq += 1;
                on_delta(seq, &delta);
                if delta.finish_reason.is_some() {
                    return Ok(accum.into_outcome());
                }
            }
            TimedStreamItem::Item(Err(e)) => {
                return Err(StreamError::Provider(e, accum.into_partial()));
            }
            TimedStreamItem::End => {
                return Ok(accum.into_outcome());
            }
            TimedStreamItem::TimedOut => {
                return Err(StreamError::Timeout(accum.into_partial()));
            }
        }
    }
}

async fn next_item<S>(stream: &mut S, timeout: Option<Duration>) -> TimedStreamItem<S::Item>
where
    S: Stream + Unpin,
{
    use futures::StreamExt;
    match timeout {
        Some(d) => match tokio::time::timeout(d, stream.next()).await {
            Ok(Some(item)) => TimedStreamItem::Item(item),
            Ok(None) => TimedStreamItem::End,
            Err(_) => TimedStreamItem::TimedOut,
        },
        None => match stream.next().await {
            Some(item) => TimedStreamItem::Item(item),
            None => TimedStreamItem::End,
        },
    }
}
