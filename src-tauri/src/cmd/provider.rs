use chrono::Utc;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::protocol::{
    ModelConfigDto, ProviderInputDto, ProviderListDto, SettingProviderChangedPayloadDto,
};
use crate::protocol::events::{emit_broadcast, event_name};
use assistant_providers::{
    base::{self, ProviderConfig},
    crypto,
};
use assistant_storage::providers::{self as provider_store, ProviderRow};

fn mask_api_key_encrypted(encrypted: &str) -> String {
    if encrypted.is_empty() {
        return String::new();
    }
    let machine_id = match crypto::get_machine_id() {
        Ok(id) => id,
        Err(_) => return "••••".to_string(),
    };
    match crypto::decrypt_api_key_b64(encrypted, &machine_id) {
        Ok(plain) => {
            let chars: Vec<char> = plain.chars().collect();
            if chars.len() <= 7 {
                "••••".to_string()
            } else {
                let head: String = chars.iter().take(3).collect();
                let tail: String = chars.iter().skip(chars.len() - 4).collect();
                format!("{}****{}", head, tail)
            }
        }
        Err(_) => "••••".to_string(),
    }
}

pub(crate) fn build_provider_config(row: &ProviderRow) -> ProviderConfig {
    ProviderConfig {
        kind: base::provider_kind_from_str(&row.type_, row.base_url.clone()),
        api_key_encrypted: row.api_key_encrypted.clone(),
        base_url: Some(row.base_url.clone()),
        default_model: row.default_model.clone(),
        max_tokens: 4096,
        temperature: 0.7,
    }
}

#[command]
pub fn list_providers(app: AppHandle) -> Result<Vec<ProviderListDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let rows = provider_store::get_providers(&conn)?;
    let masked: Vec<ProviderListDto> = rows
        .iter()
        .map(|r| {
            let models: Vec<ModelConfigDto> =
                serde_json::from_str(&r.models_json).unwrap_or_default();
            ProviderListDto {
                id: r.id.clone(),
                name: r.name.clone(),
                type_: r.type_.clone(),
                base_url: r.base_url.clone(),
                api_key_masked: mask_api_key_encrypted(&r.api_key_encrypted),
                models,
                default_model: r.default_model.clone(),
                updated_at: r.updated_at,
            }
        })
        .collect();
    Ok(masked)
}

#[command]
pub fn save_provider(app: AppHandle, provider: ProviderInputDto) -> Result<(), String> {
    let id = provider.id;
    let name = provider.name;
    let type_ = provider.type_;
    let base_url = provider.base_url;
    let api_key_plain = provider.api_key;
    let api_key_changed = provider.api_key_changed;
    let models_json = if provider.models.is_empty() {
        "[]".to_string()
    } else {
        provider.models
    };
    let default_model = provider.default_model;

    let machine_id = crypto::get_machine_id()?;
    let now = Utc::now().timestamp_millis();

    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    if id.is_empty() {
        let new_id = Uuid::new_v4().to_string();
        let api_key_encrypted = if !api_key_plain.is_empty() {
            crypto::encrypt_api_key_b64(&api_key_plain, &machine_id)?
        } else {
            String::new()
        };
        let row = ProviderRow {
            id: new_id,
            name,
            type_,
            base_url,
            api_key_encrypted,
            models_json,
            default_model,
            updated_at: now,
        };
        provider_store::insert_provider(&conn, &row)?;
    } else {
        let existing = provider_store::get_provider_by_id(&conn, &id)?;
        let api_key_encrypted = if !api_key_changed || api_key_plain.is_empty() {
            existing.map(|e| e.api_key_encrypted).unwrap_or_default()
        } else {
            crypto::encrypt_api_key_b64(&api_key_plain, &machine_id)?
        };
        let row = ProviderRow {
            id,
            name,
            type_,
            base_url,
            api_key_encrypted,
            models_json,
            default_model,
            updated_at: now,
        };
        provider_store::update_provider(&conn, &row)?;
    }

    emit_broadcast(
        &app,
        event_name::SETTING_PROVIDER_CHANGED,
        SettingProviderChangedPayloadDto {
            action: "provider-saved".to_string(),
            id: None,
        },
    );
    Ok(())
}

#[command]
pub fn delete_provider(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    provider_store::delete_provider(&conn, &id)?;
    emit_broadcast(
        &app,
        event_name::SETTING_PROVIDER_CHANGED,
        SettingProviderChangedPayloadDto {
            action: "provider-deleted".to_string(),
            id: Some(id),
        },
    );
    Ok(())
}

#[command]
pub fn list_models_cmd(app: AppHandle, provider_id: String) -> Result<Vec<ModelConfigDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let row = provider_store::get_provider_by_id(&conn, &provider_id)?
        .ok_or_else(|| "Provider not found".to_string())?;

    let config = build_provider_config(&row);
    let adapter = base::create_provider_adapter(config);
    let models = adapter.list_models().map_err(|e| e.to_string())?;
    Ok(models
        .into_iter()
        .map(|m| ModelConfigDto {
            id: m.id,
            name: m.name,
            owned_by: m.owned_by,
            context_window: m.context_window,
        })
        .collect())
}

#[command]
pub fn preview_models_cmd(
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<Vec<ModelConfigDto>, String> {
    let api_key_encrypted = crypto::encrypt_api_key_b64(&api_key, &crypto::get_machine_id()?)?;
    let config = ProviderConfig {
        kind: base::provider_kind_from_str(&provider_type, base_url.clone()),
        api_key_encrypted,
        base_url: Some(base_url),
        default_model: String::new(),
        max_tokens: 4096,
        temperature: 0.7,
    };
    let adapter = base::create_provider_adapter(config);
    let models = adapter.list_models().map_err(|e| e.to_string())?;
    Ok(models
        .into_iter()
        .map(|m| ModelConfigDto {
            id: m.id,
            name: m.name,
            owned_by: m.owned_by,
            context_window: m.context_window,
        })
        .collect())
}