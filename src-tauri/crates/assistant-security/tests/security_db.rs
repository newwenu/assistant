mod common;

use assistant_security::security_db::SecurityDbState;
use assistant_security::types::VaultError;
use common::{cleanup, temp_db_path, STRONG_PW};

#[cfg(feature = "slow-tests")]
#[test]
fn init_creates_database() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let result = db.init(STRONG_PW);
    assert!(result.is_ok(), "init should succeed: {:?}", result);
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn init_twice_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();
    let result = db.init(STRONG_PW);
    assert!(matches!(result, Err(VaultError::AlreadyExists)));
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn unlock_after_init_succeeds() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();
    db.lock().unwrap();
    let result = db.unlock(STRONG_PW);
    assert!(result.is_ok(), "unlock should succeed: {:?}", result);
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn unlock_wrong_password_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();
    db.lock().unwrap();
    let result = db.unlock("Wr0ng#P@ss!9999");
    assert!(matches!(result, Err(VaultError::InvalidPassword)));
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn is_locked_state_transitions() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());

    assert!(db.is_locked(), "should start locked");

    db.init(STRONG_PW).unwrap();
    assert!(!db.is_locked(), "should be unlocked after init");

    db.lock().unwrap();
    assert!(db.is_locked(), "should be locked after lock");

    db.unlock(STRONG_PW).unwrap();
    assert!(!db.is_locked(), "should be unlocked after unlock");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn get_master_key_locked_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let result = db.get_master_key();
    assert!(matches!(result, Err(VaultError::Locked)));
}

#[cfg(feature = "slow-tests")]
#[test]
fn get_conn_locked_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let result = db.get_conn();
    assert!(matches!(result, Err(VaultError::Locked)));
}

#[cfg(feature = "slow-tests")]
#[test]
fn lock_is_idempotent() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();
    db.lock().unwrap();
    let result = db.lock();
    assert!(result.is_ok(), "lock on already-locked should be ok");
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn backup_creates_file() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();

    let result = db.backup(&path);
    assert!(result.is_ok(), "backup should succeed: {:?}", result);
    let backup_path = result.unwrap();
    assert!(backup_path.exists(), "backup file should exist");
    assert!(
        backup_path.to_string_lossy().contains("backups"),
        "backup should live under backups/ directory"
    );

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn backup_keeps_latest_five() {
    let dir = std::env::temp_dir().join(format!(
        "assistant-security-test-rotation-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("vault.db");
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(STRONG_PW).unwrap();

    for _ in 0..7 {
        db.backup(&path).unwrap();
    }

    let backup_dir = dir.join("backups");
    let count = std::fs::read_dir(&backup_dir).unwrap().count();
    assert_eq!(count, 5, "should keep exactly 5 backups, got {}", count);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cipher_algorithm_from_conf_is_case_insensitive() {
    use assistant_security::vault_crypto::types::CipherAlgorithm;

    assert_eq!(
        SecurityDbState::cipher_algorithm_from_conf("chacha20Poly1305"),
        CipherAlgorithm::ChaCha20Poly1305
    );
    assert_eq!(
        SecurityDbState::cipher_algorithm_from_conf("CHACHA20POLY1305"),
        CipherAlgorithm::ChaCha20Poly1305
    );
    assert_eq!(
        SecurityDbState::cipher_algorithm_from_conf("aes256Gcm"),
        CipherAlgorithm::Aes256Gcm
    );
    assert_eq!(
        SecurityDbState::cipher_algorithm_from_conf(""),
        CipherAlgorithm::Aes256Gcm
    );
}
