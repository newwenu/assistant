use assistant_crypto::vault_crypto::field_aad;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, PerEntryKey, VaultPlaintext};
use assistant_crypto::vault_crypto::{decrypt_field, encrypt_field};

use crate::types::{CustomFieldInput, VaultError};

pub(crate) const FIELD_PASSWORD: &str = "password";
pub(crate) const FIELD_NOTES: &str = "notes";
pub(crate) const FIELD_TOTP: &str = "totp";
pub(crate) const FIELD_SSH_PRIVATE_KEY: &str = "ssh-private-key";

fn custom_field_name(name: &str) -> String {
    format!("custom:{}", name)
}

pub(crate) fn encrypt_opt(
    value: Option<&str>,
    key: &PerEntryKey,
    field: &str,
    entry_id: &str,
    algo: CipherAlgorithm,
) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            let aad = field_aad(entry_id, field);
            encrypt_field(key, &VaultPlaintext::new_str(v), algo, &aad)
                .map_err(|_| VaultError::EntryCorrupted(entry_id.to_string()))
        })
        .transpose()
}

pub(crate) fn decrypt_opt(
    value: Option<&str>,
    key: &PerEntryKey,
    field: &str,
    entry_id: &str,
) -> Result<Option<String>, VaultError> {
    value
        .map(|v| {
            let aad = field_aad(entry_id, field);
            let plaintext = decrypt_field(key, v, &aad)
                .map_err(|_| VaultError::EntryCorrupted(entry_id.to_string()))?;
            plaintext
                .to_str()
                .map(|s| s.to_string())
                .map_err(|_| VaultError::EntryCorrupted(entry_id.to_string()))
        })
        .transpose()
}

