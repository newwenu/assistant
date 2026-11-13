use super::types::ModelItem;
use super::types::ModelsResponse;
use super::OpenAIAdapter;
use crate::base::{resolve_context_window, ModelConfig, ProviderError};
use crate::capability::{catalog_fact, CapabilityFeature};
use crate::client::error::classify_http_error;
use crate::vision::{input_modalities_for, resolve_support_vision};

fn extract_model_configs(body: &ModelsResponse) -> Vec<ModelConfig> {
    let mut configs = Vec::new();

    if let Some(data) = &body.data {
        for m in data {
            let id = match m.resolved_id() {
                Some(id) => id,
                None => continue,
            };
            let ctx = resolve_context_window(&id, None, m.resolved_context_window());
            let name = {
                let n = m.resolved_name();
                if n.is_empty() {
                    id.clone()
                } else {
                    n
                }
            };
            let api_hint = m.resolved_support_vision();
            let vision = resolve_support_vision(&id, api_hint);
            let input_modalities = input_modalities_for(&id).unwrap_or_default();
            configs.push(ModelConfig {
                id: id.clone(),
                name,
                owned_by: m.owned_by.clone().unwrap_or_default(),
                context_window: ctx,
                support_vision: vision.is_supported(),
                input_modalities,
                vision: Some(vision),
                pdf: Some(catalog_fact(&id, CapabilityFeature::Pdf)),
                attachment: Some(catalog_fact(&id, CapabilityFeature::Attachment)),
            });
        }
    }

    if configs.is_empty() {
        if let Some(models_map) = &body.models {
            for (key, value) in models_map {
                let m: ModelItem = match serde_json::from_value(value.clone()) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let id = m.resolved_id().unwrap_or_else(|| key.clone());
                let ctx = resolve_context_window(&id, None, m.resolved_context_window());
                let name = {
                    let n = m.resolved_name();
                    if n.is_empty() {
                        id.clone()
                    } else {
                        n
                    }
                };
                let api_hint = m.resolved_support_vision();
                let vision = resolve_support_vision(&id, api_hint);
                let input_modalities = input_modalities_for(&id).unwrap_or_default();
                configs.push(ModelConfig {
                    id: id.clone(),
                    name,
                    owned_by: m.owned_by.clone().unwrap_or_default(),
                    context_window: ctx,
                    support_vision: vision.is_supported(),
                    input_modalities,
                    vision: Some(vision),
                    pdf: Some(catalog_fact(&id, CapabilityFeature::Pdf)),
                    attachment: Some(catalog_fact(&id, CapabilityFeature::Attachment)),
                });
            }
        }
    }

    configs
}

pub(crate) fn list_models(
    adapter: &OpenAIAdapter,
    api_key: &str,
) -> Result<Vec<ModelConfig>, ProviderError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ProviderError::RequestFailed(format!("Create runtime: {}", e)))?;
    rt.block_on(async {
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
            return Err(classify_http_error(status, &text).error);
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
        Ok(extract_model_configs(&body))
    })
}
