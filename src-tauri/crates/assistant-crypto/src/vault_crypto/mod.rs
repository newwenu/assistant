use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};
use hkdf::Hkdf;
use log::warn;
use rand::Rng;
use sha2::Sha256;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};

pub mod totp;
pub mod types;

use crate::error::VaultError;
use types::{CipherAlgorithm, DbKey, Dek, Kek, MetaKey, PerEntryKey, VaultPlaintext};

const ARGON2_VERSION: Version = Version::V0x13;
const NONCE_SIZE: usize = 12;

const DEK_SIZE: usize = 20;
const KEK_SIZE: usize = 32;
const KEK_SALT_SIZE: usize = 16;
const WRAPPED_DEK_SIZE: usize = NONCE_SIZE + DEK_SIZE + 16;
const HKDF_INFO_DB_KEY_PREFIX: &str = "i9:v1:sqlcipher:";
const HKDF_INFO_ENTRY_KEY_PREFIX: &str = "i9:v1:entry:";
const HKDF_INFO_META_KEY: &[u8] = b"i9:v1:meta-integrity";
const SLOT_AAD_PREFIX: &[u8] = b"i9:v1:slot";
const ENTRY_FILE_AAD_PREFIX: &[u8] = b"i9:v1:entry-file";
const FIELD_AAD_PREFIX: &[u8] = b"i9:v1:field";

pub fn derive_kek(password: &str, salt: &[u8], m: u32, t: u32, p: u32) -> Result<Kek, VaultError> {
    if salt.len() != KEK_SALT_SIZE {
        return Err(VaultError::VaultCorrupted(format!(
            "KEK salt must be {} bytes, got {}",
            KEK_SALT_SIZE,
            salt.len()
        )));
    }
    if m < 8192 || t < 1 || p < 1 {
        return Err(VaultError::VaultCorrupted(format!(
            "Invalid Argon2id parameters: m={} t={} p={}",
            m, t, p
        )));
    }

    let params = Params::new(m, t, p, Some(KEK_SIZE))
        .map_err(|e| VaultError::VaultCorrupted(format!("Argon2 params error: {}", e)))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, ARGON2_VERSION, params);
    let mut out = [0u8; KEK_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| VaultError::VaultCorrupted(format!("Argon2id derivation failed: {}", e)))?;
    Ok(Kek::new(out))
}

pub fn derive_db_key(dek: &Dek, vault_id: &str) -> DbKey {
    assert!(!vault_id.is_empty(), "vault_id must not be empty");
    let hkdf = Hkdf::<Sha256>::new(None, dek.expose());
    let mut key = [0u8; 32];
    hkdf.expand(
        format!("{}{}", HKDF_INFO_DB_KEY_PREFIX, vault_id).as_bytes(),
        &mut key,
    )
    .expect("HKDF expand to 32 bytes should not fail");
    DbKey::new(key)
}

pub fn derive_entry_key(dek: &Dek, entry_id: &str) -> PerEntryKey {
    let hkdf = Hkdf::<Sha256>::new(None, dek.expose());
    let mut key = [0u8; 32];
    hkdf.expand(
        format!("{}{}", HKDF_INFO_ENTRY_KEY_PREFIX, entry_id).as_bytes(),
        &mut key,
    )
    .expect("HKDF expand to 32 bytes should not fail");
    PerEntryKey::new(key)
}

pub fn derive_meta_key(dek: &Dek) -> MetaKey {
    let hkdf = Hkdf::<Sha256>::new(None, dek.expose());
    let mut key = [0u8; 32];
    hkdf.expand(HKDF_INFO_META_KEY, &mut key)
        .expect("HKDF expand to 32 bytes should not fail");
    MetaKey::new(key)
}

