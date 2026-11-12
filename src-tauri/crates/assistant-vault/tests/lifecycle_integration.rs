mod common;

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::derive_db_key;
use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::biometric::BiometricKeyStore;
use assistant_vault::dto::{VaultEntryCreate, VaultEntryUpdate};
use assistant_vault::recovery_key::open_recovery_session;
use assistant_vault::security_db::{LockReason, SecurityDbState, SQLCIPHER_PRAGMAS};
use assistant_vault::types::{VaultCategory, VaultEntrySnapshot};
use assistant_vault::vault_file::VaultFileManager;
use common::{cleanup, repo_for, temp_db_path, STRONG_PW};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

const NEW_PW: &str = "N3w#P@ssw0rd!2026";
const OTHER_PW: &str = "Wr0ng#P@ssw0rd!2025";

fn snapshot(id: &str, dek_version: u32) -> VaultEntrySnapshot {
    VaultEntrySnapshot {
        id: id.to_string(),
        category: VaultCategory::Login,
        revision: 1,
        dek_version,
        title: "Example".to_string(),
        username: Some("user".to_string()),
        url: Some("https://example.com".to_string()),
        password: Some("secret".to_string()),
        notes: Some("note".to_string()),
        totp_secret: Some(
            r#"{"secret":"JBSWY3DPEHPK3PXP","digits":6,"period":30,"algorithm":"sha1"}"#
                .to_string(),
        ),
        custom_fields: Vec::new(),
        tags: vec!["tag".to_string()],
        is_favorite: false,
        created_at: 1,
        updated_at: 1,
    }
}

fn entry_hash(repo: &VaultFileManager, id: &str) -> String {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;
    let bytes = fs::read(repo.entry_file_path(id)).unwrap();
    B64.encode(Sha256::digest(&bytes))
}

fn export_with_meta(
    repo: &VaultFileManager,
    dek: &assistant_crypto::vault_crypto::types::Dek,
    snapshot: &VaultEntrySnapshot,
) -> Result<(), VaultError> {
    repo.export_entry(dek, snapshot)
        .map_err(|e| VaultError::Other(e.to_string()))?;
    assistant_vault::lifecycle::sync_write_meta(repo, dek)?;
    Ok(())
}

fn entry_create(title: &str) -> VaultEntryCreate {
    VaultEntryCreate {
        category: VaultCategory::Login,
        title: title.to_string(),
        username: Some("user".to_string()),
        url: Some("https://example.com".to_string()),
        password: Some("secret".to_string()),
        notes: Some("note".to_string()),
        totp: None,
        tags: vec!["tag".to_string()],
        custom_fields: Vec::new(),
        is_favorite: false,
    }
}

fn entry_update(title: &str) -> VaultEntryUpdate {
    VaultEntryUpdate {
        category: VaultCategory::Login,
        title: title.to_string(),
        username: Some("user".to_string()),
        url: Some("https://example.com".to_string()),
        password: Some("secret".to_string()),
        notes: Some("note".to_string()),
        totp: None,
        tags: vec!["tag".to_string()],
        custom_fields: Vec::new(),
        is_favorite: false,
    }
}

/// 空 keystore：`rotate_dek` 第 1 步要求 `vault-payload` 删除成功，
/// 不存在即视为已删除（与平台语义一致）。
struct EmptyKeyStore;

impl BiometricKeyStore for EmptyKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        Ok(true)
    }
    fn set_key(&self, _key_id: &str, _data: &[u8]) -> Result<(), VaultError> {
        Ok(())
    }
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        Err(VaultError::BiometricFailed(format!("missing {}", key_id)))
    }
    fn delete_key(&self, _key_id: &str) -> Result<(), VaultError> {
        Ok(())
    }
    fn has_key(&self, _key_id: &str) -> Result<bool, VaultError> {
        Ok(false)
    }
    fn request_signature(&self, _key_id: &str) -> Result<Vec<u8>, VaultError> {
        Err(VaultError::BiometricFailed("no signature".to_string()))
    }
    fn acl_satisfied(&self) -> Result<bool, VaultError> {
        Ok(true)
    }
}

