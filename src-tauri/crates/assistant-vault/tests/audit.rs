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
    query_log(conn, 1000, None, None)
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
    );

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

    db.audit_jsonl(AuditEvent::VaultRepositoryReset, None);

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

    db.audit_jsonl(AuditEvent::RecoveryExportUsed, Some("1"));
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

    db.audit_dual(AuditEvent::VaultLock, Some("user"));
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
    for entry in query_log(guard.as_ref().unwrap(), 1000, None, None).unwrap() {
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

/// 规格 §2.7.2：全部事件变体的 Display 串互不相同（词表唯一）。
#[test]
fn audit_event_names_unique() {
    let names: Vec<String> = assistant_vault::audit::ALL_EVENTS
        .iter()
        .map(|e| e.to_string())
        .collect();
    let mut deduped = names.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        names.len(),
        deduped.len(),
        "duplicate audit event names: {:?}",
        names
    );
    assert!(
        names.iter().all(|n| !n.is_empty() && n.is_ascii()),
        "event names must be ascii: {:?}",
        names
    );
}

/// 规格 §2.5 第 2 条：J 通道事件必须全部在补录白名单内，否则该事件永不入库。
#[test]
fn j_channel_events_are_backfilled() {
    use assistant_vault::security_db::{BACKFILL_EVENTS, J_CHANNEL_EVENTS};

    let missing: Vec<&str> = J_CHANNEL_EVENTS
        .iter()
        .copied()
        .filter(|e| !BACKFILL_EVENTS.contains(e))
        .collect();
    assert!(
        missing.is_empty(),
        "J channel events missing from BACKFILL_EVENTS: {:?}",
        missing
    );

    let known: Vec<String> = assistant_vault::audit::ALL_EVENTS
        .iter()
        .map(|e| e.to_string())
        .collect();
    let unknown: Vec<&str> = J_CHANNEL_EVENTS
        .iter()
        .copied()
        .filter(|e| !known.iter().any(|k| k == e))
        .collect();
    assert!(
        unknown.is_empty(),
        "J channel events not present in AuditEvent: {:?}",
        unknown
    );
}

fn cursor_value(db: &SecurityDbState) -> i64 {
    let guard = db.get_conn().unwrap();
    assistant_vault::security_db::meta::get_meta(guard.as_ref().unwrap(), "audit_jsonl_synced_at")
        .unwrap()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

fn set_cursor(db: &SecurityDbState, value: i64) {
    let guard = db.get_conn().unwrap();
    assistant_vault::security_db::meta::set_meta(
        guard.as_ref().unwrap(),
        "audit_jsonl_synced_at",
        &value.to_string(),
    )
    .unwrap();
}

fn backfilled_rows(db: &SecurityDbState, event: &str, timestamp: i64) -> usize {
    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) FROM security_audit_log \
             WHERE event_type = ?1 AND timestamp = ?2",
        )
        .unwrap();
    stmt.query_row([event, &timestamp.to_string()], |row| row.get::<_, i64>(0))
        .unwrap() as usize
}

fn event_rows(db: &SecurityDbState, event: &str) -> i64 {
    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    conn.query_row(
        "SELECT COUNT(*) FROM security_audit_log WHERE event_type = ?1",
        [event],
        |row| row.get(0),
    )
    .unwrap()
}

fn event_details(db: &SecurityDbState, event: &str) -> Vec<Option<String>> {
    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    query_log(conn, 1000, None, None)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.event_type == event)
        .map(|entry| entry.detail)
        .collect()
}

/// 规格 §2.5 第 1 条（R2）：游标落在记录同一毫秒时，该记录不得漏补。
#[test]
fn t_a11_cursor_same_ms_boundary_not_missed() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let at = chrono::Utc::now().timestamp_millis() - 1_000;
    set_cursor(&db, at);
    db.lock().unwrap();
    db.audit_writer()
        .append_at(AuditEvent::RecoveryExportUsed, Some("1"), at)
        .unwrap();

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    assert_eq!(
        backfilled_rows(&db, "recovery_export_used", at),
        1,
        "与游标同毫秒的记录必须被补录（read_since 用 >=）"
    );

    cleanup(&path);
}

