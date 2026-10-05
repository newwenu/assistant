use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::StreamExt;
use log::warn;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

use super::base::{
    BaseProviderAdapter, ChatDelta, ChatMessage, ChatOptions, MessageRole, ModelConfig,
    ProviderConfig, ProviderError, ProviderKind, RetryConfig, SseLogConfig, StreamTimeoutConfig,
    TokenUsage,
};
use super::crypto;
use super::sse_log::SseLogCollector;

pub struct OpenAIAdapter {
    config: ProviderConfig,
    client: Client,
    retry_config: RetryConfig,
    timeout_config: StreamTimeoutConfig,
    sse_log_config: SseLogConfig,
}

impl OpenAIAdapter {
    pub fn new(config: ProviderConfig) -> Self {
        let timeout_config = StreamTimeoutConfig::default();
        let client = Client::builder()
            .timeout(timeout_config.total_timeout)
            .build()
            .unwrap_or_default();
        Self {
            config,
            client,
            retry_config: RetryConfig::default(),
            timeout_config,
            sse_log_config: SseLogConfig::default(),
        }
    }

    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry_config = retry;
        self
    }

    pub fn with_timeout(mut self, timeout: StreamTimeoutConfig) -> Self {
        self.client = Client::builder()
            .timeout(timeout.total_timeout)
            .build()
            .unwrap_or_default();
        self.timeout_config = timeout;
        self
    }

    pub fn with_sse_log(mut self, config: SseLogConfig) -> Self {
        self.sse_log_config = config;
        self
    }

    pub fn default_base_url() -> String {
        "https://api.openai.com/v1".to_string()
    }

    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }

    fn decrypt_api_key(&self) -> Result<String, ProviderError> {
        let machine_id =
            crypto::get_machine_id().map_err(|e| ProviderError::ApiKeyMissing(e))?;
        crypto::decrypt_api_key_b64(&self.config.api_key_encrypted, &machine_id)
            .map_err(|e| ProviderError::ApiKeyMissing(e))
    }

    fn role_str(role: &MessageRole) -> &str {
        match role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatRequestMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct ChatRequestMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponseChunk {
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize, Debug)]
struct ChatChoice {
    delta: Option<ChatDeltaContent>,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChatDeltaContent {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Deserialize, Debug)]
struct ChatToolCallDelta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChatToolCallFunctionDelta>,
}

#[derive(Deserialize, Debug)]
struct ChatToolCallFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ChatUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
    #[serde(default)]
    prompt_tokens_details: Option<ChatPromptTokensDetails>,
}

#[derive(Deserialize, Debug)]
struct ChatPromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct ModelsResponse {
    data: Option<Vec<ModelItem>>,
}

#[derive(Deserialize, Debug)]
struct ModelItem {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ProviderErrorResponse {
    error: Option<ProviderErrorBody>,
}

#[derive(Deserialize, Debug)]
struct ProviderErrorBody {
    #[serde(rename = "type", default)]
    error_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

fn parse_retry_after_secs(text: &str) -> Option<u64> {
    let lower = text.to_lowercase();
    for prefix in &["retry after ", "retry_after:", "retry-after:"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim();
            if let Ok(secs) = rest.parse::<u64>() {
                return Some(secs);
            }
        }
    }
    if let Some(pos) = lower.find("retry after ") {
        let rest = lower[pos + 12..].trim();
        if let Ok(secs) = rest.parse::<u64>() {
            return Some(secs);
        }
    }
    None
}

fn classify_http_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    let retry_after_ms = parse_retry_after_secs(body).map(|s| s * 1000);
    let error_detail = serde_json::from_str::<ProviderErrorResponse>(body)
        .ok()
        .and_then(|r| r.error)
        .map(|e| {
            if e.message.is_empty() {
                format!("{} {}", e.error_type, e.code).trim().to_string()
            } else {
                e.message
            }
        })
        .unwrap_or_else(|| body.chars().take(300).collect());
    let status_code = status.as_u16();
    match status_code {
        401 | 403 => ProviderError::AuthFailed(error_detail),
        429 => ProviderError::RateLimited { retry_after_ms },
        404 => ProviderError::ModelNotFound(error_detail),
        400 => {
            let lower = error_detail.to_lowercase();
            if lower.contains("context") || lower.contains("token") && lower.contains("max") {
                ProviderError::ContextOverflow(error_detail)
            } else {
                ProviderError::RequestRejected(error_detail)
            }
        }
        408 | 504 => ProviderError::Timeout(error_detail),
        _ if status.is_server_error() => ProviderError::ServerError(error_detail),
        _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status_code, error_detail)),
    }
}

