#![allow(dead_code)]

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::types::VaultError;
use assistant_vault::vault_file::VaultFileManager;
use std::path::{Path, PathBuf};

pub const STRONG_PW: &str = "Str0ng#P@ssw0rd!2025";

pub fn temp_db_path() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("assistant-security-test")
        .join(uuid::Uuid::new_v4().to_string());
    let _ = std::fs::create_dir_all(&dir);
    dir.join("vault.db")
}

pub fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub fn repo_for(db_path: &Path) -> VaultFileManager {
    let vault_dir = db_path.parent().unwrap().join("vault");
    VaultFileManager::new(vault_dir, Box::new(|| CipherAlgorithm::Aes256Gcm))
}

pub fn initialize_at(db: &SecurityDbState, password: &str) {
    let repo = repo_for(Path::new(db.db_path()));
    assistant_vault::lifecycle::initialize(db, &repo, password).unwrap();
}

pub fn try_initialize_at(db: &SecurityDbState, password: &str) -> Result<(), VaultError> {
    let repo = repo_for(Path::new(db.db_path()));
    assistant_vault::lifecycle::initialize(db, &repo, password)
}

pub fn unlock_at(db: &SecurityDbState, password: &str) -> Result<(), VaultError> {
    let repo = repo_for(Path::new(db.db_path()));
    assistant_vault::lifecycle::unlock(db, &repo, password)
}

pub fn init_vault(path: &Path, password: &str) -> SecurityDbState {
    let _ = std::fs::remove_file(path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    initialize_at(&db, password);
    db
}