/// 规格 §2.5 第 1 条（G11）：游标在未来（时钟回拨）时必须自愈并补回更早的记录。
#[test]
fn t_a12_future_cursor_self_heals() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let at = chrono::Utc::now().timestamp_millis() - 2_000;
    set_cursor(&db, chrono::Utc::now().timestamp_millis() + 3_600_000);
    db.lock().unwrap();
    db.audit_writer()
        .append_at(AuditEvent::RecoveryExportUsed, Some("2"), at)
        .unwrap();

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    assert_eq!(
        backfilled_rows(&db, "recovery_export_used", at),
        1,
        "未来游标必须按损坏处理并全量补一次"
    );
    let now = chrono::Utc::now().timestamp_millis();
    assert!(
        cursor_value(&db) <= now,
        "补录后游标必须回到实际 max(at)，不得停留在未来"
    );

    cleanup(&path);
}

/// 规格 §2.5 第 3 条（R3）：同毫秒两条同事件必须补成 2 行。
#[test]
fn t_a13_same_ms_rows_are_not_merged() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let at = chrono::Utc::now().timestamp_millis() - 1_000;
    set_cursor(&db, at);
    db.lock().unwrap();
    db.audit_writer()
        .append_at(AuditEvent::SshKeyRemoved, Some("fp-1"), at)
        .unwrap();
    db.audit_writer()
        .append_at(AuditEvent::SshKeyRemoved, Some("fp-2"), at)
        .unwrap();
    db.audit_writer()
        .append_at(AuditEvent::BiometricFailed, Some("timeout"), at)
        .unwrap();
    db.audit_writer()
        .append_at(AuditEvent::BiometricFailed, Some("denied"), at)
        .unwrap();

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
    assert_eq!(
        backfilled_rows(&db, "ssh_key_removed", at),
        2,
        "同毫秒的 N 行必须补成 N 行，不得被 (event, timestamp) 合并"
    );
    assert_eq!(
        backfilled_rows(&db, "biometric_failed", at),
        2,
        "连续重试的同毫秒 biometric_failed 必须补成 2 行"
    );

    cleanup(&path);
}

/// 规格 §2.5 第 5 / 6 条（R4）：坏行与不可读文件只跳过，不整体失败。
#[test]
fn t_a14_read_tolerates_bad_lines_and_unreadable_files() {
    let path = temp_db_path();
    let (db, _repo) = setup(&path);

    let dir = db.audit_dir().to_path_buf();
    fs::create_dir_all(&dir).unwrap();
    let good = dir.join("audit-20200101.jsonl");
    fs::write(
        &good,
        "{\"at\":1,\"event\":\"recovery_export_used\",\"detail\":\"1\"}\n\
         not-json\n\
         {\"at\":2,\"event\":\"recovery_export_used\",\"detail\":\"2\"}\n",
    )
    .unwrap();
    // 用同名目录占位：is_audit_file 命中、File::open 失败，模拟不可读文件。
    let blocked = dir.join("audit-20200102.jsonl");
    fs::create_dir_all(&blocked).unwrap();

    let records = db.audit_writer().read_since(0).unwrap();
    assert_eq!(records.len(), 2, "坏行跳过、不可读文件跳过，其余照常返回");

    fs::remove_dir_all(&blocked).unwrap();
    cleanup(&path);
}

/// 规格 §2.5 第 8 条（R1）：补录行必须可按 `id` 前缀辨识来源。
#[test]
fn t_a15_backfill_rows_are_marked() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let at = chrono::Utc::now().timestamp_millis() - 1_000;
    set_cursor(&db, at);
    db.lock().unwrap();
    db.audit_writer()
        .append_at(AuditEvent::RecoveryExportUsed, Some("3"), at)
        .unwrap();

    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();

    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let backfilled_id: String = conn
        .query_row(
            "SELECT id FROM security_audit_log WHERE event_type = 'recovery_export_used'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        backfilled_id.starts_with("jsonl-"),
        "补录行 id 必须保留 jsonl- 前缀：{}",
        backfilled_id
    );

    let native_id: String = conn
        .query_row(
            "SELECT id FROM security_audit_log WHERE event_type = 'vault_unlock' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!native_id.starts_with("jsonl-"), "原生行不得带补录前缀");

    cleanup(&path);
}