fn session_dek_bytes(db: &SecurityDbState) -> [u8; 20] {
    let guard = db.get_dek().unwrap();
    guard.as_ref().unwrap().to_bytes()
}

fn file_state(path: &Path) -> (String, Option<SystemTime>) {
    let hash = hex::encode(Sha256::digest(fs::read(path).unwrap()));
    (
        hash,
        fs::metadata(path).ok().and_then(|m| m.modified().ok()),
    )
}

fn write_raw_meta(repo: &VaultFileManager, json: &str) {
    fs::create_dir_all(repo.vault_dir()).unwrap();
    fs::write(repo.meta_path(), json).unwrap();
}

fn open_with_db_key(path: &Path, db_key: &[u8; 32]) -> Connection {
    let key_hex = hex::encode(db_key);
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(&format!(
        "PRAGMA key = \"x'{key_hex}'\"; {SQLCIPHER_PRAGMAS}"
    ))
    .unwrap();
    conn
}

#[test]
fn i0_initialize_rejects_dirty_repository() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    fs::create_dir_all(repo.vault_dir()).unwrap();
    fs::write(repo.vault_dir().join(".vault-meta.bak"), b"stale").unwrap();

    let err = assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap_err();
    assert!(
        matches!(&err, VaultError::Other(msg) if msg.contains(".vault-meta.bak")),
        "unexpected: {:?}",
        err
    );
    assert!(
        !repo.meta_path().exists(),
        "dirty repo must not be initialized"
    );

    cleanup(&path);
}

#[test]
fn i0b_initialize_refuses_to_overwrite_existing_vault() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();
    fs::remove_file(&path).unwrap();

    let err = assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap_err();
    assert!(matches!(err, VaultError::AlreadyExists), "got {:?}", err);
    assert!(repo.meta_path().exists(), "existing meta must be preserved");

    cleanup(&path);
}

#[test]
fn i0c_initialize_reports_v1_vault_as_unsupported() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    write_raw_meta(&repo, r#"{"version":1,"crypto":{}}"#);

    let err = assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap_err();
    assert!(
        matches!(err, VaultError::UnsupportedVaultVersion),
        "got {:?}",
        err
    );
    assert!(
        repo.meta_path().exists(),
        "v1 meta must be preserved for the reset flow"
    );

    cleanup(&path);
}

#[test]
fn i1_initialize_writes_v2_meta_with_single_password_slot() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);

    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.version, 2);
    assert_eq!(meta.dek_version, 1);
    assert_eq!(meta.slots.len(), 1);
    assert_eq!(meta.slots[0].id, "pwd-1");
    assert_eq!(meta.slots[0].slot_type, "password");
    assert_eq!(meta.slots[0].payload_kind, "dek");
    assert_eq!(meta.manifest.count, 0);
    assert!(meta.manifest.entries.is_empty());
    assert!(meta.recovery_backup.is_none());
    assert!(db.get_dek().unwrap().as_ref().is_some());

    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn i2_change_password_swaps_slot_but_keeps_dek_and_db_key() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let dek_before = session_dek_bytes(&db);
    let vault_id = repo.read_vault_meta_v2().unwrap().vault_id.clone();
    let db_key_before = derive_db_key(
        &assistant_crypto::vault_crypto::types::Dek::new(dek_before),
        &vault_id,
    );

    {
        let guard = db.get_dek().unwrap();
        repo.export_entry(guard.as_ref().unwrap(), &snapshot("e2e00001", 1))
            .unwrap();
    }
    let entry_hash_before = entry_hash(&repo, "e2e00001");

    let meta_before = repo.read_vault_meta_v2().unwrap();
    let wrapped_before = meta_before.slots[0].wrapped_dek.clone();

    assistant_vault::lifecycle::change_password(&db, &repo, STRONG_PW, NEW_PW).unwrap();

    let meta_after = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta_after.slots.len(), 1);
    assert_eq!(meta_after.slots[0].id, "pwd-2");
    assert_eq!(meta_after.dek_version, meta_before.dek_version);
    assert_ne!(meta_after.slots[0].wrapped_dek, wrapped_before);
    assert_ne!(meta_after.slots[0].kdf.salt, meta_before.slots[0].kdf.salt);
    assert_eq!(meta_after.vault_id, vault_id);

    assert_eq!(session_dek_bytes(&db), dek_before);
    let db_key_after = derive_db_key(
        &assistant_crypto::vault_crypto::types::Dek::new(session_dek_bytes(&db)),
        &vault_id,
    );
    assert_eq!(db_key_after.expose(), db_key_before.expose());

    assert_eq!(entry_hash(&repo, "e2e00001"), entry_hash_before);

    db.lock().unwrap();
    let conn = open_with_db_key(&path, db_key_before.expose());
    assert!(
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r
            .get::<_, i64>(0))
            .is_ok(),
        "改密后本机库必须仍可用改密前的 dbKey 打开（未触发 PRAGMA rekey）"
    );
    drop(conn);

    cleanup(&path);
}

