use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API key not configured for provider: {0}")]
    ApiKeyMissing(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Response parse error: {0}")]
    ParseError(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Model not found: {0}")]
    ModelNotFound(String),
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

pub trait BaseProviderAdapter: Send + Sync {
    fn kind(&self) -> &ProviderKind;
    fn config(&self) -> &ProviderConfig;
    fn chat_completion_stream(&self, messages: &[ChatMessage]) -> Result<(), ProviderError>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}