use hkdf::Hkdf;
use rusqlite::Connection;
use sha2::Sha256;
use zeroize::Zeroize;

use crate::audit::{log as audit_log, AuditEvent};
use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;
use crate::vault_crypto::types::{CipherAlgorithm, MasterKey, VaultPlaintext};
use crate::vault_crypto::VaultCrypto;

use super::{SecurityDbState, SQLCIPHER_PRAGMAS};

const KEY_ID_MASTER: &str = "vault-master-key";
const KEY_ID_AES_KEY: &str = "vault-master-aes-key";
const KEY_ID_CIPHERTEXT: &str = "vault-master-ciphertext";
const KEY_ID_SIG_HASH: &str = "vault-master-sig-hash";

const HKDF_SALT: &[u8] = b"assistant-vault-v1";
const HKDF_INFO: &[u8] = b"aes-256-gcm-key";

fn derive_wrapping_key_from_signature(signature: &[u8]) -> Result<[u8; 32], VaultError> {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), signature);
    let mut key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut key)
        .map_err(|e| VaultError::Crypto(format!("HKDF expand failed: {e}")))?;
    Ok(key)
}

fn compute_signature_hash(signature: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(signature);
    hex::encode(hasher.finalize())
}

fn restore_keys_from_plaintext(
    state: &SecurityDbState,
    keys_plaintext: &VaultPlaintext,
    key_store: &dyn BiometricKeyStore,
) -> Result<(), VaultError> {
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

    let path = std::path::Path::new(&state.db_path);
    let conn = Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\"", sqlcipher_key_hex))
        .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
    conn.execute_batch(SQLCIPHER_PRAGMAS)
        .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;

    {
        let mut sk = sqlcipher_key;
        sk.zeroize();
    }

    *state.master_key.lock().unwrap() = Some(master_key);
    *state.bootstrap_key.lock().unwrap() = Some(bootstrap_key);
    *state.conn.lock().unwrap() = Some(conn);

    state.check_biometric_consistency(key_store)?;

    {
        let guard = state.conn.lock().unwrap();
        if let Some(c) = guard.as_ref() {
            let _ = audit_log(c, AuditEvent::VaultUnlock, Some("biometric"));
        }
    }

    Ok(())
}

impl SecurityDbState {
    pub fn is_biometric_available(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        key_store.is_available()
    }

