use std::path::PathBuf;
use std::sync::Mutex;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::security_db::SecurityDbState;
use crate::types::{ImportOutcome, VaultCryptoInfo, VaultEntryImport, VaultEntrySnapshot};
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};

use crate::vault_file::types::{
    Argon2Params, EmergencyDataFile, EntryBlob, ImportedEntry, VaultCryptoMeta,
};
use crate::vault_file::IntegrityResult;
use crate::vault_file::{CipherProvider, VaultFileManager};
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{
    ConflictFile, ConflictPreview, ConflictResolution, EntryPreview, EntryPreviewField,
    HistoryCommit, ImportReport, ImportSummary, PendingRemovalEntry, PendingRemovals, Readyness,
    RepositoryState, SyncConfig, SyncError, SyncResult, UnreadableReason,
};

const MASKED: &str = "•••";

fn mask_entry_preview(
    commit: &str,
    id: String,
    category: String,
    revision: u64,
    blob: EntryBlob,
) -> EntryPreview {
    EntryPreview {
        id,
        commit: commit.to_string(),
        commit_author: None,
        commit_time: None,
        category,
        revision,
        title: blob.title,
        username: blob.username,
        url: blob.url,
        password: blob.password.map(|_| MASKED.to_string()),
        notes: blob.notes,
        totp_secret: blob.totp_secret.map(|_| MASKED.to_string()),
        custom_fields: blob
            .custom_fields
            .into_iter()
            .map(|cf| EntryPreviewField {
                name: cf.name,
                value: if cf.is_secret {
                    MASKED.to_string()
                } else {
                    cf.value
                },
                is_secret: cf.is_secret,
            })
            .collect(),
        tags: blob.tags,
        is_favorite: blob.is_favorite,
        created_at: blob.created_at,
        updated_at: blob.updated_at,
    }
}

fn entry_rel_path(entry_id: &str) -> String {
    format!("entries/{}.enc", entry_id)
}

