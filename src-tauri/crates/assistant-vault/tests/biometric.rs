mod common;

use std::collections::HashMap;
use std::sync::Mutex;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::encrypt_field;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, PerEntryKey, VaultPlaintext};
use assistant_vault::biometric::BiometricKeyStore;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::vault_file::VaultFileManager;
use common::{cleanup, repo_for, temp_db_path, TempDbDir, STRONG_PW};
use sha2::{Digest, Sha256};

const KEY_ID_PAYLOAD: &str = "vault-payload";
const KEY_ID_FINGERPRINT: &str = "vault-fingerprint";
const KEY_ID_SIG_HASH: &str = "vault-sig-hash";
const KEY_ID_WRAPPING_KEY: &str = "vault-wrapping-key";
const ALL_KEY_IDS: [&str; 4] = [
    KEY_ID_PAYLOAD,
    KEY_ID_FINGERPRINT,
    KEY_ID_SIG_HASH,
    KEY_ID_WRAPPING_KEY,
];

const MOCK_SIGNATURE: &[u8] = b"mock-hello-signature";
const HKDF_SALT: &[u8] = b"assistant-vault-v1";
const HKDF_INFO: &[u8] = b"aes-256-gcm-key";

/// 内存 keystore：支持注入 `delete_key` 失败，模拟真实平台错误语义。
struct MockStore {
    entries: Mutex<HashMap<String, Vec<u8>>>,
    fail_delete: Option<&'static str>,
    available: bool,
    acl: bool,
}

impl MockStore {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            fail_delete: None,
            available: true,
            acl: true,
        }
    }

    fn failing_delete(key_id: &'static str) -> Self {
        Self {
            fail_delete: Some(key_id),
            ..Self::new()
        }
    }
}

impl BiometricKeyStore for MockStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        Ok(self.available)
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        self.entries
            .lock()
            .unwrap()
            .insert(key_id.to_string(), data.to_vec());
        Ok(())
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        self.entries
            .lock()
            .unwrap()
            .get(key_id)
            .cloned()
            .ok_or_else(|| VaultError::BiometricFailed(format!("missing {}", key_id)))
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        if self.fail_delete == Some(key_id) {
            return Err(VaultError::BiometricFailed(format!(
                "delete failed: {}",
                key_id
            )));
        }
        self.entries.lock().unwrap().remove(key_id);
        Ok(())
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        Ok(self.entries.lock().unwrap().contains_key(key_id))
    }

    fn request_signature(&self, _key_id: &str) -> Result<Vec<u8>, VaultError> {
        Ok(MOCK_SIGNATURE.to_vec())
    }

    fn acl_satisfied(&self) -> Result<bool, VaultError> {
        Ok(self.acl)
    }
}

fn setup() -> (TempDbDir, SecurityDbState, VaultFileManager) {
    let path = temp_db_path();
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_for(&path);
    assistant_vault::lifecycle::initialize(&db, &repo, STRONG_PW).unwrap();
    (path, db, repo)
}

fn fingerprint_of(payload: &[u8]) -> Vec<u8> {
    hex::encode(Sha256::digest(payload)).into_bytes()
}

#[cfg(target_os = "windows")]
fn wrap_with_current_key(_store: &MockStore, payload: &[u8]) -> Vec<u8> {
    use hkdf::Hkdf;
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), MOCK_SIGNATURE);
    let mut key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut key).unwrap();
    encrypt_field(
        &PerEntryKey::new(key),
        &VaultPlaintext::from(payload.to_vec()),
        CipherAlgorithm::Aes256Gcm,
        b"",
    )
    .unwrap()
    .as_bytes()
    .to_vec()
}

#[cfg(not(target_os = "windows"))]
fn wrap_with_current_key(store: &MockStore, payload: &[u8]) -> Vec<u8> {
    let stored = store.get_key(KEY_ID_WRAPPING_KEY).unwrap();
    let text = String::from_utf8_lossy(&stored).to_string();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hex::decode(text).unwrap());
    encrypt_field(
        &PerEntryKey::new(key),
        &VaultPlaintext::from(payload.to_vec()),
        CipherAlgorithm::Aes256Gcm,
        b"",
    )
    .unwrap()
    .as_bytes()
    .to_vec()
}