/// 规格 §2.5 第 10 条（R12）：双写且在白名单内的事件不得产生自回响假补录行。
#[test]
fn t_a16_dual_write_events_do_not_echo() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    assistant_vault::lifecycle::rotate_dek(&db, &repo, &EmptyKeyStore).unwrap();
    let unlock_before = event_rows(&db, "vault_unlock");
    db.lock().unwrap();
    for _ in 0..3 {
        assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();
        db.lock().unwrap();
    }
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();

    assert_eq!(
        event_rows(&db, "dek_rotated"),
        1,
        "dek_rotated 必须恰好 1 行（多行即自回响）"
    );
    assert_eq!(
        event_rows(&db, "vault_unlock") - unlock_before,
        4,
        "vault_unlock 不在白名单内，解锁 4 次只能多 4 行"
    );

    cleanup(&path);
}

/// 规格 §2.5 第 10 条（R12）：重建库后 `local_db_rebuilt` 恰好 1 行。
#[test]
fn t_a19_local_db_rebuilt_does_not_echo() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    db.lock().unwrap();
    assistant_vault::lifecycle::rebuild_local_db(&db, &repo, STRONG_PW).unwrap();
    // 游标归零后强制走全量路径：本批必然读到 `local_db_rebuilt` 的 JSONL 行，
    // 若自回响防护缺失就会多插一条 `jsonl-` 假补录行。
    set_cursor(&db, 0);
    db.lock().unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();

    assert_eq!(
        event_rows(&db, "local_db_rebuilt"),
        1,
        "local_db_rebuilt 必须恰好 1 行"
    );

    cleanup(&path);
}

/// 规格 §2.5 第 7 条（R6）：候选文件按文件名日期裁剪，留 1 天余量。
#[test]
fn t_a17_read_prunes_files_before_cursor_date() {
    let path = temp_db_path();
    let (db, _repo) = setup(&path);

    let dir = db.audit_dir().to_path_buf();
    fs::create_dir_all(&dir).unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    let today = chrono::Local::now().date_naive();
    for offset in [-3i64, -1, 0, 1] {
        let date = if offset >= 0 {
            today.checked_add_days(chrono::Days::new(offset as u64))
        } else {
            today.checked_sub_days(chrono::Days::new((-offset) as u64))
        }
        .unwrap()
        .format("%Y%m%d")
        .to_string();
        fs::write(
            dir.join(format!("audit-{}.jsonl", date)),
            format!(
                "{{\"at\":{},\"event\":\"recovery_export_used\",\"detail\":\"1\"}}\n",
                now
            ),
        )
        .unwrap();
    }

    let records = db.audit_writer().read_since(now).unwrap();
    assert_eq!(records.len(), 3, "只应读取 D-1 / D / D+1，D-3 必须被裁剪掉");

    cleanup(&path);
}

/// 规格 §2.5 第 7 条（R6）：重建库后不得全量重扫历史 JSONL。
#[test]
fn t_a18_rebuild_does_not_rescan_history() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);

    let at = chrono::Utc::now().timestamp_millis() - 3_600_000;
    db.audit_writer()
        .append_at(AuditEvent::RecoveryExportUsed, Some("4"), at)
        .unwrap();
    db.lock().unwrap();

    assistant_vault::lifecycle::rebuild_local_db(&db, &repo, STRONG_PW).unwrap();

    assert_eq!(
        backfilled_rows(&db, "recovery_export_used", at),
        0,
        "重建后游标已对齐到重建时刻，旧 JSONL 行不得被重新灌入"
    );
    assert!(
        cursor_value(&db) > 0,
        "重建必须写入补录游标，否则首次解锁会全量重扫"
    );

    cleanup(&path);
}