pub fn seal_dek(kek: &Kek, dek: &Dek, aad: &[u8]) -> Result<Vec<u8>, VaultError> {
    let nonce_bytes: [u8; NONCE_SIZE] = rand::thread_rng().gen();
    let cipher = Aes256Gcm::new_from_slice(kek.expose())
        .map_err(|e| VaultError::VaultCorrupted(format!("AES-256-GCM init failed: {}", e)))?;
    let nonce = AesNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: dek.expose(),
                aad,
            },
        )
        .map_err(|e| VaultError::Crypto(format!("AES-256-GCM seal failed: {}", e)))?;

    let mut wrapped = Vec::with_capacity(WRAPPED_DEK_SIZE);
    wrapped.extend_from_slice(&nonce_bytes);
    wrapped.extend_from_slice(&ciphertext);
    debug_assert_eq!(wrapped.len(), WRAPPED_DEK_SIZE);
    Ok(wrapped)
}

pub fn open_dek(kek: &Kek, wrapped: &[u8], aad: &[u8]) -> Result<Dek, VaultError> {
    if wrapped.len() != WRAPPED_DEK_SIZE {
        return Err(VaultError::VaultCorrupted(format!(
            "wrapped dek must be {} bytes, got {}",
            WRAPPED_DEK_SIZE,
            wrapped.len()
        )));
    }

    let (nonce_bytes, ciphertext) = wrapped.split_at(NONCE_SIZE);
    let cipher = Aes256Gcm::new_from_slice(kek.expose())
        .map_err(|e| VaultError::VaultCorrupted(format!("AES-256-GCM init failed: {}", e)))?;
    let nonce = AesNonce::from_slice(nonce_bytes);
    // 解锁失败的判定歧义：GCM 解密失败无法区分「密码错误」与「密文损坏」，
    // 这里一律映射 `InvalidPassword`；仅当解密成功但明文长度异常时才映射
    // `VaultCorrupted`（见下方长度校验）。前端 `invalidPassword` 文案因此
    // 同时覆盖 `InvalidPassword` 与 `VaultCorrupted` 两种可能。
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| VaultError::InvalidPassword)?;

    let dek_bytes: [u8; DEK_SIZE] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| VaultError::VaultCorrupted("decrypted dek has wrong length".to_string()))?;
    Ok(Dek::new(dek_bytes))
}

