mod sync_common;

use std::fs;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MetaKey};
use assistant_vault::vault_file::types::{
    EntryFileV2, KdfParams, ManifestEntry, VaultManifest, VaultMetaFileV2, VaultSlot,
};
use assistant_vault::vault_file::VaultFileManager;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sync_common::{dek, entry_snapshot as snapshot, other_dek, vault_manager as manager};

#[test]
fn export_import_roundtrip_preserves_all_fields() {
    let (_dir, mgr) = manager();
    let key = dek();
    let original = snapshot("a1b2c3d4");

    mgr.export_entry(&key, &original).unwrap();
    let imported = mgr.import_entry(&key, "a1b2c3d4").unwrap();

    assert_eq!(imported.id, original.id);
    assert_eq!(imported.category, original.category.to_string());
    assert_eq!(imported.revision, original.revision);
    assert_eq!(imported.blob.title, original.title);
    assert_eq!(imported.blob.username, original.username);
    assert_eq!(imported.blob.url, original.url);
    assert_eq!(imported.blob.password, original.password);
    assert_eq!(imported.blob.notes, original.notes);
    let totp = imported.blob.totp.expect("totp 必须随条目文件往返");
    assert_eq!(totp.secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(totp.digits, 6);
    assert_eq!(totp.period, 30);
    assert_eq!(totp.algorithm, "sha1");
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
    mgr.export_entry(&dek(), &snapshot("a1b2c3d4")).unwrap();

    let raw = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    let file: EntryFileV2 = serde_json::from_str(&raw).unwrap();

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
fn import_with_wrong_dek_fails() {
    let (_dir, mgr) = manager();
    mgr.export_entry(&dek(), &snapshot("a1b2c3d4")).unwrap();

    assert!(mgr.import_entry(&other_dek(), "a1b2c3d4").is_err());
}

#[test]
fn list_and_remove_entry_files() {
    let (_dir, mgr) = manager();
    let key = dek();

    mgr.export_entry(&key, &snapshot("bbb")).unwrap();
    mgr.export_entry(&key, &snapshot("aaa")).unwrap();
    assert_eq!(mgr.list_entry_files().unwrap(), vec!["aaa", "bbb"]);

    mgr.remove_entry_file("aaa").unwrap();
    assert_eq!(mgr.list_entry_files().unwrap(), vec!["bbb"]);

    mgr.remove_entry_file("missing").unwrap();
}

#[test]
fn export_entry_skips_rewrite_when_revision_unchanged() {
    let (_dir, mgr) = manager();
    let key = dek();
    let snap = snapshot("a1b2c3d4");

    mgr.export_entry(&key, &snap).unwrap();
    let first = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();

    mgr.export_entry(&key, &snap).unwrap();
    let second = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    assert_eq!(first, second);
}

#[test]
fn export_entry_rewrites_when_revision_changes() {
    let (_dir, mgr) = manager();
    let key = dek();

    mgr.export_entry(&key, &snapshot("a1b2c3d4")).unwrap();
    let first = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();

    let mut updated = snapshot("a1b2c3d4");
    updated.revision = 4;
    updated.title = "GitHub (renamed)".to_string();
    mgr.export_entry(&key, &updated).unwrap();

    let second = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    assert_ne!(first, second);
    assert_eq!(mgr.import_entry(&key, "a1b2c3d4").unwrap().revision, 4);
}

#[test]
fn read_all_entries_reports_unreadable_ids() {
    let (_dir, mgr) = manager();

    mgr.export_entry(&dek(), &snapshot("ok")).unwrap();
    mgr.export_entry(&other_dek(), &snapshot("bad")).unwrap();

    let (entries, unreadable) = mgr.read_all_entries(&dek()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ok");
    assert_eq!(unreadable, vec!["bad"]);
}

#[test]
fn decode_entry_file_rejects_corrupted_blob() {
    let (_dir, mgr) = manager();
    let key = dek();
    mgr.export_entry(&key, &snapshot("a1b2c3d4")).unwrap();

    let raw = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    let mut file: EntryFileV2 = serde_json::from_str(&raw).unwrap();
    file.encrypted_blob = B64.encode([0u8; 8]);
    let tampered = serde_json::to_string(&file).unwrap();

    let result = mgr.decode_entry_file(&key, &tampered);
    assert!(result.is_err(), "截断密文必须报错，不得静默返回默认值");
}

#[test]
fn decode_entry_file_rejects_negative_revision() {
    let (_dir, mgr) = manager();
    let key = dek();
    mgr.export_entry(&key, &snapshot("a1b2c3d4")).unwrap();

    let raw = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    let mut file: EntryFileV2 = serde_json::from_str(&raw).unwrap();
    file.revision = -1;
    let tampered = serde_json::to_string(&file).unwrap();

    assert!(mgr.decode_entry_file(&key, &tampered).is_err());
}

#[test]
fn t_c7_algorithm_switch_keeps_old_entries_readable() {
    let dir = tempfile::tempdir().unwrap();
    let key = dek();
    let old_mgr = VaultFileManager::new(
        dir.path().join("vault"),
        Box::new(|| CipherAlgorithm::Aes256Gcm),
    );
    old_mgr.export_entry(&key, &snapshot("a1b2c3d4")).unwrap();

    let new_mgr = VaultFileManager::new(
        dir.path().join("vault"),
        Box::new(|| CipherAlgorithm::ChaCha20Poly1305),
    );

    let imported = new_mgr.import_entry(&key, "a1b2c3d4").unwrap();
    assert_eq!(imported.blob.title, "GitHub");

    new_mgr.export_entry(&key, &snapshot("e5f6a7b8")).unwrap();
    let raw = fs::read_to_string(new_mgr.entry_file_path("e5f6a7b8")).unwrap();
    let file: EntryFileV2 = serde_json::from_str(&raw).unwrap();
    assert_eq!(file.cipher, "chacha20Poly1305");

    let old_raw = fs::read_to_string(new_mgr.entry_file_path("a1b2c3d4")).unwrap();
    let old_file: EntryFileV2 = serde_json::from_str(&old_raw).unwrap();
    assert_eq!(old_file.cipher, "aes256Gcm");
}

#[test]
fn t_c13_repository_forbidden_file_is_rejected() {
    let (_dir, mgr) = manager();
    mgr.export_entry(&dek(), &snapshot("a1b2c3d4")).unwrap();
    assert!(mgr.check_repository_layout().is_ok());

    fs::write(mgr.vault_dir().join("security.db"), b"plaintext-db").unwrap();
    let err = mgr.check_repository_layout().unwrap_err();
    assert!(
        matches!(
            err,
            assistant_sync::types::SyncError::RepositoryContainsForbiddenFile(_)
        ),
        "unexpected: {:?}",
        err
    );

    fs::remove_file(mgr.vault_dir().join("security.db")).unwrap();
    fs::write(mgr.entries_dir().join("notes.txt"), b"leak").unwrap();
    assert!(mgr.check_repository_layout().is_err());
}

#[test]
fn t_c9_entry_file_version_one_is_rejected() {
    let (_dir, mgr) = manager();
    fs::create_dir_all(mgr.entries_dir()).unwrap();
    fs::write(
        mgr.entry_file_path("a1b2c3d4"),
        br#"{"version":1,"id":"a1b2c3d4","category":"login","revision":1}"#,
    )
    .unwrap();

    let err = mgr.import_entry(&dek(), "a1b2c3d4").unwrap_err();
    assert!(
        matches!(
            err,
            assistant_sync::types::SyncError::UnsupportedEntryFileVersion(_)
        ),
        "unexpected: {:?}",
        err
    );
}

#[test]
fn t_c9_entry_file_missing_version_is_rejected() {
    let (_dir, mgr) = manager();
    fs::create_dir_all(mgr.entries_dir()).unwrap();
    fs::write(
        mgr.entry_file_path("a1b2c3d4"),
        br#"{"id":"a1b2c3d4","category":"login","revision":1}"#,
    )
    .unwrap();

    let err = mgr.import_entry(&dek(), "a1b2c3d4").unwrap_err();
    assert!(
        matches!(
            err,
            assistant_sync::types::SyncError::UnsupportedEntryFileVersion(_)
        ),
        "unexpected: {:?}",
        err
    );
}

#[test]
fn export_entry_writes_under_entries_dir() {
    let (_dir, mgr) = manager();
    mgr.export_entry(&dek(), &snapshot("a1b2c3d4")).unwrap();

    let path = mgr.entry_file_path("a1b2c3d4");
    assert!(
        path.ends_with(std::path::Path::new("entries").join("a1b2c3d4.enc")),
        "条目文件必须落在 entries/*.enc: {}",
        path.display()
    );
}

#[test]
fn export_entry_uses_configured_cipher_algorithm() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = VaultFileManager::new(
        dir.path().join("vault"),
        Box::new(|| CipherAlgorithm::ChaCha20Poly1305),
    );
    let key = dek();

    mgr.export_entry(&key, &snapshot("a1b2c3d4")).unwrap();

    let raw = fs::read_to_string(mgr.entry_file_path("a1b2c3d4")).unwrap();
    let file: EntryFileV2 = serde_json::from_str(&raw).unwrap();
    assert_eq!(file.cipher, "chacha20Poly1305");
    assert!(!B64.decode(&file.encrypted_blob).unwrap().is_empty());

    let imported = mgr.import_entry(&key, "a1b2c3d4").unwrap();
    assert_eq!(imported.blob.title, "GitHub");
}

fn meta_v2() -> VaultMetaFileV2 {
    VaultMetaFileV2 {
        version: 2,
        vault_id: "0123456789abcdef0123456789abcdef".to_string(),
        dek_version: 1,
        cipher: "aes256Gcm".to_string(),
        slots: vec![VaultSlot {
            id: "pwd-1".to_string(),
            slot_type: "password".to_string(),
            payload_kind: "dek".to_string(),
            kdf: KdfParams {
                algo: "argon2id".to_string(),
                salt: B64.encode([0u8; 16]),
                m: 65536,
                t: 3,
                p: 4,
                v: 19,
            },
            wrapped_dek: B64.encode([0u8; 48]),
            created_at: 1_729_468_800_000,
        }],
        manifest: VaultManifest {
            count: 0,
            entries: Vec::new(),
        },
        recovery_backup: None,
        integrity: String::new(),
    }
}

fn write_raw_meta(mgr: &VaultFileManager, value: &serde_json::Value) {
    fs::create_dir_all(mgr.vault_dir()).unwrap();
    fs::write(mgr.meta_path(), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn read_vault_meta_v2_rejects_version_one() {
    let (_dir, mgr) = manager();
    let mut value = serde_json::to_value(meta_v2()).unwrap();
    value["version"] = serde_json::json!(1);
    write_raw_meta(&mgr, &value);

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::UnsupportedVaultVersion)
    ));
}

#[test]
fn read_vault_meta_v2_rejects_v1_signature_without_version() {
    let (_dir, mgr) = manager();
    write_raw_meta(
        &mgr,
        &serde_json::json!({
            "crypto": { "masterKeySalt": "AAAA", "hashSalt": "BBBB" }
        }),
    );

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::UnsupportedVaultVersion)
    ));
}

