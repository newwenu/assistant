use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use log::warn;
use rand::Rng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::security_db::SecurityDbState;
use crate::types::VaultError;
use crate::vault_crypto::types::MasterKey;
use crate::vault_crypto::VaultCrypto;

const RECOVERY_KEY_RAW_LEN: usize = 20;
const RECOVERY_KEY_ENCODED_LEN: usize = 32;
const RECOVERY_KEY_GROUP_LEN: usize = 8;
const RECOVERY_KEY_GROUPS: usize = 4;
const ARGON2_MEMORY: u32 = 65536;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_VERSION: Version = Version::V0x13;
const NONCE_SIZE: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyData {
    pub token: String,
    pub salt: String,
}

pub fn generate_recovery_key(
    master_key: &MasterKey,
    bootstrap_key: &MasterKey,
) -> Result<(String, EmergencyData), VaultError> {
    let mut raw_key = [0u8; RECOVERY_KEY_RAW_LEN];
    rand::thread_rng().fill(&mut raw_key);

    let formatted = format_recovery_key(&raw_key);

    let mut salt = [0u8; 16];
    rand::thread_rng().fill(&mut salt);

    let recovery_derived_key = derive_recovery_key(&formatted, &salt)?;

    let nonce_bytes: [u8; NONCE_SIZE] = rand::thread_rng().gen();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut plaintext = [0u8; 64];
    plaintext[..32].copy_from_slice(bootstrap_key.expose());
    plaintext[32..].copy_from_slice(master_key.expose());

    let cipher = Aes256Gcm::new_from_slice(&recovery_derived_key)
        .map_err(|e| VaultError::Crypto(format!("AES-256-GCM init failed: {}", e)))?;

    let encrypted_keys = cipher
        .encrypt(nonce, &plaintext as &[u8])
        .map_err(|e| VaultError::Crypto(format!("AES-256-GCM encrypt failed: {}", e)))?;

    plaintext.zeroize();

    let mut token_data = Vec::with_capacity(NONCE_SIZE + encrypted_keys.len());
    token_data.extend_from_slice(&nonce_bytes);
    token_data.extend_from_slice(&encrypted_keys);
    let token = B64.encode(&token_data);

    let emergency = EmergencyData {
        token,
        salt: B64.encode(salt),
    };

    raw_key.zeroize();

    warn!("Recovery key generated");
    Ok((formatted, emergency))
}

pub fn decrypt_recovery_keys(
    recovery_key: &str,
    emergency: &EmergencyData,
) -> Result<(MasterKey, MasterKey), VaultError> {
    let _parsed = parse_recovery_key(recovery_key)?;

    let salt_bytes = B64
        .decode(&emergency.salt)
        .map_err(|e| VaultError::Crypto(format!("Failed to decode emergency salt: {}", e)))?;
    let salt: [u8; 16] = salt_bytes
        .try_into()
        .map_err(|_| VaultError::Crypto("Emergency salt must be 16 bytes".to_string()))?;

    let recovery_derived_key = derive_recovery_key(recovery_key, &salt)?;

    let token_bytes = B64
        .decode(&emergency.token)
        .map_err(|e| VaultError::Crypto(format!("Failed to decode emergency token: {}", e)))?;

    if token_bytes.len() < NONCE_SIZE + 16 {
        return Err(VaultError::InvalidRecoveryKey);
    }

    let nonce = Nonce::from_slice(&token_bytes[..NONCE_SIZE]);
    let ciphertext = &token_bytes[NONCE_SIZE..];

    let cipher = Aes256Gcm::new_from_slice(&recovery_derived_key)
        .map_err(|e| VaultError::Crypto(format!("AES-256-GCM init failed: {}", e)))?;

    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| VaultError::InvalidRecoveryKey)?;

    if decrypted.len() != 64 {
        return Err(VaultError::InvalidRecoveryKey);
    }

    let mut bootstrap_arr = [0u8; 32];
    let mut master_arr = [0u8; 32];
    bootstrap_arr.copy_from_slice(&decrypted[..32]);
    master_arr.copy_from_slice(&decrypted[32..]);

    let bootstrap_key = MasterKey::new(bootstrap_arr);
    let master_key = MasterKey::new(master_arr);

    Ok((bootstrap_key, master_key))
}

