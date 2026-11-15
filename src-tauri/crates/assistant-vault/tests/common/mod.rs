#![allow(dead_code)]

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::types::VaultError;
use assistant_vault::vault_file::VaultFileManager;
use std::path::{Path, PathBuf};

pub const STRONG_PW: &str = "Str0ng#P@ssw0rd!2025";

/// 返回仍存活的临时目录 guard 与配置路径。
///
/// 只返回路径会让 `TempDir` 在语句结束时就析构并删除目录，之后的 `save_to`
/// 重建父目录便再无人回收，导致每跑一次配置测试都向 `%TEMP%` 泄漏目录。
pub fn conf_path(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    (dir, path)
}

/// 测试库目录 guard：持有 `TempDir`，测试结束（含 panic）后自动回收。
///
/// 只返回 `PathBuf` 时临时目录没有所有者，测试末尾手工 `cleanup` 一旦被断言
/// 打断就会把库目录泄漏到 `%TEMP%`。字段声明顺序保证 `Drop` 里删除时已无连接。
pub struct TempDbDir {
    dir: tempfile::TempDir,
    db_path: PathBuf,
}

impl TempDbDir {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("vault.db");
        Self { dir, db_path }
    }
}

impl std::ops::Deref for TempDbDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.db_path
    }
}

impl AsRef<Path> for TempDbDir {
    fn as_ref(&self) -> &Path {
        &self.db_path
    }
}

impl AsRef<std::ffi::OsStr> for TempDbDir {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.db_path.as_os_str()
    }
}

impl Drop for TempDbDir {
    fn drop(&mut self) {
        // 重试覆盖连接 / git 子进程句柄的滞后释放。
        for _ in 0..20 {
            match std::fs::remove_dir_all(self.dir.path()) {
                Ok(()) => return,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(25)),
            }
        }
    }
}

pub fn temp_db_path() -> TempDbDir {
    TempDbDir::new()
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
