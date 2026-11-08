use rusqlite::params;

use crate::types::{VaultCategory, VaultError};
use assistant_crypto::vault_crypto::types::{SecureString, VaultPlaintext};
use assistant_crypto::vault_crypto::VaultCrypto;

use super::SecurityDbState;

impl SecurityDbState {
    pub fn decrypt_password(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_password FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(|s| SecureString::new(s.to_string())).map_err(|_| {
                VaultError::Crypto("Decrypted password is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_notes(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_notes FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(|s| SecureString::new(s.to_string())).map_err(|_| {
                VaultError::Crypto("Decrypted notes is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_totp_secret(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_totp_secret FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)?;
            plaintext.to_str().map(|s| SecureString::new(s.to_string())).map_err(|_| {
                VaultError::Crypto("Decrypted TOTP secret is not valid UTF-8".to_string())
            })
        })
    }

    pub fn decrypt_field(&self, id: &str, field_name: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let custom_fields_json: String = conn
                .query_row(
                    "SELECT encrypted_custom_fields FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let fields: Vec<serde_json::Value> = serde_json::from_str(&custom_fields_json)
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let field = fields
                .iter()
                .find(|f| f.get("name").and_then(|n| n.as_str()) == Some(field_name))
                .ok_or_else(|| VaultError::NotFound(field_name.to_string()))?;

            if field.get("isProtected").and_then(|v| v.as_bool()) == Some(true) {
                let encrypted = field
                    .get("valueEncrypted")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| VaultError::Validation("Protected field has no encrypted value".to_string()))?;
                let entry_key = VaultCrypto::derive_entry_key(mk, id);
                let plaintext = VaultCrypto::decrypt_field(encrypted, entry_key.expose())
                    .map_err(VaultError::Crypto)?;
                plaintext.to_str().map(|s| SecureString::new(s.to_string())).map_err(|_| {
                    VaultError::Crypto("Decrypted field is not valid UTF-8".to_string())
                })
            } else {
                field
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(|s| SecureString::new(s.to_string()))
                    .ok_or_else(|| VaultError::Validation("Unprotected field has no value".to_string()))
            }
        })
    }

    pub fn decrypt_ssh_private_key(
        &self,
        id: &str,
    ) -> Result<assistant_crypto::vault_crypto::types::VaultPlaintext, VaultError> {
        self.with_conn_and_key(|conn, mk| {
            let encrypted: String = conn
                .query_row(
                    "SELECT encrypted_password FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL AND category = 'sshKey'",
                    rusqlite::params![id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
                    other => VaultError::Database(other.to_string()),
                })?;

            let entry_key = VaultCrypto::derive_entry_key(mk, id);
            VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
                .map_err(VaultError::Crypto)
        })
    }

    pub fn decrypt_entry(&self, id: &str) -> Result<crate::types::VaultEntryDecrypted, VaultError> {
        type EntryRow = (
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            i64,
        );

        self.with_conn_and_key(|conn, mk| {
            let result: Result<EntryRow, rusqlite::Error> = conn.query_row(
                "SELECT id, title, username, url, category, encrypted_password, \
                 encrypted_notes, encrypted_totp_secret, encrypted_custom_fields, \
                 tags, is_favorite, revision, created_at, updated_at \
                 FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                    ))
                },
            );

            let (
                eid,
                title,
                username,
                url,
                category_str,
                enc_pw,
                enc_notes,
                enc_totp,
                enc_cf,
                tags_json,
                is_fav,
                revision,
                created,
                updated,
            ) = match result {
                Ok(r) => r,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(VaultError::NotFound(id.to_string()))
                }
                Err(e) => return Err(VaultError::Database(e.to_string())),
            };

            let entry_key = VaultCrypto::derive_entry_key(mk, &eid);

            let password = enc_pw
                .map(|enc| VaultCrypto::decrypt_field(&enc, entry_key.expose()))
                .transpose()
                .map_err(VaultError::Crypto)?;

            let notes = enc_notes
                .map(|enc| VaultCrypto::decrypt_field(&enc, entry_key.expose()))
                .transpose()
                .map_err(VaultError::Crypto)?;

            let totp_secret = enc_totp
                .map(|enc| VaultCrypto::decrypt_field(&enc, entry_key.expose()))
                .transpose()
                .map_err(VaultError::Crypto)?;

            let custom_fields: Vec<crate::types::DecryptedCustomField> = enc_cf
                .and_then(|json| {
                    let fields: Vec<serde_json::Value> = serde_json::from_str(&json).ok()?;
                    Some(
                        fields
                            .iter()
                            .filter_map(|f| {
                                let name = f.get("name")?.as_str()?.to_string();
                                if f.get("isProtected").and_then(|v| v.as_bool()) == Some(true) {
                                    let enc = f.get("valueEncrypted")?.as_str()?;
                                    let pt =
                                        VaultCrypto::decrypt_field(enc, entry_key.expose()).ok()?;
                                    Some(crate::types::DecryptedCustomField {
                                        name,
                                        value: pt,
                                        is_protected: true,
                                    })
                                } else {
                                    let val = f.get("value")?.as_str()?.to_string();
                                    Some(crate::types::DecryptedCustomField {
                                        name,
                                        value: VaultPlaintext::new_str(&val),
                                        is_protected: false,
                                    })
                                }
                            })
                            .collect(),
                    )
                })
                .unwrap_or_default();

            Ok(crate::types::VaultEntryDecrypted {
                id: eid,
                category: VaultCategory::parse(&category_str).unwrap_or_default(),
                title,
                username,
                url,
                password,
                notes,
                totp_secret,
                custom_fields,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                is_favorite: is_fav != 0,
                revision,
                created_at: created,
                updated_at: updated,
            })
        })
    }
}