#[test]
fn i3_old_dek_still_decrypts_entries_after_password_change() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let dek_before = assistant_crypto::vault_crypto::types::Dek::new(session_dek_bytes(&db));
    let original = snapshot("e3e00001", 1);
    repo.export_entry(&dek_before, &original).unwrap();

    assistant_vault::lifecycle::change_password(&db, &repo, STRONG_PW, NEW_PW).unwrap();
    db.lock().unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, NEW_PW).unwrap();

    let imported = repo.import_entry(&dek_before, "e3e00001").unwrap();
    assert_eq!(imported.blob.title, original.title);
    assert_eq!(imported.blob.password, original.password);
    assert_eq!(
        imported.blob.totp.expect("totp 必须保留").secret,
        "JBSWY3DPEHPK3PXP"
    );

    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn i4_wrong_password_returns_invalid_password() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let err = assistant_vault::lifecycle::unlock(&db, &repo, OTHER_PW).unwrap_err();
    assert!(matches!(err, VaultError::InvalidPassword), "got {:?}", err);

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn i4_meta_tamper_returns_rollback_detected() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    let raw = fs::read_to_string(repo.meta_path()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    value["integrity"] = serde_json::Value::String("AAAA".to_string());
    write_raw_meta(&repo, &serde_json::to_string_pretty(&value).unwrap());

    let err = assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap_err();
    assert!(
        matches!(err, VaultError::VaultRollbackDetected),
        "got {:?}",
        err
    );

    cleanup(&path);
}

#[test]
fn i17_rebuild_local_db_recovers_index_from_entry_files() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let dek = assistant_crypto::vault_crypto::types::Dek::new(session_dek_bytes(&db));
    let vault_id = repo.read_vault_meta_v2().unwrap().vault_id.clone();
    let db_key = derive_db_key(&dek, &vault_id);

    let first = snapshot("e17e0001", 1);
    let second = snapshot("e17e0002", 1);
    repo.export_entry(&dek, &first).unwrap();
    repo.export_entry(&dek, &second).unwrap();

    let anchor = {
        let bytes = fs::read(repo.meta_path()).unwrap();
        assistant_vault::security_db::Anchor {
            meta_hash: hex::encode(Sha256::digest(&bytes)),
            dek_version: 1,
            revision: 3,
        }
    };

    db.lock().unwrap();
    db.rebuild_local_db(&repo, &dek, &db_key, &anchor).unwrap();

    let backups = fs::read_dir(path.parent().unwrap().join("backups")).unwrap();
    assert!(backups.count() > 0, "重建前必须保留一份旧库备份");
    assert!(path.exists(), "重建后必须存在新的 security.db");

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();

    let count: i64 = db
        .with_conn(|conn| {
            conn.query_row("SELECT count(*) FROM vault_entries", [], |row| row.get(0))
                .map_err(|e| VaultError::Database(e.to_string()))
        })
        .unwrap();
    assert_eq!(count, 2, "索引必须从 entries/*.enc 全量重建");

    let revision: String = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT value FROM vault_meta WHERE key = 'anchor_revision'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| VaultError::Database(e.to_string()))
        })
        .unwrap();
    assert_eq!(revision, "0", "重建后 anchor revision 必须重置为 0");

    let rebuilt_events: i64 = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT count(*) FROM security_audit_log WHERE event_type = 'local_db_rebuilt'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| VaultError::Database(e.to_string()))
        })
        .unwrap();
    assert_eq!(rebuilt_events, 1, "重建必须落一条 LocalDbRebuilt 审计");

    let detail = db.get_entry("e17e0001").unwrap();
    assert_eq!(detail.title, first.title);
    assert_eq!(detail.username, first.username);
    assert!(detail.has_password);
    assert!(detail.has_totp);
    assert_eq!(detail.tags, first.tags);

    db.lock().unwrap();
    cleanup(&path);
}

