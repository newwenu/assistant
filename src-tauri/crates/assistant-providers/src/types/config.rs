use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(default)]
    pub context_window_tokens: u32,
    #[serde(default)]
    pub reasoning_enabled: Option<bool>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
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
    pub fn delay_ms(&self, attempt: u32, error: &crate::base::ProviderError) -> u64 {
        let shift = attempt.min(6);
        let is_rate_limit = matches!(error, crate::base::ProviderError::RateLimited { .. });
        let fallback = if is_rate_limit {
            (self.rate_limit_base_ms << shift).min(self.rate_limit_max_ms)
        } else {
            (self.base_delay_ms << shift).min(self.max_delay_ms)
        };
        match error.retry_after_ms() {
            Some(hint) if hint > 0 && is_rate_limit => {
                hint.max(fallback).min(self.rate_limit_max_ms)
            }
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

pub fn resolve_context_window(model_id: &str, api_context: Option<u32>) -> u32 {
    if let Some(ctx) = api_context {
        if ctx > 0 {
            return ctx;
        }
    }
    get_context_window_tokens(model_id)
}

pub fn get_context_window_tokens(model_id: &str) -> u32 {
    let id = model_id.to_lowercase();
    if id.contains("gpt-4o") || id.contains("gpt-4-turbo") {
        128_000
    } else if id.contains("gpt-4-32k") {
        32_768
    } else if id.contains("gpt-4") && !id.contains("turbo") {
        8_192
    } else if id.contains("gpt-3.5-turbo-16k") {
        16_384
    } else if id.contains("gpt-3.5") {
        4_096
    } else if id.contains("claude-3.5-sonnet")
        || id.contains("claude-3-5-sonnet")
        || id.contains("claude-3-opus")
        || id.contains("claude-3-haiku")
        || id.contains("claude-3-5-haiku")
    {
        200_000
    } else if id.contains("deepseek-reasoner")
        || id.contains("deepseek-r1")
        || id.contains("deepseek")
    {
        64_000
    } else if id.contains("gemini-1.5-pro")
        || id.contains("gemini-2.0")
        || id.contains("gemini-2.5")
        || id.contains("gemini-1.5-flash")
        || id.contains("gemini-2.0-flash")
    {
        1_000_000
    } else if id.contains("gemini-pro") {
        32_000
    } else if id.contains("qwen-max") || id.contains("qwen-long") || id.contains("qwen-turbo") {
        1_000_000
    } else if id.contains("qwen-plus") {
        131_072
    } else if id.contains("glm-4")
        || id.contains("llama-3.1-405b")
        || id.contains("llama-3.1-70b")
        || id.contains("llama-3.1-8b")
    {
        128_000
    } else if id.contains("llama-3-70b") || id.contains("llama-3-8b") {
        8_192
    } else if id.contains("mistral-large")
        || id.contains("mistral-medium")
        || id.contains("mistral-7b")
        || id.contains("mixtral")
    {
        32_000
    } else {
        128_000
    }
}
