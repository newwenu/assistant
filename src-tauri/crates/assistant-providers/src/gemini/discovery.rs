use crate::base::{resolve_context_window, ModelConfig, ProviderError};
use crate::capability::{catalog_fact, CapabilityFeature};
use crate::client::error::classify_http_error;
use crate::vision::{input_modalities_for, resolve_support_vision};

use super::request::gemini_base_url;
use super::types::GeminiModelsResponse;
use super::GeminiAdapter;

pub(crate) fn list_models(
    adapter: &GeminiAdapter,
    api_key: &str,
) -> Result<Vec<ModelConfig>, ProviderError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ProviderError::RequestFailed(format!("Create runtime: {}", e)))?;
    rt.block_on(async {
        let url = resolve_models_url(adapter.base_url());

        let resp = adapter
            .client
            .get(&url)
            .header("x-goog-api-key", api_key)
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
                let ctx = resolve_context_window(&id, None, model.input_token_limit);
                let vision = resolve_support_vision(&id, None);
                let input_modalities = input_modalities_for(&id).unwrap_or_default();
                ModelConfig {
                    id: id.clone(),
                    name: model.display_name.unwrap_or(id.clone()),
                    owned_by: "google".to_string(),
                    context_window: ctx,
                    support_vision: vision.is_supported(),
                    input_modalities,
                    vision: Some(vision),
                    pdf: Some(catalog_fact(&id, CapabilityFeature::Pdf)),
                    attachment: Some(catalog_fact(&id, CapabilityFeature::Attachment)),
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