#[test]
fn read_vault_meta_v2_rejects_emergency_only_v1() {
    let (_dir, mgr) = manager();
    write_raw_meta(
        &mgr,
        &serde_json::json!({ "emergency": { "token": "t", "salt": "s" } }),
    );

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::UnsupportedVaultVersion)
    ));
}

#[test]
fn read_vault_meta_v2_rejects_missing_version_without_markers() {
    let (_dir, mgr) = manager();
    write_raw_meta(&mgr, &serde_json::json!({ "slots": [] }));

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::VaultCorrupted(_))
    ));
}

#[test]
fn read_vault_meta_v2_rejects_unparsable_file() {
    let (_dir, mgr) = manager();
    fs::create_dir_all(mgr.vault_dir()).unwrap();
    fs::write(mgr.meta_path(), b"not json").unwrap();

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::VaultCorrupted(_))
    ));
}

#[test]
fn read_vault_meta_v2_requires_not_initialized_when_absent() {
    let (_dir, mgr) = manager();
    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::NotInitialized)
    ));
}

#[test]
fn read_vault_meta_v2_rejects_invalid_slot_id() {
    let (_dir, mgr) = manager();
    let mut value = serde_json::to_value(meta_v2()).unwrap();
    value["slots"][0]["id"] = serde_json::json!("pwd-0");
    write_raw_meta(&mgr, &value);

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::VaultCorrupted(_))
    ));
}