impl BaseProviderAdapter for OpenAIAdapter {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        options: &ChatOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatDelta, ProviderError>> + Send>>, ProviderError> {
        let api_key = self.decrypt_api_key()?;
        let base_url = self.base_url().to_string();
        let req_messages: Vec<ChatRequestMessage> = messages
            .iter()
            .map(|m| ChatRequestMessage {
                role: Self::role_str(&m.role).to_string(),
                content: m.content.clone(),
            })
            .collect();
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
        };

        let client = self.client.clone();
        let retry_config = self.retry_config.clone();
        let timeout_config = self.timeout_config.clone();
        let sse_log_config = self.sse_log_config.clone();
        let stream = async_stream::stream! {
            let mut attempt: u32 = 0;
            'retry: loop {
                let url = format!("{}/chat/completions", base_url);
                let body = match serde_json::to_string(&req_body) {
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
                        let err = if e.is_timeout() {
                            ProviderError::Timeout(format!("HTTP request: {}", e))
                        } else if e.is_connect() {
                            ProviderError::NetworkError(format!("Connection: {}", e))
                        } else {
                            ProviderError::NetworkError(format!("HTTP request: {}", e))
                        };
                        if err.is_retryable() && attempt < retry_config.max_attempts {
                            let delay = retry_config.delay_ms(attempt, &err);
                            warn!("Attempt {} failed, retrying in {}ms: {}", attempt + 1, delay, err);
                            yield Ok(ChatDelta {
                                delta_text: Some(format!("[retry {}] {}\n", attempt + 1, err)),
                                delta_reasoning: None,
                                delta_tool_call_id: None,
                                delta_tool_call_name: None,
                                delta_tool_call_arguments: None,
                                finish_reason: None,
                                usage: None,
                            });
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
                        yield Ok(ChatDelta {
                            delta_text: Some(format!("[retry {}] HTTP {}\n", attempt + 1, status)),
                            delta_reasoning: None,
                            delta_tool_call_id: None,
                            delta_tool_call_name: None,
                            delta_tool_call_arguments: None,
                            finish_reason: None,
                            usage: None,
                        });
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt += 1;
                        continue 'retry;
                    }
                    yield Err(err);
                    return;
                }

                let mut stream = resp.bytes_stream();
                let mut buffer = String::new();
                const MAX_BUFFER_SIZE: usize = 10 * 1024 * 1024;
                let mut sse_error_count: u32 = 0;
                let mut sse_log = SseLogCollector::new(sse_log_config.clone());
                let mut first_token_received = false;
                let stream_start = Instant::now();
                let mut last_chunk_time = Instant::now();

                while let Some(chunk_result) = stream.next().await {
                    let now = Instant::now();

                    if !first_token_received {
                        if now.duration_since(stream_start) > timeout_config.first_token_timeout {
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
                    } else {
                        if now.duration_since(last_chunk_time) > timeout_config.idle_timeout {
                            let err = ProviderError::Timeout(format!(
                                "No chunk received for {}s (stream idle)",
                                timeout_config.idle_timeout.as_secs()
                            ));
                            sse_log.flush_on_error("stream idle timeout");
                            if attempt < retry_config.max_attempts {
                                let delay = retry_config.delay_ms(attempt, &err);
                                warn!("Stream idle timeout, retrying in {}ms", delay);
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                                attempt += 1;
                                continue 'retry;
                            }
                            yield Err(err);
                            return;
                        }
                    }

                    let chunk = match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            let err = if e.is_timeout() {
                                ProviderError::Timeout(format!("Stream read: {}", e))
                            } else {
                                ProviderError::StreamError(format!("Stream read: {}", e))
                            };
                            sse_log.flush_on_error(&err.to_string());
                            if err.is_retryable() && attempt < retry_config.max_attempts {
                                let delay = retry_config.delay_ms(attempt, &err);
                                warn!("Stream read error, retrying in {}ms: {}", delay, err);
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                                attempt += 1;
                                continue 'retry;
                            }
                            yield Err(err);
                            return;
                        }
                    };

                    last_chunk_time = Instant::now();
                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                    if buffer.len() > MAX_BUFFER_SIZE {
                        let err = ProviderError::StreamError(format!(
                            "SSE buffer exceeded {}MB, possible malformed stream",
                            MAX_BUFFER_SIZE / (1024 * 1024)
                        ));
                        sse_log.flush_on_error(&err.to_string());
                        yield Err(err);
                        return;
                    }

                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        if line.is_empty() {
                            continue;
                        }
                        if line == "data: [DONE]" {
                            yield Ok(ChatDelta {
                                delta_text: None,
                                delta_reasoning: None,
                                delta_tool_call_id: None,
                                delta_tool_call_name: None,
                                delta_tool_call_arguments: None,
                                finish_reason: Some("stop".to_string()),
                                usage: None,
                            });
                            return;
                        }
                        let data = line.strip_prefix("data: ").unwrap_or(&line);
                        sse_log.push(data.to_string());
                        let chunk: ChatResponseChunk = match serde_json::from_str(data) {
                            Ok(c) => c,
                            Err(e) => {
                                sse_error_count += 1;
                                warn!("SSE parse error #{}: {} for line: {}", sse_error_count, e, data);
                                if sse_error_count <= 3 {
                                    continue;
                                }
                                let err = ProviderError::StreamError(format!(
                                    "SSE parse failed {} times, last: {}", sse_error_count, e
                                ));
                                sse_log.flush_on_error(&err.to_string());
                                yield Err(err);
                                return;
                            }
                        };
                        if let Some(usage) = chunk.usage {
                            let cache_read = usage.prompt_tokens_details.as_ref().and_then(|d| d.cached_tokens).unwrap_or(0);
                            yield Ok(ChatDelta {
                                delta_text: None,
                                delta_reasoning: None,
                                delta_tool_call_id: None,
                                delta_tool_call_name: None,
                                delta_tool_call_arguments: None,
                                finish_reason: None,
                                usage: Some(TokenUsage {
                                    prompt_tokens: usage.prompt_tokens.unwrap_or(0),
                                    completion_tokens: usage.completion_tokens.unwrap_or(0),
                                    total_tokens: usage.total_tokens.unwrap_or(0),
                                    cache_read_tokens: cache_read,
                                    cache_write_tokens: 0,
                                }),
                            });
                        }
                        if let Some(choices) = chunk.choices {
                            for choice in choices {
                                let delta_text = choice.delta.as_ref().and_then(|d| d.content.clone());
                                let delta_reasoning = choice.delta.as_ref().and_then(|d| d.reasoning_content.clone());
                                let (tc_id, tc_name, tc_args) = if let Some(ref delta) = choice.delta {
                                    if let Some(ref tool_calls) = delta.tool_calls {
                                        if let Some(tc) = tool_calls.first() {
                                            let id = tc.id.clone();
                                            let name = tc.function.as_ref().and_then(|f| f.name.clone());
                                            let args = tc.function.as_ref().and_then(|f| f.arguments.clone());
                                            (id, name, args)
                                        } else {
                                            (None, None, None)
                                        }
                                    } else {
                                        (None, None, None)
                                    }
                                } else {
                                    (None, None, None)
                                };
                                let finish = choice.finish_reason;
                                let has_content = delta_text.is_some()
                                    || delta_reasoning.is_some()
                                    || tc_id.is_some()
                                    || tc_name.is_some()
                                    || tc_args.is_some()
                                    || finish.is_some();
                                if has_content {
                                    if !first_token_received && (delta_text.is_some() || delta_reasoning.is_some()) {
                                        first_token_received = true;
                                    }
                                    yield Ok(ChatDelta {
                                        delta_text,
                                        delta_reasoning,
                                        delta_tool_call_id: tc_id,
                                        delta_tool_call_name: tc_name,
                                        delta_tool_call_arguments: tc_args,
                                        finish_reason: finish,
                                        usage: None,
                                    });
                                }
                            }
                        }
                    }
                }
                return;
            }
        };
        Ok(Box::pin(stream))
    }

    fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            ProviderError::RequestFailed(format!("Create runtime: {}", e))
        })?;
        rt.block_on(async {
            let api_key = self.decrypt_api_key()?;
            let url = format!("{}/models", self.base_url());
            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        ProviderError::Timeout(e.to_string())
                    } else {
                        ProviderError::NetworkError(e.to_string())
                    }
                })?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(classify_http_error(status, &text));
            }
            let text = resp.text().await.map_err(|e| {
                ProviderError::ParseError(format!("Read response body: {}", e))
            })?;
            let body: ModelsResponse = serde_json::from_str(&text).map_err(|e| {
                ProviderError::ParseError(format!(
                    "Parse models response: {} (body: {})",
                    e,
                    &text[..text.len().min(200)]
                ))
            })?;
            let models = body
                .data
                .unwrap_or_default()
                .into_iter()
                .map(|m| ModelConfig {
                    id: m.id.clone(),
                    name: m.id,
                    owned_by: m.owned_by.unwrap_or_default(),
                })
                .collect();
            Ok(models)
        })
    }
}