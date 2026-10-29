use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};
use hkdf::Hkdf;
use log::warn;
use rand::Rng;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};

pub mod totp;
pub mod types;

use types::{CipherAlgorithm, MasterKey, PerEntryKey, VaultPlaintext};

const ARGON2_MEMORY: u32 = 65536;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_VERSION: Version = Version::V0x13;
const HKDF_INFO_SQLCIPHER: &[u8] = b"sqlcipher";
const NONCE_SIZE: usize = 12;

pub struct VaultCrypto;

impl VaultCrypto {
    pub fn argon2_params() -> (u32, u32, u32) {
        (ARGON2_MEMORY, ARGON2_ITERATIONS, ARGON2_PARALLELISM)
    }

    pub fn derive_master_key(password: &str, salt: &[u8; 32]) -> Result<[u8; 32], String> {
        let params = Params::new(
            ARGON2_MEMORY,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(32),
        )
        .map_err(|e| format!("Argon2 params error: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, ARGON2_VERSION, params);
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| format!("Argon2id derivation failed: {}", e))?;
        Ok(key)
    }

    pub fn verify_master_password(
        password: &str,
        stored_hash: &[u8],
        hash_salt: &[u8; 32],
    ) -> Result<bool, String> {
        let mut derived = Self::derive_master_key(password, hash_salt)?;
        let stored_arr: [u8; 32] = stored_hash
            .try_into()
            .map_err(|_| "Stored hash must be 32 bytes".to_string())?;
        let eq = derived.ct_eq(&stored_arr).into();
        derived.zeroize();
        Ok(eq)
    }

    pub fn derive_entry_key(master_key: &MasterKey, entry_id: &str) -> PerEntryKey {
        let hkdf = Hkdf::<Sha256>::new(None, master_key.expose());
        let mut key = [0u8; 32];
        hkdf.expand(entry_id.as_bytes(), &mut key)
            .expect("HKDF expand to 32 bytes should not fail");
        PerEntryKey::new(key)
    }

    pub fn derive_sqlcipher_key(master_key: &MasterKey) -> [u8; 32] {
        let hkdf = Hkdf::<Sha256>::new(None, master_key.expose());
        let mut key = [0u8; 32];
        hkdf.expand(HKDF_INFO_SQLCIPHER, &mut key)
            .expect("HKDF expand to 32 bytes should not fail");
        key
    }

    pub fn encrypt_field(
        plaintext: &VaultPlaintext,
        entry_key: &[u8; 32],
        algo: CipherAlgorithm,
    ) -> Result<String, String> {
        let nonce_bytes: [u8; NONCE_SIZE] = rand::thread_rng().gen();
        let algo_byte = algo.discriminant();

        let ciphertext = match algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(entry_key)
                    .map_err(|e| format!("AES-256-GCM init failed: {}", e))?;
                let nonce = AesNonce::from_slice(&nonce_bytes);
                cipher
                    .encrypt(nonce, plaintext.expose())
                    .map_err(|e| format!("AES-256-GCM encrypt failed: {}", e))?
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(entry_key)
                    .map_err(|e| format!("ChaCha20-Poly1305 init failed: {}", e))?;
                let nonce = ChaChaNonce::from_slice(&nonce_bytes);
                cipher
                    .encrypt(nonce, plaintext.expose())
                    .map_err(|e| format!("ChaCha20-Poly1305 encrypt failed: {}", e))?
            }
        };

        let mut combined = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
        combined.push(algo_byte);
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(B64.encode(&combined))
    }

    pub fn decrypt_field(
        encrypted_b64: &str,
        entry_key: &[u8; 32],
    ) -> Result<VaultPlaintext, String> {
        let combined = B64
            .decode(encrypted_b64)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        if combined.len() < 1 + NONCE_SIZE {
            warn!(
                "Vault decrypt failed: encrypted data too short ({} bytes)",
                combined.len()
            );
            return Err("Encrypted data too short".to_string());
        }

        let algo_byte = combined[0];
        let algo = CipherAlgorithm::from_discriminant(algo_byte)?;
        let nonce_bytes = &combined[1..1 + NONCE_SIZE];
        let ciphertext = &combined[1 + NONCE_SIZE..];

        let plaintext = match algo {
            CipherAlgorithm::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(entry_key)
                    .map_err(|e| format!("AES-256-GCM init failed: {}", e))?;
                let nonce = AesNonce::from_slice(nonce_bytes);
                cipher.decrypt(nonce, ciphertext).map_err(|e| {
                    warn!("AES-256-GCM decrypt failed: {}", e);
                    format!("AES-256-GCM decrypt failed: {}", e)
                })?
            }
            CipherAlgorithm::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(entry_key)
                    .map_err(|e| format!("ChaCha20-Poly1305 init failed: {}", e))?;
                let nonce = ChaChaNonce::from_slice(nonce_bytes);
                cipher.decrypt(nonce, ciphertext).map_err(|e| {
                    warn!("ChaCha20-Poly1305 decrypt failed: {}", e);
                    format!("ChaCha20-Poly1305 decrypt failed: {}", e)
                })?
            }
        };

        Ok(VaultPlaintext::from(plaintext))
    }
}