/// 规格 §2.6：查询接口参数化 + 同毫秒稳定次级排序 + limit 夹取。
#[test]
fn t_a20_query_log_filters_and_orders_deterministically() {
    let path = temp_db_path();
    let (db, _repo) = setup(&path);
    let at = chrono::Utc::now().timestamp_millis() + 86_400_000;

    {
        let guard = db.get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        for id in ["same-1", "same-2", "same-3"] {
            conn.execute(
                "INSERT INTO security_audit_log (id, event_type, detail, timestamp) \
                 VALUES (?1, 'vault_unlock', NULL, ?2)",
                rusqlite::params![id, at],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO security_audit_log (id, event_type, detail, timestamp) \
             VALUES ('older', 'vault_lock', 'manual', ?1)",
            rusqlite::params![at - 1],
        )
        .unwrap();
    }

    let guard = db.get_conn().unwrap();
    let conn = guard.as_ref().unwrap();

    let all = query_log(conn, 100, None, None).unwrap();
    let same_ms: Vec<&str> = all
        .iter()
        .filter(|entry| entry.timestamp == at)
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        same_ms,
        vec!["same-3", "same-2", "same-1"],
        "同毫秒多条必须按 id 降序稳定排列，否则翻页会重复或漏项"
    );

    let since = query_log(conn, 100, Some(at), None).unwrap();
    assert_eq!(since.len(), 3, "since 必须为闭区间下界");

    let filtered = query_log(conn, 100, None, Some(&["vault_lock".to_string()])).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "older");

    let empty = query_log(conn, 100, None, Some(&[])).unwrap();
    assert_eq!(empty.len(), all.len(), "空事件集合必须按不过滤处理");

    let clamped = assistant_vault::audit::clamp_query_limit(Some(-1));
    assert_eq!(clamped, 100, "非正 limit 必须回落到默认值");
    assert_eq!(assistant_vault::audit::clamp_query_limit(Some(9999)), 500);
    assert_eq!(assistant_vault::audit::clamp_query_limit(None), 100);

    cleanup(&path);
}

/// 规格 §2.4 消歧规则：detail → entryId 必须按段数与首段封闭标签有序匹配。
#[test]
fn t_a21_detail_entry_id_follows_disambiguation_rules() {
    use assistant_vault::audit::detail_entry_id;

    let id = "11111111-1111-4111-8111-111111111111";
    assert_eq!(detail_entry_id(id).as_deref(), Some(id));
    assert_eq!(
        detail_entry_id(&format!("{}@remote", id)).as_deref(),
        Some(id)
    );
    assert_eq!(
        detail_entry_id(&format!("{}@HEAD@{{1}}", id)).as_deref(),
        Some(id),
        "suffix 自身含 @ 时必须按首个 @ 切分"
    );
    assert_eq!(
        detail_entry_id(&format!("totp:{}", id)).as_deref(),
        Some(id)
    );
    assert_eq!(detail_entry_id(&format!("{}:3", id)).as_deref(), Some(id));
    assert_eq!(
        detail_entry_id(&format!(
            "conflict:{}:local:password:{}",
            id,
            "0".repeat(64)
        ))
        .as_deref(),
        Some(id)
    );
    assert_eq!(
        detail_entry_id("conflict:a:b:c"),
        None,
        "畸形冲突行不得退化"
    );
    assert_eq!(detail_entry_id("json:3:deadbeef"), None);
    assert_eq!(detail_entry_id("csv:3:deadbeef:failed"), None);
    assert_eq!(detail_entry_id("manual"), None);
    assert_eq!(detail_entry_id("unknown"), None, "哨兵值不对应真实条目");
}

/// 规格 §2.4：冲突揭示 detail 固定 5 段，路径只以摘要出现。
#[test]
fn t_a22_conflict_audit_detail_is_well_formed() {
    use assistant_vault::sync_engine::conflict_audit_detail;

    let id = "11111111-1111-4111-8111-111111111111";
    let path = format!("entries/{}.enc", id);
    let detail = conflict_audit_detail(&path, "local", "password", false);
    let segments: Vec<&str> = detail.split(':').collect();
    assert_eq!(segments.len(), 5, "冲突揭示 detail 必须恰好 5 段: {detail}");
    assert_eq!(segments[0], "conflict");
    assert_eq!(segments[1], id);
    assert_eq!(segments[2], "local");
    assert_eq!(segments[3], "password");
    assert_eq!(segments[4].len(), 64, "摘要固定 64 字符");
    assert!(
        segments[4]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "摘要必须是固定 64 字符小写十六进制"
    );
    assert!(!detail.contains("entries/"), "路径原文不得进入 detail");
    assert_eq!(
        assistant_vault::audit::detail_entry_id(&detail).as_deref(),
        Some(id),
        "冲突行必须能被 §2.4 规则解析回 entryId"
    );

    let bad_path = conflict_audit_detail("entries/a:b.enc", "remote", "notes", false);
    let bad_segments: Vec<&str> = bad_path.split(':').collect();
    assert_eq!(bad_segments.len(), 5, "畸形路径也不得改变段数");
    assert_eq!(
        bad_segments[1], "unknown",
        "无法解析的 entryId 必须收敛为 unknown"
    );

    let bad_side = conflict_audit_detail(&path, "a:b", "notes", false);
    assert_eq!(bad_side.split(':').nth(2), Some("unknown"));

    let unresolved = conflict_audit_detail(&path, "local", "weird:name", false);
    assert_eq!(
        unresolved.split(':').nth(3),
        Some("unknown"),
        "限流分支无法证明字段合法，必须收敛为 unknown"
    );
    let resolved = conflict_audit_detail(&path, "local", "weird:name", true);
    assert_eq!(
        resolved.split(':').nth(3),
        Some("customFields"),
        "揭示成功后的非标准字段名即真实自定义字段"
    );
    assert_eq!(
        conflict_audit_detail(&path, "local", "totpSecret", true)
            .split(':')
            .nth(3),
        Some("totp")
    );
}

