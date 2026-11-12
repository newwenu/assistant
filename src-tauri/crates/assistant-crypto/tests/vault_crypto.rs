use assistant_crypto::vault_crypto::types::{CipherAlgorithm, PerEntryKey, VaultPlaintext};
use zeroize::Zeroize;

#[test]
fn vault_plaintext_debug_redacted() {
    let pt = VaultPlaintext::new_str("sensitive");
    let debug_str = format!("{:?}", pt);
    assert!(debug_str.contains("REDACTED"));
    assert!(!debug_str.contains("sensitive"));
}

#[test]
fn vault_plaintext_from_vec_preserves_data() {
    let data = vec![0xDEu8; 16];
    let pt = VaultPlaintext::from(data);
    assert_eq!(pt.expose(), &[0xDEu8; 16]);
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