#[test]
fn i5_strict_meta_parse_rejects_v1_and_unknown_meta() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);

    write_raw_meta(&repo, r#"{"version":1,"crypto":{}}"#);
    let err = repo.read_vault_meta_v2().unwrap_err();
    assert!(
        matches!(err, VaultError::UnsupportedVaultVersion),
        "version=1 got {:?}",
        err
    );

    write_raw_meta(&repo, r#"{"version":3,"vaultId":"aaaa","slots":[]}"#);
    let err = repo.read_vault_meta_v2().unwrap_err();
    assert!(
        matches!(err, VaultError::VaultVersionTooNew),
        "version=3 必须与损坏区分，got {:?}",
        err
    );

    write_raw_meta(&repo, r#"{"version":0,"vaultId":"aaaa","slots":[]}"#);
    let err = repo.read_vault_meta_v2().unwrap_err();
    assert!(
        matches!(err, VaultError::VaultCorrupted(_)),
        "version=0 不是版本过新，got {:?}",
        err
    );

    write_raw_meta(&repo, r#"{"crypto":{"masterKeySalt":"AAAA"},"entries":[]}"#);
    let err = repo.read_vault_meta_v2().unwrap_err();
    assert!(
        matches!(err, VaultError::UnsupportedVaultVersion),
        "v1 特征键 got {:?}",
        err
    );

    write_raw_meta(&repo, r#"{"vaultId":"aaaa","slots":[]}"#);
    let err = repo.read_vault_meta_v2().unwrap_err();
    assert!(
        matches!(err, VaultError::VaultCorrupted(_)),
        "无版本无特征键 got {:?}",
        err
    );

    drop(db);
    cleanup(&path);
}

#[test]
fn i10_unsupported_version_does_not_touch_files() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    db.lock().unwrap();

    write_raw_meta(&repo, r#"{"version":1,"crypto":{"masterKeySalt":"AAAA"}}"#);
    let before = file_state(&repo.meta_path());

    let err = assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap_err();
    assert!(
        matches!(err, VaultError::UnsupportedVaultVersion),
        "got {:?}",
        err
    );

    let after = file_state(&repo.meta_path());
    assert_eq!(before.0, after.0, "严格解析失败不得改写 .vault-meta 内容");
    assert_eq!(before.1, after.1, "严格解析失败不得改写 .vault-meta mtime");

    cleanup(&path);
}

#[test]
fn i6_manifest_tracks_entry_crud_in_same_transaction() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let algo = CipherAlgorithm::Aes256Gcm;
    let snapshot = db
        .create_entry_tx(entry_create("first"), algo, |dek, snap| {
            export_with_meta(&repo, dek, snap)
        })
        .unwrap();

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.manifest.count, 1, "新建条目必须同事务写入 manifest");
    assert_eq!(meta.manifest.entries.len(), 1);
    assert_eq!(meta.manifest.entries[0].id, snapshot.id);
    assert_eq!(meta.manifest.entries[0].dek_version, meta.dek_version);
    assert_eq!(
        meta.manifest.entries[0].sha256,
        entry_hash(&repo, &snapshot.id)
    );
    repo.verify_manifest().unwrap();

    let before_sha = meta.manifest.entries[0].sha256.clone();
    db.update_entry_tx(&snapshot.id, entry_update("second"), algo, |dek, snap| {
        export_with_meta(&repo, dek, snap)
    })
    .unwrap();

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.manifest.count, 1);
    assert_ne!(
        meta.manifest.entries[0].sha256, before_sha,
        "内容变更后 manifest 哈希必须同步更新"
    );
    assert_eq!(
        meta.manifest.entries[0].sha256,
        entry_hash(&repo, &snapshot.id)
    );
    repo.verify_manifest().unwrap();

    db.delete_entry_tx(&snapshot.id, |dek, id| {
        repo.remove_entry_file(id)
            .map_err(|e| VaultError::Other(e.to_string()))?;
        assistant_vault::lifecycle::sync_write_meta(&repo, dek)?;
        Ok(())
    })
    .unwrap();

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.manifest.count, 0, "删除后 manifest 不得残留条目");
    assert!(meta.manifest.entries.is_empty());
    repo.verify_manifest().unwrap();

    let vault_id = meta.vault_id.clone();
    let dek_version = meta.dek_version;
    assistant_vault::lifecycle::change_password(
        &db,
        &repo,
        STRONG_PW,
        "another-strong-password-123",
    )
    .unwrap();

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.manifest.count, 0, "change_password 不得重算 manifest");
    assert!(meta.manifest.entries.is_empty());
    assert_eq!(meta.vault_id, vault_id);
    assert_eq!(meta.dek_version, dek_version);
    repo.verify_manifest().unwrap();

    db.lock().unwrap();
    cleanup(&path);
}