/// 规格 §2.4「入参校验前置」：限流分支写未校验前端 ID 前必须收敛为 UUID 或 `unknown`。
#[test]
fn t_a23_rate_limited_id_is_sanitized() {
    use assistant_vault::audit::sanitized_entry_id;

    let id = "11111111-1111-4111-8111-111111111111";
    assert_eq!(sanitized_entry_id(id), id);
    for bad in ["a:b", "123", "x\ny", "", "not-a-uuid"] {
        assert_eq!(
            sanitized_entry_id(bad),
            "unknown",
            "非 UUID 入参不得原样进入 detail: {bad:?}"
        );
    }
}

/// 规格 §2.4（R14）：`dek_rotated` 的 detail 必须是 `{old}:{new}:{count}` 三段纯数字。
#[test]
fn t_a24_dek_rotated_detail_is_three_numeric_segments() {
    use assistant_vault::audit::{parse_detail, ParsedDetail};

    let path = temp_db_path();
    let (db, repo) = setup(&path);
    assistant_vault::lifecycle::rotate_dek(&db, &repo, &EmptyKeyStore).unwrap();

    let details = event_details(&db, "dek_rotated");
    assert_eq!(details.len(), 1, "DEK 轮换必须恰好写一行");
    let detail = details[0].as_deref().expect("dek_rotated detail 不得为空");

    let segments: Vec<&str> = detail.split(':').collect();
    assert_eq!(
        segments.len(),
        3,
        "dek_rotated detail 必须恰好 3 段: {detail}"
    );
    for segment in &segments {
        assert!(
            !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()),
            "dek_rotated 每段都必须是纯数字: {detail}"
        );
    }
    assert!(
        matches!(parse_detail(detail), ParsedDetail::DekRotated { .. }),
        "dek_rotated 必须解析为三元组: {detail}"
    );

    db.lock().unwrap();
    cleanup(&path);
}

/// 规格 §2.4（R15）：批量删除必须逐条写 `{entryId}@{source}`，不得只写来源标签。
#[test]
fn t_a25_batch_delete_detail_is_entry_id_at_source() {
    use assistant_vault::audit::{parse_detail, ParsedDetail};

    let path = temp_db_path();
    let (db, _repo) = setup(&path);

    let ids: Vec<String> = (0..3)
        .map(|index| {
            let mut entry = sample_entry();
            entry.title = format!("bulk-{index}");
            db.create_entry(entry, CipherAlgorithm::Aes256Gcm).unwrap()
        })
        .collect();

    assert_eq!(db.delete_entries_tx(&ids, Some("remote")).unwrap(), 3);

    let mut expected: Vec<String> = ids.iter().map(|id| format!("{}@remote", id)).collect();
    let mut actual: Vec<String> = event_details(&db, "vault_entry_deleted")
        .into_iter()
        .flatten()
        .collect();
    expected.sort();
    actual.sort();
    assert_eq!(
        actual, expected,
        "每条删除必须写各自的 {{entryId}}@{{source}}"
    );

    for detail in &actual {
        match parse_detail(detail) {
            ParsedDetail::EntryWithSuffix {
                entry_id: Some(_),
                suffix,
            } => assert_eq!(suffix, "remote"),
            other => panic!("批量删除 detail 必须解析为 {{entryId}}@{{source}}: {other:?}"),
        }
    }

    cleanup(&path);
}

