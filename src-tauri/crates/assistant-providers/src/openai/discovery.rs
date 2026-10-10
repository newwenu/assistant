use super::types::ModelsResponse;
use super::OpenAIAdapter;
use crate::base::{get_context_window_tokens, ModelConfig, ProviderError};
use crate::client::error::classify_http_error;

pub(crate) fn list_models(adapter: &OpenAIAdapter) -> Result<Vec<ModelConfig>, ProviderError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ProviderError::RequestFailed(format!("Create runtime: {}", e)))?;
    rt.block_on(async {
        let api_key = adapter.decrypt_api_key()?;
        let url = format!("{}/models", adapter.base_url());
        let resp = adapter
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
        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::ParseError(format!("Read response body: {}", e)))?;
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
            .map(|m| {
                let ctx = get_context_window_tokens(&m.id);
                ModelConfig {
                    id: m.id.clone(),
                    name: m.id,
                    owned_by: m.owned_by.unwrap_or_default(),
                    context_window: ctx,
                }
            })
            .collect();
        Ok(models)
    })
}
