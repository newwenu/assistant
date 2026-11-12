use assistant_sync::types::{RemoteStatus, RepositoryState, SyncConfig, SyncError};

use super::SyncEngine;

impl SyncEngine {
    pub fn get_config(&self) -> SyncConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_config(&self, config: SyncConfig) {
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = config;
    }

    pub fn init(&self, security_db: &crate::security_db::SecurityDbState) -> Result<(), SyncError> {
        self.vault_file.ensure_structure()?;
        self.vault_file.ensure_gitignore()?;
        if let Ok(removed) = self.vault_file.clean_temp_files() {
            if removed > 0 {
                log::info!(
                    "Cleaned {} stale temporary file(s) from vault repo",
                    removed
                );
            }
        }
        {
            let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
            let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;
            crate::lifecycle::sync_write_meta(&self.vault_file, dek)
                .map_err(|e| SyncError::Other(e.to_string()))?;
        }
        self.refresh_anchor(security_db)?;
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .init()
    }

    pub fn clone_repo(&self, url: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(url, true)
    }

    pub fn add_remote(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add_remote_url(remote, url)
    }

    pub fn set_remote(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_remote_url(remote, url)
    }

    pub fn remove_remote(&self, remote: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove_remote(remote)
    }

    pub fn abort_repo_state(&self) -> Result<RepositoryState, SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .abort_state()
    }

    pub fn discard_local_changes(&self) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset_hard_all()
    }

    pub fn test_connection(&self, url: Option<&str>) -> Result<usize, SyncError> {
        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
        let target = match url {
            Some(u) if !u.trim().is_empty() => u.trim().to_string(),
            _ => self
                .config
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remote_url
                .clone()
                .ok_or_else(|| SyncError::GitCli("No remote URL configured".to_string()))?,
        };
        git.ls_remote(&target)
    }

    pub fn ensure_remote(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
        if !git.is_init() {
            return Ok(());
        }
        if let Ok(existing) = git.remote_url(remote) {
            if existing.trim() == url.trim() {
                return Ok(());
            }
        }
        if git.remotes()?.iter().any(|r| r == remote) {
            git.set_remote_url(remote, url)
        } else {
            git.add_remote_url(remote, url)
        }
    }

    pub fn remote_status(&self, fetch: bool) -> Result<RemoteStatus, SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remote_status(fetch)
    }

    pub fn remote_url(&self, remote: &str) -> Result<Option<String>, SyncError> {
        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
        if !git.is_init() {
            return Ok(None);
        }
        match git.remote_url(remote) {
            Ok(url) if !url.trim().is_empty() => Ok(Some(url.trim().to_string())),
            _ => Ok(None),
        }
    }
}
