mod common;

use assistant_vault::security_db::SecurityDbState;
#[cfg(feature = "slow-tests")]
use assistant_vault::types::VaultError;
#[cfg(feature = "slow-tests")]
use common::{
    cleanup, initialize_at, repo_for, temp_db_path, try_initialize_at, unlock_at, STRONG_PW,
};

#[cfg(feature = "slow-tests")]
#[test]
fn init_creates_database() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let result = try_initialize_at(&db, STRONG_PW);
    assert!(result.is_ok(), "init should succeed: {:?}", result);
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn init_twice_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);
    let result = try_initialize_at(&db, STRONG_PW);
    assert!(matches!(result, Err(VaultError::AlreadyExists)));
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn unlock_after_init_succeeds() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);
    db.lock().unwrap();
    let result = unlock_at(&db, STRONG_PW);
    assert!(result.is_ok(), "unlock should succeed: {:?}", result);
    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn unlock_wrong_password_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);
    db.lock().unwrap();
    let result = unlock_at(&db, "Wr0ng#P@ss!9999");
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

    initialize_at(&db, STRONG_PW);
    assert!(!db.is_locked(), "should be unlocked after init");

    db.lock().unwrap();
    assert!(db.is_locked(), "should be locked after lock");

    unlock_at(&db, STRONG_PW).unwrap();
    assert!(!db.is_locked(), "should be unlocked after unlock");

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn get_dek_locked_fails() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let result = db.get_dek();
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
    initialize_at(&db, STRONG_PW);
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
    initialize_at(&db, STRONG_PW);

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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);

    for _ in 0..7 {
        db.backup(&path).unwrap();
    }

    let backup_dir = dir.path().join("backups");
    let count = std::fs::read_dir(&backup_dir).unwrap().count();
    assert_eq!(count, 5, "should keep exactly 5 backups, got {}", count);
}

#[cfg(feature = "slow-tests")]
#[test]
fn backup_rejects_relative_or_parentless_db_path() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);

    let relative = std::path::Path::new("relative").join("security.db");
    assert!(
        db.backup(&relative).is_err(),
        "相对路径必须拒绝，避免 backups/ 落到 cwd"
    );
    assert!(!std::path::Path::new("relative").join("backups").exists());

    let parentless = std::path::Path::new("security.db");
    assert!(db.backup(parentless).is_err(), "无父目录的路径必须拒绝");
    assert!(!std::path::Path::new("backups").exists());

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn corrupted_ciphertext_reports_entry_corrupted() {
    use assistant_crypto::vault_crypto::types::CipherAlgorithm;
    use assistant_vault::dto::VaultEntryCreate;
    use assistant_vault::types::VaultCategory;

    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);

    let id = db
        .create_entry(
            VaultEntryCreate {
                category: VaultCategory::Login,
                title: "Corrupt Me".to_string(),
                username: None,
                url: None,
                password: Some("s3cret".to_string()),
                notes: Some("note".to_string()),
                totp: None,
                tags: vec![],
                custom_fields: vec![],
                is_favorite: false,
            },
            CipherAlgorithm::Aes256Gcm,
        )
        .unwrap();

    {
        let guard = db.get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute(
            "UPDATE vault_entries SET encrypted_password = ?1 WHERE id = ?2",
            rusqlite::params!["not-a-valid-ciphertext", &id],
        )
        .unwrap();
    }

    let password = db.decrypt_password(&id);
    assert!(
        matches!(
            password,
            Err(VaultError::EntryCorrupted(_)) | Err(VaultError::Crypto(_))
        ),
        "篡改密文必须报错，不得静默返回空值"
    );

    let detail = db.get_entry(&id);
    assert!(
        matches!(detail, Err(VaultError::EntryCorrupted(_))),
        "整条解密失败必须报 EntryCorrupted"
    );

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn change_master_password_purges_old_backups() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, STRONG_PW);

    let backup_path = db.backup(&path).unwrap();
    assert!(backup_path.exists());
    let backup_dir = path.parent().unwrap().join("backups");
    assert_eq!(std::fs::read_dir(&backup_dir).unwrap().count(), 1);

    assistant_vault::lifecycle::change_password(
        &db,
        &repo_for(&path),
        STRONG_PW,
        "N3w#P@ssw0rd!2026",
    )
    .unwrap();

    let remaining = std::fs::read_dir(&backup_dir).unwrap().count();
    assert_eq!(remaining, 0, "改密后不得残留任何旧备份");

    cleanup(&path);
}

#[test]
fn cipher_algorithm_from_conf_is_case_insensitive() {
    use assistant_crypto::vault_crypto::types::CipherAlgorithm;

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
