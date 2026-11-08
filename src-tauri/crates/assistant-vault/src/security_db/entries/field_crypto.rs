use assistant_crypto::vault_crypto::types::{CipherAlgorithm, VaultPlaintext};
use assistant_crypto::vault_crypto::VaultCrypto;

use crate::types::{CustomFieldInput, VaultError};

pub(super) fn encrypt_opt(
    value: Option<&str>,
    entry_key: &[u8; 32],
    algo: CipherAlgorithm,
) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            VaultCrypto::encrypt_field(&VaultPlaintext::new_str(v), entry_key, algo)
                .map_err(VaultError::Crypto)
        })
        .transpose()
}

pub(super) fn decrypt_opt(
    value: Option<&str>,
    entry_key: &[u8; 32],
) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            let plaintext = VaultCrypto::decrypt_field(v, entry_key).map_err(VaultError::Crypto)?;
            Ok(plaintext.to_str().unwrap_or_default().to_string())
        })
        .transpose()
}

pub(super) fn encrypt_custom_fields(
    fields: &[CustomFieldInput],
    entry_key: &[u8; 32],
    algo: CipherAlgorithm,
) -> Result<String, VaultError> {
    let values: Vec<serde_json::Value> = fields
        .iter()
        .map(|cf| -> Result<serde_json::Value, VaultError> {
            if cf.is_protected {
                let encrypted = VaultCrypto::encrypt_field(
                    &VaultPlaintext::new_str(&cf.value),
                    entry_key,
                    algo,
                )
                .map_err(VaultError::Crypto)?;
                Ok(serde_json::json!({
                    "name": cf.name,
                    "valueEncrypted": encrypted,
                    "isProtected": true
                }))
            } else {
                Ok(serde_json::json!({
                    "name": cf.name,
                    "value": cf.value,
                    "isProtected": false
                }))
            }
        })
        .collect::<Result<Vec<_>, VaultError>>()?;

    serde_json::to_string(&values).map_err(|e| VaultError::Validation(e.to_string()))
}

pub(super) fn decrypt_custom_fields(
    json: Option<&str>,
    entry_key: &[u8; 32],
) -> Result<Vec<CustomFieldInput>, VaultError> {
    let Some(json) = json else {
        return Ok(Vec::new());
    };

    let values: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| VaultError::Database(e.to_string()))?;

    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let is_protected = value
            .get("isProtected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let plain = if is_protected {
            let encrypted = value
                .get("valueEncrypted")
                .and_then(|v| v.as_str())
                .ok_or_else(|| VaultError::Database("missing valueEncrypted".to_string()))?;
            let plaintext =
                VaultCrypto::decrypt_field(encrypted, entry_key).map_err(VaultError::Crypto)?;
            plaintext.to_str().unwrap_or_default().to_string()
        } else {
            value
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        fields.push(CustomFieldInput {
            name,
            value: plain,
            is_protected,
        });
    }

    Ok(fields)
}