pub fn build_slot_aad(
    payload_kind: &str,
    slot_type: &str,
    slot_id: &str,
    dek_version: u32,
) -> Vec<u8> {
    for (name, value) in [
        ("payload_kind", payload_kind),
        ("slot_type", slot_type),
        ("slot_id", slot_id),
    ] {
        assert!(!value.is_empty(), "{} must not be empty", name);
        assert!(
            value.is_ascii() && !value.contains('\0'),
            "{} must be printable ASCII without NUL",
            name
        );
        assert!(
            value.bytes().all(|b| (0x20..0x7f).contains(&b)),
            "{} must be printable ASCII",
            name
        );
    }

    let mut aad = Vec::with_capacity(SLOT_AAD_PREFIX.len() + 4 + 24);
    aad.extend_from_slice(SLOT_AAD_PREFIX);
    aad.push(0x00);
    aad.extend_from_slice(payload_kind.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(slot_type.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(slot_id.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(&(dek_version as u64).to_be_bytes());
    aad
}

fn push_aad_segment(aad: &mut Vec<u8>, segment: &str, name: &str) {
    assert!(!segment.is_empty(), "{} must not be empty", name);
    assert!(
        segment.is_ascii() && !segment.contains('\0'),
        "{} must be printable ASCII without NUL",
        name
    );
    aad.extend_from_slice(segment.as_bytes());
    aad.push(0x00);
}

/// 条目文件 AAD：`"i9:v1:entry-file" 0x00 id 0x00 category 0x00 be64(revision) be64(dekVersion)`。
///
/// 能力边界：AAD 只能阻止「改字段后仍通过认证」，**不能阻止整块旧文件重放**；
/// 重放由仓库 `manifest` + 本机 `anchor` 检测。
pub fn entry_file_aad(id: &str, category: &str, revision: i64, dek_version: u32) -> Vec<u8> {
    assert!(revision >= 0, "entry revision must be >= 0");
    let mut aad = Vec::with_capacity(ENTRY_FILE_AAD_PREFIX.len() + 4 + 24);
    aad.extend_from_slice(ENTRY_FILE_AAD_PREFIX);
    aad.push(0x00);
    push_aad_segment(&mut aad, id, "entry id");
    push_aad_segment(&mut aad, category, "category");
    aad.extend_from_slice(&(revision as u64).to_be_bytes());
    aad.extend_from_slice(&(dek_version as u64).to_be_bytes());
    aad
}

/// 字段 AAD：`"i9:v1:field" 0x00 entryId 0x00 fieldName`。
pub fn field_aad(entry_id: &str, field_name: &str) -> Vec<u8> {
    assert!(!field_name.is_empty(), "field name must not be empty");
    let mut aad =
        Vec::with_capacity(FIELD_AAD_PREFIX.len() + 2 + entry_id.len() + field_name.len());
    aad.extend_from_slice(FIELD_AAD_PREFIX);
    aad.push(0x00);
    push_aad_segment(&mut aad, entry_id, "entry id");
    aad.extend_from_slice(field_name.as_bytes());
    aad
}

fn aead_encrypt(
    key: &[u8; 32],
    cipher: CipherAlgorithm,
    nonce_bytes: &[u8; NONCE_SIZE],
    msg: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, VaultError> {
    match cipher {
        CipherAlgorithm::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| VaultError::Crypto(format!("AES-256-GCM init failed: {}", e)))?;
            cipher
                .encrypt(AesNonce::from_slice(nonce_bytes), Payload { msg, aad })
                .map_err(|e| VaultError::Crypto(format!("AES-256-GCM encrypt failed: {}", e)))
        }
        CipherAlgorithm::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key)
                .map_err(|e| VaultError::Crypto(format!("ChaCha20-Poly1305 init failed: {}", e)))?;
            cipher
                .encrypt(ChaChaNonce::from_slice(nonce_bytes), Payload { msg, aad })
                .map_err(|e| VaultError::Crypto(format!("ChaCha20-Poly1305 encrypt failed: {}", e)))
        }
    }
}

fn aead_decrypt(
    key: &[u8; 32],
    cipher: CipherAlgorithm,
    nonce_bytes: &[u8; NONCE_SIZE],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, VaultError> {
    match cipher {
        CipherAlgorithm::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| VaultError::Crypto(format!("AES-256-GCM init failed: {}", e)))?;
            cipher
                .decrypt(
                    AesNonce::from_slice(nonce_bytes),
                    Payload {
                        msg: ciphertext,
                        aad,
                    },
                )
                .map_err(|e| VaultError::Crypto(format!("AES-256-GCM decrypt failed: {}", e)))
        }
        CipherAlgorithm::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key)
                .map_err(|e| VaultError::Crypto(format!("ChaCha20-Poly1305 init failed: {}", e)))?;
            cipher
                .decrypt(
                    ChaChaNonce::from_slice(nonce_bytes),
                    Payload {
                        msg: ciphertext,
                        aad,
                    },
                )
                .map_err(|e| VaultError::Crypto(format!("ChaCha20-Poly1305 decrypt failed: {}", e)))
        }
    }
}

/// 条目文件密文布局：`b64(nonce(12) ‖ ct ‖ tag(16))`，算法只由 `EntryFileV2.cipher` 承载。
/// 传入的是已序列化的 `EntryBlobV2` 字节（crypto 层不依赖 vault 侧类型）。
pub fn encrypt_entry_blob(
    key: &PerEntryKey,
    plaintext: &VaultPlaintext,
    cipher: CipherAlgorithm,
    aad: &[u8],
) -> Result<String, VaultError> {
    let nonce_bytes: [u8; NONCE_SIZE] = rand::thread_rng().gen();
    let ciphertext = aead_encrypt(key.expose(), cipher, &nonce_bytes, plaintext.expose(), aad)?;

    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(B64.encode(&combined))
}