    pub fn enroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        self.enroll_biometric_with_hint(key_store, || {})
    }

    pub fn enroll_biometric_with_hint(
        &self,
        key_store: &dyn BiometricKeyStore,
        on_credential_ensured: impl FnOnce(),
    ) -> Result<(), VaultError> {
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
            let mut plaintext = Vec::with_capacity(64);
            plaintext.extend_from_slice(bootstrap_key.expose());
            plaintext.extend_from_slice(master_key.expose());

            #[cfg(target_os = "windows")]
            {
                self.enroll_windows(key_store, &plaintext, on_credential_ensured)?;
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = on_credential_ensured;
                self.enroll_separated(key_store, &plaintext)?;
            }

            plaintext.zeroize();
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

        #[cfg(target_os = "windows")]
        {
            self.unlock_windows(key_store)?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            self.unlock_separated(key_store)?;
        }

        log::info!("Vault unlocked via biometric");
        Ok(())
    }

    pub fn disenroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        let _ = key_store.delete_key(KEY_ID_MASTER);
        let _ = key_store.delete_key(KEY_ID_AES_KEY);
        let _ = key_store.delete_key(KEY_ID_CIPHERTEXT);
        let _ = key_store.delete_key(KEY_ID_SIG_HASH);

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
        #[cfg(target_os = "windows")]
        {
            key_store.has_key(KEY_ID_MASTER)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let has_aes = key_store.has_key(KEY_ID_AES_KEY)?;
            let has_ct = key_store.has_key(KEY_ID_CIPHERTEXT)?;
            Ok(has_aes && has_ct)
        }
    }

    pub fn check_biometric_consistency(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        #[cfg(target_os = "windows")]
        {
            if !key_store.has_key(KEY_ID_MASTER)? {
                return Err(VaultError::BiometricReset);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if !key_store.has_key(KEY_ID_AES_KEY)? || !key_store.has_key(KEY_ID_CIPHERTEXT)? {
                let _ = key_store.delete_key(KEY_ID_AES_KEY);
                let _ = key_store.delete_key(KEY_ID_CIPHERTEXT);
                return Err(VaultError::BiometricReset);
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl SecurityDbState {
    fn enroll_windows(
        &self,
        key_store: &dyn BiometricKeyStore,
        plaintext: &[u8],
        on_credential_ensured: impl FnOnce(),
    ) -> Result<(), VaultError> {
        let mut signature = match key_store.request_signature(KEY_ID_MASTER) {
            Ok(sig) => sig,
            Err(VaultError::BiometricUnavailable) => {
                key_store.ensure_credential(KEY_ID_MASTER)?;
                on_credential_ensured();
                key_store.request_signature(KEY_ID_MASTER)?
            }
            Err(e) => return Err(e),
        };

        let sig_hash = compute_signature_hash(&signature);
        let mut wrapping_key = derive_wrapping_key_from_signature(&signature)?;

        let encrypted_keys = VaultCrypto::encrypt_field(
            &VaultPlaintext::from(plaintext.to_vec()),
            &wrapping_key,
            CipherAlgorithm::Aes256Gcm,
        )
        .map_err(VaultError::Crypto)?;

        key_store.set_key(KEY_ID_MASTER, encrypted_keys.as_bytes())?;
        key_store.set_key(KEY_ID_SIG_HASH, sig_hash.as_bytes())?;

        wrapping_key.zeroize();
        signature.zeroize();
        Ok(())
    }

    fn unlock_windows(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        let blob = key_store.get_key(KEY_ID_MASTER)?;
        let encrypted_keys_str = String::from_utf8_lossy(&blob).to_string();

        let mut signature = key_store.request_signature(KEY_ID_MASTER)?;
        let sig_hash = compute_signature_hash(&signature);

        let stored_hash_bytes = key_store.get_key(KEY_ID_SIG_HASH)?;
        let stored_hash = String::from_utf8_lossy(&stored_hash_bytes).to_string();

        if sig_hash != stored_hash {
            signature.zeroize();
            return Err(VaultError::BiometricFailed(
                "TPM credential has changed. Please re-enroll biometric authentication.".into(),
            ));
        }

        let mut wrapping_key = derive_wrapping_key_from_signature(&signature)?;

        let keys_plaintext = VaultCrypto::decrypt_field(&encrypted_keys_str, &wrapping_key)
            .map_err(VaultError::Crypto)?;

        let result = restore_keys_from_plaintext(self, &keys_plaintext, key_store);

        wrapping_key.zeroize();
        signature.zeroize();
        result
    }
}

#[cfg(not(target_os = "windows"))]
impl SecurityDbState {
    fn enroll_separated(
        &self,
        key_store: &dyn BiometricKeyStore,
        plaintext: &[u8],
    ) -> Result<(), VaultError> {
        let mut wrapping_key = [0u8; 32];
        rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);

        let encrypted_keys = VaultCrypto::encrypt_field(
            &VaultPlaintext::from(plaintext.to_vec()),
            &wrapping_key,
            CipherAlgorithm::Aes256Gcm,
        )
        .map_err(VaultError::Crypto)?;

        let key_hex = hex::encode(wrapping_key);

        key_store.set_key(KEY_ID_AES_KEY, key_hex.as_bytes())?;
        key_store.set_key_unprotected(KEY_ID_CIPHERTEXT, encrypted_keys.as_bytes())?;

        wrapping_key.zeroize();
        Ok(())
    }

    fn unlock_separated(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        let aes_key_bytes = key_store.get_key(KEY_ID_AES_KEY)?;
        let ciphertext_b64 =
            String::from_utf8_lossy(&key_store.get_key(KEY_ID_CIPHERTEXT)?).to_string();

        let key_hex = String::from_utf8_lossy(&aes_key_bytes).to_string();
        let key_material = hex::decode(&key_hex)
            .map_err(|e| VaultError::Crypto(format!("Failed to decode AES key material: {}", e)))?;

        if key_material.len() != 32 {
            return Err(VaultError::Crypto(format!(
                "Invalid AES key material length: expected 32, got {}",
                key_material.len()
            )));
        }

        let mut wrapping_key = [0u8; 32];
        wrapping_key.copy_from_slice(&key_material[..32]);

        let keys_plaintext = VaultCrypto::decrypt_field(&ciphertext_b64, &wrapping_key)
            .map_err(|e| VaultError::Crypto(format!("Biometric key decryption failed: {}", e)))?;

        let result = restore_keys_from_plaintext(self, &keys_plaintext, key_store);

        wrapping_key.zeroize();
        result
    }
}