#[test]
fn read_vault_meta_v2_rejects_unsorted_manifest() {
    let (_dir, mgr) = manager();
    let mut meta = meta_v2();
    meta.manifest = VaultManifest {
        count: 2,
        entries: vec![
            ManifestEntry {
                id: "b".to_string(),
                sha256: B64.encode([1u8; 32]),
                dek_version: 1,
            },
            ManifestEntry {
                id: "a".to_string(),
                sha256: B64.encode([2u8; 32]),
                dek_version: 1,
            },
        ],
    };
    write_raw_meta(&mgr, &serde_json::to_value(meta).unwrap());

    assert!(matches!(
        mgr.read_vault_meta_v2(),
        Err(VaultError::VaultCorrupted(_))
    ));
}

#[test]
fn read_vault_meta_v2_accepts_valid_meta() {
    let (_dir, mgr) = manager();
    let meta_key = MetaKey::new([3u8; 32]);
    let mut meta = meta_v2();
    mgr.write_vault_meta_v2(&mut meta, &meta_key).unwrap();

    let read = mgr.read_vault_meta_v2().unwrap();
    assert_eq!(read.version, 2);
    assert_eq!(read.vault_id, meta.vault_id);
    assert_eq!(read.slots.len(), 1);
    assert_eq!(read.slots[0].slot_type, "password");
    assert!(!read.integrity.is_empty());
}

