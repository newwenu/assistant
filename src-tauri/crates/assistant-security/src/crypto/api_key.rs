use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use log::warn;
use rand::Rng;
use sha2::{Digest, Sha256};

pub fn get_machine_id() -> Result<Vec<u8>, String> {
    let uid = machine_uid::get().map_err(|e| format!("Failed to get machine uid: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(uid.as_bytes());
    let hash = hasher.finalize();
    Ok(hash.to_vec())
}

pub fn encrypt_api_key(key: &str, machine_id: &[u8]) -> Result<Vec<u8>, String> {
    let cipher =
        Aes256Gcm::new_from_slice(machine_id).map_err(|e| format!("Invalid key length: {}", e))?;
    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, key.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt_api_key(encrypted: &[u8], machine_id: &[u8]) -> Result<String, String> {
    if encrypted.len() < 12 {
        warn!(
            "API key decrypt failed: encrypted data too short ({} bytes)",
            encrypted.len()
        );
        return Err("Encrypted data too short".to_string());
    }
    let cipher =
        Aes256Gcm::new_from_slice(machine_id).map_err(|e| format!("Invalid key length: {}", e))?;
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
        warn!("API key decryption failed: {}", e);
        format!("Decryption failed: {}", e)
    })?;
    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e))
}

pub fn encrypt_api_key_b64(key: &str, machine_id: &[u8]) -> Result<String, String> {
    let encrypted = encrypt_api_key(key, machine_id)?;
    Ok(B64.encode(&encrypted))
}

pub fn decrypt_api_key_b64(encrypted_b64: &str, machine_id: &[u8]) -> Result<String, String> {
    let encrypted = B64
        .decode(encrypted_b64)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;
    decrypt_api_key(&encrypted, machine_id)
}
