mod common;

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::dto::VaultEntryCreate;
use assistant_vault::security_db::{LockReason, SecurityDbState};
use assistant_vault::types::{VaultCategory, VaultError};
use common::{cleanup, init_vault, temp_db_path, try_initialize_at, unlock_at, STRONG_PW};

fn entry(title: &str) -> VaultEntryCreate {
    VaultEntryCreate {
        category: VaultCategory::Login,
        title: title.to_string(),
        username: Some("user".to_string()),
        url: None,
        password: Some("p@ssw0rd".to_string()),
        notes: None,
        totp: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        is_favorite: false,
    }
}

fn audit_details(db: &SecurityDbState, event: &str) -> Vec<Option<String>> {
    db.audit_writer()
        .read_since(0)
        .unwrap()
        .into_iter()
        .filter(|record| record.event == event)
        .map(|record| record.detail)
        .collect()
}

/// T-R1：解锁 → 锁定后会话被丢弃、连接与密钥不可再取。
#[test]
fn t_r1_lock_drops_session_conn_and_keys() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    assert!(!db.is_locked());

    db.lock_session(LockReason::User).unwrap();

    assert!(db.is_locked(), "锁定后必须处于锁定态");
    assert!(
        matches!(db.get_conn(), Err(VaultError::Locked)),
        "锁定后不得再借出连接"
    );
    assert!(
        matches!(db.get_dek(), Err(VaultError::Locked)),
        "锁定后不得再借出 dek"
    );

    cleanup(&path);
}

/// T-R2：锁定后任意业务命令返回 `Locked`，不返回部分结果。
#[test]
fn t_r2_business_commands_after_lock_return_locked() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    db.create_entry(entry("before-lock"), CipherAlgorithm::Aes256Gcm)
        .unwrap();

    db.lock_session(LockReason::User).unwrap();

    assert!(matches!(
        db.create_entry(entry("after-lock"), CipherAlgorithm::Aes256Gcm),
        Err(VaultError::Locked)
    ));
    assert!(matches!(db.get_entry("any-id"), Err(VaultError::Locked)));
    assert!(matches!(
        db.list_entries(None, None, None),
        Err(VaultError::Locked)
    ));
    assert!(matches!(db.delete_entry("any-id"), Err(VaultError::Locked)));

    cleanup(&path);
}

/// T-R3：锁定会推进 epoch，旧 epoch 的会话结果一律按 `Locked` 处理（不复用、不写入）。
#[test]
fn t_r3_lock_bumps_epoch_and_rejects_stale_session() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let epoch_before = db.current_epoch();
    db.lock_session(LockReason::AutoLock).unwrap();
    let epoch_after = db.current_epoch();
    assert_eq!(
        epoch_after,
        epoch_before + 1,
        "每次锁定必须推进 epoch，使旧会话结果失效"
    );

    assert!(matches!(
        db.list_entries(None, None, None),
        Err(VaultError::Locked)
    ));

    // 再次锁定（已锁定态）也要推进 epoch，避免旧结果在重新解锁后仍被复用。
    db.lock_session(LockReason::AutoLock).unwrap();
    assert_eq!(db.current_epoch(), epoch_after + 1);

    cleanup(&path);
}

/// T-R4：解锁失败后仍为锁定态，且不残留连接 / dek。
#[test]
fn t_r4_failed_unlock_stays_locked_without_residue() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    db.lock_session(LockReason::User).unwrap();

    let epoch_before = db.current_epoch();
    let result = unlock_at(&db, "Wr0ng#P@ss!9999");
    assert!(
        matches!(result, Err(VaultError::InvalidPassword)),
        "错误口令必须失败: {:?}",
        result
    );

    assert!(db.is_locked(), "解锁失败后必须仍为锁定态");
    assert!(matches!(db.get_conn(), Err(VaultError::Locked)));
    assert!(matches!(db.get_dek(), Err(VaultError::Locked)));
    assert_eq!(db.current_epoch(), epoch_before, "失败不得推进 epoch");

    cleanup(&path);
}

/// T-R8：屏幕锁事件走 `lock_session(ScreenLock)`，审计 detail 为 `screen_lock`。
#[test]
fn t_r8_screen_lock_writes_audit_with_reason() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    db.lock_session(LockReason::ScreenLock).unwrap();

    assert!(db.is_locked());
    let details = audit_details(&db, "vault_lock");
    assert!(
        details.iter().any(|d| d.as_deref() == Some("screen_lock")),
        "审计中必须存在 screen_lock 原因，实际 {:?}",
        details
    );

    cleanup(&path);
}

/// T-R9 / T-C14：退出路径走 `lock_session(AppExit)`，审计 detail 为 `app_exit`。
#[test]
fn t_r9_app_exit_writes_audit_with_reason() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    db.lock_session(LockReason::AppExit).unwrap();

    assert!(db.is_locked());
    let details = audit_details(&db, "vault_lock");
    assert!(
        details.iter().any(|d| d.as_deref() == Some("app_exit")),
        "审计中必须存在 app_exit 原因，实际 {:?}",
        details
    );

    cleanup(&path);
}

/// 锁定原因集合稳定映射到审计字符串（封闭集合，禁止自由文本）。
#[test]
fn lock_reason_strings_are_stable() {
    assert_eq!(LockReason::User.as_str(), "user");
    assert_eq!(LockReason::AutoLock.as_str(), "auto_lock");
    assert_eq!(LockReason::ScreenLock.as_str(), "screen_lock");
    assert_eq!(LockReason::AppExit.as_str(), "app_exit");
    assert_eq!(LockReason::Reset.as_str(), "reset");
    assert_eq!(LockReason::Panic.as_str(), "panic");
}

/// 初始化失败（库已存在）时不得留下会话。
#[test]
fn init_failure_leaves_no_session() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    db.lock_session(LockReason::User).unwrap();

    let result = try_initialize_at(&db, STRONG_PW);
    assert!(
        matches!(result, Err(VaultError::AlreadyExists)),
        "重复初始化必须失败: {:?}",
        result
    );
    assert!(db.is_locked());
    assert!(matches!(db.get_conn(), Err(VaultError::Locked)));

    cleanup(&path);
}
