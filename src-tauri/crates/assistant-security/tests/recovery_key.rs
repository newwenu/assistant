use assistant_security::recovery_key::{
    format_recovery_key, generate_recovery_key, parse_recovery_key, unlock_with_recovery_key,
};
use assistant_security::security_db::SecurityDbState;
use assistant_security::types::VaultError;

fn temp_db_path() -> String {
    let dir = std::env::temp_dir().join("assistant-security-test-rk");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("test_vault_{}.db", uuid::Uuid::new_v4()))
        .to_str()
        .unwrap()
        .to_string()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_file(path);
    let backup_dir = std::path::Path::new(path).parent().unwrap().join("backups");
    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }
}

#[test]
fn format_recovery_key_has_four_groups_of_eight() {
    let raw = [0xABu8; 20];
    let formatted = format_recovery_key(&raw);
    let groups: Vec<&str> = formatted.split('-').collect();
    assert_eq!(groups.len(), 4, "recovery key should have 4 groups");
    for (i, g) in groups.iter().enumerate() {
        assert_eq!(g.len(), 8, "group {} should be 8 chars, got '{}'", i, g);
    }
}

#[test]
fn format_then_parse_consistency() {
    let raw = [0x42u8; 20];
    let formatted = format_recovery_key(&raw);
    let groups: Vec<&str> = formatted.split('-').collect();
    assert_eq!(groups.len(), 4);
    let cleaned: String = formatted.chars().filter(|c| *c != '-').collect();
    assert_eq!(
        cleaned.len(),
        32,
        "formatted key (without dashes) should be 32 chars"
    );
    let parsed = parse_recovery_key(&formatted);
    assert!(
        parsed.is_ok(),
        "format→parse should succeed, got {:?}",
        parsed
    );
}

#[test]
fn parse_rejects_wrong_length() {
    assert!(parse_recovery_key("ABCDE").is_err());
    assert!(parse_recovery_key("ABCDE-FGHIJ-KLMNO-PQRST-UVWXY").is_err());
}

#[test]
fn parse_rejects_invalid_base32() {
    assert!(parse_recovery_key("!!!!@-FGHIJ-KLMNO-PQRST").is_err());
}

#[test]
fn generate_recovery_key_produces_valid_key() {
    let path = temp_db_path();
    let db = SecurityDbState::new(&path);
    db.init("Str0ng#P@ssw0rd!2025").unwrap();

    let (key, emergency) = {
        let mk_guard = db.get_master_key().unwrap();
        let bk_guard = db.get_bootstrap_key().unwrap();
        let mk = mk_guard.as_ref().unwrap();
        let bk = bk_guard.as_ref().unwrap();
        let result = generate_recovery_key(mk, bk).unwrap();
        drop(mk_guard);
        drop(bk_guard);
        result
    };

    let groups: Vec<&str> = key.split('-').collect();
    assert_eq!(groups.len(), 4);
    for g in &groups {
        assert_eq!(g.len(), 8);
    }
    assert!(!emergency.token.is_empty());
    assert!(!emergency.salt.is_empty());

    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn generate_recovery_key_unique_each_time() {
    let path = temp_db_path();
    let db = SecurityDbState::new(&path);
    db.init("Str0ng#P@ssw0rd!2025").unwrap();

    let ((key1, emergency1), (key2, emergency2)) = {
        let mk_guard = db.get_master_key().unwrap();
        let bk_guard = db.get_bootstrap_key().unwrap();
        let mk = mk_guard.as_ref().unwrap();
        let bk = bk_guard.as_ref().unwrap();
        let r1 = generate_recovery_key(mk, bk).unwrap();
        let r2 = generate_recovery_key(mk, bk).unwrap();
        drop(mk_guard);
        drop(bk_guard);
        (r1, r2)
    };

    assert_ne!(key1, key2, "each generation should produce a unique key");
    assert_ne!(emergency1.token, emergency2.token);

    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn unlock_with_recovery_key_succeeds() {
    let path = temp_db_path();
    let db = SecurityDbState::new(&path);
    let password = "Str0ng#P@ssw0rd!2025";

    db.init(password).unwrap();

    let (recovery_key, emergency) = {
        let mk_guard = db.get_master_key().unwrap();
        let bk_guard = db.get_bootstrap_key().unwrap();
        let mk = mk_guard.as_ref().unwrap();
        let bk = bk_guard.as_ref().unwrap();
        let result = generate_recovery_key(mk, bk).unwrap();
        drop(mk_guard);
        drop(bk_guard);
        result
    };

    db.lock().unwrap();
    assert!(db.is_locked());

    let result = unlock_with_recovery_key(&recovery_key, &emergency, &db);
    assert!(
        result.is_ok(),
        "unlock with recovery key should succeed: {:?}",
        result
    );
    assert!(
        !db.is_locked(),
        "vault should be unlocked after recovery key unlock"
    );

    cleanup(&path);
}

#[test]
fn unlock_with_wrong_recovery_key_fails() {
    let path = temp_db_path();
    let db = SecurityDbState::new(&path);
    let password = "Str0ng#P@ssw0rd!2025";

    db.init(password).unwrap();

    let (_real_key, emergency) = {
        let mk_guard = db.get_master_key().unwrap();
        let bk_guard = db.get_bootstrap_key().unwrap();
        let mk = mk_guard.as_ref().unwrap();
        let bk = bk_guard.as_ref().unwrap();
        let result = generate_recovery_key(mk, bk).unwrap();
        drop(mk_guard);
        drop(bk_guard);
        result
    };

    db.lock().unwrap();

    let fake_key = "AAAAA-BBBBB-CCCCC-DDDDD";
    let result = unlock_with_recovery_key(fake_key, &emergency, &db);
    assert!(
        matches!(result, Err(VaultError::InvalidRecoveryKey)),
        "wrong recovery key should fail with InvalidRecoveryKey, got {:?}",
        result
    );

    cleanup(&path);
}

#[test]
fn recovery_key_cannot_unlock_after_password_change() {
    let path = temp_db_path();
    let db = SecurityDbState::new(&path);
    let password = "Str0ng#P@ssw0rd!2025";
    let new_password = "N3wStr0ng#P@ss!2026";

    db.init(password).unwrap();

    let (recovery_key, emergency) = {
        let mk_guard = db.get_master_key().unwrap();
        let bk_guard = db.get_bootstrap_key().unwrap();
        let mk = mk_guard.as_ref().unwrap();
        let bk = bk_guard.as_ref().unwrap();
        let result = generate_recovery_key(mk, bk).unwrap();
        drop(mk_guard);
        drop(bk_guard);
        result
    };

    db.change_master_password(password, new_password).unwrap();

    db.lock().unwrap();

    let result = unlock_with_recovery_key(&recovery_key, &emergency, &db);
    assert!(
        result.is_err(),
        "old recovery key should not unlock vault after password change, got {:?}",
        result
    );

    cleanup(&path);
}
