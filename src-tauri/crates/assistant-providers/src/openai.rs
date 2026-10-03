use super::base::{BaseProviderAdapter, ChatMessage, ProviderConfig, ProviderError, ProviderKind};

pub struct OpenAIAdapter {
    config: ProviderConfig,
}

impl OpenAIAdapter {
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
    }

    pub fn default_base_url() -> String {
        "https://api.openai.com/v1".to_string()
    }
}

impl BaseProviderAdapter for OpenAIAdapter {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn chat_completion_stream(&self, _messages: &[ChatMessage]) -> Result<(), ProviderError> {
        Ok(())
    }
}