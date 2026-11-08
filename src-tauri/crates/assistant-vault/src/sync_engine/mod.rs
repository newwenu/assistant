use std::path::PathBuf;
use std::sync::Mutex;

use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{PendingRemovals, SyncConfig};

use crate::vault_file::{CipherProvider, VaultFileManager};

mod emergency;
mod export;
mod history;
mod lifecycle;
mod preview;
mod pull_push;
mod remote_admin;

pub struct SyncEngine {
    git_sync: Mutex<GitSync>,
    vault_file: VaultFileManager,
    config: Mutex<SyncConfig>,
    pending_removals: Mutex<Option<PendingRemovals>>,
}

impl SyncEngine {
    pub fn new(vault_dir: PathBuf, config: SyncConfig, cipher: CipherProvider) -> Self {
        let git_sync = GitSync::new(vault_dir.clone(), config.clone());
        let vault_file = VaultFileManager::new(vault_dir, cipher);
        Self {
            git_sync: Mutex::new(git_sync),
            vault_file,
            config: Mutex::new(config),
            pending_removals: Mutex::new(None),
        }
    }

    pub fn vault_file_manager(&self) -> &VaultFileManager {
        &self.vault_file
    }
}
