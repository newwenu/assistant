use assistant_security::crypto::{
    decrypt_api_key, decrypt_api_key_b64, encrypt_api_key, encrypt_api_key_b64,
};

fn mid() -> Vec<u8> {
    vec![42u8; 32]
}

#[test]
fn encrypt_decrypt_roundtrip() {
    let m = mid();
    for key in [
        "",
        "sk-abc123",
        "sk-密钥-🔑-test",
        &("sk-".to_string() + &"a".repeat(10000)),
    ] {
        let enc = encrypt_api_key(key, &m).unwrap();
        assert!(enc.len() > key.len());
        assert_eq!(decrypt_api_key(&enc, &m).unwrap(), key);
    }
}

#[test]
fn decrypt_invalid_input_returns_error() {
    let m = mid();
    assert!(decrypt_api_key(&[], &m).is_err());
    assert!(decrypt_api_key(&[0u8; 11], &m).is_err());
    let mut corrupted = encrypt_api_key("test", &m).unwrap();
    corrupted[15] ^= 0xFF;
    assert!(decrypt_api_key(&corrupted, &m).is_err());
}

#[test]
fn wrong_machine_id_returns_error() {
    let m1 = mid();
    let m2 = vec![99u8; 32];
    let enc = encrypt_api_key("test", &m1).unwrap();
    assert!(decrypt_api_key(&enc, &m2).is_err());
}

#[test]
fn b64_roundtrip_and_invalid() {
    let m = mid();
    let enc = encrypt_api_key_b64("test", &m).unwrap();
    assert_eq!(decrypt_api_key_b64(&enc, &m).unwrap(), "test");
    assert!(decrypt_api_key_b64("!!!not-base64!!!", &m).is_err());
}

#[test]
fn each_encryption_produces_different_ciphertext() {
    let m = mid();
    assert_ne!(
        encrypt_api_key("same", &m).unwrap(),
        encrypt_api_key("same", &m).unwrap()
    );
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn encrypt_decrypt_roundtrip_arbitrary(key in ".{0,200}") {
            let mid = vec![42u8; 32];
            let encrypted = encrypt_api_key(&key, &mid).unwrap();
            assert_eq!(decrypt_api_key(&encrypted, &mid).unwrap(), key);
        }

        #[test]
        fn ciphertext_min_12_bytes(key in ".{0,10}") {
            let mid = vec![42u8; 32];
            assert!(encrypt_api_key(&key, &mid).unwrap().len() >= 12);
        }
    }
}