/// 规格 §2.4：导出成功（3 段）与失败（4 段 `:failed`）不得互串，畸形段数原样兜底。
#[test]
fn t_a26_export_detail_forms_do_not_mix() {
    use assistant_vault::audit::{parse_detail, ParsedDetail};

    let ok = parse_detail("json:3:deadbeef");
    assert!(
        matches!(ok, ParsedDetail::Export { ref format, count: 3, failed: false, .. } if format == "json"),
        "3 段必须解析为导出成功: {ok:?}"
    );

    let failed = parse_detail("csv:3:deadbeef:failed");
    assert!(
        matches!(
            failed,
            ParsedDetail::Export {
                count: 3,
                failed: true,
                ..
            }
        ),
        "4 段且末段为 failed 必须解析为导出失败: {failed:?}"
    );

    assert!(matches!(parse_detail("json:3"), ParsedDetail::Raw(_)));
    assert!(matches!(
        parse_detail("json:3:deadbeef:oops"),
        ParsedDetail::Raw(_)
    ));
    assert!(matches!(
        parse_detail("json:x:deadbeef"),
        ParsedDetail::Raw(_)
    ));
}

/// 规格 §2.4（R13）：`{entryId}:{count}` 与 `{tag}:{entryId}` 必须按首段封闭标签区分。
#[test]
fn t_a27_hibp_and_tagged_details_disambiguate() {
    use assistant_vault::audit::{parse_detail, ParsedDetail};

    let hibp = parse_detail(&format!("{}:3", SAMPLE_ID));
    assert!(
        matches!(
            hibp,
            ParsedDetail::EntryCount {
                entry_id: Some(_),
                count: 3
            }
        ),
        "hibp_leak_found 必须解析为 {{entryId}}:{{count}}: {hibp:?}"
    );
    assert!(
        !matches!(hibp, ParsedDetail::TaggedEntry { .. }),
        "hibp_leak_found 不得被解析成 tag:entryId"
    );

    let tagged = parse_detail(&format!("totp:{}", SAMPLE_ID));
    assert!(
        matches!(tagged, ParsedDetail::TaggedEntry { ref tag, entry_id: Some(_) } if tag == "totp"),
        "vault_copy_field 必须解析为 {{tag}}:{{entryId}}: {tagged:?}"
    );

    let with_reflog = parse_detail(&format!("{}@HEAD@{{1}}", SAMPLE_ID));
    assert!(
        matches!(with_reflog, ParsedDetail::EntryWithSuffix { entry_id: Some(_), ref suffix } if suffix == "HEAD@{1}"),
        "含 @ 的 suffix 必须按首个 @ 切分: {with_reflog:?}"
    );
}

/// 规格 §2.4：SSH 指纹入参必须收敛到 `SHA256:{digest}`，其余一律 `unknown`。
#[test]
fn t_a28_fingerprint_detail_is_sanitized_and_parsed() {
    use assistant_vault::audit::{parse_detail, sanitized_fingerprint, ParsedDetail};

    let good = "SHA256:abcDEF1234567890+/=";
    assert_eq!(sanitized_fingerprint(good), good);
    for bad in [
        "",
        "MD5:aa:bb:cc",
        "ssh-rsa",
        "SHA256:",
        "SHA256:has space",
        "a:b",
    ] {
        assert_eq!(
            sanitized_fingerprint(bad),
            "unknown",
            "非 SHA256:{{base64}} 的指纹不得原样进入 detail: {bad:?}"
        );
    }

    let parsed = parse_detail(good);
    assert!(
        matches!(parsed, ParsedDetail::Fingerprint { ref key_type, .. } if key_type == "SHA256"),
        "指纹 detail 必须解析为 Fingerprint: {parsed:?}"
    );
    assert!(matches!(parse_detail("unknown"), ParsedDetail::Label(_)));
}

const SAMPLE_ID: &str = "11111111-1111-4111-8111-111111111111";