#[test]
fn write_vault_meta_v2_signs_and_verifies() {
    let (_dir, mgr) = manager();
    let meta_key = MetaKey::new([3u8; 32]);
    let mut meta = meta_v2();
    mgr.write_vault_meta_v2(&mut meta, &meta_key).unwrap();

    let result = mgr.verify_meta_integrity(&meta_key).unwrap();
    assert!(result.valid, "{}", result.message);

    let other = mgr.verify_meta_integrity(&MetaKey::new([4u8; 32])).unwrap();
    assert!(!other.valid);
}

#[test]
fn tampered_meta_v2_fails_integrity() {
    let (_dir, mgr) = manager();
    let meta_key = MetaKey::new([3u8; 32]);
    let mut meta = meta_v2();
    mgr.write_vault_meta_v2(&mut meta, &meta_key).unwrap();

    let mut value = serde_json::to_value(mgr.read_vault_meta_v2().unwrap()).unwrap();
    value["dekVersion"] = serde_json::json!(9);
    write_raw_meta(&mgr, &value);

    let result = mgr.verify_meta_integrity(&meta_key).unwrap();
    assert!(!result.valid);
}

#[test]
fn canonical_meta_bytes_matches_golden_vector() {
    let meta = meta_v2();
    let bytes = assistant_vault::vault_file::canonical_meta_bytes(&meta).unwrap();
    let text = String::from_utf8(bytes).unwrap();

    let salt = B64.encode([0u8; 16]);
    let wrapped = B64.encode([0u8; 48]);
    let expected = format!(
        concat!(
            r#"{{"version":2,"vaultId":"0123456789abcdef0123456789abcdef","dekVersion":1,"#,
            r#""cipher":"aes256Gcm","slots":[{{"id":"pwd-1","type":"password","#,
            r#""payloadKind":"dek","kdf":{{"algo":"argon2id","salt":"{}","m":65536,"#,
            r#""t":3,"p":4,"v":19}},"wrappedDek":"{}","createdAt":1729468800000}}],"#,
            r#""manifest":{{"count":0,"entries":[]}},"recoveryBackup":null}}"#
        ),
        salt, wrapped
    );

    assert_eq!(text, expected);
}

#[test]
fn meta_file_hash_tracks_full_file_bytes() {
    let (_dir, mgr) = manager();
    let meta_key = MetaKey::new([3u8; 32]);
    let mut meta = meta_v2();
    mgr.write_vault_meta_v2(&mut meta, &meta_key).unwrap();
    let first = mgr.meta_file_hash().unwrap();

    let raw = fs::read(mgr.meta_path()).unwrap();
    fs::write(mgr.meta_path(), raw).unwrap();
    assert_eq!(mgr.meta_file_hash().unwrap(), first);

    let mut reformatted = serde_json::to_value(mgr.read_vault_meta_v2().unwrap()).unwrap();
    reformatted["slots"][0]["createdAt"] = serde_json::json!(1);
    write_raw_meta(&mgr, &reformatted);
    assert_ne!(mgr.meta_file_hash().unwrap(), first);
}

#[test]
fn compute_manifest_lists_entries_sorted_by_id() {
    let (_dir, mgr) = manager();
    let key = dek();
    mgr.export_entry(&key, &snapshot("b2")).unwrap();
    mgr.export_entry(&key, &snapshot("a1")).unwrap();

    let manifest = mgr.compute_manifest(2).unwrap();
    assert_eq!(manifest.count, 2);
    assert_eq!(manifest.entries[0].id, "a1");
    assert_eq!(manifest.entries[1].id, "b2");
    for entry in &manifest.entries {
        assert_eq!(B64.decode(&entry.sha256).unwrap().len(), 32);
        assert_eq!(entry.dek_version, 2);
    }
}

#[test]
fn verify_manifest_detects_entry_mutation() {
    let (_dir, mgr) = manager();
    let key = dek();
    mgr.export_entry(&key, &snapshot("a1")).unwrap();

    let meta_key = MetaKey::new([3u8; 32]);
    let mut meta = meta_v2();
    meta.manifest = mgr.compute_manifest(meta.dek_version).unwrap();
    mgr.write_vault_meta_v2(&mut meta, &meta_key).unwrap();
    mgr.verify_manifest().unwrap();

    let mut tampered = snapshot("a1");
    tampered.revision = 99;
    mgr.export_entry(&key, &tampered).unwrap();
    assert!(matches!(
        mgr.verify_manifest(),
        Err(VaultError::VaultManifestMismatch)
    ));
}