/// 回归：本地写 `.vault-meta`（条目 CRUD、push）后必须前移本机 anchor，
/// 否则锁库再解锁会误报 `VaultRollbackDetected`（key §4.6 `revision += 1`）。
#[test]
fn i7_local_meta_write_advances_anchor_before_lock() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let algo = CipherAlgorithm::Aes256Gcm;
    let snapshot = db
        .create_entry_tx(entry_create("anchor"), algo, |dek, snap| {
            export_with_meta(&repo, dek, snap)
        })
        .unwrap();
    assistant_vault::lifecycle::refresh_anchor(&db, &repo).unwrap();

    db.update_entry_tx(
        &snapshot.id,
        entry_update("anchor-v2"),
        algo,
        |dek, snap| export_with_meta(&repo, dek, snap),
    )
    .unwrap();
    assistant_vault::lifecycle::refresh_anchor(&db, &repo).unwrap();

    db.delete_entry_tx(&snapshot.id, |dek, id| {
        repo.remove_entry_file(id)
            .map_err(|e| VaultError::Other(e.to_string()))?;
        assistant_vault::lifecycle::sync_write_meta(&repo, dek)?;
        Ok(())
    })
    .unwrap();
    assistant_vault::lifecycle::refresh_anchor(&db, &repo).unwrap();

    let meta_hash = repo.meta_file_hash().unwrap();
    db.lock_session(LockReason::User).unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW)
        .expect("本地写 meta 后已前移 anchor，锁库再解锁不得误报回滚");

    let anchor = db.with_conn(|conn| {
        assistant_vault::security_db::meta::get_anchor(conn).map_err(VaultError::Database)
    });
    assert_eq!(
        anchor.unwrap().map(|a| a.meta_hash),
        Some(meta_hash),
        "解锁后 anchor 必须与本机 meta 文件 hash 一致"
    );

    db.lock_session(LockReason::User).unwrap();
    cleanup(&path);
}

