pub mod git;
pub mod ssh;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::{ConflictFile, Readyness, RepositoryState, SyncConfig, SyncError};

pub const STORE_GIT_DIR: &str = ".git/";
pub const GIT_PULL_OUTDATED: Duration = Duration::from_secs(30);

pub struct GitSync {
    repo_path: PathBuf,
    #[allow(dead_code)]
    config: SyncConfig,
}

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

    pub fn readyness(&self) -> Result<Readyness, SyncError> {
        if !self.is_init() {
            return Ok(Readyness::NoSync);
        }

        match git::git_state(&self.repo_path)? {
            RepositoryState::Clean => {
                if git::git_has_changes(&self.repo_path)? {
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

        let repo = &self.repo_path;
        if git::git_branch_upstream(repo, "HEAD")?.is_none() {
            let remotes = self.tracked_remote_or_remotes()?;
            if remotes.len() != 1 {
                return Ok(());
            }

            let remote = &remotes[0];
            git::git_fetch(repo, Some(remote))?;

            let remote_branches = git::git_branch_remote(repo)?;
            if remote_branches.is_empty() {
                return Ok(());
            }

            let branch = git::git_current_branch(repo)?;
            let upstream_ref = format!("{remote}/{branch}");

            if !remote_branches.contains(&upstream_ref) {
                return Ok(());
            }
            git::git_branch_set_upstream(repo, None, &upstream_ref)?;
        }

        self.pull()
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

    pub fn remotes(&self) -> Result<Vec<String>, SyncError> {
        git::git_remote(&self.repo_path)
    }

    pub fn tracked_remote_or_remotes(&self) -> Result<Vec<String>, SyncError> {
        let branch = git::git_current_branch(&self.repo_path)?;
        let remote = git::git_config_branch_remote(&self.repo_path, &branch);
        if let Ok(Some(remote)) = remote {
            return Ok(vec![remote]);
        }
        git::git_remote(&self.repo_path)
    }

    pub fn remote_url(&self, remote: &str) -> Result<String, SyncError> {
        git::git_remote_get_url(&self.repo_path, remote)
    }

    pub fn add_remote_url(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        git::git_remote_add(&self.repo_path, remote, url)?;

        let branch = git::git_current_branch(&self.repo_path);
        if let Ok(ref branch) = branch {
            let _ = git::git_config_branch_set_remote(&self.repo_path, branch, remote);
        }

        Ok(())
    }

    pub fn set_remote_url(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        git::git_remote_remove(&self.repo_path, remote)?;
        self.add_remote_url(remote, url)
    }

    pub fn remove_remote(&self, remote: &str) -> Result<(), SyncError> {
        git::git_remote_remove(&self.repo_path, remote)
    }

    pub fn ls_remote(&self, url: &str) -> Result<usize, SyncError> {
        let out = git::git_ls_remote(&self.repo_path, url)?;
        Ok(out.lines().filter(|l| !l.trim().is_empty()).count())
    }

    pub fn has_remote(&self) -> Result<bool, SyncError> {
        if !self.is_init() {
            return Ok(false);
        }
        git::git_has_remote(&self.repo_path)
    }

    pub fn pull(&self) -> Result<(), SyncError> {
        git::git_pull(&self.repo_path)
    }

    pub fn push(
        &self,
        set_branch: Option<&str>,
        set_upstream: Option<&str>,
    ) -> Result<(), SyncError> {
        git::git_push(&self.repo_path, set_branch, set_upstream)
    }

    pub fn force_push(&self) -> Result<(), SyncError> {
        git::git(&self.repo_path, ["push", "-q", "--force"], true)
    }

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

    pub fn abort_state(&self) -> Result<RepositoryState, SyncError> {
        let state = git::git_state(&self.repo_path)?;
        git::git_abort(&self.repo_path, state)?;
        Ok(state)
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

fn safe_need_to_push(repo: &Path) -> bool {
    match need_to_push(repo) {
        Ok(push) => push,
        Err(err) => {
            log::warn!(
                "Failed to test if local branch differs from remote, assuming push needed: {err}"
            );
            true
        }
    }
}

fn need_to_push(repo: &Path) -> Result<bool, SyncError> {
    let last_pulled = git::git_last_pull_time(repo)?;
    if last_pulled.elapsed().unwrap_or(GIT_PULL_OUTDATED) > GIT_PULL_OUTDATED {
        return Ok(true);
    }

    let branch = git::git_current_branch(repo)?;
    let upstream = match git::git_branch_upstream(repo, &branch)? {
        Some(u) => u,
        None => return Ok(true),
    };

    Ok(git::git_ref_hash(repo, &branch)? != git::git_ref_hash(repo, &upstream)?)
}
