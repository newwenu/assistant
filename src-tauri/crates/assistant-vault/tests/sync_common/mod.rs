#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::SyncConfig;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;
use assistant_vault::types::{CustomFieldInput, VaultCategory, VaultEntrySnapshot};
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

fn template_dir() -> &'static Path {
    static TEMPLATE: OnceLock<PathBuf> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        let dir = test_dir().join("template");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vault.db");
        let db = SecurityDbState::new(path.to_str().unwrap());
        let repo = repo_at(&path);
        assistant_vault::lifecycle::initialize(&db, &repo, PASSWORD).unwrap();
        db.lock().unwrap();
        dir
    })
}

fn repo_at(db_path: &Path) -> VaultFileManager {
    VaultFileManager::new(
        db_path.parent().unwrap().join("vault"),
        Box::new(|| CipherAlgorithm::Aes256Gcm),
    )
}

pub fn open_db(path: &Path) -> SecurityDbState {
    let template = template_dir();
    std::fs::copy(template.join("vault.db"), path).unwrap();
    let dst_repo_dir = path.parent().unwrap().join("vault");
    std::fs::create_dir_all(&dst_repo_dir).unwrap();
    std::fs::copy(
        template.join("vault").join(".vault-meta"),
        dst_repo_dir.join(".vault-meta"),
    )
    .unwrap();
    let db = SecurityDbState::new(path.to_str().unwrap());
    assistant_vault::lifecycle::unlock(&db, &repo_at(path), PASSWORD).unwrap();
    db
}

pub fn open_db_with_password(path: &Path, password: &str) -> SecurityDbState {
    let _ = std::fs::remove_file(path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    let repo = repo_at(path);
    assistant_vault::lifecycle::initialize(&db, &repo, password).unwrap();
    db
}

pub fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub fn dek() -> Dek {
    Dek::new([7u8; 20])
}

pub fn other_dek() -> Dek {
    Dek::new([9u8; 20])
}

pub fn entry_snapshot(id: &str) -> VaultEntrySnapshot {
    VaultEntrySnapshot {
        id: id.to_string(),
        category: VaultCategory::Login,
        revision: 3,
        dek_version: 1,
        title: "GitHub".to_string(),
        username: Some("user@example.com".to_string()),
        url: Some("https://github.com".to_string()),
        password: Some("s3cret!".to_string()),
        notes: Some("Personal account".to_string()),
        totp_secret: Some(
            r#"{"secret":"JBSWY3DPEHPK3PXP","digits":6,"period":30,"algorithm":"sha1"}"#
                .to_string(),
        ),
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
    revision: i64,
    updated_at: i64,
) -> VaultEntrySnapshot {
    VaultEntrySnapshot {
        id: id.to_string(),
        category: VaultCategory::Login,
        revision,
        dek_version: 1,
        title: title.to_string(),
        username: Some("remote".to_string()),
        url: None,
        password: Some("remote-pw".to_string()),
        notes: None,
        totp_secret: None,
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
        totp: None,
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

pub fn seed_meta(engine: &SyncEngine, db: &SecurityDbState) {
    let mgr = engine.vault_file_manager();
    if mgr.meta_path().exists() {
        return;
    }
    let seed_repo = repo_at(Path::new(db.db_path()));
    let seed = std::fs::read(seed_repo.meta_path()).unwrap();
    std::fs::create_dir_all(mgr.vault_dir()).unwrap();
    std::fs::write(mgr.meta_path(), seed).unwrap();
}

pub fn rewrite_meta(engine: &SyncEngine, db: &SecurityDbState) {
    let dek_guard = db.get_dek().unwrap();
    let dek = dek_guard.as_ref().unwrap();
    assistant_vault::lifecycle::sync_write_meta(engine.vault_file_manager(), dek).unwrap();
}

pub fn write_remote(engine: &SyncEngine, db: &SecurityDbState, snapshots: &[VaultEntrySnapshot]) {
    seed_meta(engine, db);
    let dek_guard = db.get_dek().unwrap();
    let dek = dek_guard.as_ref().unwrap();
    let mgr = engine.vault_file_manager();
    for snap in snapshots {
        mgr.export_entry(dek, snap).unwrap();
    }
    drop(dek_guard);
    rewrite_meta(engine, db);
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
