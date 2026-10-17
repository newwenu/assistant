use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;
use futures::Stream;
use tokio_util::sync::CancellationToken;

pub use crate::types::chat_delta::{ChatDelta, TokenUsage};
pub use crate::types::config::{
    get_context_window_tokens, resolve_context_window, ChatOptions, RetryConfig, SseLogConfig,
    StreamTimeoutConfig,
};
pub use crate::types::message::{message_role_from_str, ChatMessage, MessageRole};
pub use crate::types::tool::ToolCall;

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
    pub http_status: Option<u16>,
    pub provider: Option<String>,
    pub provider_code: Option<String>,
}

impl ProviderErrorPayload {
    pub fn from_error(
        e: &ProviderError,
        http_status: Option<u16>,
        provider: Option<&str>,
        provider_code: Option<&str>,
    ) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
            retry_after_ms: e.retry_after_ms(),
            retryable: e.is_retryable(),
            http_status,
            provider: provider.map(String::from),
            provider_code: provider_code.map(String::from),
        }
    }
}

impl From<&ProviderError> for ProviderErrorPayload {
    fn from(e: &ProviderError) -> Self {
        Self::from_error(e, None, None, None)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProviderKind {
    OpenAI,
    OpenAIResponses,
    Anthropic,
    Gemini,
    GeminiCodeAssist,
    Custom { base_url: String },
}

impl ProviderKind {
    pub fn use_native_api(&self) -> bool {
        matches!(
            self,
            ProviderKind::Anthropic
                | ProviderKind::Gemini
                | ProviderKind::OpenAIResponses
                | ProviderKind::GeminiCodeAssist
        )
    }
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
}

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatDelta, ProviderError>> + Send>>;

pub trait BaseProviderAdapter: Send + Sync {
    fn kind(&self) -> &ProviderKind;
    fn config(&self) -> &ProviderConfig;
    fn chat_completion_stream(
        &self,
        messages: &[ChatMessage],
        model: &str,
        options: &ChatOptions,
        cancel_token: CancellationToken,
    ) -> Result<ChatStream, ProviderError>;
    fn list_models(&self) -> Result<Vec<ModelConfig>, ProviderError>;
}

pub fn provider_kind_from_str(type_str: &str, base_url: String) -> ProviderKind {
    let lower = type_str.to_ascii_lowercase();
    match lower.as_str() {
        "openai" => ProviderKind::OpenAI,
        "openairesponses" => ProviderKind::OpenAIResponses,
        "anthropic" => ProviderKind::Anthropic,
        "gemini" => ProviderKind::Gemini,
        "geminicodeassist" => ProviderKind::GeminiCodeAssist,
        _ => ProviderKind::Custom { base_url },
    }
}

pub fn create_provider_adapter(config: ProviderConfig) -> Box<dyn BaseProviderAdapter> {
    match config.kind {
        ProviderKind::Anthropic => {
            use crate::anthropic::AnthropicAdapter;
            Box::new(AnthropicAdapter::new(config))
        }
        ProviderKind::Gemini | ProviderKind::GeminiCodeAssist => {
            use crate::gemini::GeminiAdapter;
            Box::new(GeminiAdapter::new(config))
        }
        ProviderKind::OpenAIResponses => {
            use crate::openai::OpenAIAdapter;
            Box::new(OpenAIAdapter::new(config))
        }
        _ => {
            use crate::openai::OpenAIAdapter;
            Box::new(OpenAIAdapter::new(config))
        }
    }
}