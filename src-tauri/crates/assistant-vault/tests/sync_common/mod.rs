#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::SyncConfig;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;
use assistant_vault::types::{CustomFieldInput, VaultCategory, VaultEntrySnapshot};
use assistant_vault::vault_file::types::VaultMetaFile;
use assistant_vault::vault_file::VaultFileManager;

pub const PASSWORD: &str = "Str0ng#P@ssw0rd!2025";
pub const OTHER_PASSWORD: &str = "An0ther#P@ssw0rd!2026";

pub fn git_env() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        std::env::set_var("GIT_AUTHOR_NAME", "Vault Test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "vault-test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Vault Test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "vault-test@example.com");
    });
}

static REKEY: Mutex<()> = Mutex::new(());

pub fn rekey_guard() -> MutexGuard<'static, ()> {
    REKEY.lock().unwrap_or_else(|e| e.into_inner())
}

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("assistant-sync-test");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn temp_db_path() -> PathBuf {
    let dir = test_dir().join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("vault.db")
}

fn template_db() -> &'static Path {
    static TEMPLATE: OnceLock<PathBuf> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        let path = test_dir().join("template.db");
        let _ = std::fs::remove_file(&path);
        let db = SecurityDbState::new(path.to_str().unwrap());
        db.init(PASSWORD).unwrap();
        db.lock().unwrap();
        path
    })
}

pub fn open_db(path: &Path) -> SecurityDbState {
    std::fs::copy(template_db(), path).unwrap();
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.unlock(PASSWORD).unwrap();
    db
}

pub fn open_db_with_password(path: &Path, password: &str) -> SecurityDbState {
    let _ = std::fs::remove_file(path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(password).unwrap();
    db
}

pub fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub fn master_key() -> MasterKey {
    MasterKey::new([7u8; 32])
}

pub fn other_key() -> MasterKey {
    MasterKey::new([9u8; 32])
}

pub fn entry_snapshot(id: &str) -> VaultEntrySnapshot {
    VaultEntrySnapshot {
        id: id.to_string(),
        category: VaultCategory::Login,
        revision: 3,
        title: "GitHub".to_string(),
        username: Some("user@example.com".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("s3cret!".to_string()),
        notes: Some("Personal account".to_string()),
        totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
        totp_digits: 6,
        totp_period: 30,
        custom_fields: vec![CustomFieldInput {
            name: "API Key".to_string(),
            value: "cf-value".to_string(),
            is_protected: true,
        }],
        tags: vec!["dev".to_string(), "work".to_string()],
        is_favorite: true,
        created_at: 1729468800,
        updated_at: 1729555200,
    }
}

pub fn remote_snapshot(
    id: &str,
    title: &str,
    revision: u64,
    updated_at: i64,
) -> VaultEntrySnapshot {
    VaultEntrySnapshot {
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
        custom_fields: vec![CustomFieldInput {
            name: "PIN".to_string(),
            value: "1234".to_string(),
            is_protected: false,
        }],
        tags: vec!["remote".to_string()],
        is_favorite: true,
        created_at: 1_000,
        updated_at,
    }
}

pub fn local_entry(title: &str) -> assistant_vault::dto::VaultEntryCreate {
    assistant_vault::dto::VaultEntryCreate {
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
    }
}

pub fn vault_manager() -> (tempfile::TempDir, VaultFileManager) {
    let dir = tempfile::tempdir().unwrap();
    let mgr = VaultFileManager::new(
        dir.path().join("vault"),
        Box::new(|| CipherAlgorithm::Aes256Gcm),
    );
    (dir, mgr)
}

pub fn meta_path(vault_dir: &Path) -> PathBuf {
    vault_dir.join(".vault-meta")
}

pub fn write_meta(vault_dir: &Path, meta: &VaultMetaFile) {
    std::fs::write(
        meta_path(vault_dir),
        serde_json::to_string_pretty(meta).unwrap(),
    )
    .unwrap();
}

pub fn engine(dir: &Path) -> SyncEngine {
    SyncEngine::new(
        dir.join("vault"),
        SyncConfig::default(),
        Box::new(|| CipherAlgorithm::Aes256Gcm),
    )
}

pub fn git_repo(path: PathBuf) -> GitSync {
    GitSync::new(path, SyncConfig::default())
}

pub fn write_remote(engine: &SyncEngine, db: &SecurityDbState, snapshots: &[VaultEntrySnapshot]) {
    let guard = db.get_master_key().unwrap();
    let key = guard.as_ref().unwrap();
    for snap in snapshots {
        engine
            .vault_file_manager()
            .export_entry(key, snap, false)
            .unwrap();
    }
    let meta_path = engine.vault_file_manager().vault_dir().join(".vault-meta");
    let existing = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|content| serde_json::from_str::<VaultMetaFile>(&content).ok());
    let (crypto, emergency) = match existing {
        Some(meta) => (meta.crypto, meta.emergency),
        None => (None, None),
    };
    engine
        .vault_file_manager()
        .export_vault_meta(key, crypto, emergency)
        .unwrap();
}

pub fn bare_repo(dir: &Path) -> PathBuf {
    git_env();
    let path = dir.join("remote.git");
    let status = Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

pub fn commit_repo(engine: &SyncEngine, msg: &str) {
    git_env();
    git_repo(engine.vault_file_manager().vault_dir().to_path_buf())
        .commit_all(msg, false)
        .unwrap();
}
