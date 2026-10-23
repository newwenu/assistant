use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::Rng;
use rusqlite::Connection;
use zeroize::Zeroize;

use crate::audit::{log as audit_log, AuditEvent};
use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;
use crate::vault_crypto::types::{CipherAlgorithm, MasterKey, VaultPlaintext};
use crate::vault_crypto::VaultCrypto;

use super::{SecurityDbState, SQLCIPHER_PRAGMAS};

impl SecurityDbState {
    pub fn is_biometric_available(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        key_store.is_available()
    }

    pub fn enroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if self.is_locked() {
            return Err(VaultError::Locked);
        }

        if !key_store.is_available()? {
            return Err(VaultError::BiometricUnavailable);
        }

        if self.is_biometric_enrolled(key_store)? {
            self.disenroll_biometric(key_store)?;
        }

        let mk_guard = self.master_key.lock().unwrap();
        let master_key = mk_guard.as_ref().ok_or(VaultError::Locked)?;

        let bk_guard = self.bootstrap_key.lock().unwrap();
        let bootstrap_key = bk_guard.as_ref().ok_or(VaultError::Locked)?;

        {
            let mut wrapping_key = [0u8; 32];
            rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);

            let mut plaintext = Vec::with_capacity(64);
            plaintext.extend_from_slice(bootstrap_key.expose());
            plaintext.extend_from_slice(master_key.expose());

            let nonce: [u8; 12] = rand::thread_rng().gen();
            let encrypted_keys = VaultCrypto::encrypt_field(
                &VaultPlaintext::from(plaintext),
                &wrapping_key,
                CipherAlgorithm::Aes256Gcm,
            )
            .map_err(VaultError::Crypto)?;

            let mut combined = Vec::with_capacity(12 + encrypted_keys.len());
            combined.extend_from_slice(&nonce);
            combined.extend_from_slice(encrypted_keys.as_bytes());
            let biometric_key_enc = B64.encode(&combined);

            let mut blob = Vec::with_capacity(32 + biometric_key_enc.len());
            blob.extend_from_slice(&wrapping_key);
            blob.extend_from_slice(biometric_key_enc.as_bytes());
            key_store.set_key("vault-master-key", &blob)?;

            wrapping_key.zeroize();
        }

        {
            let guard = self.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::BiometricEnrolled, None);
            }
        }

        log::info!("Biometric enrolled successfully");
        Ok(())
    }

    pub fn unlock_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        if !self.is_locked() {
            return Err(VaultError::AlreadyUnlocked);
        }

        if !key_store.is_available()? {
            return Err(VaultError::BiometricUnavailable);
        }

        {
            let blob = key_store.get_key("vault-master-key")?;
            if blob.len() < 33 {
                return Err(VaultError::Crypto(
                    "Invalid biometric blob length".to_string(),
                ));
            }
            let mut wrapping_key = [0u8; 32];
            wrapping_key.copy_from_slice(&blob[..32]);
            let biometric_key_enc = String::from_utf8_lossy(&blob[32..]).to_string();

            let combined = B64.decode(&biometric_key_enc).map_err(|e| {
                VaultError::Crypto(format!("Failed to decode biometric_key_enc: {}", e))
            })?;

            if combined.len() < 12 {
                return Err(VaultError::Crypto(
                    "biometric_key_enc too short".to_string(),
                ));
            }

            let encrypted_keys_str = String::from_utf8_lossy(&combined[12..]).to_string();

            let keys_plaintext = VaultCrypto::decrypt_field(&encrypted_keys_str, &wrapping_key)
                .map_err(|e| {
                    VaultError::Crypto(format!("Biometric key decryption failed: {}", e))
                })?;

            if keys_plaintext.expose().len() != 64 {
                return Err(VaultError::Crypto(
                    "Invalid key length from biometric decrypt (expected 64 bytes)".to_string(),
                ));
            }

            let mut bootstrap_arr = [0u8; 32];
            let mut master_arr = [0u8; 32];
            bootstrap_arr.copy_from_slice(&keys_plaintext.expose()[..32]);
            master_arr.copy_from_slice(&keys_plaintext.expose()[32..]);
            let bootstrap_key = MasterKey::new(bootstrap_arr);
            let master_key = MasterKey::new(master_arr);

            let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&bootstrap_key);
            let sqlcipher_key_hex = hex::encode(sqlcipher_key);

            let path = std::path::Path::new(&self.db_path);
            let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\"", sqlcipher_key_hex))
                .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
            conn.execute_batch(SQLCIPHER_PRAGMAS).map_err(|e| {
                VaultError::Database(format!("PRAGMA cipher settings failed: {}", e))
            })?;

            {
                let mut sk = sqlcipher_key;
                sk.zeroize();
            }

            *self.master_key.lock().unwrap() = Some(master_key);
            *self.bootstrap_key.lock().unwrap() = Some(bootstrap_key);
            *self.conn.lock().unwrap() = Some(conn);

            self.check_biometric_consistency(key_store)?;

            {
                let guard = self.conn.lock().unwrap();
                if let Some(c) = guard.as_ref() {
                    let _ = audit_log(c, AuditEvent::VaultUnlock, Some("biometric"));
                }
            }

            wrapping_key.zeroize();

            log::info!("Vault unlocked via biometric (two-layer)");
        }

        Ok(())
    }

    pub fn disenroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        let _ = key_store.delete_key("vault-master-key");

        {
            let guard = self.conn.lock().unwrap();
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultLock, Some("biometric_disenrolled"));
            }
        }

        log::info!("Biometric disenrolled");
        Ok(())
    }

    pub fn is_biometric_enrolled(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        key_store.has_key("vault-master-key")
    }

    pub fn check_biometric_consistency(
        &self,
        _key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        Ok(())
    }
}