pub fn decrypt_entry_blob(
    key: &PerEntryKey,
    encrypted_b64: &str,
    cipher: CipherAlgorithm,
    aad: &[u8],
) -> Result<VaultPlaintext, VaultError> {
    let combined = B64
        .decode(encrypted_b64)
        .map_err(|e| VaultError::Crypto(format!("Base64 decode failed: {}", e)))?;

    if combined.len() < NONCE_SIZE + 16 {
        warn!(
            "Vault decrypt failed: encrypted blob too short ({} bytes)",
            combined.len()
        );
        return Err(VaultError::Crypto("Encrypted data too short".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let plaintext = aead_decrypt(
        key.expose(),
        cipher,
        nonce_bytes.try_into().expect("split_at NONCE_SIZE"),
        ciphertext,
        aad,
    )?;
    Ok(VaultPlaintext::from(plaintext))
}

/// 字段级密文布局：`b64(algoByte ‖ nonce(12) ‖ ct ‖ tag(16))`。
/// 字段密文存在本机库列里，需要自描述算法，故保留首字节。
pub fn encrypt_field(
    key: &PerEntryKey,
    plaintext: &VaultPlaintext,
    cipher: CipherAlgorithm,
    aad: &[u8],
) -> Result<String, VaultError> {
    let nonce_bytes: [u8; NONCE_SIZE] = rand::thread_rng().gen();
    let ciphertext = aead_encrypt(key.expose(), cipher, &nonce_bytes, plaintext.expose(), aad)?;

    let mut combined = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
    combined.push(cipher.discriminant());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(B64.encode(&combined))
}

pub fn decrypt_field(
    key: &PerEntryKey,
    encrypted_b64: &str,
    aad: &[u8],
) -> Result<VaultPlaintext, VaultError> {
    let combined = B64
        .decode(encrypted_b64)
        .map_err(|e| VaultError::Crypto(format!("Base64 decode failed: {}", e)))?;

    if combined.len() < 1 + NONCE_SIZE + 16 {
        warn!(
            "Vault decrypt failed: encrypted data too short ({} bytes)",
            combined.len()
        );
        return Err(VaultError::Crypto("Encrypted data too short".to_string()));
    }

    let cipher = CipherAlgorithm::from_discriminant(combined[0])
        .map_err(|e| VaultError::Crypto(format!("Unknown cipher: {}", e)))?;
    let (nonce_bytes, ciphertext) = combined[1..].split_at(NONCE_SIZE);
    let plaintext = aead_decrypt(
        key.expose(),
        cipher,
        nonce_bytes.try_into().expect("split_at NONCE_SIZE"),
        ciphertext,
        aad,
    )?;
    Ok(VaultPlaintext::from(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_slot_aad_matches_spec_byte_sequence() {
        let aad = build_slot_aad("dek", "password", "pwd-3", 7);

        let mut expected = Vec::new();
        expected.extend_from_slice(b"i9:v1:slot");
        expected.push(0x00);
        expected.extend_from_slice(b"dek");
        expected.push(0x00);
        expected.extend_from_slice(b"password");
        expected.push(0x00);
        expected.extend_from_slice(b"pwd-3");
        expected.push(0x00);
        expected.extend_from_slice(&7u64.to_be_bytes());

        assert_eq!(aad, expected);
        assert_eq!(aad.len(), 10 + 1 + 3 + 1 + 8 + 1 + 5 + 1 + 8);
    }

    #[test]
    #[should_panic(expected = "slot_id must not be empty")]
    fn build_slot_aad_rejects_empty_segment() {
        build_slot_aad("dek", "password", "", 1);
    }

    #[test]
    fn seal_open_dek_roundtrip() {
        let kek = Kek::new([0x11; 32]);
        let dek = Dek::new([0x22; 20]);
        let aad = build_slot_aad("dek", "password", "pwd-1", 1);

        let wrapped = seal_dek(&kek, &dek, &aad).unwrap();
        assert_eq!(wrapped.len(), 48, "wrappedDek 必须恰好 48 字节");

        let opened = open_dek(&kek, &wrapped, &aad).unwrap();
        assert!(opened.ct_eq(&dek));
    }

    #[test]
    fn open_dek_rejects_wrong_aad() {
        let kek = Kek::new([0x11; 32]);
        let dek = Dek::new([0x22; 20]);
        let aad = build_slot_aad("dek", "password", "pwd-1", 1);
        let other_aad = build_slot_aad("dek", "password", "pwd-1", 2);

        let wrapped = seal_dek(&kek, &dek, &aad).unwrap();
        assert!(matches!(
            open_dek(&kek, &wrapped, &other_aad),
            Err(VaultError::InvalidPassword)
        ));
    }

    #[test]
    fn open_dek_rejects_wrong_wrapper_length() {
        let kek = Kek::new([0x11; 32]);
        let aad = build_slot_aad("dek", "password", "pwd-1", 1);

        assert!(matches!(
            open_dek(&kek, &[0u8; 47], &aad),
            Err(VaultError::VaultCorrupted(_))
        ));
    }

    #[test]
    fn open_dek_rejects_wrong_kek() {
        let aad = build_slot_aad("dek", "password", "pwd-1", 1);
        let wrapped = seal_dek(&Kek::new([0x11; 32]), &Dek::new([0x22; 20]), &aad).unwrap();

        assert!(matches!(
            open_dek(&Kek::new([0x33; 32]), &wrapped, &aad),
            Err(VaultError::InvalidPassword)
        ));
    }

    #[test]
    fn derive_kek_rejects_bad_salt_length() {
        assert!(matches!(
            derive_kek("pw", &[0u8; 8], 65536, 3, 4),
            Err(VaultError::VaultCorrupted(_))
        ));
    }

    #[test]
    fn derive_kek_rejects_invalid_params() {
        let salt = [0u8; 16];
        assert!(matches!(
            derive_kek("pw", &salt, 4096, 3, 4),
            Err(VaultError::VaultCorrupted(_))
        ));
        assert!(matches!(
            derive_kek("pw", &salt, 65536, 0, 4),
            Err(VaultError::VaultCorrupted(_))
        ));
        assert!(matches!(
            derive_kek("pw", &salt, 65536, 3, 0),
            Err(VaultError::VaultCorrupted(_))
        ));
    }

    #[test]
    fn derive_kek_is_deterministic_per_salt() {
        let salt = [0x5A; 16];
        let a = derive_kek("pw", &salt, 8192, 1, 1).unwrap();
        let b = derive_kek("pw", &salt, 8192, 1, 1).unwrap();
        assert!(a.ct_eq(&b));

        let c = derive_kek("pw", &[0x5B; 16], 8192, 1, 1).unwrap();
        assert!(!a.ct_eq(&c), "不同 salt 必须派生出不同 KEK");
    }

    #[test]
    fn derive_db_key_binds_vault_id() {
        let dek = Dek::new([0x44; 20]);
        let a = derive_db_key(&dek, "vault-a");
        let b = derive_db_key(&dek, "vault-b");
        assert!(!a.ct_eq(&b), "db_key 必须绑定 vault_id");
    }

    #[test]
    fn derive_entry_key_binds_entry_id_and_uses_prefix() {
        let dek = Dek::new([0x55; 20]);
        let a = derive_entry_key(&dek, "entry-a");
        let b = derive_entry_key(&dek, "entry-b");
        assert!(!a.ct_eq(&b));
    }

    #[test]
    fn derive_meta_key_is_stable() {
        let dek = Dek::new([0x66; 20]);
        let a = derive_meta_key(&dek);
        let b = derive_meta_key(&dek);
        assert!(a.ct_eq(&b));
    }
}
