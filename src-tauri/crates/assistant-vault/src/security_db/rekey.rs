use std::path::Path;

use rand::Rng;
use rusqlite::params;
use zeroize::Zeroize;

use crate::audit::{log as audit_log, AuditEvent};
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_crypto::vault_crypto::VaultCrypto;

use super::{meta, SecurityDbState, BOOTSTRAP_SALT};

impl SecurityDbState {
    pub fn change_master_password(
        &self,
        current_password: &str,
        new_password: &str,
        algo: CipherAlgorithm,
    ) -> Result<(), VaultError> {
        {
            let strength = crate::password_strength::estimate_strength(new_password);
            if strength.score < 2 {
                return Err(VaultError::Validation(format!(
                    "New master password too weak (score {}/4, need ≥2). {}",
                    strength.score, strength.crack_time_display
                )));
            }
        }

        let conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let hash_salt = meta::get_salt(conn, "hash_salt")?
            .ok_or_else(|| VaultError::Database("hash_salt not found".to_string()))?;
        let stored_hash = meta::get_master_key_hash(conn)?
            .ok_or_else(|| VaultError::Database("master_key_hash not found".to_string()))?;
        let valid = VaultCrypto::verify_master_password(current_password, &stored_hash, &hash_salt)
            .map_err(VaultError::Crypto)?;
        if !valid {
            return Err(VaultError::InvalidPassword);
        }

        let backup_path = Self::backup_conn(conn, Path::new(&self.db_path))?;

        let new_master_key_salt: [u8; 32] = rand::thread_rng().gen();
        let new_hash_salt: [u8; 32] = rand::thread_rng().gen();
        let new_master_key_bytes =
            VaultCrypto::derive_master_key(new_password, &new_master_key_salt)
                .map_err(VaultError::Crypto)?;
        let new_master_key_hash = VaultCrypto::derive_master_key(new_password, &new_hash_salt)
            .map_err(VaultError::Crypto)?;
        let new_master_key = MasterKey::new(new_master_key_bytes);

        let entry_ids: Vec<String>;
        {
            let mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
            let old_master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;

            entry_ids = {
                let mut stmt = conn
                    .prepare("SELECT id FROM vault_entries WHERE deleted_at IS NULL")
                    .map_err(|e| VaultError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map([], |row| row.get(0))
                    .map_err(|e| VaultError::Database(e.to_string()))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            conn.execute_batch("BEGIN TRANSACTION")
                .map_err(|e| VaultError::Database(e.to_string()))?;

            let re_encrypt_result: Result<(), VaultError> = (|| {
                for eid in &entry_ids {
                    let old_entry_key = VaultCrypto::derive_entry_key(old_master_key, eid);
                    let new_entry_key = VaultCrypto::derive_entry_key(&new_master_key, eid);

                    let re_encrypt = |col: &str| -> Result<Option<String>, VaultError> {
                        let encrypted: Option<String> = conn
                            .query_row(
                                &format!("SELECT {} FROM vault_entries WHERE id = ?1", col),
                                params![eid],
                                |row| row.get(0),
                            )
                            .map_err(|e| VaultError::Database(e.to_string()))?;
                        match encrypted {
                            Some(enc) => {
                                let pt = VaultCrypto::decrypt_field(&enc, old_entry_key.expose())
                                    .map_err(VaultError::Crypto)?;
                                let re_enc =
                                    VaultCrypto::encrypt_field(&pt, new_entry_key.expose(), algo)
                                        .map_err(VaultError::Crypto)?;
                                Ok(Some(re_enc))
                            }
                            None => Ok(None),
                        }
                    };

                    let new_password_enc = re_encrypt("encrypted_password")?;
                    let new_notes_enc = re_encrypt("encrypted_notes")?;
                    let new_totp_enc = re_encrypt("encrypted_totp_secret")?;

                    let new_custom_fields_enc: Option<String> = {
                        let cf_json: Option<String> = conn
                            .query_row(
                                "SELECT encrypted_custom_fields FROM vault_entries WHERE id = ?1",
                                params![eid],
                                |row| row.get(0),
                            )
                            .map_err(|e| VaultError::Database(e.to_string()))?;
                        match cf_json {
                            Some(json) => {
                                let fields: Vec<serde_json::Value> = serde_json::from_str(&json)
                                    .map_err(|e| VaultError::Database(e.to_string()))?;
                                let re_encrypted: Vec<serde_json::Value> = fields
                                    .iter()
                                    .map(|f| {
                                        if f.get("isProtected").and_then(|v| v.as_bool())
                                            == Some(true)
                                        {
                                            if let Some(enc_val) =
                                                f.get("valueEncrypted").and_then(|v| v.as_str())
                                            {
                                                match VaultCrypto::decrypt_field(
                                                    enc_val,
                                                    old_entry_key.expose(),
                                                ) {
                                                    Ok(pt) => {
                                                        match VaultCrypto::encrypt_field(
                                                            &pt,
                                                            new_entry_key.expose(),
                                                            algo,
                                                        ) {
                                                            Ok(re_enc) => serde_json::json!({
                                                                "name": f.get("name"),
                                                                "valueEncrypted": re_enc,
                                                                "isProtected": true
                                                            }),
                                                            Err(_) => f.clone(),
                                                        }
                                                    }
                                                    Err(_) => f.clone(),
                                                }
                                            } else {
                                                f.clone()
                                            }
                                        } else {
                                            f.clone()
                                        }
                                    })
                                    .collect();
                                Some(
                                    serde_json::to_string(&re_encrypted)
                                        .map_err(|e| VaultError::Validation(e.to_string()))?,
                                )
                            }
                            None => None,
                        }
                    };

                    conn.execute(
                        "UPDATE vault_entries SET encrypted_password = ?1, encrypted_notes = ?2, encrypted_totp_secret = ?3, encrypted_custom_fields = ?4 WHERE id = ?5",
                        params![new_password_enc, new_notes_enc, new_totp_enc, new_custom_fields_enc, eid],
                    ).map_err(|e| VaultError::Database(e.to_string()))?;
                }
                Ok(())
            })();

            if let Err(e) = re_encrypt_result {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }

            meta::set_salt(conn, "master_key_salt", &new_master_key_salt)?;
            meta::set_salt(conn, "hash_salt", &new_hash_salt)?;
            meta::set_master_key_hash(conn, &new_master_key_hash)?;

            if let Some(test_id) = entry_ids.first() {
                let test_entry_key = VaultCrypto::derive_entry_key(&new_master_key, test_id);
                let test_enc: Option<String> = conn
                    .query_row(
                        "SELECT encrypted_password FROM vault_entries WHERE id = ?1",
                        params![test_id],
                        |row| row.get(0),
                    )
                    .ok()
                    .flatten();
                if let Some(enc) = test_enc {
                    if VaultCrypto::decrypt_field(&enc, test_entry_key.expose()).is_err() {
                        let _ = conn.execute_batch("ROLLBACK");
                        return Err(VaultError::Crypto(
                            "Verification failed after re-encryption, transaction rolled back"
                                .to_string(),
                        ));
                    }
                }
            }

            conn.execute_batch("COMMIT")
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }

        let new_bootstrap_key_bytes = VaultCrypto::derive_master_key(new_password, &BOOTSTRAP_SALT)
            .map_err(VaultError::Crypto)?;
        let new_bootstrap_key = MasterKey::new(new_bootstrap_key_bytes);
        let new_sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&new_bootstrap_key);
        let new_sqlcipher_key_hex = hex::encode(new_sqlcipher_key);
        conn.execute_batch(&format!("PRAGMA rekey = \"x'{}'\"", new_sqlcipher_key_hex))
            .map_err(|e| {
                VaultError::Database(format!(
                    "PRAGMA rekey failed: {}. Backup at: {}",
                    e,
                    backup_path.display()
                ))
            })?;
        {
            let mut key_arr = new_sqlcipher_key;
            key_arr.zeroize();
        }

        {
            let mut mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
            *mk_guard = Some(new_master_key);
        }
        {
            let mut bk_guard = self.bootstrap_key.lock().unwrap_or_else(|e| e.into_inner());
            *bk_guard = Some(new_bootstrap_key);
        }

        let _ = audit_log(conn, AuditEvent::MasterPasswordChange, None);
        log::info!("Master password changed successfully (PRAGMA rekey + entry re-encryption + custom_fields re-encryption)");

        drop(conn_guard);

        Ok(())
    }
}