pub(crate) fn encrypt_custom_fields(
    fields: &[CustomFieldInput],
    key: &PerEntryKey,
    entry_id: &str,
    algo: CipherAlgorithm,
) -> Result<String, VaultError> {
    let values: Vec<serde_json::Value> = fields
        .iter()
        .map(|cf| -> Result<serde_json::Value, VaultError> {
            if cf.is_protected {
                let aad = field_aad(entry_id, &custom_field_name(&cf.name));
                let encrypted = encrypt_field(key, &VaultPlaintext::new_str(&cf.value), algo, &aad)
                    .map_err(|_| VaultError::EntryCorrupted(entry_id.to_string()))?;
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

pub(crate) fn decrypt_custom_fields(
    json: Option<&str>,
    key: &PerEntryKey,
    entry_id: &str,
) -> Result<Vec<CustomFieldInput>, VaultError> {
    let Some(json) = json else {
        return Ok(Vec::new());
    };

    let corrupted = || VaultError::EntryCorrupted(entry_id.to_string());

    let values: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|_| corrupted())?;

    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(corrupted)?
            .to_string();
        let is_protected = value
            .get("isProtected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let plain = if is_protected {
            let encrypted = value
                .get("valueEncrypted")
                .and_then(|v| v.as_str())
                .ok_or_else(corrupted)?;
            let aad = field_aad(entry_id, &custom_field_name(&name));
            let plaintext = decrypt_field(key, encrypted, &aad).map_err(|_| corrupted())?;
            plaintext.to_str().map_err(|_| corrupted())?.to_string()
        } else {
            value
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(corrupted)?
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

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY_ID: &str = "entry-1";

    fn entry_key() -> PerEntryKey {
        PerEntryKey::new([7u8; 32])
    }

    fn assert_entry_corrupted_for(err: &VaultError, entry_id: &str) {
        match err {
            VaultError::EntryCorrupted(id) => assert_eq!(id, entry_id),
            other => panic!("expected EntryCorrupted, got {:?}", other),
        }
    }

    fn assert_entry_corrupted(err: &VaultError) {
        assert_entry_corrupted_for(err, ENTRY_ID);
    }

    fn encrypt_secret(value: &str, field: &str) -> String {
        let aad = field_aad(ENTRY_ID, field);
        encrypt_field(
            &entry_key(),
            &VaultPlaintext::new_str(value),
            CipherAlgorithm::Aes256Gcm,
            &aad,
        )
        .expect("encrypt")
    }

    #[test]
    fn decrypt_opt_rejects_non_utf8_plaintext() {
        let aad = field_aad(ENTRY_ID, FIELD_PASSWORD);
        let ciphertext = encrypt_field(
            &entry_key(),
            &VaultPlaintext::from(vec![0xff, 0xfe]),
            CipherAlgorithm::Aes256Gcm,
            &aad,
        )
        .expect("encrypt");

        let err =
            decrypt_opt(Some(&ciphertext), &entry_key(), FIELD_PASSWORD, ENTRY_ID).unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_opt_rejects_tampered_ciphertext() {
        let ciphertext = encrypt_secret("secret", FIELD_PASSWORD);

        let err = decrypt_opt(
            Some(&ciphertext[..ciphertext.len() - 2]),
            &entry_key(),
            FIELD_PASSWORD,
            ENTRY_ID,
        )
        .unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_opt_rejects_ciphertext_moved_to_another_field() {
        let ciphertext = encrypt_secret("secret", FIELD_PASSWORD);

        let err = decrypt_opt(Some(&ciphertext), &entry_key(), FIELD_NOTES, ENTRY_ID).unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_opt_rejects_ciphertext_moved_to_another_entry() {
        let ciphertext = encrypt_secret("secret", FIELD_PASSWORD);

        let err =
            decrypt_opt(Some(&ciphertext), &entry_key(), FIELD_PASSWORD, "entry-2").unwrap_err();

        assert_entry_corrupted_for(&err, "entry-2");
    }

    #[test]
    fn decrypt_custom_fields_rejects_malformed_json() {
        let err = decrypt_custom_fields(Some("not-json"), &entry_key(), ENTRY_ID).unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_custom_fields_rejects_protected_field_failure_instead_of_dropping_it() {
        let ciphertext = encrypt_secret("secret", "custom:broken");
        let json = serde_json::json!([
            { "name": "plain", "value": "visible", "isProtected": false },
            { "name": "broken", "valueEncrypted": ciphertext, "isProtected": true },
            { "name": "tampered", "valueEncrypted": "AAAA", "isProtected": true }
        ])
        .to_string();

        let err = decrypt_custom_fields(Some(&json), &entry_key(), ENTRY_ID).unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_custom_fields_rejects_swapped_protected_fields() {
        let ciphertext = encrypt_secret("secret", "custom:a");
        let json = serde_json::json!([
            { "name": "a", "valueEncrypted": ciphertext, "isProtected": true },
            { "name": "b", "valueEncrypted": ciphertext, "isProtected": true }
        ])
        .to_string();

        let err = decrypt_custom_fields(Some(&json), &entry_key(), ENTRY_ID).unwrap_err();

        assert_entry_corrupted(&err);
    }

    #[test]
    fn decrypt_custom_fields_round_trips_plain_and_protected_fields() {
        let input = vec![
            CustomFieldInput {
                name: "plain".to_string(),
                value: "visible".to_string(),
                is_protected: false,
            },
            CustomFieldInput {
                name: "secret".to_string(),
                value: "hidden".to_string(),
                is_protected: true,
            },
        ];

        let json =
            encrypt_custom_fields(&input, &entry_key(), ENTRY_ID, CipherAlgorithm::Aes256Gcm)
                .expect("encrypt");
        let decoded = decrypt_custom_fields(Some(&json), &entry_key(), ENTRY_ID).expect("decrypt");

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].name, "plain");
        assert_eq!(decoded[0].value, "visible");
        assert!(!decoded[0].is_protected);
        assert_eq!(decoded[1].name, "secret");
        assert_eq!(decoded[1].value, "hidden");
        assert!(decoded[1].is_protected);
    }

    #[test]
    fn encrypt_opt_passes_through_none() {
        assert_eq!(
            encrypt_opt(
                None,
                &entry_key(),
                FIELD_NOTES,
                ENTRY_ID,
                CipherAlgorithm::Aes256Gcm
            )
            .expect("encrypt"),
            None
        );
    }
}
