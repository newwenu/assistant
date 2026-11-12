mod common;

use std::fs;
use std::path::Path;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_sync::types::SyncConfig;
use assistant_vault::audit::{query_log, AuditEvent, AuditJsonlWriter};
use assistant_vault::biometric::BiometricKeyStore;
use assistant_vault::dto::VaultEntryCreate;
use assistant_vault::recovery_key::open_recovery_session;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;
use assistant_vault::types::VaultCategory;
use assistant_vault::vault_file::VaultFileManager;
use common::{cleanup, repo_for, temp_db_path, STRONG_PW};
use sha2::{Digest, Sha256};

const NEW_PW: &str = "N3w#P@ssw0rd!2026";

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

fn setup(path: &Path) -> (SecurityDbState, VaultFileManager) {
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    db.create_entry(sample_entry(), CipherAlgorithm::Aes256Gcm)
        .unwrap();
    (db, repo)
}

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

fn jsonl_events(db: &SecurityDbState) -> Vec<String> {
    db.audit_writer()
        .read_since(0)
        .unwrap()
        .into_iter()
        .map(|record| record.event)
        .collect()
}

fn db_events(db: &SecurityDbState) -> Vec<String> {
    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    query_log(conn, 1000)
        .unwrap()
        .into_iter()
        .map(|entry| entry.event_type)
        .collect()
}

fn db_file_hash(path: &Path) -> String {
    hex::encode(Sha256::digest(fs::read(path).unwrap()))
}

/// T-A1：纸面恢复导出 → JSONL 有 `recovery_export_used`，库文件零写入。
#[test]
fn t_a1_paper_recovery_writes_jsonl_only() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    engine_for(&path).export_all_entries(&db, &[]).unwrap();
    let session = open_recovery_session(&repo, &exported.paper_key).unwrap();
    let export = session.export_all().unwrap();
    assert_eq!(export.entries.len(), 1);

    db.lock().unwrap();
    let before = db_file_hash(&path);

    db.audit_jsonl(
        AuditEvent::RecoveryExportUsed,
        Some(&export.entries.len().to_string()),
    )
    .unwrap();

    assert_eq!(db_file_hash(&path), before, "恢复态审计不得写 security.db");
    let records = db.audit_writer().read_since(0).unwrap();
    let last = records.last().unwrap();
    assert_eq!(last.event, "recovery_export_used");
    assert_eq!(last.detail.as_deref(), Some("1"));

    cleanup(&path);
}

/// T-A2：导出恢复纸面 → 双写都有 `recovery_key_exported`。
#[test]
fn t_a2_export_recovery_key_dual_writes() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();

    assert!(jsonl_events(&db).contains(&"recovery_key_exported".to_string()));
    assert!(db_events(&db).contains(&"recovery_key_exported".to_string()));

    cleanup(&path);
}

/// T-A3：改密 → 双写有 `master_password_change`，且没有 `recovery_key_invalidated`。
#[test]
fn t_a3_change_password_dual_writes_without_invalidated() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    db.change_master_password(&repo, STRONG_PW, NEW_PW).unwrap();

    let jsonl = jsonl_events(&db);
    let db_log = db_events(&db);
    assert!(jsonl.contains(&"master_password_change".to_string()));
    assert!(db_log.contains(&"master_password_change".to_string()));
    assert!(!jsonl.contains(&"recovery_key_invalidated".to_string()));
    assert!(!db_log.contains(&"recovery_key_invalidated".to_string()));

    cleanup(&path);
}

/// T-A4：轮换数据密钥 → 双写都有 `dek_rotated`，且记录新版本号。
#[test]
fn t_a4_rotate_dek_dual_writes() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let rotated = assistant_vault::lifecycle::rotate_dek(&db, &repo, &EmptyKeyStore).unwrap();
    assert_eq!(rotated.dek_version, 2);

    assert!(jsonl_events(&db).contains(&"dek_rotated".to_string()));
    assert!(db_events(&db).contains(&"dek_rotated".to_string()));

    db.lock().unwrap();
    cleanup(&path);
}

/// 空 keystore：`rotate_dek` 第 1 步删除 `vault-payload`，不存在即视为成功。
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

/// T-A5：生物识别解锁失败 → JSONL 有 `biometric_failed{reason}`，仍未解锁。
struct FailingKeyStore {
    sig_hash: Vec<u8>,
}

impl FailingKeyStore {
    fn new() -> Self {
        let stored = Sha256::digest(b"other-signature");
        Self {
            sig_hash: hex::encode(stored).into_bytes(),
        }
    }
}

impl BiometricKeyStore for FailingKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        Ok(true)
    }
    fn set_key(&self, _key_id: &str, _data: &[u8]) -> Result<(), VaultError> {
        Ok(())
    }
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        match key_id {
            "vault-payload" => Ok(b"not-a-valid-blob".to_vec()),
            "vault-sig-hash" => Ok(self.sig_hash.clone()),
            "vault-wrapping-key" => Ok(b"zz".to_vec()),
            _ => Err(VaultError::BiometricFailed("missing".to_string())),
        }
    }
    fn delete_key(&self, _key_id: &str) -> Result<(), VaultError> {
        Ok(())
    }
    fn has_key(&self, _key_id: &str) -> Result<bool, VaultError> {
        Ok(true)
    }
    fn request_signature(&self, _key_id: &str) -> Result<Vec<u8>, VaultError> {
        Ok(b"attacker-signature".to_vec())
    }
}

