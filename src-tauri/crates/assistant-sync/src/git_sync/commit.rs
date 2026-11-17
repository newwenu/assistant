use crate::types::SyncError;

use super::{git, GitSync};

impl GitSync {
    pub fn commit_all(&self, msg: &str, commit_empty: bool) -> Result<(), SyncError> {
        let path = &self.repo_path;
        git::git_add_all(path)?;
        git::git_commit(path, msg, commit_empty)
    }

    pub fn reset_hard_all(&self) -> Result<(), SyncError> {
        let path = &self.repo_path;
        git::git_add_all(path)?;
        git::git_reset_hard(path)
    }

    pub fn log(
        &self,
        path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<git::RawLogEntry>, SyncError> {
        git::git_log(&self.repo_path, path, limit)
    }

    pub fn show_file(&self, commit: &str, path: &str) -> Result<String, SyncError> {
        git::git_show_file(&self.repo_path, commit, path)
    }

    pub fn checkout_detached(&self, commit: &str) -> Result<(), SyncError> {
        git::git_checkout_detached(&self.repo_path, commit)
    }

    pub fn last_commit_info(
        &self,
        reference: Option<&str>,
        path: &str,
    ) -> Result<(String, i64), SyncError> {
        git::git_last_commit_info(&self.repo_path, reference, path)
    }
}
