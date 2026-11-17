use std::path::{Path, PathBuf};

use crate::types::SyncConfig;

use super::{GitSync, STORE_GIT_DIR};

impl GitSync {
    pub fn new(repo_path: PathBuf, config: SyncConfig) -> Self {
        Self { repo_path, config }
    }

    pub fn path(&self) -> &Path {
        &self.repo_path
    }

    pub fn is_init(&self) -> bool {
        self.repo_path.join(STORE_GIT_DIR).is_dir()
    }
}
