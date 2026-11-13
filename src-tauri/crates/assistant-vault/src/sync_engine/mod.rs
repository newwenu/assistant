use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{PendingRemovals, SyncConfig};

use crate::vault_file::{CipherProvider, VaultFileManager};

mod export;
mod history;
mod lifecycle;
mod preview;
mod pull_push;
mod remote_admin;

pub use preview::mask_entry_preview;

pub struct SyncEngine {
    git_sync: Mutex<GitSync>,
    vault_file: Arc<VaultFileManager>,
    config: Mutex<SyncConfig>,
    pending_removals: Mutex<Option<PendingRemovals>>,
}

impl SyncEngine {
    /// 过渡构造入口：自建仓库实例，仅供既有测试使用。应用层应改用 `new_with_repo`。
    pub fn new(vault_dir: PathBuf, config: SyncConfig, cipher: CipherProvider) -> Self {
        Self::new_with_repo(Arc::new(VaultFileManager::new(vault_dir, cipher)), config)
    }

    /// 复用应用层 `VaultRepo` 持有的同一实例：git 目录直接取自仓库句柄，保证两者指向同一路径。
    pub fn new_with_repo(repo: Arc<VaultFileManager>, config: SyncConfig) -> Self {
        let git_sync = GitSync::new(repo.vault_dir().to_path_buf(), config.clone());
        Self {
            git_sync: Mutex::new(git_sync),
            vault_file: repo,
            config: Mutex::new(config),
            pending_removals: Mutex::new(None),
        }
    }

    pub fn vault_file_manager(&self) -> &VaultFileManager {
        &self.vault_file
    }
}
