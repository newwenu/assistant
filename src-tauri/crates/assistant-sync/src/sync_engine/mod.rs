use std::path::PathBuf;
use std::sync::Mutex;

use assistant_security::security_db::SecurityDbState;

use crate::git_sync::GitSync;
use crate::types::{ConflictResolution, Readyness, SyncConfig, SyncError, SyncResult};
use crate::vault_file::types::EmergencyDataFile;
use crate::vault_file::IntegrityResult;
use crate::vault_file::VaultFileManager;

pub struct SyncEngine {
    git_sync: Mutex<GitSync>,
    vault_file: VaultFileManager,
    config: Mutex<SyncConfig>,
}

impl SyncEngine {
    pub fn new(vault_dir: PathBuf, config: SyncConfig) -> Self {
        let git_sync = GitSync::new(vault_dir.clone(), config.clone());
        let vault_file = VaultFileManager::new(vault_dir);
        Self {
            git_sync: Mutex::new(git_sync),
            vault_file,
            config: Mutex::new(config),
        }
    }

    pub fn readyness(&self) -> Result<Readyness, SyncError> {
        self.git_sync.lock().unwrap().readyness()
    }

    pub fn prepare(&self) -> Result<(), SyncError> {
        self.git_sync.lock().unwrap().prepare()
    }

    pub fn finalize(&self, msg: &str) -> Result<(), SyncError> {
        self.git_sync.lock().unwrap().finalize(msg)
    }

    pub fn pull(&self) -> Result<SyncResult, SyncError> {
        let git = self.git_sync.lock().unwrap();

        if !git.is_init() {
            return Ok(SyncResult::UpToDate);
        }

        match git.pull() {
            Ok(()) => {
                let readyness = git.readyness()?;
                match readyness {
                    Readyness::Ready | Readyness::Dirty => Ok(SyncResult::Success),
                    Readyness::RepoState(_) => Ok(SyncResult::Conflict(Vec::new())),
                    Readyness::NoSync => Ok(SyncResult::UpToDate),
                }
            }
            Err(SyncError::GitStatus(_)) => Ok(SyncResult::Conflict(Vec::new())),
            Err(e) => Err(e),
        }
    }

    pub fn push(&self) -> Result<SyncResult, SyncError> {
        let git = self.git_sync.lock().unwrap();

        if !git.is_init() {
            return Ok(SyncResult::UpToDate);
        }

        if !git.has_remote()? {
            return Ok(SyncResult::UpToDate);
        }

        git.push(None, None)?;
        Ok(SyncResult::Success)
    }

    pub fn sync_now(&self) -> Result<SyncResult, SyncError> {
        self.prepare()?;

        let pull_result = self.pull()?;

        if let SyncResult::Conflict(files) = &pull_result {
            if !files.is_empty() {
                return Ok(pull_result);
            }
        }

        let git = self.git_sync.lock().unwrap();
        if git.has_remote()? {
            let msg = format!("Sync at {}", chrono::Utc::now().to_rfc3339());
            git.finalize(&msg)?;
        }

        Ok(SyncResult::Success)
    }

    pub fn force_push(&self) -> Result<SyncResult, SyncError> {
        let git = self.git_sync.lock().unwrap();
        git.force_push()?;
        Ok(SyncResult::Success)
    }

    pub fn resolve_conflict(
        &self,
        path: &str,
        resolution: ConflictResolution,
    ) -> Result<(), SyncError> {
        let git = self.git_sync.lock().unwrap();
        match resolution {
            ConflictResolution::UseLocal => git.resolve_conflict_local(path),
            ConflictResolution::UseRemote => git.resolve_conflict_remote(path),
            ConflictResolution::MergeBoth => {
                git.resolve_conflict_local(path)?;
                Ok(())
            }
            ConflictResolution::Skip => Ok(()),
        }
    }

    pub fn get_config(&self) -> SyncConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn set_config(&self, config: SyncConfig) {
        *self.config.lock().unwrap() = config;
    }

    pub fn init(&self) -> Result<(), SyncError> {
        self.vault_file.ensure_structure()?;
        self.git_sync.lock().unwrap().init()
    }

    pub fn clone_repo(&self, url: &str) -> Result<(), SyncError> {
        self.git_sync.lock().unwrap().clone(url, true)
    }

    pub fn add_remote(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        self.git_sync.lock().unwrap().add_remote_url(remote, url)
    }

    pub fn set_remote(&self, remote: &str, url: &str) -> Result<(), SyncError> {
        self.git_sync.lock().unwrap().set_remote_url(remote, url)
    }

    pub fn vault_file_manager(&self) -> &VaultFileManager {
        &self.vault_file
    }

    pub fn export_all_entries(&self, security_db: &SecurityDbState) -> Result<u32, SyncError> {
        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        let summaries = security_db
            .list_entries(None, None, None)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        self.vault_file.ensure_structure()?;

        let mut exported = 0u32;
        for summary in &summaries {
            let _ = self.vault_file.export_entry(
                master_key,
                &summary.id,
                &summary.category.to_string(),
                &summary.title,
                summary.username.as_deref(),
                summary.url.as_deref(),
                None,
                None,
                None,
                None,
                &summary.tags,
                summary.is_favorite,
                0,
                0,
                summary.updated_at,
            );
            exported += 1;
        }

        Ok(exported)
    }

    pub fn write_emergency_data(
        &self,
        master_key: &assistant_security::vault_crypto::types::MasterKey,
        emergency: &assistant_security::recovery_key::EmergencyData,
    ) -> Result<(), SyncError> {
        let file_emergency = EmergencyDataFile {
            token: emergency.token.clone(),
            salt: emergency.salt.clone(),
        };
        self.vault_file
            .write_emergency_data(master_key, &file_emergency)
    }

    pub fn read_emergency_data(&self) -> Result<Option<EmergencyDataFile>, SyncError> {
        self.vault_file.read_emergency_data()
    }

    pub fn verify_vault_meta_integrity(
        &self,
        master_key: &assistant_security::vault_crypto::types::MasterKey,
    ) -> Result<IntegrityResult, SyncError> {
        self.vault_file.verify_vault_meta_integrity(master_key)
    }
}