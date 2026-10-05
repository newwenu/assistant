use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;
use tokio_stream::Stream;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorCode {
    ApiKeyMissing,
    AuthFailed,
    RateLimited,
    ModelNotFound,
    ContextOverflow,
    NetworkError,
    Timeout,
    ServerError,
    RequestRejected,
    ParseError,
    Cancelled,
    StreamError,
    InternalError,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API key not configured for provider: {0}")]
    ApiKeyMissing(String),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Response parse error: {0}")]
    ParseError(String),
    #[error("Rate limited")]
    RateLimited { retry_after_ms: Option<u64> },
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Context length exceeded: {0}")]
    ContextOverflow(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Request timeout: {0}")]
    Timeout(String),
    #[error("Server error: {0}")]
    ServerError(String),
    #[error("Request rejected: {0}")]
    RequestRejected(String),
    #[error("Stream error: {0}")]
    StreamError(String),
    #[error("Cancelled")]
    Cancelled,
}

impl ProviderError {
    pub fn code(&self) -> ProviderErrorCode {
        match self {
            ProviderError::ApiKeyMissing(_) => ProviderErrorCode::ApiKeyMissing,
            ProviderError::AuthFailed(_) => ProviderErrorCode::AuthFailed,
            ProviderError::RequestFailed(_) => ProviderErrorCode::InternalError,
            ProviderError::ParseError(_) => ProviderErrorCode::ParseError,
            ProviderError::RateLimited { .. } => ProviderErrorCode::RateLimited,
            ProviderError::ModelNotFound(_) => ProviderErrorCode::ModelNotFound,
            ProviderError::ContextOverflow(_) => ProviderErrorCode::ContextOverflow,
            ProviderError::NetworkError(_) => ProviderErrorCode::NetworkError,
            ProviderError::Timeout(_) => ProviderErrorCode::Timeout,
            ProviderError::ServerError(_) => ProviderErrorCode::ServerError,
            ProviderError::RequestRejected(_) => ProviderErrorCode::RequestRejected,
            ProviderError::StreamError(_) => ProviderErrorCode::StreamError,
            ProviderError::Cancelled => ProviderErrorCode::Cancelled,
        }
    }

    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimited { retry_after_ms } => *retry_after_ms,
            _ => None,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::RateLimited { .. }
                | ProviderError::NetworkError(_)
                | ProviderError::Timeout(_)
                | ProviderError::ServerError(_)
                | ProviderError::StreamError(_)
        )
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ProviderErrorPayload {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,
}

impl From<&ProviderError> for ProviderErrorPayload {
    fn from(e: &ProviderError) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
            retry_after_ms: e.retry_after_ms(),
            retryable: e.is_retryable(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Gemini,
    Custom { base_url: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub api_key_encrypted: String,
    pub base_url: Option<String>,
    pub default_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatDelta {
    pub delta_text: Option<String>,
    pub delta_reasoning: Option<String>,
    pub delta_tool_call_id: Option<String>,
    pub delta_tool_call_name: Option<String>,
    pub delta_tool_call_arguments: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub rate_limit_base_ms: u64,
    pub rate_limit_max_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 30_000,
            rate_limit_base_ms: 2_000,
            rate_limit_max_ms: 60_000,
        }
    }
}

impl RetryConfig {
    pub fn delay_ms(&self, attempt: u32, error: &ProviderError) -> u64 {
        let shift = attempt.min(6) as u32;
        let is_rate_limit = matches!(error, ProviderError::RateLimited { .. });
        let fallback = if is_rate_limit {
            (self.rate_limit_base_ms << shift).min(self.rate_limit_max_ms)
        } else {
            (self.base_delay_ms << shift).min(self.max_delay_ms)
        };
        match error.retry_after_ms() {
            Some(hint) if hint > 0 && is_rate_limit => hint.max(fallback).min(self.rate_limit_max_ms),
            Some(hint) if hint > 0 => hint.min(self.max_delay_ms),
            _ => fallback,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StreamTimeoutConfig {
    pub first_token_timeout: Duration,
    pub idle_timeout: Duration,
    pub total_timeout: Duration,
}

impl Default for StreamTimeoutConfig {
    fn default() -> Self {
        Self {
            first_token_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SseLogConfig {
    pub max_events: usize,
    pub max_retained_bytes: usize,
}

impl Default for SseLogConfig {
    fn default() -> Self {
        Self {
            max_events: 256,
            max_retained_bytes: 1_048_576,
        }
    }
}

pub trait BaseProviderAdapter: Send + Sync {
    fn kind(&self) -> &ProviderKind;
    fn config(&self) -> &ProviderConfig;
    fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        options: &ChatOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatDelta, ProviderError>> + Send>>, ProviderError>;
    fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError>;
}

pub fn provider_kind_from_str(type_str: &str, base_url: String) -> ProviderKind {
    match type_str {
        "OpenAI" => ProviderKind::OpenAI,
        "Anthropic" => ProviderKind::Anthropic,
        "Gemini" => ProviderKind::Gemini,
        _ => ProviderKind::Custom { base_url },
    }
}

pub fn message_role_from_str(role: &str) -> MessageRole {
    match role {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::User,
    }
}

pub fn create_provider_adapter(config: ProviderConfig) -> Box<dyn BaseProviderAdapter> {
    use crate::openai::OpenAIAdapter;
    Box::new(OpenAIAdapter::new(config))
}