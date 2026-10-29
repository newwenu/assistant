mod common;

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::types::{
    CustomFieldInput, VaultCategory, VaultEntryCreate, VaultEntryUpdate, VaultError,
};
use common::{cleanup, init_vault, temp_db_path, STRONG_PW};

#[cfg(feature = "slow-tests")]
#[test]
fn full_crud_lifecycle() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let create = VaultEntryCreate {
        category: VaultCategory::Login,
        title: "GitHub".to_string(),
        username: Some("octocat".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("ghp_supersecret123".to_string()),
        notes: Some("Personal account".to_string()),
        totp_secret: None,
        tags: vec!["dev".to_string()],
        custom_fields: vec![],
        is_favorite: true,
    };

    let id = db.create_entry(create, CipherAlgorithm::Aes256Gcm).unwrap();

    let detail = db.get_entry(&id).unwrap();
    assert_eq!(detail.title, "GitHub");
    assert_eq!(detail.category, VaultCategory::Login);
    assert!(detail.has_password);
    assert!(detail.has_notes);
    assert!(!detail.has_totp);
    assert!(detail.is_favorite);

    let password = db.decrypt_password(&id).unwrap();
    assert_eq!(password.expose_str(), "ghp_supersecret123");

    let notes = db.decrypt_notes(&id).unwrap();
    assert_eq!(notes.expose_str(), "Personal account");

    let update = VaultEntryUpdate {
        category: VaultCategory::Login,
        title: "GitHub Inc".to_string(),
        username: Some("octocat".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("ghp_newsecret456".to_string()),
        notes: Some("Updated notes".to_string()),
        totp_secret: None,
        tags: vec!["dev".to_string(), "work".to_string()],
        custom_fields: vec![],
        is_favorite: false,
    };
    db.update_entry(&id, update, CipherAlgorithm::Aes256Gcm)
        .unwrap();

    let detail2 = db.get_entry(&id).unwrap();
    assert_eq!(detail2.title, "GitHub Inc");
    assert!(!detail2.is_favorite);
    assert_eq!(detail2.revision, 2);

    let new_pw = db.decrypt_password(&id).unwrap();
    assert_eq!(new_pw.expose_str(), "ghp_newsecret456");

    db.delete_entry(&id).unwrap();
    assert!(matches!(db.get_entry(&id), Err(VaultError::NotFound(_))));

    let entries = db.list_entries(None, None, None).unwrap();
    assert!(entries.is_empty());

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn create_entry_with_protected_custom_field() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let create = VaultEntryCreate {
        category: VaultCategory::Login,
        title: "AWS".to_string(),
        username: Some("admin".to_string()),
        url: None,
        password: Some("aws_secret_key".to_string()),
        notes: None,
        totp_secret: None,
        tags: vec![],
        custom_fields: vec![
            CustomFieldInput {
                name: "accessKeyId".to_string(),
                value: "AKIAIOSFODNN7EXAMPLE".to_string(),
                is_protected: false,
            },
            CustomFieldInput {
                name: "secretAccessKey".to_string(),
                value: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
                is_protected: true,
            },
        ],
        is_favorite: false,
    };

    let id = db.create_entry(create, CipherAlgorithm::Aes256Gcm).unwrap();

    let detail = db.get_entry(&id).unwrap();
    assert_eq!(
        detail.custom_field_names,
        vec!["accessKeyId", "secretAccessKey"]
    );

    let unprotected = db.decrypt_field(&id, "accessKeyId").unwrap();
    assert_eq!(unprotected.expose_str(), "AKIAIOSFODNN7EXAMPLE");

    let protected = db.decrypt_field(&id, "secretAccessKey").unwrap();
    assert_eq!(
        protected.expose_str(),
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
    );

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn create_entry_with_totp() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let create = VaultEntryCreate {
        category: VaultCategory::Login,
        title: "Google".to_string(),
        username: Some("user@gmail.com".to_string()),
        url: Some("https://accounts.google.com".to_string()),
        password: Some("google_pw".to_string()),
        notes: None,
        totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
        tags: vec![],
        custom_fields: vec![],
        is_favorite: false,
    };

    let id = db.create_entry(create, CipherAlgorithm::Aes256Gcm).unwrap();
    let detail = db.get_entry(&id).unwrap();
    assert!(detail.has_totp);

    let totp_secret = db.decrypt_totp_secret(&id).unwrap();
    assert_eq!(totp_secret.expose_str(), "JBSWY3DPEHPK3PXP");

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn list_entries_filter_by_category() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    for (i, cat) in [
        VaultCategory::Login,
        VaultCategory::Note,
        VaultCategory::Card,
    ]
    .iter()
    .enumerate()
    {
        db.create_entry(
            VaultEntryCreate {
                category: *cat,
                title: format!("Entry {}", i),
                username: None,
                url: None,
                password: None,
                notes: None,
                totp_secret: None,
                tags: vec![],
                custom_fields: vec![],
                is_favorite: false,
            },
            CipherAlgorithm::Aes256Gcm,
        )
        .unwrap();
    }

    let all = db.list_entries(None, None, None).unwrap();
    assert_eq!(all.len(), 3);

    let logins = db
        .list_entries(Some(VaultCategory::Login), None, None)
        .unwrap();
    assert_eq!(logins.len(), 1);
    assert_eq!(logins[0].title, "Entry 0");

    let notes = db
        .list_entries(Some(VaultCategory::Note), None, None)
        .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "Entry 1");

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn list_entries_search_by_title() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    for name in &["GitHub", "GitLab", "Bitbucket"] {
        db.create_entry(
            VaultEntryCreate {
                category: VaultCategory::Login,
                title: name.to_string(),
                username: None,
                url: None,
                password: None,
                notes: None,
                totp_secret: None,
                tags: vec![],
                custom_fields: vec![],
                is_favorite: false,
            },
            CipherAlgorithm::Aes256Gcm,
        )
        .unwrap();
    }

    let git_results = db.list_entries(None, Some("Git"), None).unwrap();
    assert_eq!(git_results.len(), 2);

    let github = db.list_entries(None, Some("GitHub"), None).unwrap();
    assert_eq!(github.len(), 1);

    let no_match = db.list_entries(None, Some("Azure"), None).unwrap();
    assert!(no_match.is_empty());

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn delete_entry_is_soft_delete() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let id = db
        .create_entry(
            VaultEntryCreate {
                category: VaultCategory::Note,
                title: "Temp note".to_string(),
                username: None,
                url: None,
                password: None,
                notes: Some("will be deleted".to_string()),
                totp_secret: None,
                tags: vec![],
                custom_fields: vec![],
                is_favorite: false,
            },
            CipherAlgorithm::Aes256Gcm,
        )
        .unwrap();

    db.delete_entry(&id).unwrap();

    assert!(matches!(db.get_entry(&id), Err(VaultError::NotFound(_))));

    let entries = db.list_entries(None, None, None).unwrap();
    assert!(entries.is_empty());

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn operations_on_locked_vault_fail() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    db.lock().unwrap();

    let create = VaultEntryCreate {
        category: VaultCategory::Login,
        title: "Test".to_string(),
        username: None,
        url: None,
        password: None,
        notes: None,
        totp_secret: None,
        tags: vec![],
        custom_fields: vec![],
        is_favorite: false,
    };

    assert!(matches!(
        db.create_entry(create, CipherAlgorithm::Aes256Gcm),
        Err(VaultError::Locked)
    ));
    assert!(matches!(
        db.list_entries(None, None, None),
        Err(VaultError::Locked)
    ));
    assert!(matches!(db.get_entry("any"), Err(VaultError::Locked)));
    assert!(matches!(db.delete_entry("any"), Err(VaultError::Locked)));
    assert!(matches!(
        db.decrypt_password("any"),
        Err(VaultError::Locked)
    ));

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn update_nonexistent_entry_fails() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let update = VaultEntryUpdate {
        category: VaultCategory::Login,
        title: "Ghost".to_string(),
        username: None,
        url: None,
        password: None,
        notes: None,
        totp_secret: None,
        tags: vec![],
        custom_fields: vec![],
        is_favorite: false,
    };

    assert!(matches!(
        db.update_entry("nonexistent-id", update, CipherAlgorithm::Aes256Gcm),
        Err(VaultError::NotFound(_))
    ));

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn change_master_password_and_relock_unlock() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let id = db
        .create_entry(
            VaultEntryCreate {
                category: VaultCategory::Login,
                title: "Test".to_string(),
                username: Some("user".to_string()),
                url: None,
                password: Some("secret123".to_string()),
                notes: None,
                totp_secret: None,
                tags: vec![],
                custom_fields: vec![],
                is_favorite: false,
            },
            CipherAlgorithm::Aes256Gcm,
        )
        .unwrap();

    let new_password = "N3wStr0ng#P@ss!2026";
    db.change_master_password(STRONG_PW, new_password, CipherAlgorithm::Aes256Gcm)
        .unwrap();

    db.lock().unwrap();
    assert!(db.is_locked());

    assert!(matches!(
        db.unlock(STRONG_PW),
        Err(VaultError::InvalidPassword)
    ));

    db.unlock(new_password).unwrap();
    assert!(!db.is_locked());

    let pw = db.decrypt_password(&id).unwrap();
    assert_eq!(pw.expose_str(), "secret123");

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn change_master_password_rejects_weak_new_password() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let result = db.change_master_password(STRONG_PW, "123", CipherAlgorithm::Aes256Gcm);
    assert!(matches!(result, Err(VaultError::Validation(_))));

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn change_master_password_rejects_wrong_current_password() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let result = db.change_master_password(
        "wrong_password",
        "N3wStr0ng#P@ss!2026",
        CipherAlgorithm::Aes256Gcm,
    );
    assert!(matches!(result, Err(VaultError::InvalidPassword)));

    cleanup(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn init_rejects_weak_master_password() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);
    let db = SecurityDbState::new(path.to_str().unwrap());

    let result = db.init("123");
    assert!(
        matches!(result, Err(VaultError::Validation(_))),
        "weak password should be rejected"
    );

    let _ = std::fs::remove_file(&path);
}

#[cfg(feature = "slow-tests")]
#[test]
fn purge_deleted_removes_only_soft_deleted_entries() {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);

    let make = |title: &str| VaultEntryCreate {
        category: VaultCategory::Login,
        title: title.to_string(),
        username: None,
        url: None,
        password: None,
        notes: None,
        totp_secret: None,
        tags: vec![],
        custom_fields: vec![],
        is_favorite: false,
    };

    let keep = db
        .create_entry(make("Keep"), CipherAlgorithm::Aes256Gcm)
        .unwrap();
    let removed = db
        .create_entry(make("Drop"), CipherAlgorithm::Aes256Gcm)
        .unwrap();
    db.delete_entry(&removed).unwrap();

    assert_eq!(db.list_deleted_entry_ids().unwrap(), vec![removed.clone()]);
    assert_eq!(db.purge_deleted().unwrap(), 1);
    assert!(db.list_deleted_entry_ids().unwrap().is_empty());
    assert_eq!(db.list_entry_ids(false).unwrap(), vec![keep.clone()]);
    assert_eq!(db.list_entry_ids(true).unwrap(), vec![keep]);
    assert_eq!(db.purge_deleted().unwrap(), 0);

    cleanup(&path);
}
