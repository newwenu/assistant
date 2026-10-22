use assistant_security::vault_crypto::types::{
    CipherAlgorithm, MasterKey, PerEntryKey, VaultPlaintext,
};
use assistant_security::vault_crypto::VaultCrypto;
use zeroize::Zeroize;

#[test]
fn derive_master_key_deterministic() {
    let salt = [0xABu8; 32];
    let key1 = VaultCrypto::derive_master_key("test_password", &salt).unwrap();
    let key2 = VaultCrypto::derive_master_key("test_password", &salt).unwrap();
    assert_eq!(key1, key2);
}

#[test]
fn derive_master_key_different_passwords() {
    let salt = [0xABu8; 32];
    let key1 = VaultCrypto::derive_master_key("password1", &salt).unwrap();
    let key2 = VaultCrypto::derive_master_key("password2", &salt).unwrap();
    assert_ne!(key1, key2);
}

#[test]
fn derive_master_key_different_salts() {
    let salt1 = [0x01u8; 32];
    let salt2 = [0x02u8; 32];
    let key1 = VaultCrypto::derive_master_key("same_password", &salt1).unwrap();
    let key2 = VaultCrypto::derive_master_key("same_password", &salt2).unwrap();
    assert_ne!(key1, key2);
}

#[test]
fn verify_master_password_correct() {
    let hash_salt = [0xCDu8; 32];
    let hash = VaultCrypto::derive_master_key("correct_password", &hash_salt).unwrap();
    let valid = VaultCrypto::verify_master_password("correct_password", &hash, &hash_salt).unwrap();
    assert!(valid);
}

#[test]
fn verify_master_password_incorrect() {
    let hash_salt = [0xCDu8; 32];
    let hash = VaultCrypto::derive_master_key("correct_password", &hash_salt).unwrap();
    let valid = VaultCrypto::verify_master_password("wrong_password", &hash, &hash_salt).unwrap();
    assert!(!valid);
}

#[test]
fn derive_entry_key_deterministic() {
    let master_bytes = VaultCrypto::derive_master_key("password", &[0u8; 32]).unwrap();
    let master_key = MasterKey::new(master_bytes);
    let entry_key1 = VaultCrypto::derive_entry_key(&master_key, "entry-001");
    let entry_key2 = VaultCrypto::derive_entry_key(&master_key, "entry-001");
    assert_eq!(entry_key1.expose(), entry_key2.expose());
}

#[test]
fn derive_entry_key_different_ids() {
    let master_bytes = VaultCrypto::derive_master_key("password", &[0u8; 32]).unwrap();
    let master_key = MasterKey::new(master_bytes);
    let entry_key1 = VaultCrypto::derive_entry_key(&master_key, "entry-001");
    let entry_key2 = VaultCrypto::derive_entry_key(&master_key, "entry-002");
    assert_ne!(entry_key1.expose(), entry_key2.expose());
}

#[test]
fn derive_sqlcipher_key_deterministic() {
    let master_bytes = VaultCrypto::derive_master_key("password", &[0u8; 32]).unwrap();
    let master_key = MasterKey::new(master_bytes);
    let key1 = VaultCrypto::derive_sqlcipher_key(&master_key);
    let key2 = VaultCrypto::derive_sqlcipher_key(&master_key);
    assert_eq!(key1, key2);
}

#[test]
fn encrypt_decrypt_aes256gcm_roundtrip() {
    let entry_key_bytes = [0x42u8; 32];
    let plaintext = VaultPlaintext::from_str("hello world");
    let encrypted =
        VaultCrypto::encrypt_field(&plaintext, &entry_key_bytes, CipherAlgorithm::Aes256Gcm)
            .unwrap();
    let decrypted = VaultCrypto::decrypt_field(&encrypted, &entry_key_bytes).unwrap();
    assert_eq!(plaintext.expose(), decrypted.expose());
}

#[test]
fn encrypt_decrypt_chacha20poly1305_roundtrip() {
    let entry_key_bytes = [0x42u8; 32];
    let plaintext = VaultPlaintext::from_str("hello world");
    let encrypted = VaultCrypto::encrypt_field(
        &plaintext,
        &entry_key_bytes,
        CipherAlgorithm::ChaCha20Poly1305,
    )
    .unwrap();
    let decrypted = VaultCrypto::decrypt_field(&encrypted, &entry_key_bytes).unwrap();
    assert_eq!(plaintext.expose(), decrypted.expose());
}

#[test]
fn encrypt_field_algo_byte_in_output() {
    let entry_key_bytes = [0x42u8; 32];
    let plaintext = VaultPlaintext::from_str("test");
    let encrypted_aes =
        VaultCrypto::encrypt_field(&plaintext, &entry_key_bytes, CipherAlgorithm::Aes256Gcm)
            .unwrap();
    let encrypted_chacha = VaultCrypto::encrypt_field(
        &plaintext,
        &entry_key_bytes,
        CipherAlgorithm::ChaCha20Poly1305,
    )
    .unwrap();

    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    let aes_bytes = B64.decode(&encrypted_aes).unwrap();
    let chacha_bytes = B64.decode(&encrypted_chacha).unwrap();

    assert_eq!(aes_bytes[0], CipherAlgorithm::Aes256Gcm as u8);
    assert_eq!(chacha_bytes[0], CipherAlgorithm::ChaCha20Poly1305 as u8);
}

#[test]
fn decrypt_wrong_key_fails() {
    let entry_key_bytes = [0x42u8; 32];
    let wrong_key_bytes = [0x43u8; 32];
    let plaintext = VaultPlaintext::from_str("secret data");
    let encrypted =
        VaultCrypto::encrypt_field(&plaintext, &entry_key_bytes, CipherAlgorithm::Aes256Gcm)
            .unwrap();
    let result = VaultCrypto::decrypt_field(&encrypted, &wrong_key_bytes);
    assert!(result.is_err());
}

#[test]
fn vault_plaintext_debug_redacted() {
    let pt = VaultPlaintext::from_str("sensitive");
    let debug_str = format!("{:?}", pt);
    assert!(debug_str.contains("REDACTED"));
    assert!(!debug_str.contains("sensitive"));
}

#[test]
fn vault_plaintext_from_vec_zeroes_original() {
    let original = vec![0xDEu8; 16];
    let original_ptr = original.as_ptr();
    let _pt = VaultPlaintext::from(original);
    unsafe {
        let slice = std::slice::from_raw_parts(original_ptr, 16);
        assert!(slice.iter().all(|&b| b == 0));
    }
}

#[test]
fn master_key_zeroize_on_drop() {
    let bytes = [0xFFu8; 32];
    let mut key = MasterKey::new(bytes);
    key.zeroize();
    assert!(key.expose().iter().all(|&b| b == 0));
}

#[test]
fn per_entry_key_zeroize_on_drop() {
    let bytes = [0xFFu8; 32];
    let mut key = PerEntryKey::new(bytes);
    key.zeroize();
    assert!(key.expose().iter().all(|&b| b == 0));
}

#[test]
fn cipher_algorithm_discriminant_consistency() {
    assert_eq!(CipherAlgorithm::Aes256Gcm.discriminant(), 0);
    assert_eq!(CipherAlgorithm::ChaCha20Poly1305.discriminant(), 1);
    assert_eq!(
        CipherAlgorithm::from_discriminant(0).unwrap(),
        CipherAlgorithm::Aes256Gcm
    );
    assert_eq!(
        CipherAlgorithm::from_discriminant(1).unwrap(),
        CipherAlgorithm::ChaCha20Poly1305
    );
    assert!(CipherAlgorithm::from_discriminant(2).is_err());
}
