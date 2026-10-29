mod common;

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::types::{
    ImportOutcome, VaultCategory, VaultEntryCreate, VaultEntryImport, VaultError,
};
use common::{cleanup, init_vault, temp_db_path, STRONG_PW};

const ALGO: CipherAlgorithm = CipherAlgorithm::Aes256Gcm;

fn create(title: &str) -> VaultEntryCreate {
    VaultEntryCreate {
        category: VaultCategory::Login,
        title: title.to_string(),
        username: Some("octocat".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("pw-123".to_string()),
        notes: None,
        totp_secret: None,
        tags: vec!["dev".to_string()],
        custom_fields: vec![],
        is_favorite: false,
    }
}

fn import(id: &str, revision: u64, title: &str, updated_at: i64) -> VaultEntryImport {
    VaultEntryImport {
        id: id.to_string(),
        category: VaultCategory::Login,
        revision,
        title: title.to_string(),
        username: Some("remote".to_string()),
        url: None,
        password: Some("remote-pw".to_string()),
        notes: None,
        totp_secret: None,
        totp_digits: 6,
        totp_period: 30,
        custom_fields: vec![],
        tags: vec![],
        is_favorite: false,
        created_at: 1_000,
        updated_at,
    }
}

fn db() -> (std::path::PathBuf, SecurityDbState) {
    let path = temp_db_path();
    let db = init_vault(&path, STRONG_PW);
    (path, db)
}

#[test]
fn create_tx_rolls_back_when_export_fails() {
    let (path, db) = db();

    let result = db.create_entry_tx(create("GitHub"), ALGO, |_, _| {
        Err(VaultError::Validation("export failed".to_string()))
    });

    assert!(result.is_err());
    assert!(db.list_entry_ids(true).unwrap().is_empty());
    cleanup(&path);
}

#[test]
fn create_tx_exports_snapshot_and_persists() {
    let (path, db) = db();
    let mut exported = None;

    let snapshot = db
        .create_entry_tx(create("GitHub"), ALGO, |_, s| {
            exported = Some(s.clone());
            Ok(())
        })
        .unwrap();

    let exported = exported.expect("export callback must run");
    assert_eq!(exported.id, snapshot.id);
    assert_eq!(exported.title, "GitHub");
    assert_eq!(exported.password.as_deref(), Some("pw-123"));
    assert_eq!(db.list_entry_ids(false).unwrap(), vec![snapshot.id]);
    cleanup(&path);
}

#[test]
fn delete_tx_soft_deletes_and_reports_id() {
    let (path, db) = db();
    let snapshot = db
        .create_entry_tx(create("GitHub"), ALGO, |_, _| Ok(()))
        .unwrap();

    let mut exported = None;
    db.delete_entry_tx(&snapshot.id, |id| {
        exported = Some(id.to_string());
        Ok(())
    })
    .unwrap();

    assert_eq!(exported.as_deref(), Some(snapshot.id.as_str()));
    assert!(db.list_entry_ids(false).unwrap().is_empty());
    assert_eq!(
        db.list_deleted_entry_ids().unwrap(),
        vec![snapshot.id.clone()]
    );
    assert!(matches!(
        db.get_entry(&snapshot.id),
        Err(VaultError::NotFound(_))
    ));
    cleanup(&path);
}

#[test]
fn delete_tx_rolls_back_when_export_fails() {
    let (path, db) = db();
    let snapshot = db
        .create_entry_tx(create("GitHub"), ALGO, |_, _| Ok(()))
        .unwrap();

    assert!(db
        .delete_entry_tx(&snapshot.id, |_| {
            Err(VaultError::Validation("export failed".to_string()))
        })
        .is_err());

    assert_eq!(db.list_entry_ids(false).unwrap(), vec![snapshot.id]);
    assert!(db.list_deleted_entry_ids().unwrap().is_empty());
    cleanup(&path);
}

#[test]
fn upsert_imported_inserts_updates_and_skips() {
    let (path, db) = db();

    assert_eq!(
        db.upsert_imported_entries(vec![import("e1", 1, "First", 1_000)], ALGO)
            .unwrap(),
        vec![ImportOutcome::Inserted]
    );
    assert_eq!(db.get_entry("e1").unwrap().title, "First");

    assert_eq!(
        db.upsert_imported_entries(vec![import("e1", 2, "Second", 2_000)], ALGO)
            .unwrap(),
        vec![ImportOutcome::Updated]
    );
    assert_eq!(db.get_entry("e1").unwrap().title, "Second");
    assert_eq!(
        db.get_entry("e1").unwrap().username.as_deref(),
        Some("remote")
    );

    assert_eq!(
        db.upsert_imported_entries(vec![import("e1", 1, "Stale", 500)], ALGO)
            .unwrap(),
        vec![ImportOutcome::SkippedStale]
    );
    assert_eq!(db.get_entry("e1").unwrap().title, "Second");

    db.delete_entry("e1").unwrap();
    assert_eq!(
        db.upsert_imported_entries(vec![import("e1", 9, "Remote", 9_000)], ALGO)
            .unwrap(),
        vec![ImportOutcome::SkippedLocallyDeleted]
    );
    assert!(matches!(db.get_entry("e1"), Err(VaultError::NotFound(_))));
    cleanup(&path);
}

#[test]
fn export_all_snapshots_excludes_deleted_entries() {
    let (path, db) = db();
    let kept = db
        .create_entry_tx(create("Kept"), ALGO, |_, _| Ok(()))
        .unwrap();
    let removed = db
        .create_entry_tx(create("Removed"), ALGO, |_, _| Ok(()))
        .unwrap();
    db.delete_entry(&removed.id).unwrap();

    let snapshots = db.export_all_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, kept.id);
    assert_eq!(snapshots[0].password.as_deref(), Some("pw-123"));
    cleanup(&path);
}

#[test]
fn vault_crypto_info_exposes_salts_and_argon2_params() {
    let (path, db) = db();

    let info = db.vault_crypto_info(ALGO).unwrap();
    assert_eq!(info.default_cipher, ALGO);
    assert!(info.argon2_m > 0 && info.argon2_t > 0 && info.argon2_p > 0);
    assert_ne!(info.master_key_salt, [0u8; 32]);
    assert_ne!(info.hash_salt, [0u8; 32]);
    cleanup(&path);
}
