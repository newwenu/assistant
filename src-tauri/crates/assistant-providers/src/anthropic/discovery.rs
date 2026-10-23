use crate::base::{resolve_context_window, ModelConfig, ProviderError};
use crate::client::error::classify_http_error;
use crate::client::quirks::wants_bearer_auth;

use super::types::AnthropicModelsResponse;
use super::AnthropicAdapter;

pub(crate) fn list_models(adapter: &AnthropicAdapter) -> Result<Vec<ModelConfig>, ProviderError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ProviderError::RequestFailed(format!("Create runtime: {}", e)))?;
    rt.block_on(async {
        let api_key = adapter.decrypt_api_key()?;
        let url = resolve_models_url(adapter.base_url());

        let mut req_builder = adapter.client.get(&url);

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

        let resp = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(e.to_string())
            } else {
                ProviderError::NetworkError(e.to_string())
            }
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http_error(status, &text).error);
        }

        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::ParseError(format!("Read response body: {}", e)))?;

        let body: AnthropicModelsResponse = serde_json::from_str(&text).map_err(|e| {
            ProviderError::ParseError(format!(
                "Parse models response: {} (body: {})",
                e,
                &text[..text.len().min(200)]
            ))
        })?;

        let models = body
            .data
            .into_iter()
            .map(|m| {
                let ctx = resolve_context_window(&m.id, None, m.resolved_context_window());
                ModelConfig {
                    id: m.id.clone(),
                    name: m.display_name.unwrap_or(m.id),
                    owned_by: "anthropic".to_string(),
                    context_window: ctx,
                    support_vision: true,
                }
            })
            .collect();
        Ok(models)
    })
}

fn resolve_models_url(base_url: &str) -> String {
    let mut base = base_url.to_string();

    if base.ends_with("/v1/messages") {
        base.truncate(base.len() - "/v1/messages".len());
        return format!("{}/v1/models", base);
    }

    if base.ends_with("/v1/models") {
        return base;
    }

    if base.ends_with("/v1") {
        return format!("{}/models", base);
    }

    if base.is_empty() {
        return "v1/models".to_string();
    }

    format!("{}/v1/models", base)
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn resolve_models_url_cases() {
        let expected = "https://api.anthropic.com/v1/models";
        assert_eq!(resolve_models_url("https://api.anthropic.com/v1"), expected);
        assert_eq!(
            resolve_models_url("https://api.anthropic.com/v1/messages"),
            expected
        );
        assert_eq!(
            resolve_models_url("https://api.anthropic.com/v1/models"),
            expected
        );
    }
}
