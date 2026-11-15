use std::path::Path;

use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::audit::{log_dual, AuditEvent};
use crate::biometric::BiometricKeyStore;
use crate::vault_file::VaultFileManager;
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{
    CipherAlgorithm, DbKey, Dek, PerEntryKey, VaultPlaintext,
};
use assistant_crypto::vault_crypto::{decrypt_field, encrypt_field};

use super::{open_connection, probe_read, AnchorExpectation, SecurityDbState, VaultSession};

const BIOMETRIC_PAYLOAD_LEN: usize = 52;
const BIOMETRIC_DEK_LEN: usize = 20;
const BIOMETRIC_DB_KEY_LEN: usize = 32;

const KEY_ID_PAYLOAD: &str = "vault-payload";
const KEY_ID_FINGERPRINT: &str = "vault-fingerprint";
const KEY_ID_SIG_HASH: &str = "vault-sig-hash";
const KEY_ID_WRAPPING_KEY: &str = "vault-wrapping-key";

const BIOMETRIC_KEY_IDS: [&str; 4] = [
    KEY_ID_PAYLOAD,
    KEY_ID_FINGERPRINT,
    KEY_ID_SIG_HASH,
    KEY_ID_WRAPPING_KEY,
];

const HKDF_SALT: &[u8] = b"assistant-vault-v1";
const HKDF_INFO: &[u8] = b"aes-256-gcm-key";

const BIOMETRIC_CIPHER: CipherAlgorithm = CipherAlgorithm::Aes256Gcm;

const PAYLOAD_AAD: &[u8] = b"";

/// 失败原因：封闭集合，禁止自由文本进审计。
fn audit_failure_jsonl(state: &SecurityDbState, reason: &'static str) {
    let writer = state.audit_writer();
    log_dual(
        None,
        Some(&writer),
        AuditEvent::BiometricFailed,
        Some(reason),
    );
}

fn derive_wrapping_key_from_signature(signature: &[u8]) -> Result<[u8; 32], VaultError> {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), signature);
    let mut key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut key)
        .map_err(|e| VaultError::Crypto(format!("HKDF expand failed: {e}")))?;
    Ok(key)
}

fn compute_signature_hash(signature: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signature);
    hex::encode(hasher.finalize())
}

