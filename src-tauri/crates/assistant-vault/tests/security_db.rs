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

/// 备份文件名前缀是 `%Y%m%d-%H%M%S-%3f`，升序即「最旧在前」（`E8` 裁剪依据）。
fn backup_path(db_path: &std::path::Path, stamp: &str) -> std::path::PathBuf {
    db_path
        .parent()
        .unwrap()
        .join("backups")
        .join(format!("security-{stamp}.db.bak"))
}

fn write_backup(db_path: &std::path::Path, stamp: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = backup_path(db_path, stamp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    path
}

fn vault_db_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("vault.db");
    (dir, db_path)
}

#[test]
fn prune_backups_to_keeps_the_newest_and_ignores_other_files() {
    let (_dir, db_path) = vault_db_path();
    let oldest = write_backup(&db_path, "20240101-000000-001", b"0123456789");
    let middle = write_backup(&db_path, "20240102-000000-002", b"01234567890123456789");
    let newest = write_backup(
        &db_path,
        "20240103-000000-003",
        b"012345678901234567890123456789",
    );
    let unrelated = db_path.parent().unwrap().join("notes.txt");
    std::fs::write(&unrelated, b"not a backup").unwrap();

    let report = SecurityDbState::prune_backups_to(&db_path, 1).unwrap();

    assert_eq!(report.removed, 2, "只裁掉最旧的超限部分");
    assert_eq!(report.removed_bytes, 30, "按实际文件长度累加");
    assert_eq!(report.failed, 0);
    assert!(!oldest.exists());
    assert!(!middle.exists());
    assert!(newest.exists(), "最新一份在任何情况下都不得删");
    assert!(unrelated.exists(), "非备份文件不参与裁剪");
}

#[test]
fn prune_backups_to_keeps_one_copy_even_when_keep_is_zero() {
    let (_dir, db_path) = vault_db_path();
    write_backup(&db_path, "20240101-000000-001", b"aa");
    write_backup(&db_path, "20240102-000000-002", b"bb");
    let newest = write_backup(&db_path, "20240103-000000-003", b"cc");

    let report = SecurityDbState::prune_backups_to(&db_path, 0).unwrap();

    assert_eq!(report.removed, 2, "keep 至少 1：只裁到一份");
    assert_eq!(report.removed_bytes, 4);
    assert!(newest.exists());
    let remaining: Vec<_> = std::fs::read_dir(newest.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(remaining.len(), 1, "keep = 0 时目录里只剩一份备份");
}

#[test]
fn prune_backups_to_is_a_noop_within_keep_or_without_a_backup_dir() {
    let (_dir, db_path) = vault_db_path();
    let empty = SecurityDbState::prune_backups_to(&db_path, 3).unwrap();
    assert_eq!(empty, assistant_vault::security_db::PruneReport::default());

    write_backup(&db_path, "20240101-000000-001", b"aa");
    let newest = write_backup(&db_path, "20240102-000000-002", b"bb");

    let within = SecurityDbState::prune_backups_to(&db_path, 2).unwrap();
    assert_eq!(within.removed, 0, "未超限时不得删任何备份");
    assert!(newest.exists());
    assert!(backup_path(&db_path, "20240101-000000-001").exists());
}

#[cfg(windows)]
#[test]
fn prune_backups_to_counts_failure_and_keeps_the_extra_copy() {
    use std::os::windows::fs::OpenOptionsExt;

    let (_dir, db_path) = vault_db_path();
    let oldest = write_backup(&db_path, "20240101-000000-001", b"aa");
    let middle = write_backup(&db_path, "20240102-000000-002", b"bb");
    let newest = write_backup(&db_path, "20240103-000000-003", b"cc");

    // 共享模式只给 READ | WRITE：没有 FILE_SHARE_DELETE，`DeleteFileW` 会失败，
    // 这是 Windows 上唯一可稳定注入「单份删除失败」的方式（只读属性挡不住删除）。
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(&oldest)
        .unwrap();

    let report = SecurityDbState::prune_backups_to(&db_path, 2).unwrap();

    assert_eq!(report.removed, 0, "删除失败不得计入成功");
    assert_eq!(report.failed, 1, "单份失败只计数，不中断");
    assert!(oldest.exists(), "失败时保留多出的那份备份");
    assert!(middle.exists());
    assert!(newest.exists());

    drop(handle);
}

#[cfg(unix)]
#[test]
fn prune_backups_to_skips_symlinks() {
    let (_dir, db_path) = vault_db_path();
    let target_dir = tempfile::tempdir().unwrap();
    let target = target_dir.path().join("outside.bin");
    std::fs::write(&target, b"keep me").unwrap();

    let link = backup_path(&db_path, "20240101-000000-001");
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let newest = write_backup(&db_path, "20240102-000000-002", b"bb");

    let report = SecurityDbState::prune_backups_to(&db_path, 1).unwrap();

    assert_eq!(report.removed, 0, "符号链接不计入备份份数");
    assert_eq!(report.failed, 0);
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(target.exists(), "绝不删链接目标");
    assert!(newest.exists());
}