fn parse_entry_id(path: &str) -> Option<String> {
    path.trim()
        .strip_prefix("entries/")
        .and_then(|rest| rest.strip_suffix(".enc"))
        .map(|id| id.to_string())
}

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

    pub fn pending_removals(&self) -> Option<PendingRemovals> {
        self.pending_removals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn stage_pending_removals(
        &self,
        security_db: &SecurityDbState,
        removed_ids: &[String],
    ) -> Result<PendingRemovals, SyncError> {
        let local = security_db
            .list_entries(None, None, None)
            .map_err(|e| SyncError::Other(e.to_string()))?;
        let entries = removed_ids
            .iter()
            .map(|id| PendingRemovalEntry {
                id: id.clone(),
                title: local
                    .iter()
                    .find(|entry| &entry.id == id)
                    .map(|entry| entry.title.clone()),
            })
            .collect();
        let pending = PendingRemovals {
            entries,
            total_local: local.len(),
        };
        *self
            .pending_removals
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(pending.clone());
        Ok(pending)
    }

    pub fn confirm_removals(
        &self,
        security_db: &SecurityDbState,
        ids: &[String],
    ) -> Result<u32, SyncError> {
        self.prepare()?;
        let deleted = security_db
            .delete_entries_tx(ids, Some("remote"))
            .map_err(|e| SyncError::Other(e.to_string()))?;
        self.export_all_entries(security_db, false, &[])?;
        self.finalize("vault: sync")?;
        *self
            .pending_removals
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        log::info!(
            "Vault sync confirmed {} remote deletion(s): {}",
            deleted,
            ids.join(", ")
        );
        Ok(deleted as u32)
    }

    pub fn readyness(&self) -> Result<Readyness, SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .readyness()
    }

    pub fn prepare(&self) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare()
    }

    pub fn finalize(&self, msg: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finalize(msg)
    }

    pub fn pull(&self, security_db: &SecurityDbState) -> Result<SyncResult, SyncError> {
        let before = self.vault_file.list_entry_files()?;

        {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }

            if !git.has_remote()? {
                return Ok(SyncResult::NoRemote);
            }

            match git.pull() {
                Ok(()) => {}
                Err(SyncError::GitStatus(_)) => {
                    let mut conflicts = collect_conflicts(&git);
                    attach_conflict_titles(security_db, &mut conflicts);
                    return Ok(SyncResult::Conflict(conflicts));
                }
                Err(e) => return Err(e),
            }
        }

        let report = self.import_from_repo(security_db, &before)?;
        log::info!(
            "Vault pull imported {} entries (inserted {}, updated {}, skipped {}, removed {})",
            report.summary.total,
            report.summary.inserted,
            report.summary.updated,
            report.summary.skipped,
            report.removed_ids.len()
        );

        if !report.removed_ids.is_empty() {
            let pending = self.stage_pending_removals(security_db, &report.removed_ids)?;
            return Ok(SyncResult::PendingRemovals(pending));
        }

        if !report.unreadable_ids.is_empty() {
            return Ok(SyncResult::UnreadableEntries {
                reason: self.unreadable_reason(security_db)?,
                ids: report.unreadable_ids,
            });
        }

        Ok(SyncResult::Success)
    }

    pub fn import_from_repo(
        &self,
        security_db: &SecurityDbState,
        before: &[String],
    ) -> Result<ImportReport, SyncError> {
        let (imported, unreadable_ids) = {
            let mk_guard = security_db
                .get_master_key()
                .map_err(|_| SyncError::VaultLocked)?;
            let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

            let integrity = self.vault_file.verify_vault_meta_integrity(master_key)?;
            if !integrity.valid {
                return Err(self.attributed_integrity_error(security_db, integrity.message)?);
            }

            let (entries, unreadable_ids) = self.vault_file.read_all_entries(master_key)?;
            if !unreadable_ids.is_empty() && self.remote_uses_other_master_key(security_db)? {
                return Err(SyncError::VaultKeyMismatch(format!(
                    "{} entry file(s) cannot be decrypted: {}",
                    unreadable_ids.len(),
                    unreadable_ids.join(", ")
                )));
            }
            (entries, unreadable_ids)
        };

        let entries: Vec<VaultEntryImport> =
            imported.into_iter().map(|e| e.into_import()).collect();
        let total = entries.len() as u32;

        let outcomes = security_db
            .upsert_imported_entries(entries, self.vault_file.cipher_algorithm())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        let mut summary = ImportSummary {
            total,
            ..Default::default()
        };
        for outcome in outcomes {
            match outcome {
                ImportOutcome::Inserted => summary.inserted += 1,
                ImportOutcome::Updated => summary.updated += 1,
                ImportOutcome::SkippedLocallyDeleted | ImportOutcome::SkippedStale => {
                    summary.skipped += 1
                }
            }
        }

        let after = self.vault_file.list_entry_files()?;
        let removed_ids: Vec<String> = before
            .iter()
            .filter(|id| !after.iter().any(|existing| existing == *id))
            .cloned()
            .collect();

        if !removed_ids.is_empty() {
            log::warn!(
                "Vault pull removed {} entry file(s) from repository: {}",
                removed_ids.len(),
                removed_ids.join(", ")
            );
        }

        if !unreadable_ids.is_empty() {
            log::warn!(
                "Vault pull found {} undecryptable entry file(s): {}",
                unreadable_ids.len(),
                unreadable_ids.join(", ")
            );
        }

        Ok(ImportReport {
            summary,
            removed_ids,
            unreadable_ids,
        })
    }

    fn remote_uses_other_master_key(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<bool, SyncError> {
        let remote_salt = self
            .vault_file
            .read_vault_meta()
            .ok()
            .and_then(|meta| meta.crypto.map(|crypto| crypto.master_key_salt));

        let Some(remote_salt) = remote_salt else {
            return Ok(false);
        };

        let local = security_db
            .vault_crypto_info(self.vault_file.cipher_algorithm())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(remote_salt.trim() != B64.encode(local.master_key_salt))
    }

    fn attributed_integrity_error(
        &self,
        security_db: &SecurityDbState,
        message: String,
    ) -> Result<SyncError, SyncError> {
        if self.remote_uses_other_master_key(security_db)? {
            Ok(SyncError::VaultKeyMismatch(
                "Remote repository was written with a different master key".to_string(),
            ))
        } else {
            Ok(SyncError::VaultMetaIntegrityFailed(message))
        }
    }

    fn unreadable_reason(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<UnreadableReason, SyncError> {
        if self.remote_uses_other_master_key(security_db)? {
            Ok(UnreadableReason::KeyMismatch)
        } else {
            Ok(UnreadableReason::Corrupt)
        }
    }

    pub fn push(&self) -> Result<SyncResult, SyncError> {
        if let Some(pending) = self.pending_removals() {
            return Ok(SyncResult::PendingRemovals(pending));
        }

        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

        if !git.is_init() {
            return Err(SyncError::NotInitialized);
        }

        if !git.has_remote()? {
            return Err(SyncError::NoRemoteConfigured);
        }

        git.push(None, None)?;
        Ok(SyncResult::Success)
    }

    pub fn sync_now(&self, security_db: &SecurityDbState) -> Result<SyncResult, SyncError> {
        if let Some(pending) = self.pending_removals() {
            return Ok(SyncResult::PendingRemovals(pending));
        }

        let before = self.vault_file.list_entry_files()?;

        let has_remote = {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }

            git.has_remote()?
        };

        if !has_remote {
            self.export_all_entries(security_db, false, &[])?;
            return Ok(SyncResult::NoRemote);
        }

        if let Err(e) = self.prepare() {
            match e {
                SyncError::GitStatus(_) => {
                    let mut conflicts = {
                        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
                        collect_conflicts(&git)
                    };
                    attach_conflict_titles(security_db, &mut conflicts);
                    return Ok(SyncResult::Conflict(conflicts));
                }
                other => return Err(other),
            }
        }

        let report = self.import_from_repo(security_db, &before)?;

        if !report.removed_ids.is_empty() {
            log::warn!(
                "Sync detected {} remote deletion(s) awaiting confirmation: {}",
                report.removed_ids.len(),
                report.removed_ids.join(", ")
            );
            let pending = self.stage_pending_removals(security_db, &report.removed_ids)?;
            return Ok(SyncResult::PendingRemovals(pending));
        }

        self.export_all_entries(security_db, false, &report.unreadable_ids)?;

        self.finalize("vault: sync")?;

        if !report.unreadable_ids.is_empty() {
            return Ok(SyncResult::UnreadableEntries {
                reason: self.unreadable_reason(security_db)?,
                ids: report.unreadable_ids,
            });
        }

        Ok(SyncResult::Success)
    }

    pub fn force_push(&self) -> Result<SyncResult, SyncError> {
        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

        if !git.is_init() {
            return Err(SyncError::NotInitialized);
        }

        if !git.has_remote()? {
            return Err(SyncError::NoRemoteConfigured);
        }

        git.force_push()?;
        Ok(SyncResult::Success)
    }

    pub fn conflicts(&self, security_db: &SecurityDbState) -> Result<Vec<ConflictFile>, SyncError> {
        let mut conflicts = self
            .git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .detect_conflicts()?;
        attach_conflict_titles(security_db, &mut conflicts);
        Ok(conflicts)
    }

    pub fn resolve_conflict(
        &self,
        security_db: &SecurityDbState,
        path: &str,
        resolution: ConflictResolution,
        remote_copy_suffix: Option<&str>,
    ) -> Result<(), SyncError> {
        let entry_id = parse_entry_id(path).ok_or_else(|| {
            SyncError::Other(format!("conflict path is not a vault entry file: {}", path))
        })?;

        let remote_content = {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
            match resolution {
                ConflictResolution::UseLocal => {
                    git.resolve_conflict_local(path)?;
                    None
                }
                ConflictResolution::UseRemote => {
                    let content = git.show_file(":3", path)?;
                    git.resolve_conflict_remote(path)?;
                    Some(content)
                }
                ConflictResolution::MergeBoth => {
                    let content = git.show_file(":3", path)?;
                    git.resolve_conflict_local(path)?;
                    Some(content)
                }
                ConflictResolution::Skip => None,
            }
        };

        match (resolution, remote_content) {
            (ConflictResolution::UseLocal, _) => {
                self.export_entry_from_db(security_db, &entry_id)?;
            }
            (ConflictResolution::UseRemote, Some(content)) => {
                self.import_remote_entry(security_db, &content)?;
            }
            (ConflictResolution::MergeBoth, Some(content)) => {
                self.create_remote_copy(security_db, &content, remote_copy_suffix)?;
            }
            _ => {}
        }

        if self.conflicts(security_db)?.is_empty() {
            self.finalize("vault: resolve conflict")?;
        }

        Ok(())
    }

    fn export_entry_from_db(
        &self,
        security_db: &SecurityDbState,
        entry_id: &str,
    ) -> Result<(), SyncError> {
        let snapshot = security_db
            .snapshot(entry_id)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        self.vault_file.export_entry(master_key, &snapshot, false)
    }

    fn decode_entry_content(
        &self,
        security_db: &SecurityDbState,
        content: &str,
    ) -> Result<ImportedEntry, SyncError> {
        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        self.vault_file
            .decode_entry_file(master_key, content)
            .map_err(|e| SyncError::HistoryDecryptFailed(e.to_string()))
    }

    fn import_remote_entry(
        &self,
        security_db: &SecurityDbState,
        content: &str,
    ) -> Result<VaultEntrySnapshot, SyncError> {
        let entry = self
            .decode_entry_content(security_db, content)?
            .into_import();
        let vault_file = &self.vault_file;

        security_db
            .force_import_entry_tx(entry, self.vault_file.cipher_algorithm(), |mk, snapshot| {
                vault_file
                    .export_entry(mk, snapshot, false)
                    .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))
            })
            .map_err(|e| SyncError::Other(e.to_string()))
    }

    fn create_remote_copy(
        &self,
        security_db: &SecurityDbState,
        content: &str,
        suffix: Option<&str>,
    ) -> Result<VaultEntrySnapshot, SyncError> {
        let mut entry = self
            .decode_entry_content(security_db, content)?
            .into_import();
        let suffix = suffix.unwrap_or(" (remote)");
        let now = chrono::Utc::now().timestamp_millis();

        entry.id = uuid::Uuid::new_v4().to_string();
        entry.title = format!("{}{}", entry.title, suffix);
        entry.created_at = now;
        entry.updated_at = now;

        let vault_file = &self.vault_file;
        security_db
            .create_remote_copy_tx(entry, self.vault_file.cipher_algorithm(), |mk, snapshot| {
                vault_file
                    .export_entry(mk, snapshot, false)
                    .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))
            })
            .map_err(|e| SyncError::Other(e.to_string()))
    }

    pub fn get_config(&self) -> SyncConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_config(&self, config: SyncConfig) {
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = config;
    }

    pub fn init(&self, security_db: &SecurityDbState) -> Result<(), SyncError> {
        self.vault_file.ensure_structure()?;
        self.write_signed_vault_meta(security_db)?;
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

    pub fn clone_recovery_repo(&self, url: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(url, true)
    }

    pub fn checkout_recovery_repo(&self, commit: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .checkout_detached(commit)
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

    pub fn vault_file_manager(&self) -> &VaultFileManager {
        &self.vault_file
    }

    pub fn history_list(
        &self,
        entry_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HistoryCommit>, SyncError> {
        let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
        if !git.is_init() {
            return Err(SyncError::NotInitialized);
        }

        let path = entry_id.map(entry_rel_path);
        let raw = git.log(path.as_deref(), limit)?;

        Ok(raw
            .into_iter()
            .map(|entry| HistoryCommit {
                entry_id: entry_id
                    .map(|id| id.to_string())
                    .or_else(|| entry.paths.iter().find_map(|p| parse_entry_id(p))),
                hash: entry.hash,
                timestamp: entry.timestamp,
                message: entry.message,
            })
            .collect())
    }

    pub fn history_preview(
        &self,
        security_db: &SecurityDbState,
        commit: &str,
        entry_id: &str,
    ) -> Result<EntryPreview, SyncError> {
        let imported = self.read_historical_entry(security_db, commit, entry_id)?;
        Ok(mask_entry_preview(
            commit,
            imported.id,
            imported.category,
            imported.revision,
            imported.blob,
        ))
    }

    pub fn conflict_preview(
        &self,
        security_db: &SecurityDbState,
        path: &str,
    ) -> Result<ConflictPreview, SyncError> {
        let entry_id = parse_entry_id(path).ok_or_else(|| {
            SyncError::Other(format!("conflict path is not a vault entry file: {}", path))
        })?;

        let (local_content, remote_content, local_meta, remote_meta) = {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }
            (
                git.show_file(":2", path).ok(),
                git.show_file(":3", path).ok(),
                git.last_commit_info(Some("HEAD"), path).ok(),
                git.last_commit_info(Some("MERGE_HEAD"), path).ok(),
            )
        };

        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        let decode = |content: Option<&String>| -> (Option<ImportedEntry>, Option<String>) {
            let Some(content) = content else {
                return (None, None);
            };
            match self.vault_file.decode_entry_file(master_key, content) {
                Ok(imported) => (Some(imported), None),
                Err(e) => (None, Some(e.to_string())),
            }
        };

        let (local_entry, local_error) = decode(local_content.as_ref());
        let (remote_entry, remote_error) = decode(remote_content.as_ref());

        let diff_fields = match (&local_entry, &remote_entry) {
            (Some(local), Some(remote)) => diff_entry_fields(local, remote),
            _ => Vec::new(),
        };

        let build = |entry: ImportedEntry, meta: Option<(String, i64)>| -> EntryPreview {
            let mut preview =
                mask_entry_preview("", entry.id, entry.category, entry.revision, entry.blob);
            if let Some((author, time)) = meta {
                preview.commit_author = Some(author);
                preview.commit_time = Some(time);
            }
            preview
        };

        Ok(ConflictPreview {
            entry_id,
            local: local_entry.map(|entry| build(entry, local_meta)),
            remote: remote_entry.map(|entry| build(entry, remote_meta)),
            local_error,
            remote_error,
            diff_fields,
        })
    }

    pub fn reveal_conflict_field(
        &self,
        security_db: &SecurityDbState,
        path: &str,
        side: &str,
        field: &str,
    ) -> Result<String, SyncError> {
        let stage = match side {
            "local" => ":2",
            "remote" => ":3",
            _ => return Err(SyncError::Other(format!("Unknown conflict side: {}", side))),
        };

        let content = {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }
            git.show_file(stage, path)?
        };

        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;
        let imported = self.vault_file.decode_entry_file(master_key, &content)?;
        let blob = imported.blob;

        match field {
            "password" => blob.password,
            "totpSecret" => blob.totp_secret,
            "notes" => blob.notes,
            other => blob
                .custom_fields
                .iter()
                .find(|custom| custom.name == other)
                .map(|custom| custom.value.clone()),
        }
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            SyncError::Other(format!(
                "Field {} is not present in the {} version",
                field, side
            ))
        })
    }

    pub fn history_restore(
        &self,
        security_db: &SecurityDbState,
        commit: &str,
        entry_id: &str,
    ) -> Result<VaultEntrySnapshot, SyncError> {
        if let Readyness::RepoState(state) = self.readyness()? {
            return Err(SyncError::Other(format!(
                "Repository is in {:?} state; finish or abort the sync before restoring history",
                state
            )));
        }

        let imported = self.read_historical_entry(security_db, commit, entry_id)?;
        let entry = imported.into_import();

        let vault_file = &self.vault_file;
        let snapshot = security_db
            .restore_from_history_tx(
                entry,
                self.vault_file.cipher_algorithm(),
                commit,
                |mk, snap| {
                    vault_file
                        .export_entry(mk, snap, false)
                        .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))
                },
            )
            .map_err(|e| SyncError::Other(e.to_string()))?;

        self.finalize(&format!("vault: restore {}", snapshot.id))?;
        Ok(snapshot)
    }

    fn read_historical_entry(
        &self,
        security_db: &SecurityDbState,
        commit: &str,
        entry_id: &str,
    ) -> Result<ImportedEntry, SyncError> {
        let path = entry_rel_path(entry_id);
        let content = {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }
            git.show_file(commit, &path)?
        };

        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        self.vault_file
            .decode_entry_file(master_key, &content)
            .map_err(|e| SyncError::HistoryDecryptFailed(e.to_string()))
    }

    pub fn export_all_entries(
        &self,
        security_db: &SecurityDbState,
        force: bool,
        skip_ids: &[String],
    ) -> Result<u32, SyncError> {
        let snapshots = security_db
            .export_all_snapshots()
            .map_err(|e| SyncError::Other(e.to_string()))?;
        let deleted_ids = security_db
            .list_deleted_entry_ids()
            .map_err(|e| SyncError::Other(e.to_string()))?;
        let crypto_info = security_db
            .vault_crypto_info(self.vault_file.cipher_algorithm())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        self.vault_file.ensure_structure()?;

        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        let mut exported = 0u32;
        for snapshot in &snapshots {
            if skip_ids.iter().any(|id| id == &snapshot.id) {
                continue;
            }
            self.vault_file.export_entry(master_key, snapshot, force)?;
            exported += 1;
        }

        for id in &deleted_ids {
            if skip_ids.iter().any(|skip| skip == id) {
                continue;
            }
            self.vault_file.remove_entry_file(id)?;
        }

        let emergency = self.vault_file.read_emergency_data()?;
        self.vault_file.export_vault_meta(
            master_key,
            Some(to_crypto_meta(&crypto_info)),
            emergency,
        )?;

        Ok(exported)
    }

    fn write_signed_vault_meta(&self, security_db: &SecurityDbState) -> Result<(), SyncError> {
        let crypto_info = security_db
            .vault_crypto_info(self.vault_file.cipher_algorithm())
            .map_err(|e| SyncError::Other(e.to_string()))?;
        let mk_guard = security_db
            .get_master_key()
            .map_err(|_| SyncError::VaultLocked)?;
        let master_key = mk_guard.as_ref().ok_or(SyncError::VaultLocked)?;
        let emergency = self.vault_file.read_emergency_data()?;
        self.vault_file
            .export_vault_meta(master_key, Some(to_crypto_meta(&crypto_info)), emergency)
    }

    pub fn write_emergency_data(
        &self,
        master_key: &MasterKey,
        emergency: &crate::recovery_key::EmergencyData,
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

    pub fn clear_emergency_data(&self, master_key: &MasterKey) -> Result<(), SyncError> {
        self.vault_file.clear_emergency_data(master_key)
    }

    pub fn verify_vault_meta_integrity(
        &self,
        master_key: &MasterKey,
    ) -> Result<IntegrityResult, SyncError> {
        self.vault_file.verify_vault_meta_integrity(master_key)
    }

    pub fn rewrite_vault_meta_with_new_key(&self, master_key: &MasterKey) -> Result<(), SyncError> {
        self.vault_file.rewrite_vault_meta_with_new_key(master_key)
    }

    pub fn disaster_recovery_export(
        &self,
        master_key: &MasterKey,
    ) -> Result<(Vec<crate::types::VaultEntryExport>, Vec<String>), SyncError> {
        let (entries, skipped_ids) = self.vault_file.read_all_entries(master_key)?;
        let exports = entries
            .into_iter()
            .map(|entry| {
                let imported = entry.into_import();
                crate::types::VaultEntryExport {
                    category: imported.category,
                    title: imported.title,
                    username: imported.username,
                    url: imported.url,
                    password: imported.password,
                    notes: imported.notes,
                    totp_secret: imported.totp_secret,
                    tags: imported.tags,
                    custom_fields: imported.custom_fields,
                    is_favorite: imported.is_favorite,
                    created_at: imported.created_at,
                    updated_at: imported.updated_at,
                }
            })
            .collect();
        Ok((exports, skipped_ids))
    }
}