pub fn unlock_with_recovery_key(
    recovery_key: &str,
    emergency: &EmergencyData,
    db_state: &SecurityDbState,
) -> Result<bool, VaultError> {
    let _parsed = parse_recovery_key(recovery_key)?;

    let salt_bytes = B64
        .decode(&emergency.salt)
        .map_err(|e| VaultError::Crypto(format!("Failed to decode emergency salt: {}", e)))?;
    let salt: [u8; 16] = salt_bytes
        .try_into()
        .map_err(|_| VaultError::Crypto("Emergency salt must be 16 bytes".to_string()))?;

    let recovery_derived_key = derive_recovery_key(recovery_key, &salt)?;

    let token_bytes = B64
        .decode(&emergency.token)
        .map_err(|e| VaultError::Crypto(format!("Failed to decode emergency token: {}", e)))?;

    if token_bytes.len() < NONCE_SIZE + 16 {
        return Err(VaultError::InvalidRecoveryKey);
    }

    let nonce = Nonce::from_slice(&token_bytes[..NONCE_SIZE]);
    let ciphertext = &token_bytes[NONCE_SIZE..];

    let cipher = Aes256Gcm::new_from_slice(&recovery_derived_key)
        .map_err(|e| VaultError::Crypto(format!("AES-256-GCM init failed: {}", e)))?;

    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| VaultError::InvalidRecoveryKey)?;

    if decrypted.len() != 64 {
        return Err(VaultError::InvalidRecoveryKey);
    }

    let mut bootstrap_arr = [0u8; 32];
    let mut master_arr = [0u8; 32];
    bootstrap_arr.copy_from_slice(&decrypted[..32]);
    master_arr.copy_from_slice(&decrypted[32..]);

    let bootstrap_key = MasterKey::new(bootstrap_arr);
    let master_key = MasterKey::new(master_arr);

    let sqlcipher_key = VaultCrypto::derive_sqlcipher_key(&bootstrap_key);
    let sqlcipher_key_hex = hex::encode(sqlcipher_key);

    {
        let mut sk = sqlcipher_key;
        sk.zeroize();
    }

    let db_path = db_state.db_path();
    let path = std::path::Path::new(db_path);

    if !path.exists() {
        return Err(VaultError::NotInitialized);
    }

    let conn = rusqlite::Connection::open(path).map_err(|e| VaultError::Database(e.to_string()))?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", sqlcipher_key_hex))
        .map_err(|e| VaultError::Database(format!("PRAGMA key set failed: {}", e)))?;
    conn.execute_batch("PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter = 256000; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;")
        .map_err(|e| VaultError::Database(format!("PRAGMA cipher settings failed: {}", e)))?;

    let test_result: Result<String, _> = conn.query_row(
        "SELECT value FROM vault_meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    );

    if test_result.is_err() {
        return Err(VaultError::InvalidRecoveryKey);
    }

    db_state.set_conn_and_key(conn, master_key, bootstrap_key);

    warn!("Vault unlocked via recovery key");
    Ok(true)
}

pub fn format_recovery_key(raw: &[u8]) -> String {
    use data_encoding::BASE32_NOPAD;
    let encoded = BASE32_NOPAD.encode(raw);
    let groups: Vec<String> = (0..RECOVERY_KEY_GROUPS)
        .map(|i| {
            let start = i * RECOVERY_KEY_GROUP_LEN;
            let end = start + RECOVERY_KEY_GROUP_LEN;
            encoded[start..end].to_string()
        })
        .collect();
    groups.join("-")
}

pub fn parse_recovery_key(formatted: &str) -> Result<Vec<u8>, VaultError> {
    let cleaned: String = formatted.chars().filter(|c| *c != '-').collect();
    if cleaned.len() != RECOVERY_KEY_ENCODED_LEN {
        return Err(VaultError::InvalidRecoveryKey);
    }

    use data_encoding::BASE32_NOPAD;
    let decoded = BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| VaultError::InvalidRecoveryKey)?;

    Ok(decoded)
}

fn derive_recovery_key(password: &str, salt: &[u8; 16]) -> Result<[u8; 32], VaultError> {
    let params = Params::new(
        ARGON2_MEMORY,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(32),
    )
    .map_err(|e| VaultError::Crypto(format!("Argon2 params error: {}", e)))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, ARGON2_VERSION, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| VaultError::Crypto(format!("Argon2id derivation failed: {}", e)))?;
    Ok(key)
}