fn audit_reasons(db: &SecurityDbState, event: &str) -> Vec<Option<String>> {
    db.audit_writer()
        .read_since(0)
        .unwrap()
        .into_iter()
        .filter(|record| record.event == event)
        .map(|record| record.detail)
        .collect()
}

/// T-B1：`rotate_dek` 删除 `vault-payload` 失败 → 中止，`dekVersion` 不递增。
#[test]
fn t_b1_rotate_aborts_when_payload_delete_fails() {
    let (path, db, repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();

    let failing = MockStore::failing_delete(KEY_ID_PAYLOAD);
    let err = assistant_vault::lifecycle::rotate_dek(&db, &repo, &failing).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricFailed(_)),
        "unexpected error: {err:?}"
    );

    let meta = repo.read_vault_meta_v2().unwrap();
    assert_eq!(meta.dek_version, 1);
    assert!(store.has_key(KEY_ID_PAYLOAD).unwrap());

    cleanup(&path);
}

/// I13：`rotate_dek` 后旧凭据全部删除；旧 payload 无法解出新 `dek`，必须重新注册。
#[test]
fn i13_rotate_deletes_credentials_and_old_payload_is_unusable() {
    let (path, db, repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();
    assert!(db.is_biometric_enrolled(&store).unwrap());

    // 模拟攻击者保留了轮换前的全部凭据副本。
    let stale = MockStore::new();
    for key in ALL_KEY_IDS {
        if let Ok(data) = store.get_key(key) {
            stale.set_key(key, &data).unwrap();
        }
    }

    assistant_vault::lifecycle::rotate_dek(&db, &repo, &store).unwrap();

    for key in ALL_KEY_IDS {
        assert!(
            !store.has_key(key).unwrap(),
            "rotate_dek 后 {} 必须被删除",
            key
        );
    }
    assert!(!db.is_biometric_enrolled(&store).unwrap());

    db.lock().unwrap();
    let err = db.unlock_biometric(&repo, &stale).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricReset),
        "旧 payload 必须被判为过期并引导重新注册，实际 {err:?}"
    );
    assert!(db.is_locked(), "旧 payload 不得解锁仓库");

    let revoked = audit_reasons(&db, "biometric_revoked");
    assert!(
        !revoked.is_empty(),
        "rotate_dek 必须写 BiometricRevoked 审计"
    );

    cleanup(&path);
}

/// T-B2：`reset_repository` 后 4 个 key id 全部不存在，且文件已删除。
#[test]
fn t_b2_reset_clears_all_biometric_entries() {
    let (path, db, _repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();
    assert!(db.is_biometric_enrolled(&store).unwrap());

    let vault_dir = path.parent().unwrap().join("vault");
    let entries_dir = vault_dir.join("entries");
    let audit_dir = path.parent().unwrap().join("audit");
    let recovery_dir = path.parent().unwrap().join("recovery");
    for dir in [&entries_dir, &audit_dir, &recovery_dir] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let meta_path = vault_dir.join(".vault-meta");
    assert!(meta_path.exists());

    db.lock().unwrap();
    let targets = vec![
        meta_path.clone(),
        path.to_path_buf(),
        entries_dir.clone(),
        audit_dir.clone(),
        recovery_dir.clone(),
    ];
    assistant_vault::lifecycle::reset_repository(&db, &store, &targets).unwrap();

    for key_id in ALL_KEY_IDS {
        assert!(!store.has_key(key_id).unwrap(), "残留 {}", key_id);
    }
    assert!(!db.is_biometric_enrolled(&store).unwrap());

    assert!(!meta_path.exists());
    assert!(!path.exists());
    assert!(!entries_dir.exists());
    assert!(!recovery_dir.exists());
    assert_eq!(
        audit_reasons(&db, "vault_repository_reset"),
        vec![None],
        "重置事件必须落 JSONL"
    );

    cleanup(&path);
}

/// T-B3：注入 `delete_key` 失败 → 重置中止，`.vault-meta` / `security.db` / `entries/` 均保留。
#[test]
fn t_b3_reset_aborts_when_keystore_clear_fails() {
    let (path, db, _repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();

    let vault_dir = path.parent().unwrap().join("vault");
    let entries_dir = vault_dir.join("entries");
    std::fs::create_dir_all(&entries_dir).unwrap();
    let meta_path = vault_dir.join(".vault-meta");
    assert!(meta_path.exists());

    let failing = MockStore::failing_delete(KEY_ID_FINGERPRINT);
    db.lock().unwrap();
    let targets = vec![meta_path.clone(), path.to_path_buf(), entries_dir.clone()];
    let err = assistant_vault::lifecycle::reset_repository(&db, &failing, &targets).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricFailed(_)),
        "unexpected error: {err:?}"
    );

    assert!(meta_path.exists());
    assert!(path.exists());
    assert!(entries_dir.exists());

    cleanup(&path);
}

