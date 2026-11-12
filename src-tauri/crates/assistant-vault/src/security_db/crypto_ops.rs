use rusqlite::params;

use crate::types::{VaultCategory, VaultError};
use assistant_crypto::vault_crypto::field_aad;
use assistant_crypto::vault_crypto::types::{SecureString, VaultPlaintext};
use assistant_crypto::vault_crypto::{decrypt_field, derive_entry_key};

use super::entries::field_crypto::{
    decrypt_custom_fields, decrypt_opt, FIELD_NOTES, FIELD_PASSWORD, FIELD_SSH_PRIVATE_KEY,
    FIELD_TOTP,
};
use super::SecurityDbState;

fn map_row_error(id: &str, e: rusqlite::Error) -> VaultError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => VaultError::NotFound(id.to_string()),
        other => VaultError::Database(other.to_string()),
    }
}

fn read_encrypted_field(
    conn: &rusqlite::Connection,
    id: &str,
    column: &str,
) -> Result<String, VaultError> {
    let sql = format!("SELECT {column} FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL");
    let value: Option<String> = conn
        .query_row(&sql, params![id], |row| row.get(0))
        .map_err(|e| map_row_error(id, e))?;
    value.ok_or_else(|| VaultError::NotFound(id.to_string()))
}

fn secure_string(entry_id: &str, plaintext: VaultPlaintext) -> Result<SecureString, VaultError> {
    plaintext
        .to_str()
        .map(|s| SecureString::new(s.to_string()))
        .map_err(|_| VaultError::EntryCorrupted(entry_id.to_string()))
}

impl SecurityDbState {
    pub fn decrypt_password(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let encrypted = read_encrypted_field(conn, id, "encrypted_password")?;
            let entry_key = derive_entry_key(dek, id);
            let aad = field_aad(id, FIELD_PASSWORD);
            let plaintext = decrypt_field(&entry_key, &encrypted, &aad)
                .map_err(|_| VaultError::EntryCorrupted(id.to_string()))?;
            secure_string(id, plaintext)
        })
    }

    pub fn decrypt_notes(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let encrypted = read_encrypted_field(conn, id, "encrypted_notes")?;
            let entry_key = derive_entry_key(dek, id);
            let aad = field_aad(id, FIELD_NOTES);
            let plaintext = decrypt_field(&entry_key, &encrypted, &aad)
                .map_err(|_| VaultError::EntryCorrupted(id.to_string()))?;
            secure_string(id, plaintext)
        })
    }

    pub fn decrypt_totp_secret(&self, id: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let encrypted = read_encrypted_field(conn, id, "encrypted_totp_secret")?;
            let entry_key = derive_entry_key(dek, id);
            let aad = field_aad(id, FIELD_TOTP);
            let plaintext = decrypt_field(&entry_key, &encrypted, &aad)
                .map_err(|_| VaultError::EntryCorrupted(id.to_string()))?;
            secure_string(id, plaintext)
        })
    }

    pub fn decrypt_field(&self, id: &str, field_name: &str) -> Result<SecureString, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let json = read_encrypted_field(conn, id, "encrypted_custom_fields")?;
            let entry_key = derive_entry_key(dek, id);
            let fields = decrypt_custom_fields(Some(json.as_str()), &entry_key, id)?;
            fields
                .into_iter()
                .find(|f| f.name == field_name)
                .map(|f| SecureString::new(f.value))
                .ok_or_else(|| VaultError::NotFound(field_name.to_string()))
        })
    }

    pub fn decrypt_ssh_private_key(&self, id: &str) -> Result<VaultPlaintext, VaultError> {
        self.with_conn_and_dek(|conn, dek, _dv| {
            let encrypted: Option<String> = conn
                .query_row(
                    "SELECT encrypted_password FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL AND category = 'sshKey'",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|e| map_row_error(id, e))?;
            let encrypted = encrypted.ok_or_else(|| VaultError::NotFound(id.to_string()))?;
            let entry_key = derive_entry_key(dek, id);
            let aad = field_aad(id, FIELD_SSH_PRIVATE_KEY);
            decrypt_field(&entry_key, &encrypted, &aad).map_err(|_| VaultError::EntryCorrupted(id.to_string()))
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

        self.with_conn_and_dek(|conn, dek, _dv| {
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

            let entry_key = derive_entry_key(dek, &eid);

            let password = decrypt_opt(enc_pw.as_deref(), &entry_key, FIELD_PASSWORD, &eid)?;
            let notes = decrypt_opt(enc_notes.as_deref(), &entry_key, FIELD_NOTES, &eid)?;
            let totp_secret = decrypt_opt(enc_totp.as_deref(), &entry_key, FIELD_TOTP, &eid)?;

            let custom_fields = decrypt_custom_fields(enc_cf.as_deref(), &entry_key, &eid)?
                .into_iter()
                .map(|f| crate::types::DecryptedCustomField {
                    name: f.name,
                    value: VaultPlaintext::new_str(&f.value),
                    is_protected: f.is_protected,
                })
                .collect();

            Ok(crate::types::VaultEntryDecrypted {
                id: eid,
                category: VaultCategory::parse(&category_str).unwrap_or_default(),
                title,
                username,
                url,
                password: password.map(|s| VaultPlaintext::new_str(&s)),
                notes: notes.map(|s| VaultPlaintext::new_str(&s)),
                totp_secret: totp_secret.map(|s| VaultPlaintext::new_str(&s)),
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