/// 每个事件的样本 detail（规格 §2.4 五形态 + 非条目形态）。
///
/// 穷尽 match：新增事件漏补样本会直接编译失败，强于运行期测试失败（§2.7.2 三张表一致性）。
fn sample_detail(event: AuditEvent) -> Option<&'static str> {
    match event {
        AuditEvent::VaultUnlock => None,
        AuditEvent::VaultUnlockFailed => Some("wrong_password"),
        AuditEvent::VaultLock => Some("user"),
        AuditEvent::VaultReveal => Some(SAMPLE_ID),
        AuditEvent::VaultRevealRateLimited => Some(SAMPLE_ID),
        AuditEvent::VaultEntryCreated => Some(SAMPLE_ID),
        AuditEvent::VaultEntryUpdated => Some(SAMPLE_ID),
        AuditEvent::VaultEntryDeleted => Some("11111111-1111-4111-8111-111111111111@remote"),
        AuditEvent::VaultEntryRestored => Some("11111111-1111-4111-8111-111111111111@HEAD@{1}"),
        AuditEvent::VaultEntryPurged => Some("3"),
        AuditEvent::VaultCopyPassword => Some(SAMPLE_ID),
        AuditEvent::VaultCopyField => Some("totp:11111111-1111-4111-8111-111111111111"),
        AuditEvent::HibpAudit => None,
        AuditEvent::HibpLeakFound => Some("11111111-1111-4111-8111-111111111111:3"),
        AuditEvent::MasterPasswordChange => None,
        AuditEvent::RecoveryKeyExported => Some("2"),
        AuditEvent::RecoveryExportUsed => Some("1"),
        AuditEvent::DekRotated => Some("1:2:5"),
        AuditEvent::LocalDbRebuilt => Some("5"),
        AuditEvent::AnchorRollbackRejected => Some("meta_or_dek_version"),
        AuditEvent::BiometricEnrolled => None,
        AuditEvent::BiometricUnlocked => None,
        AuditEvent::BiometricFailed => Some("platform_error"),
        AuditEvent::BiometricRevoked => None,
        AuditEvent::SshKeyLoaded => Some(SAMPLE_ID),
        AuditEvent::SshKeyRemoved => Some("SHA256:abcDEF1234567890+/="),
        AuditEvent::VaultRepositoryReset => None,
        AuditEvent::VaultUnsupportedVersion => Some("unsupported_version"),
        AuditEvent::VaultEntriesExported => Some("json:3:deadbeef"),
        AuditEvent::VaultEntriesImported => Some("csv:3:deadbeef"),
    }
}

/// 规格 §2.4 / §2.7.2：每个事件的样本 detail 必须落在文法形态集合内，且含条目 ID 者必须可解析。
#[test]
fn t_a29_detail_grammar_samples_fall_in_spec_forms() {
    use assistant_vault::audit::{detail_entry_id, parse_detail, ParsedDetail};

    for event in assistant_vault::audit::ALL_EVENTS {
        let Some(detail) = sample_detail(*event) else {
            continue;
        };
        let parsed = parse_detail(detail);
        assert!(
            !matches!(parsed, ParsedDetail::Raw(_)),
            "事件 {} 的样本 detail 不在 §2.4 形态集合内: {detail} -> {parsed:?}",
            event
        );
        if detail.contains(SAMPLE_ID) {
            assert_eq!(
                detail_entry_id(detail).as_deref(),
                Some(SAMPLE_ID),
                "含条目 ID 的 detail 必须能解析出 entryId: {} / {detail}",
                event
            );
        }
    }
}

/// 规格 §4（R18 / R19）：审计写入失败只 `warn`，既 panic 也不阻断业务。
#[test]
fn t_a30_audit_write_failure_does_not_block() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();

    assistant_vault::audit::log_dual(Some(&conn), None, AuditEvent::VaultLock, Some("user"));
    assistant_vault::audit::log(&conn, AuditEvent::VaultLock, Some("user"));

    conn.execute("CREATE TABLE still_usable (x INTEGER)", [])
        .unwrap();
}

/// 相对今天偏移 `days` 天的日文件名日期串（`YYYYMMDD`），与生产写入用的本地日期同源。
fn date_offset(days: i64) -> String {
    let today = chrono::Local::now().date_naive();
    let date = if days >= 0 {
        today.checked_add_days(chrono::Days::new(days as u64))
    } else {
        today.checked_sub_days(chrono::Days::new((-days) as u64))
    }
    .unwrap();
    date.format("%Y%m%d").to_string()
}