/// 指纹只用于识别「payload 与当前密钥不匹配」，不是防回滚机制。
fn compute_fingerprint(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

/// 删除全部生物识别条目；任一失败返回首个错误。
pub fn clear_all_biometric_entries(key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
    let mut first_error: Option<VaultError> = None;
    for key_id in BIOMETRIC_KEY_IDS {
        if let Err(e) = key_store.delete_key(key_id) {
            log::warn!("Failed to delete biometric key '{}': {}", key_id, e);
            if first_error.is_none() {
                first_error = Some(e);
            }
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn restore_keys_from_plaintext(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    payload: &VaultPlaintext,
    key_store: &dyn BiometricKeyStore,
) -> Result<(), VaultError> {
    let bytes = payload.expose();
    if bytes.len() != BIOMETRIC_PAYLOAD_LEN {
        audit_failure_jsonl(state, "payload_len");
        return Err(VaultError::BiometricReset);
    }

    state.check_biometric_consistency(key_store)?;

    let fingerprint = compute_fingerprint(bytes);
    let stored = match key_store.get_key(KEY_ID_FINGERPRINT) {
        Ok(value) => value,
        Err(_) => {
            audit_failure_jsonl(state, "stale_payload");
            return Err(VaultError::BiometricReset);
        }
    };
    if stored.as_slice() != fingerprint.as_bytes() {
        audit_failure_jsonl(state, "stale_payload");
        return Err(VaultError::BiometricReset);
    }

    let mut dek_arr = [0u8; BIOMETRIC_DEK_LEN];
    dek_arr.copy_from_slice(&bytes[..BIOMETRIC_DEK_LEN]);
    let mut db_key_arr = [0u8; BIOMETRIC_DB_KEY_LEN];
    db_key_arr.copy_from_slice(&bytes[BIOMETRIC_DEK_LEN..]);
    let dek = Dek::new(dek_arr);
    let db_key = DbKey::new(db_key_arr);

    let path = Path::new(state.db_path());
    if !path.exists() {
        return Err(VaultError::NotInitialized);
    }

    let conn = match open_connection(path, &db_key) {
        Ok(conn) => conn,
        Err(_) => {
            audit_failure_jsonl(state, "probe_failed");
            return Err(VaultError::BiometricReset);
        }
    };

    if probe_read(&conn).is_err() {
        audit_failure_jsonl(state, "probe_failed");
        return Err(VaultError::BiometricReset);
    }

    let meta = repo.read_vault_meta_v2()?;
    let expected = AnchorExpectation::new(repo.meta_file_hash()?, meta.dek_version);

    state.unlock(
        VaultSession::new(conn, dek, db_key, expected.dek_version),
        &expected,
    )?;

    let writer = state.audit_writer();
    let _ = state.with_conn(|conn| {
        log_dual(
            Some(conn),
            Some(&writer),
            AuditEvent::BiometricUnlocked,
            None,
        );
        Ok(())
    });

    Ok(())
}

impl SecurityDbState {
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

        if !key_store.acl_satisfied()? {
            log::warn!("Biometric ACL unsatisfied; enrollment rejected");
            return Err(VaultError::BiometricUnavailable);
        }

        let mut payload: Vec<u8> = self.with_session(|session| {
            let mut payload = Vec::with_capacity(BIOMETRIC_PAYLOAD_LEN);
            payload.extend_from_slice(session.dek().expose());
            payload.extend_from_slice(session.db_key().expose());
            Ok(payload)
        })?;

        #[cfg(target_os = "windows")]
        let enroll_result = self.enroll_windows(key_store, &payload, on_credential_ensured);

        #[cfg(not(target_os = "windows"))]
        let enroll_result = {
            let _ = on_credential_ensured;
            self.enroll_separated(key_store, &payload)
        };

        let fingerprint = compute_fingerprint(&payload);
        payload.zeroize();
        enroll_result?;

        key_store.set_key(KEY_ID_FINGERPRINT, fingerprint.as_bytes())?;

        self.audit_dual(AuditEvent::BiometricEnrolled, None);

        log::info!("Biometric enrolled successfully");
        Ok(())
    }

    pub fn unlock_biometric(
        &self,
        repo: &VaultFileManager,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        if !self.is_locked() {
            return Err(VaultError::AlreadyUnlocked);
        }

        if !key_store.is_available()? {
            return Err(VaultError::BiometricUnavailable);
        }

        #[cfg(target_os = "windows")]
        self.unlock_windows(repo, key_store)?;

        #[cfg(not(target_os = "windows"))]
        self.unlock_separated(repo, key_store)?;

        log::info!("Vault unlocked via biometric");
        Ok(())
    }

    pub fn disenroll_biometric(&self, key_store: &dyn BiometricKeyStore) -> Result<(), VaultError> {
        clear_all_biometric_entries(key_store)?;

        self.audit_dual(AuditEvent::BiometricRevoked, None);

        log::info!("Biometric disenrolled");
        Ok(())
    }

    /// R8：轮换 `dek` 前必须吊销凭据；`vault-payload` 删除失败即中止。
    pub fn revoke_biometric_for_rotation(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        if let Err(e) = key_store.delete_key(KEY_ID_PAYLOAD) {
            log::warn!("Failed to delete biometric payload before rotation: {}", e);
            audit_failure_jsonl(self, "delete_failed");
            return Err(e);
        }

        for key_id in [KEY_ID_FINGERPRINT, KEY_ID_SIG_HASH, KEY_ID_WRAPPING_KEY] {
            if let Err(e) = key_store.delete_key(key_id) {
                log::warn!("Failed to delete biometric key '{}': {}", key_id, e);
            }
        }

        self.audit_dual(AuditEvent::BiometricRevoked, None);

        log::info!("Biometric revoked for dek rotation");
        Ok(())
    }

    pub fn is_biometric_enrolled(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<bool, VaultError> {
        #[cfg(target_os = "windows")]
        {
            key_store.has_key(KEY_ID_PAYLOAD)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let has_wrapping = key_store.has_key(KEY_ID_WRAPPING_KEY)?;
            let has_payload = key_store.has_key(KEY_ID_PAYLOAD)?;
            Ok(has_wrapping && has_payload)
        }
    }

    pub fn check_biometric_consistency(
        &self,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        #[cfg(target_os = "windows")]
        {
            if !key_store.has_key(KEY_ID_PAYLOAD)? {
                if key_store.has_key(KEY_ID_FINGERPRINT)? {
                    let _ = key_store.delete_key(KEY_ID_FINGERPRINT);
                }
                return Err(VaultError::BiometricReset);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let has_wrapping = key_store.has_key(KEY_ID_WRAPPING_KEY)?;
            let has_payload = key_store.has_key(KEY_ID_PAYLOAD)?;
            if !has_wrapping || !has_payload {
                let _ = key_store.delete_key(KEY_ID_WRAPPING_KEY);
                let _ = key_store.delete_key(KEY_ID_PAYLOAD);
                let _ = key_store.delete_key(KEY_ID_FINGERPRINT);
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
        payload: &[u8],
        on_credential_ensured: impl FnOnce(),
    ) -> Result<(), VaultError> {
        let mut signature = match key_store.request_signature(KEY_ID_PAYLOAD) {
            Ok(sig) => sig,
            Err(VaultError::BiometricUnavailable) => {
                key_store.ensure_credential(KEY_ID_PAYLOAD)?;
                on_credential_ensured();
                key_store.request_signature(KEY_ID_PAYLOAD)?
            }
            Err(e) => return Err(e),
        };

        let sig_hash = compute_signature_hash(&signature);
        let mut wrapping_key = derive_wrapping_key_from_signature(&signature)?;

        let encrypted = encrypt_field(
            &PerEntryKey::new(wrapping_key),
            &VaultPlaintext::from(payload.to_vec()),
            BIOMETRIC_CIPHER,
            PAYLOAD_AAD,
        )?;

        key_store.set_key(KEY_ID_PAYLOAD, encrypted.as_bytes())?;
        key_store.set_key(KEY_ID_SIG_HASH, sig_hash.as_bytes())?;

        wrapping_key.zeroize();
        signature.zeroize();
        Ok(())
    }

    fn unlock_windows(
        &self,
        repo: &VaultFileManager,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        let blob = match key_store.get_key(KEY_ID_PAYLOAD) {
            Ok(blob) => blob,
            Err(_) => return Err(VaultError::BiometricReset),
        };
        let blob_str = String::from_utf8_lossy(&blob).to_string();

        let mut signature = key_store.request_signature(KEY_ID_PAYLOAD)?;
        let sig_hash = compute_signature_hash(&signature);

        let stored_hash = match key_store.get_key(KEY_ID_SIG_HASH) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => {
                signature.zeroize();
                audit_failure_jsonl(self, "sig_hash_missing");
                return Err(VaultError::BiometricReset);
            }
        };

        if stored_hash != sig_hash {
            signature.zeroize();
            audit_failure_jsonl(self, "sig_changed");
            return Err(VaultError::BiometricReset);
        }

        let mut wrapping_key = derive_wrapping_key_from_signature(&signature)?;

        let plaintext = match decrypt_field(&PerEntryKey::new(wrapping_key), &blob_str, PAYLOAD_AAD)
        {
            Ok(plaintext) => plaintext,
            Err(_) => {
                wrapping_key.zeroize();
                signature.zeroize();
                audit_failure_jsonl(self, "decrypt_failed");
                return Err(VaultError::BiometricReset);
            }
        };

        wrapping_key.zeroize();
        signature.zeroize();

        restore_keys_from_plaintext(self, repo, &plaintext, key_store)
    }
}

#[cfg(not(target_os = "windows"))]
impl SecurityDbState {
    fn enroll_separated(
        &self,
        key_store: &dyn BiometricKeyStore,
        payload: &[u8],
    ) -> Result<(), VaultError> {
        let mut wrapping_key = [0u8; 32];
        rand::Rng::fill(&mut rand::thread_rng(), &mut wrapping_key);

        let encrypted = encrypt_field(
            &PerEntryKey::new(wrapping_key),
            &VaultPlaintext::from(payload.to_vec()),
            BIOMETRIC_CIPHER,
            PAYLOAD_AAD,
        )?;

        let key_hex = hex::encode(wrapping_key);

        key_store.set_key(KEY_ID_WRAPPING_KEY, key_hex.as_bytes())?;
        key_store.set_key_unprotected(KEY_ID_PAYLOAD, encrypted.as_bytes())?;

        wrapping_key.zeroize();
        Ok(())
    }

    fn unlock_separated(
        &self,
        repo: &VaultFileManager,
        key_store: &dyn BiometricKeyStore,
    ) -> Result<(), VaultError> {
        let key_bytes = match key_store.get_key(KEY_ID_WRAPPING_KEY) {
            Ok(bytes) => bytes,
            Err(_) => return Err(VaultError::BiometricReset),
        };
        let blob = match key_store.get_key(KEY_ID_PAYLOAD) {
            Ok(blob) => blob,
            Err(_) => return Err(VaultError::BiometricReset),
        };
        let blob_str = String::from_utf8_lossy(&blob).to_string();
        let key_hex = String::from_utf8_lossy(&key_bytes).to_string();

        let mut key_material = match hex::decode(&key_hex) {
            Ok(material) => material,
            Err(_) => {
                audit_failure_jsonl(self, "wrapping_key_len");
                return Err(VaultError::BiometricReset);
            }
        };

        if key_material.len() != 32 {
            key_material.zeroize();
            audit_failure_jsonl(self, "wrapping_key_len");
            return Err(VaultError::BiometricReset);
        }

        let mut wrapping_key = [0u8; 32];
        wrapping_key.copy_from_slice(&key_material);
        key_material.zeroize();

        let plaintext = match decrypt_field(&PerEntryKey::new(wrapping_key), &blob_str, PAYLOAD_AAD)
        {
            Ok(plaintext) => plaintext,
            Err(_) => {
                wrapping_key.zeroize();
                audit_failure_jsonl(self, "decrypt_failed");
                return Err(VaultError::BiometricReset);
            }
        };

        wrapping_key.zeroize();

        restore_keys_from_plaintext(self, repo, &plaintext, key_store)
    }
}
