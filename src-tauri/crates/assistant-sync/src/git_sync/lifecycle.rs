use crate::types::{Readyness, RepositoryState, SyncError};

use super::{git, helpers::safe_need_to_push, GitSync};

impl GitSync {
    pub fn readyness(&self) -> Result<Readyness, SyncError> {
        if !self.is_init() {
            return Ok(Readyness::NoSync);
        }

        match git::git_state(&self.repo_path)? {
            RepositoryState::Clean => {
                if !self.has_remote()? {
                    Ok(Readyness::NoRemote)
                } else if git::git_has_changes(&self.repo_path)? {
                    Ok(Readyness::Dirty)
                } else {
                    Ok(Readyness::Ready)
                }
            }
            state => Ok(Readyness::RepoState(state)),
        }
    }

    pub fn prepare(&self) -> Result<(), SyncError> {
        if !self.is_init() {
            return Ok(());
        }

        if !self.has_remote()? {
            return Ok(());
        }

        self.ensure_upstream()?;
        self.pull()
    }

    /// 尝试自动建立上游。返回 `Ok(true)` 表示过程中执行了一次 fetch，
    /// 调用方可据此跳过重复 fetch；条件不满足时静默返回 `Ok(false)`。
    pub fn ensure_upstream(&self) -> Result<bool, SyncError> {
        if !self.is_init() || !self.has_remote()? {
            return Ok(false);
        }

        let repo = &self.repo_path;
        if git::git_branch_upstream(repo, "HEAD")?.is_some() {
            return Ok(false);
        }

        let remotes = self.tracked_remote_or_remotes()?;
        if remotes.len() != 1 {
            return Ok(false);
        }

        let remote = &remotes[0];
        git::git_fetch(repo, Some(remote))?;

        let remote_branches = git::git_branch_remote(repo)?;
        if remote_branches.is_empty() {
            return Ok(true);
        }

        let branch = git::git_current_branch(repo)?;
        let upstream_ref = format!("{remote}/{branch}");
        if !remote_branches.contains(&upstream_ref) {
            return Ok(true);
        }

        git::git_branch_set_upstream(repo, None, &upstream_ref)?;
        Ok(true)
    }

    pub fn state(&self) -> Result<RepositoryState, SyncError> {
        git::git_state(&self.repo_path)
    }

    pub fn finalize(&self, msg: &str) -> Result<(), SyncError> {
        if !self.is_init() {
            return Ok(());
        }

        if git::git_has_changes(&self.repo_path)? {
            self.commit_all(msg, false)?;
        }

        if !self.has_remote()? || !safe_need_to_push(&self.repo_path) {
            return Ok(());
        }

        let mut set_branch = None;
        let mut set_upstream = None;
        let repo = &self.repo_path;

        if git::git_branch_upstream(repo, "HEAD")?.is_none() {
            let remotes = git::git_remote(repo)?;
            if remotes.len() == 1 {
                let remote = &remotes[0];
                git::git_fetch(repo, Some(remote))?;
                let remote_branches = git::git_branch_remote(repo)?;

                let branch = git::git_current_branch(repo)?;
                let upstream_ref = format!("{remote}/{branch}");

                if !remote_branches.contains(&upstream_ref) {
                    set_branch = Some(branch);
                    set_upstream = Some(remote.to_string());
                }
            }
        }

        self.push(set_branch.as_deref(), set_upstream.as_deref())
    }

    pub fn finalize_force(&self, msg: &str) -> Result<(), SyncError> {
        if !self.is_init() {
            return Ok(());
        }

        if git::git_has_changes(&self.repo_path)? {
            self.commit_all(msg, false)?;
        }

        if !self.has_remote()? {
            return Ok(());
        }

        if git::git_branch_upstream(&self.repo_path, "HEAD")?.is_none() {
            self.ensure_upstream()?;
        }

        if git::git_branch_upstream(&self.repo_path, "HEAD")?.is_none() {
            return Err(SyncError::NoUpstreamConfigured);
        }

        git::git(&self.repo_path, ["push", "-q", "--force"], true)
    }

    pub fn init(&self) -> Result<(), SyncError> {
        if !self.repo_path.exists() {
            std::fs::create_dir_all(&self.repo_path)?;
        }
        git::git_init(&self.repo_path)?;
        self.commit_all("Initialize vault sync", true)?;
        Ok(())
    }

    pub fn clone(&self, url: &str, quiet: bool) -> Result<(), SyncError> {
        let path = self
            .repo_path
            .to_str()
            .ok_or_else(|| SyncError::Other("Invalid repo path".to_string()))?;

        if self.repo_path.exists() {
            let mut entries = std::fs::read_dir(&self.repo_path)?;
            if entries.next().is_some() {
                return Err(SyncError::Other(format!(
                    "Clone target is not empty: {}",
                    self.repo_path.display()
                )));
            }
        } else {
            std::fs::create_dir_all(&self.repo_path)?;
        }

        git::git_clone(&self.repo_path, url, path, quiet)
    }

    pub fn abort_state(&self) -> Result<RepositoryState, SyncError> {
        let state = git::git_state(&self.repo_path)?;
        git::git_abort(&self.repo_path, state)?;
        Ok(state)
    }
}