/// T-C11：keystore 不可用 → 生物识别 `BiometricUnavailable`，口令路径不受影响。
#[test]
fn t_c11_unavailable_keystore_reports_biometric_unavailable() {
    let (path, db, repo) = setup();
    let unavailable = MockStore {
        available: false,
        ..MockStore::new()
    };

    let err = db
        .enroll_biometric_with_hint(&unavailable, || {})
        .unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricUnavailable),
        "unexpected error: {err:?}"
    );

    db.lock().unwrap();
    let err = db.unlock_biometric(&repo, &unavailable).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricUnavailable),
        "unexpected error: {err:?}"
    );
    assert!(db.is_locked());

    common::unlock_at(&db, STRONG_PW).unwrap();
    assert!(!db.is_locked());

    cleanup(&path);
}

/// T-C11：ACL 不满足 → 注册返回 `BiometricUnavailable`，禁止静默降级。
#[test]
fn t_c11_unsatisfied_acl_rejects_enrollment() {
    let (path, db, _repo) = setup();
    let no_acl = MockStore {
        acl: false,
        ..MockStore::new()
    };

    let err = db.enroll_biometric_with_hint(&no_acl, || {}).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricUnavailable),
        "unexpected error: {err:?}"
    );
    assert!(!no_acl.has_key(KEY_ID_PAYLOAD).unwrap());

    cleanup(&path);
}

/// T-B5：篡改 `vault-payload` 使解密失败 → `BiometricReset`，不是 `VaultError::Crypto`。
#[test]
fn t_b5_tampered_payload_maps_to_biometric_reset() {
    let (path, db, repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();
    db.lock().unwrap();

    store.set_key(KEY_ID_PAYLOAD, b"tampered-blob").unwrap();

    let err = db.unlock_biometric(&repo, &store).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricReset),
        "unexpected error: {err:?}"
    );
    assert!(db.is_locked());

    cleanup(&path);
}

/// T-B6：payload 与当前 `dek` / `dbKey` 不匹配 → `BiometricReset`，不得用错误密钥开库。
#[test]
fn t_b6_mismatched_payload_maps_to_biometric_reset() {
    let (path, db, repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();
    db.lock().unwrap();

    let mut wrong_payload = [0u8; 52];
    wrong_payload[0] = 1;
    let blob = wrap_with_current_key(&store, &wrong_payload);
    store.set_key(KEY_ID_PAYLOAD, &blob).unwrap();
    store
        .set_key(KEY_ID_FINGERPRINT, &fingerprint_of(&wrong_payload))
        .unwrap();

    let err = db.unlock_biometric(&repo, &store).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricReset),
        "unexpected error: {err:?}"
    );
    assert!(db.is_locked());

    cleanup(&path);
}

/// T-B4（Windows）：改 `vault-sig-hash` → `BiometricReset`，审计 `sig_changed`，不进入解锁。
#[cfg(target_os = "windows")]
#[test]
fn t_b4_sig_hash_mismatch_maps_to_biometric_reset() {
    let (path, db, repo) = setup();
    let store = MockStore::new();
    db.enroll_biometric_with_hint(&store, || {}).unwrap();
    db.lock().unwrap();

    let other = hex::encode(Sha256::digest(b"attacker-signature"));
    store.set_key(KEY_ID_SIG_HASH, other.as_bytes()).unwrap();

    let err = db.unlock_biometric(&repo, &store).unwrap_err();
    assert!(
        matches!(err, VaultError::BiometricReset),
        "unexpected error: {err:?}"
    );
    assert!(db.is_locked());
    assert_eq!(
        audit_reasons(&db, "biometric_failed"),
        vec![Some("sig_changed".to_string())]
    );

    cleanup(&path);
}