fn write_audit_file(dir: &Path, days: i64, at: i64) -> std::path::PathBuf {
    let path = dir.join(format!("audit-{}.jsonl", date_offset(days)));
    fs::write(
        &path,
        format!(
            "{{\"at\":{},\"event\":\"recovery_export_used\",\"detail\":\"1\"}}\n",
            at
        ),
    )
    .unwrap();
    path
}

/// 规格 §5（G12）：3 个日文件、游标落在中间 —— 只删「早于游标所在日」且「已超期」的。
#[test]
fn t_a31_prune_deletes_only_expired_files_before_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let writer = AuditJsonlWriter::new(dir.path().to_path_buf());
    let now = chrono::Utc::now().timestamp_millis();

    // 游标 = D-10；保留 90 天。
    let expired_before_cursor = write_audit_file(dir.path(), -200, now);
    let fresh_before_cursor = write_audit_file(dir.path(), -30, now);
    let after_cursor = write_audit_file(dir.path(), -1, now);

    let cursor = (chrono::Utc::now() - chrono::Days::new(10)).timestamp_millis();
    let report = writer.prune_before(cursor, 90);

    assert_eq!(report.removed, 1, "只允许删游标之前且已超期的 1 个文件");
    assert_eq!(report.failed, 0);
    assert!(
        !expired_before_cursor.exists(),
        "D-200 早于游标且超期，必须删除"
    );
    assert!(
        fresh_before_cursor.exists(),
        "D-30 虽早于游标但未到 90 天保留期，必须保留"
    );
    assert!(after_cursor.exists(), "游标之后的文件必须保留");
}

/// 规格 §5（G12）：`0` 完全禁用清理；无法证明「已补录」的畸形文件名一律保留。
#[test]
fn t_a32_prune_zero_disables_and_keeps_unparsable_names() {
    let dir = tempfile::tempdir().unwrap();
    let writer = AuditJsonlWriter::new(dir.path().to_path_buf());
    let now = chrono::Utc::now().timestamp_millis();

    let expired = write_audit_file(dir.path(), -200, now);
    let malformed = dir.path().join("audit-not-a-date.jsonl");
    fs::write(&malformed, "").unwrap();
    let unrelated = dir.path().join("other.jsonl");
    fs::write(&unrelated, "").unwrap();

    let cursor = now;
    assert_eq!(
        writer.prune_before(cursor, 0).removed,
        0,
        "retention_days == 0 必须完全禁用清理"
    );
    assert!(expired.exists());

    let report = writer.prune_before(cursor, 90);
    assert_eq!(report.removed, 1, "只删能解析出日期且超期的日文件");
    assert!(!expired.exists());
    assert!(
        malformed.exists(),
        "文件名解析不出日期，无法证明已补录，必须保留"
    );
    assert!(unrelated.exists(), "非审计文件一律不得删除");
}

/// 规格 §5（G12）+ 调用点：补录成功后按「刚推进的游标」清理，而不是按 `now`。
#[test]
fn t_a33_unlock_prunes_expired_files_after_backfill() {
    let path = temp_db_path();
    let (db, repo) = setup(&path);
    db.set_audit_retention_days(90);

    let dir = db.audit_dir().to_path_buf();
    fs::create_dir_all(&dir).unwrap();
    let old_at = (chrono::Utc::now() - chrono::Days::new(200)).timestamp_millis();
    let expired = write_audit_file(&dir, -200, old_at);
    // 未超期文件用独立时间戳：两条记录的 `at` 相同会让补录行数断言失去区分力。
    let fresh = write_audit_file(&dir, -1, chrono::Utc::now().timestamp_millis());

    db.lock().unwrap();
    assistant_vault::lifecycle::unlock(&db, &repo, STRONG_PW).unwrap();

    assert_eq!(
        backfilled_rows(&db, "recovery_export_used", old_at),
        1,
        "清理的前提是补录已完成，本行必须先入库"
    );
    assert!(!expired.exists(), "补录完成且超期的日文件必须被清理");
    assert!(fresh.exists(), "未超期的日文件必须保留");
    assert!(cursor_value(&db) > old_at, "游标必须推进到实际 max(at)");

    cleanup(&path);
}
