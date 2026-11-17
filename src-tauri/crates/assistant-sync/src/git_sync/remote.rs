use crate::types::{RemoteStatus, RemoteStatusErrorKind, SyncError};

use super::helpers::{now_ms, set_status_error};
use super::{git, GitSync};

impl GitSync {
    pub fn remote_status(&self, fetch: bool) -> Result<RemoteStatus, SyncError> {
        if !self.is_init() {
            return Err(SyncError::NotInitialized);
        }

        let repo = &self.repo_path;
        let mut status = RemoteStatus {
            upstream: None,
            ahead: None,
            behind: None,
            fetched: false,
            checked_at: now_ms(),
            error_kind: None,
            error_detail: None,
        };

        let has_remote = match self.has_remote() {
            Ok(value) => value,
            Err(error) => {
                set_status_error(
                    &mut status,
                    repo,
                    RemoteStatusErrorKind::CompareFailed,
                    error,
                );
                return Ok(status);
            }
        };
        if !has_remote {
            status.error_kind = Some(RemoteStatusErrorKind::NoRemote);
            return Ok(status);
        }

        let mut upstream = match git::git_branch_upstream(repo, "HEAD") {
            Ok(value) => value,
            Err(error) => {
                set_status_error(
                    &mut status,
                    repo,
                    RemoteStatusErrorKind::CompareFailed,
                    error,
                );
                return Ok(status);
            }
        };

        let mut fetched_by_upstream = false;
        if upstream.is_none() && fetch {
            match self.ensure_upstream() {
                Ok(fetched) => {
                    fetched_by_upstream = fetched;
                    upstream = match git::git_branch_upstream(repo, "HEAD") {
                        Ok(value) => value,
                        Err(error) => {
                            set_status_error(
                                &mut status,
                                repo,
                                RemoteStatusErrorKind::CompareFailed,
                                error,
                            );
                            return Ok(status);
                        }
                    };
                }
                Err(error) => {
                    set_status_error(&mut status, repo, RemoteStatusErrorKind::FetchFailed, error);
                    return Ok(status);
                }
            }
        }

        status.fetched = fetched_by_upstream;
        let Some(upstream) = upstream else {
            status.error_kind = Some(RemoteStatusErrorKind::NoUpstream);
            status.checked_at = now_ms();
            return Ok(status);
        };
        status.upstream = Some(upstream.clone());

        if fetch && !fetched_by_upstream {
            match git::git_fetch(repo, None) {
                Ok(()) => status.fetched = true,
                Err(error) => {
                    set_status_error(&mut status, repo, RemoteStatusErrorKind::FetchFailed, error);
                }
            }
        }

        match git::git_rev_list_left_right_count(repo, &format!("HEAD...{upstream}")) {
            Ok((ahead, behind)) => {
                status.ahead = Some(ahead);
                status.behind = Some(behind);
            }
            Err(error) => {
                if status.error_kind.is_none() {
                    set_status_error(
                        &mut status,
                        repo,
                        RemoteStatusErrorKind::CompareFailed,
                        error,
                    );
                }
            }
        }

        status.checked_at = now_ms();
        Ok(status)
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
}