/// I14：`rotate_dek` 全量重加密 —— `dekVersion+1`、条目密文全变、旧纸面失效、
/// 新纸面可用、`recoveryBackup` 清空、`dbKey` 随 `dek` 换钥（key §4.9）。
#[test]
fn i14_rotate_dek_reencrypts_entries_and_invalidates_old_paper() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();

    let id_a = db
        .create_entry_tx(
            entry_create("Rotate A"),
            CipherAlgorithm::Aes256Gcm,
            |dek, snap| export_with_meta(&repo, dek, snap),
        )
        .unwrap()
        .id;
    let id_b = db
        .create_entry_tx(
            entry_create("Rotate B"),
            CipherAlgorithm::Aes256Gcm,
            |dek, snap| export_with_meta(&repo, dek, snap),
        )
        .unwrap()
        .id;

    let dek_before = session_dek_bytes(&db);
    let meta_before = repo.read_vault_meta_v2().unwrap();
    let vault_id = meta_before.vault_id.clone();
    let db_key_before = derive_db_key(
        &assistant_crypto::vault_crypto::types::Dek::new(dek_before),
        &vault_id,
    );
    let hash_a_before = entry_hash(&repo, &id_a);
    let hash_b_before = entry_hash(&repo, &id_b);
    let old_paper = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    assert!(repo.read_vault_meta_v2().unwrap().recovery_backup.is_some());

    let rotated = assistant_vault::lifecycle::rotate_dek(&db, &repo, &EmptyKeyStore).unwrap();

    assert_eq!(rotated.dek_version, meta_before.dek_version + 1);
    let meta_after = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta_after.dek_version, 2);
    assert_eq!(meta_after.vault_id, vault_id, "轮换不得更换 vaultId");
    assert!(
        meta_after.recovery_backup.is_none(),
        "轮换必须清空 recoveryBackup（旧纸面已失效）"
    );
    assert_eq!(meta_after.manifest.count, 2);
    assert!(
        meta_after
            .manifest
            .entries
            .iter()
            .all(|entry| entry.dek_version == 2),
        "manifest 必须按新 dekVersion 全量重算"
    );
    assert_ne!(
        meta_after.slots[0].wrapped_dek, meta_before.slots[0].wrapped_dek,
        "slot 必须用当前 KEK 重包 dek_new"
    );
    repo.verify_manifest().unwrap();

    assert_ne!(
        session_dek_bytes(&db),
        dek_before,
        "轮换后会话 dek 必须是新 dek"
    );
    assert_ne!(
        entry_hash(&repo, &id_a),
        hash_a_before,
        "条目文件必须全量重加密"
    );
    assert_ne!(entry_hash(&repo, &id_b), hash_b_before);

    assert_eq!(
        db.decrypt_password(&id_a).unwrap().expose_str(),
        "secret",
        "轮换后条目仍须可读"
    );
    assert_eq!(db.list_entries(None, None, None).unwrap().len(), 2);

    assert!(
        matches!(
            open_recovery_session(&repo, &old_paper.paper_key),
            Err(VaultError::InvalidRecoveryKeyOrMetaTampered)
        ),
        "旧纸面必须失效"
    );
    let session = open_recovery_session(&repo, &rotated.paper_key).unwrap();
    assert_eq!(session.export_all().unwrap().entries.len(), 2);

    db.lock().unwrap();

    let key_hex = hex::encode(db_key_before.expose());
    let old_conn = Connection::open(&path).unwrap();
    old_conn
        .execute_batch(&format!(
            "PRAGMA key = \"x'{key_hex}'\"; {SQLCIPHER_PRAGMAS}"
        ))
        .unwrap();
    assert!(
        old_conn
            .query_row("SELECT count(*) FROM sqlite_master", [], |r| r
                .get::<_, i64>(0))
            .is_err(),
        "轮换后本机库必须已换钥（旧 dbKey 打不开）"
    );
    drop(old_conn);

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    assert_eq!(db.list_entries(None, None, None).unwrap().len(), 2);

    db.lock().unwrap();
    cleanup(&path);
}

/// I15：`rotate_dek` 必须在口令会话内执行 —— 锁定态调用直接拒绝，仓库零改动。
#[test]
fn i15_rotate_dek_requires_unlocked_password_session() {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    let id = db
        .create_entry_tx(
            entry_create("Rotate locked"),
            CipherAlgorithm::Aes256Gcm,
            |dek, snap| export_with_meta(&repo, dek, snap),
        )
        .unwrap()
        .id;

    let meta_before = repo.read_vault_meta_v2().unwrap();
    let hash_before = entry_hash(&repo, &id);

    db.lock().unwrap();
    let err = assistant_vault::lifecycle::rotate_dek(&db, &repo, &EmptyKeyStore).unwrap_err();
    assert!(
        matches!(err, VaultError::Locked),
        "unexpected error: {err:?}"
    );

    let meta_after = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta_after.dek_version, meta_before.dek_version);
    assert_eq!(meta_after.vault_id, meta_before.vault_id);
    assert_eq!(entry_hash(&repo, &id), hash_before);

    cleanup(&path);
}
