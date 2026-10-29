pub mod code_assist;
pub mod discovery;
pub mod message_converter;
pub mod request;
pub mod stream;
pub mod types;

use reqwest::Client;

use crate::base::{
    BaseProviderAdapter, ChatStream, ProviderConfig, ProviderError, ProviderKind, RetryConfig,
    SseLogConfig, StreamTimeoutConfig,
};

pub struct GeminiAdapter {
    pub(crate) config: ProviderConfig,
    pub(crate) client: Client,
    pub(crate) retry_config: RetryConfig,
    pub(crate) timeout_config: StreamTimeoutConfig,
    pub(crate) sse_log_config: SseLogConfig,
}

impl GeminiAdapter {
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
        "https://generativelanguage.googleapis.com".to_string()
    }

    pub(crate) fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or("https://generativelanguage.googleapis.com")
    }
}

impl BaseProviderAdapter for GeminiAdapter {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn chat_completion_stream(
        &self,
        api_key: &str,
        messages: &[crate::base::ChatMessage],
        model: &str,
        options: &crate::base::ChatOptions,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Result<ChatStream, ProviderError> {
        match self.config.kind {
            ProviderKind::GeminiCodeAssist => code_assist::code_assist_stream(
                self,
                api_key,
                messages,
                model,
                options,
                cancel_token,
            ),
            _ => request::chat_completion_stream(
                self,
                api_key,
                messages,
                model,
                options,
                cancel_token,
            ),
        }
    }

    fn list_models(&self, api_key: &str) -> Result<Vec<crate::base::ModelConfig>, ProviderError> {
        match self.config.kind {
            ProviderKind::GeminiCodeAssist => Ok(code_assist::list_code_assist_models()),
            _ => discovery::list_models(self, api_key),
        }
    }
}