fn collect_conflicts(git: &GitSync) -> Vec<ConflictFile> {
    match git.detect_conflicts() {
        Ok(conflicts) => conflicts,
        Err(e) => {
            log::warn!("Failed to enumerate conflicted files: {e}");
            Vec::new()
        }
    }
}

fn attach_conflict_titles(security_db: &SecurityDbState, conflicts: &mut [ConflictFile]) {
    for conflict in conflicts.iter_mut() {
        let Some(entry_id) = parse_entry_id(&conflict.path) else {
            continue;
        };
        match security_db.get_entry(&entry_id) {
            Ok(entry) => conflict.title = Some(entry.title),
            Err(_) => continue,
        }
    }
}

fn diff_entry_fields(local: &ImportedEntry, remote: &ImportedEntry) -> Vec<String> {
    let mut fields = Vec::new();

    if local.category != remote.category {
        fields.push("category".to_string());
    }
    if local.revision != remote.revision {
        fields.push("revision".to_string());
    }

    let a = &local.blob;
    let b = &remote.blob;

    if a.title != b.title {
        fields.push("title".to_string());
    }
    if a.username != b.username {
        fields.push("username".to_string());
    }
    if a.url != b.url {
        fields.push("url".to_string());
    }
    if a.password != b.password {
        fields.push("password".to_string());
    }
    if a.notes != b.notes {
        fields.push("notes".to_string());
    }
    if a.totp_secret != b.totp_secret {
        fields.push("totpSecret".to_string());
    }
    if a.tags != b.tags {
        fields.push("tags".to_string());
    }
    if a.is_favorite != b.is_favorite {
        fields.push("isFavorite".to_string());
    }
    if a.created_at != b.created_at {
        fields.push("createdAt".to_string());
    }
    if a.updated_at != b.updated_at {
        fields.push("updatedAt".to_string());
    }

    let names: std::collections::BTreeSet<&str> = a
        .custom_fields
        .iter()
        .map(|field| field.name.as_str())
        .chain(b.custom_fields.iter().map(|field| field.name.as_str()))
        .collect();

    for name in names {
        let left = a.custom_fields.iter().find(|field| field.name == name);
        let right = b.custom_fields.iter().find(|field| field.name == name);
        let same = match (left, right) {
            (Some(left), Some(right)) => {
                left.value == right.value && left.is_secret == right.is_secret
            }
            _ => false,
        };
        if !same {
            fields.push(format!("custom:{}", name));
        }
    }

    fields
}

fn to_crypto_meta(info: &VaultCryptoInfo) -> VaultCryptoMeta {
    VaultCryptoMeta {
        master_key_salt: B64.encode(info.master_key_salt),
        hash_salt: B64.encode(info.hash_salt),
        argon2_params: Argon2Params {
            m: info.argon2_m,
            t: info.argon2_t,
            p: info.argon2_p,
        },
        default_cipher: match info.default_cipher {
            CipherAlgorithm::ChaCha20Poly1305 => "chacha20poly1305".to_string(),
            CipherAlgorithm::Aes256Gcm => "aes256Gcm".to_string(),
        },
    }
}
