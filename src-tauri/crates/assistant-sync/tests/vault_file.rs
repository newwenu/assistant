mod common;

use std::fs;

use assistant_sync::vault_file::types::{EntryFile, VaultMetaFile};
use common::{
    entry_snapshot as snapshot, master_key, meta_path, other_key, vault_manager as manager,
    write_meta,
};

#[test]
fn export_import_roundtrip_preserves_all_fields() {
    let (_dir, mgr) = manager();
    let key = master_key();
    let original = snapshot("a1b2c3d4");

    mgr.export_entry(&key, &original, false).unwrap();
    let imported = mgr.import_entry(&key, "a1b2c3d4").unwrap();

    assert_eq!(imported.id, original.id);
    assert_eq!(imported.category, original.category.to_string());
    assert_eq!(imported.revision, original.revision);
    assert_eq!(imported.blob.title, original.title);
    assert_eq!(imported.blob.username, original.username);
    assert_eq!(imported.blob.url, original.url);
    assert_eq!(imported.blob.password, original.password);
    assert_eq!(imported.blob.notes, original.notes);
    assert_eq!(imported.blob.totp_secret, original.totp_secret);
    assert_eq!(imported.blob.totp_digits, original.totp_digits);
    assert_eq!(imported.blob.totp_period, original.totp_period);
    assert_eq!(imported.blob.tags, original.tags);
    assert_eq!(imported.blob.is_favorite, original.is_favorite);
    assert_eq!(imported.blob.created_at, original.created_at);
    assert_eq!(imported.blob.updated_at, original.updated_at);
    assert_eq!(imported.blob.custom_fields.len(), 1);
    assert_eq!(imported.blob.custom_fields[0].name, "API Key");
    assert_eq!(imported.blob.custom_fields[0].value, "cf-value");
    assert!(imported.blob.custom_fields[0].is_secret);
}

#[test]
fn entry_file_keeps_only_id_category_revision_in_plaintext() {
    let (_dir, mgr) = manager();
    mgr.export_entry(&master_key(), &snapshot("a1b2c3d4"), false)
        .unwrap();

    let raw = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    let file: EntryFile = serde_json::from_str(&raw).unwrap();

    assert_eq!(file.id, "a1b2c3d4");
    assert_eq!(file.category, "login");
    assert_eq!(file.revision, 3);
    assert!(!file.encrypted_blob.is_empty());

    assert!(!raw.contains("password"));
    assert!(!raw.contains("s3cret!"));
    assert!(!raw.contains("title"));
    assert!(!raw.contains("totpSecret"));
}

#[test]
fn import_with_wrong_master_key_fails() {
    let (_dir, mgr) = manager();
    mgr.export_entry(&master_key(), &snapshot("a1b2c3d4"), false)
        .unwrap();

    assert!(mgr.import_entry(&other_key(), "a1b2c3d4").is_err());
}

#[test]
fn list_and_remove_entry_files() {
    let (_dir, mgr) = manager();
    let key = master_key();

    mgr.export_entry(&key, &snapshot("bbb"), false).unwrap();
    mgr.export_entry(&key, &snapshot("aaa"), false).unwrap();
    assert_eq!(mgr.list_entry_files().unwrap(), vec!["aaa", "bbb"]);

    mgr.remove_entry_file("aaa").unwrap();
    assert_eq!(mgr.list_entry_files().unwrap(), vec!["bbb"]);

    mgr.remove_entry_file("missing").unwrap();
}

#[test]
fn exported_vault_meta_is_signed_and_verifies() {
    let (_dir, mgr) = manager();
    let key = master_key();

    mgr.export_vault_meta(&key, None, None).unwrap();

    let meta: VaultMetaFile =
        serde_json::from_str(&fs::read_to_string(meta_path(mgr.vault_dir())).unwrap()).unwrap();
    assert!(!meta.integrity.is_empty());

    let result = mgr.verify_vault_meta_integrity(&key).unwrap();
    assert!(result.valid, "{}", result.message);
}

#[test]
fn tampered_vault_meta_fails_verification() {
    let (_dir, mgr) = manager();
    let key = master_key();
    mgr.export_vault_meta(&key, None, None).unwrap();

    let mut meta: VaultMetaFile =
        serde_json::from_str(&fs::read_to_string(meta_path(mgr.vault_dir())).unwrap()).unwrap();
    meta.version = 2;
    write_meta(mgr.vault_dir(), &meta);

    let result = mgr.verify_vault_meta_integrity(&key).unwrap();
    assert!(!result.valid);
}

#[test]
fn unsigned_vault_meta_fails_verification() {
    let (_dir, mgr) = manager();
    let key = master_key();
    mgr.export_vault_meta(&key, None, None).unwrap();

    let mut meta: VaultMetaFile =
        serde_json::from_str(&fs::read_to_string(meta_path(mgr.vault_dir())).unwrap()).unwrap();
    meta.integrity = String::new();
    write_meta(mgr.vault_dir(), &meta);

    let result = mgr.verify_vault_meta_integrity(&key).unwrap();
    assert!(!result.valid);
}

#[test]
fn missing_vault_meta_fails_verification() {
    let (_dir, mgr) = manager();
    mgr.ensure_structure().unwrap();

    let result = mgr.verify_vault_meta_integrity(&master_key()).unwrap();
    assert!(!result.valid);
}

#[test]
fn vault_meta_signed_by_other_key_fails_verification() {
    let (_dir, mgr) = manager();
    mgr.export_vault_meta(&master_key(), None, None).unwrap();

    let result = mgr.verify_vault_meta_integrity(&other_key()).unwrap();
    assert!(!result.valid);
}

#[test]
fn export_entry_skips_rewrite_when_revision_unchanged() {
    let (_dir, mgr) = manager();
    let key = master_key();
    let snap = snapshot("a1b2c3d4");

    mgr.export_entry(&key, &snap, false).unwrap();
    let first = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();

    mgr.export_entry(&key, &snap, false).unwrap();
    let second = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    assert_eq!(first, second);

    mgr.export_entry(&key, &snap, true).unwrap();
    let forced = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    assert_ne!(first, forced);
}

#[test]
fn export_entry_rewrites_when_revision_changes() {
    let (_dir, mgr) = manager();
    let key = master_key();

    mgr.export_entry(&key, &snapshot("a1b2c3d4"), false)
        .unwrap();
    let first = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();

    let mut updated = snapshot("a1b2c3d4");
    updated.revision = 4;
    updated.title = "GitHub (renamed)".to_string();
    mgr.export_entry(&key, &updated, false).unwrap();

    let second = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    assert_ne!(first, second);
    assert_eq!(mgr.import_entry(&key, "a1b2c3d4").unwrap().revision, 4);
}

#[test]
fn export_vault_meta_skips_rewrite_when_content_unchanged() {
    let (_dir, mgr) = manager();
    let key = master_key();

    mgr.export_vault_meta(&key, None, None).unwrap();
    let first = fs::read_to_string(meta_path(mgr.vault_dir())).unwrap();

    mgr.export_vault_meta(&key, None, None).unwrap();
    let second = fs::read_to_string(meta_path(mgr.vault_dir())).unwrap();
    assert_eq!(first, second);
}

#[test]
fn read_all_entries_reports_unreadable_ids() {
    let (_dir, mgr) = manager();

    mgr.export_entry(&master_key(), &snapshot("ok"), false)
        .unwrap();
    mgr.export_entry(&other_key(), &snapshot("bad"), false)
        .unwrap();

    let (entries, unreadable) = mgr.read_all_entries(&master_key()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ok");
    assert_eq!(unreadable, vec!["bad"]);
}
