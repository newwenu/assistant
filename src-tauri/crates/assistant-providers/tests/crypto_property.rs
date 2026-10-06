#![cfg(feature = "slow-tests")]

use assistant_providers::crypto::{decrypt_api_key, encrypt_api_key};
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
