#![cfg(feature = "slow-tests")]

mod common;

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_sync::types::SyncConfig;
use assistant_vault::dto::VaultEntryCreate;
use assistant_vault::recovery_key::{decode_paper, encode_paper, open_recovery_session};
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;
use assistant_vault::types::{VaultCategory, VaultError as DomainError};
use assistant_vault::vault_file::VaultFileManager;
use common::{cleanup, repo_for, temp_db_path, STRONG_PW};
use sha2::{Digest, Sha256};

fn engine_for(path: &Path) -> SyncEngine {
    SyncEngine::new(
        path.parent().unwrap().join("vault"),
        SyncConfig {
            remote_url: None,
            remote_name: "origin".to_string(),
            ssh_key_path: None,
        },
        Box::new(|| CipherAlgorithm::Aes256Gcm),
    )
}

fn sample_entry() -> VaultEntryCreate {
    VaultEntryCreate {
        category: VaultCategory::Login,
        title: "GitHub".to_string(),
        username: Some("octocat".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("ghp_supersecret123".to_string()),
        notes: Some("Personal account".to_string()),
        totp: None,
        tags: vec!["dev".to_string()],
        custom_fields: Vec::new(),
        is_favorite: true,
    }
}

fn file_state(path: &Path) -> (String, Option<SystemTime>) {
    let bytes = fs::read(path).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    (
        hash,
        fs::metadata(path).ok().and_then(|m| m.modified().ok()),
    )
}

fn setup_vault_with_entry(path: &Path) -> (SecurityDbState, VaultFileManager, String) {
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    let id = db
        .create_entry(sample_entry(), CipherAlgorithm::Aes256Gcm)
        .unwrap();
    engine_for(path).export_all_entries(&db, &[]).unwrap();
    (db, repo, id)
}

fn flip_char(key: &str, index: usize) -> String {
    let mut chars: Vec<char> = key.chars().collect();
    let replacement = if chars[index] == 'A' { 'B' } else { 'A' };
    chars[index] = replacement;
    chars.into_iter().collect()
}

#[test]
fn paper_roundtrip_preserves_dek() {
    let dek = assistant_crypto::vault_crypto::types::Dek::new([
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC,
        0xFE, 0x11, 0x22, 0x33, 0x44,
    ]);
    let paper = encode_paper(&dek);
    let decoded = decode_paper(&paper).unwrap();
    assert!(dek.ct_eq(&decoded));
}

#[test]
fn exported_paper_recovers_all_entries() {
    let path = temp_db_path();
    let (db, repo, id) = setup_vault_with_entry(&path);

    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    assert_eq!(exported.paper_key.len(), 40);
    assert_eq!(exported.groups.len(), 5);
    assert!(exported.groups.iter().all(|group| group.len() == 8));
    assert_eq!(exported.dek_version, 1);

    let backup = assistant_vault::lifecycle::recovery_backup(&repo)
        .unwrap()
        .expect("recovery backup should be recorded");
    assert_eq!(backup.dek_version, exported.dek_version);

    db.lock().unwrap();
    assert!(db.is_locked());

    let session = open_recovery_session(&repo, &exported.paper_key).unwrap();
    let recovery = session.export_all().unwrap();
    assert_eq!(recovery.entries.len(), 1);
    assert!(recovery.skipped.is_empty());
    assert_eq!(recovery.entries[0].id, id);
    assert_eq!(
        recovery.entries[0].blob.password.as_deref(),
        Some("ghp_supersecret123")
    );
    assert!(db.is_locked());

    cleanup(&path);
}

#[test]
fn recovery_does_not_touch_security_db() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);
    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let before = file_state(&path);
    let session = open_recovery_session(&repo, &exported.paper_key).unwrap();
    session.export_all().unwrap();
    let after = file_state(&path);

    assert_eq!(before, after, "recovery must not write security.db");
    assert!(db.is_locked());

    cleanup(&path);
}

#[test]
fn anchor_stays_consistent_after_recovery_export() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);
    assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();

    db.lock().unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW)
        .expect("unlock must not report rollback after meta rewrite");

    cleanup(&path);
}

#[test]
fn export_recovery_key_requires_unlocked_vault() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);
    db.lock().unwrap();

    let result = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW);
    assert!(matches!(result, Err(DomainError::Locked)), "got {result:?}");

    cleanup(&path);
}

#[test]
fn export_recovery_key_rejects_wrong_password() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);

    let result = assistant_vault::lifecycle::export_recovery_key(&db, &repo, "Wr0ng#P@ssw0rd!2025");
    assert!(
        matches!(result, Err(DomainError::InvalidPassword)),
        "got {result:?}"
    );

    cleanup(&path);
}

#[test]
fn recovery_backup_is_absent_before_export() {
    let path = temp_db_path();
    let (_db, repo, _id) = setup_vault_with_entry(&path);
    assert!(assistant_vault::lifecycle::recovery_backup(&repo)
        .unwrap()
        .is_none());
    cleanup(&path);
}

#[test]
fn wrong_paper_is_rejected_with_checksum_error() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);
    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let tampered = flip_char(&exported.paper_key, 10);
    let result = open_recovery_session(&repo, &tampered);
    assert!(
        matches!(result, Err(DomainError::InvalidRecoveryKeyChecksum)),
        "got {:?}",
        result.err()
    );

    cleanup(&path);
}

#[test]
fn malformed_paper_is_rejected_with_format_error() {
    let path = temp_db_path();
    let (_db, repo, _id) = setup_vault_with_entry(&path);

    let result = open_recovery_session(&repo, "ABCDE");
    assert!(
        matches!(result, Err(DomainError::InvalidRecoveryKeyFormat)),
        "got {:?}",
        result.err()
    );

    cleanup(&path);
}

#[test]
fn tampered_meta_reports_combined_error() {
    let path = temp_db_path();
    let (db, repo, _id) = setup_vault_with_entry(&path);
    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let meta_path = repo.meta_path();
    let mut meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
    meta["integrity"] = serde_json::Value::String("A".repeat(44));
    fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let result = open_recovery_session(&repo, &exported.paper_key);
    assert!(
        matches!(result, Err(DomainError::InvalidRecoveryKeyOrMetaTampered)),
        "got {:?}",
        result.err()
    );

    cleanup(&path);
}

#[test]
fn tampered_entry_file_reports_manifest_mismatch() {
    let path = temp_db_path();
    let (db, repo, id) = setup_vault_with_entry(&path);
    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let entry_path = repo.entry_file_path(&id);
    let mut content = fs::read_to_string(&entry_path).unwrap();
    content.push('\n');
    fs::write(&entry_path, content).unwrap();

    let result = open_recovery_session(&repo, &exported.paper_key);
    assert!(
        matches!(result, Err(DomainError::VaultManifestMismatch)),
        "got {:?}",
        result.err()
    );

    cleanup(&path);
}