#[test]
fn t_a5_biometric_failure_writes_jsonl_reason() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);
    db.lock().unwrap();

    assert!(db.unlock_biometric(&repo, &FailingKeyStore::new()).is_err());
    assert!(db.is_locked());

    let expected_reason = if cfg!(target_os = "windows") {
        "sig_changed"
    } else {
        "wrapping_key_len"
    };
    let records = db.audit_writer().read_since(0).unwrap();
    let failed = records
        .iter()
        .find(|record| record.event == "biometric_failed")
        .expect("biometric_failed 必须落 JSONL");
    assert_eq!(failed.detail.as_deref(), Some(expected_reason));

    cleanup(&path);
}

/// T-A6：重置仓库 → JSONL 有 `vault_repository_reset`，DB 已不存在。
#[test]
fn t_a6_repository_reset_writes_jsonl_only() {
    let path = temp_db_path();
    let (db, _repo) = setup(&path);
    db.lock().unwrap();

    let security_dir = path.parent().unwrap().join("security");
    let _ = fs::remove_dir_all(security_dir.join("audit"));
    fs::remove_file(&path).unwrap();

    db.audit_jsonl(AuditEvent::VaultRepositoryReset, None)
        .unwrap();

    assert!(!path.exists());
    assert!(jsonl_events(&db).contains(&"vault_repository_reset".to_string()));

    cleanup(&path);
}

/// T-A7：解锁补录 JSONL 中的恢复事件；重复解锁不重复插入。
#[test]
fn t_a7_backfill_is_idempotent() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);
    db.lock().unwrap();

    db.audit_jsonl(AuditEvent::RecoveryExportUsed, Some("1"))
        .unwrap();
    let pending = db.audit_writer().read_since(0).unwrap();
    let at = pending.last().unwrap().at;

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    let count = db_events(&db)
        .into_iter()
        .filter(|event| event == "recovery_export_used")
        .count();
    assert_eq!(count, 1, "解锁必须补录恢复态事件");

    let guard = db.get_conn().unwrap();
    assistant_vault::security_db::meta::set_meta(
        guard.as_ref().unwrap(),
        "audit_jsonl_synced_at",
        "0",
    )
    .unwrap();
    drop(guard);

    db.lock().unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    let count_again = db_events(&db)
        .into_iter()
        .filter(|event| event == "recovery_export_used")
        .count();
    assert_eq!(count_again, 1, "同一 JSONL 行重复补录必须被去重");
    assert!(at > 0);

    cleanup(&path);
}

/// T-A8：JSONL 目录不可写 → 业务成功，DB 仍写入。
#[test]
fn t_a8_unwritable_jsonl_does_not_block() {
    let path = temp_db_path();
    let (db, _repo) = setup(&path);

    let audit_dir = db.audit_dir().to_path_buf();
    fs::create_dir_all(audit_dir.parent().unwrap()).unwrap();
    if audit_dir.exists() {
        fs::remove_dir_all(&audit_dir).unwrap();
    }
    fs::write(&audit_dir, b"not a directory").unwrap();

    db.audit_dual(AuditEvent::VaultLock, Some("user")).unwrap();
    assert!(db_events(&db).contains(&"vault_lock".to_string()));

    fs::remove_file(&audit_dir).unwrap();
    cleanup(&path);
}

/// T-A9：审计内容不含密钥、口令、恢复码。
#[test]
fn t_a9_no_secrets_in_audit() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let exported = assistant_vault::lifecycle::export_recovery_key(&db, &repo, STRONG_PW).unwrap();
    db.change_master_password(&repo, STRONG_PW, NEW_PW).unwrap();

    let mut blob = String::new();
    for record in db.audit_writer().read_since(0).unwrap() {
        blob.push_str(&record.event);
        blob.push('|');
        blob.push_str(record.detail.as_deref().unwrap_or(""));
        blob.push('\n');
    }
    let guard = db.get_conn().unwrap();
    for entry in query_log(guard.as_ref().unwrap(), 1000).unwrap() {
        blob.push_str(&entry.event_type);
        blob.push('|');
        blob.push_str(entry.detail.as_deref().unwrap_or(""));
        blob.push('\n');
    }

    assert!(!blob.contains(&exported.paper_key));
    assert!(!blob.contains(STRONG_PW));
    assert!(!blob.contains(NEW_PW));
    assert!(!blob.contains("ghp_supersecret123"));

    cleanup(&path);
}

/// T-A10：明文出口审计用路径摘要，不含明文路径。
#[test]
fn t_a10_export_detail_uses_path_digest() {
    let path = "/home/user/Downloads/vault-export.csv";
    let digest = assistant_vault::audit::path_digest(path);
    assert_eq!(digest.len(), 64);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!digest.contains("Downloads"));

    let writer = AuditJsonlWriter::new(temp_db_path().parent().unwrap().join("audit"));
    writer
        .append(
            AuditEvent::VaultEntriesExported,
            Some(&format!("csv:1:{}", digest)),
        )
        .unwrap();
    let records = writer.read_since(0).unwrap();
    assert_eq!(records.len(), 1);
    assert!(!records[0].detail.as_deref().unwrap().contains(path));
}
