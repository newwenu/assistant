mod common;

use assistant_security::security_db::SecurityDbState;
use assistant_security::types::VaultError;
use common::{temp_db_path, STRONG_PW};

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

    let backup_dir = std::env::temp_dir().join("assistant-security-test-backup");
    let _ = std::fs::create_dir_all(&backup_dir);
    let backup_path = backup_dir.join(format!("backup_{}.db", uuid::Uuid::new_v4()));

    let result = db.backup(&backup_path);
    assert!(result.is_ok(), "backup should succeed: {:?}", result);
    assert!(backup_path.exists(), "backup file should exist");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&backup_path);
}
