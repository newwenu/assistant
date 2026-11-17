use crate::types::{ConflictFile, SyncError};

use super::{git, GitSync};

impl GitSync {
    pub fn resolve_conflict_local(&self, path: &str) -> Result<(), SyncError> {
        git::git_checkout_ours(&self.repo_path, path)?;
        git::git_add(&self.repo_path, path)
    }

    pub fn resolve_conflict_remote(&self, path: &str) -> Result<(), SyncError> {
        git::git_checkout_theirs(&self.repo_path, path)?;
        git::git_add(&self.repo_path, path)
    }

    pub fn detect_conflicts(&self) -> Result<Vec<ConflictFile>, SyncError> {
        let mut conflicts = Vec::new();
        for path in git::git_unmerged_paths(&self.repo_path)? {
            let (local_hash, remote_hash) = git::git_conflict_stage_hashes(&self.repo_path, &path)?;
            conflicts.push(ConflictFile {
                path,
                local_hash,
                remote_hash,
                title: None,
            });
        }
        Ok(conflicts)
    }
}
