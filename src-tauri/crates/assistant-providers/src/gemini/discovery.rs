use crate::base::{resolve_context_window, ModelConfig, ProviderError};
use crate::client::error::classify_http_error;

use super::request::gemini_base_url;
use super::types::GeminiModelsResponse;
use super::GeminiAdapter;

pub(crate) fn list_models(adapter: &GeminiAdapter) -> Result<Vec<ModelConfig>, ProviderError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ProviderError::RequestFailed(format!("Create runtime: {}", e)))?;
    rt.block_on(async {
        let api_key = adapter.decrypt_api_key()?;
        let url = resolve_models_url(adapter.base_url());

        let resp = adapter
            .client
            .get(&url)
            .header("x-goog-api-key", &api_key)
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
            return Err(classify_http_error(status, &text).error);
        }

        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::ParseError(format!("Read response body: {}", e)))?;

        let body: GeminiModelsResponse = serde_json::from_str(&text).map_err(|e| {
            ProviderError::ParseError(format!(
                "Parse models response: {} (body: {})",
                e,
                &text[..text.len().min(200)]
            ))
        })?;

        let models = body
            .models
            .into_iter()
            .filter(|model| {
                model.supported_generation_methods.is_empty()
                    || model
                        .supported_generation_methods
                        .iter()
                        .any(|m| m == "generateContent")
            })
            .map(|model| {
                let id = model
                    .name
                    .strip_prefix("models/")
                    .unwrap_or(&model.name)
                    .to_string();
                let ctx = resolve_context_window(&id, model.input_token_limit);
                ModelConfig {
                    id: id.clone(),
                    name: model.display_name.unwrap_or(id),
                    owned_by: "google".to_string(),
                    context_window: ctx,
                }
            })
            .collect();
        Ok(models)
    })
}

fn resolve_models_url(base_url: &str) -> String {
    let base = gemini_base_url(base_url);
    format!("{}/v1beta/models", base)
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn resolve_models_url_cases() {
        let expected = "https://generativelanguage.googleapis.com/v1beta/models";
        assert_eq!(
            resolve_models_url("https://generativelanguage.googleapis.com"),
            expected
        );
        assert_eq!(
            resolve_models_url("https://generativelanguage.googleapis.com/v1beta"),
            expected
        );
    }
}
