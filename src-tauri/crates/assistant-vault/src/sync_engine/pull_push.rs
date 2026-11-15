use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::derive_meta_key;
use assistant_sync::types::{
    ImportReport, ImportSummary, PendingRemovalEntry, PendingRemovals, RepositoryState, SyncError,
    SyncResult, UnreadableReason,
};

use crate::security_db::SecurityDbState;
use crate::types::{ImportOutcome, VaultEntryImport};
use crate::vault_file::types::VaultMetaFileV2;

use super::preview::{attach_conflict_titles, collect_conflicts};
use super::SyncEngine;

fn map_meta_error(err: VaultError) -> SyncError {
    match err {
        VaultError::VaultVersionTooNew => SyncError::UnsupportedVaultVersion,
        VaultError::VaultCorrupted(message) => SyncError::VaultCorrupted(message),
        VaultError::NotInitialized => SyncError::NotInitialized,
        other => SyncError::Other(other.to_string()),
    }
}

impl SyncEngine {
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
        // `"remote"` 是**来源标签**（写进 `{entryId}@remote` 的 suffix），不是整条 `detail`。
        let deleted = security_db
            .delete_entries_tx(ids, Some("remote"))
            .map_err(|e| SyncError::Other(e.to_string()))?;
        self.export_all_entries(security_db, &[])?;
        self.refresh_anchor(security_db)?;
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
        u32::try_from(deleted)
            .map_err(|_| SyncError::Other("deleted entry count exceeds u32".to_string()))
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

            git.ensure_upstream()?;

            match git.pull() {
                Ok(()) => {}
                Err(e) => {
                    let mut conflicts = collect_conflicts(&git);
                    let blocked =
                        !conflicts.is_empty() || !matches!(git.state()?, RepositoryState::Clean);
                    if !blocked {
                        return Err(e);
                    }
                    attach_conflict_titles(security_db, &mut conflicts);
                    return Ok(SyncResult::Conflict(conflicts));
                }
            }
        }

        let remote_meta = self.validate_remote_meta(security_db)?;

        let report = self.import_from_repo(security_db, &before)?;
        log::info!(
            "Vault pull imported {} entries (inserted {}, updated {}, skipped {}, removed {})",
            report.summary.total,
            report.summary.inserted,
            report.summary.updated,
            report.summary.skipped,
            report.removed_ids.len()
        );

        self.advance_anchor(security_db, &remote_meta)?;

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

    pub(super) fn validate_meta_identity(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<VaultMetaFileV2, SyncError> {
        self.vault_file.check_repository_layout()?;

        let remote = self
            .vault_file
            .read_vault_meta_v2()
            .map_err(map_meta_error)?;

        let local_vault_id = security_db
            .vault_id()
            .map_err(|e| SyncError::Other(e.to_string()))?
            .ok_or_else(|| SyncError::Other("local vault id is missing".to_string()))?;
        if remote.vault_id != local_vault_id {
            return Err(SyncError::VaultKeyMismatch(format!(
                "remote repository belongs to vault {}",
                remote.vault_id
            )));
        }

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;
        let local_dek_version = dek_guard.dek_version().ok_or(SyncError::VaultLocked)?;
        if remote.dek_version > local_dek_version {
            return Err(SyncError::VaultDekVersionAhead {
                remote: remote.dek_version,
                local: local_dek_version,
            });
        }
        if remote.dek_version < local_dek_version {
            return Err(SyncError::VaultDekVersionBehind {
                remote: remote.dek_version,
                local: local_dek_version,
            });
        }

        let meta_key = derive_meta_key(dek);
        let integrity = self
            .vault_file
            .verify_meta_integrity(&meta_key)
            .map_err(|e| SyncError::Other(e.to_string()))?;
        if !integrity.valid {
            return Err(SyncError::VaultMetaIntegrityFailed(integrity.message));
        }

        Ok(remote)
    }

    fn validate_remote_meta(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<VaultMetaFileV2, SyncError> {
        let remote = self.validate_meta_identity(security_db)?;

        self.vault_file.verify_manifest().map_err(|e| match e {
            assistant_crypto::error::VaultError::VaultManifestMismatch => {
                SyncError::VaultManifestMismatch
            }
            other => SyncError::Other(other.to_string()),
        })?;

        Ok(remote)
    }

    pub(super) fn advance_anchor(
        &self,
        security_db: &SecurityDbState,
        remote_meta: &VaultMetaFileV2,
    ) -> Result<(), SyncError> {
        let meta_hash = self
            .vault_file
            .meta_file_hash()
            .map_err(|e| SyncError::Other(e.to_string()))?;
        security_db
            .refresh_anchor(&meta_hash, remote_meta.dek_version)
            .map_err(|e| SyncError::Other(e.to_string()))
    }

    /// 本地写完 `.vault-meta` 后前移 anchor。调用方必须保证此刻不持有 session 锁。
    pub(super) fn refresh_anchor(&self, security_db: &SecurityDbState) -> Result<(), SyncError> {
        crate::lifecycle::refresh_anchor(security_db, &self.vault_file)
            .map_err(|e| SyncError::Other(e.to_string()))
    }

    pub fn import_from_repo(
        &self,
        security_db: &SecurityDbState,
        before: &[String],
    ) -> Result<ImportReport, SyncError> {
        let remote_meta = self
            .vault_file
            .read_vault_meta_v2()
            .map_err(map_meta_error)?;

        let (imported, unreadable_ids) = {
            let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
            let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;
            self.vault_file.read_all_entries(dek)?
        };

        let entries: Vec<VaultEntryImport> =
            imported.into_iter().map(|e| e.into_import()).collect();
        let total = u32::try_from(entries.len())
            .map_err(|_| SyncError::Other("entry count exceeds u32".to_string()))?;

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
        let manifest_ids: Vec<&str> = remote_meta
            .manifest
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let removed_ids: Vec<String> = before
            .iter()
            .filter(|id| !after.iter().any(|existing| existing == *id))
            .filter(|id| !manifest_ids.contains(&id.as_str()))
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

    fn remote_uses_other_dek(&self, security_db: &SecurityDbState) -> Result<bool, SyncError> {
        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let Some(dek) = dek_guard.as_ref() else {
            return Ok(false);
        };
        if !self.vault_file.meta_path().exists() {
            return Ok(false);
        }
        let meta_key = derive_meta_key(dek);
        let result = self
            .vault_file
            .verify_meta_integrity(&meta_key)
            .map_err(|e| SyncError::Other(e.to_string()))?;
        Ok(!result.valid)
    }

    fn unreadable_reason(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<UnreadableReason, SyncError> {
        if self.remote_uses_other_dek(security_db)? {
            Ok(UnreadableReason::KeyMismatch)
        } else {
            Ok(UnreadableReason::Corrupt)
        }
    }

    pub fn push(&self, security_db: &SecurityDbState) -> Result<SyncResult, SyncError> {
        if let Some(pending) = self.pending_removals() {
            return Ok(SyncResult::PendingRemovals(pending));
        }

        {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }

            if !git.has_remote()? {
                return Err(SyncError::NoRemoteConfigured);
            }
        }

        self.export_all_entries(security_db, &[])?;
        self.refresh_anchor(security_db)?;
        self.finalize("vault: sync")?;
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
            self.export_all_entries(security_db, &[])?;
            self.refresh_anchor(security_db)?;
            self.finalize("vault: sync")?;
            return Ok(SyncResult::NoRemote);
        }

        if let Err(e) = self.prepare() {
            let (mut conflicts, blocked) = {
                let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
                let conflicts = collect_conflicts(&git);
                let blocked =
                    !conflicts.is_empty() || !matches!(git.state()?, RepositoryState::Clean);
                (conflicts, blocked)
            };
            if !blocked {
                return Err(e);
            }
            attach_conflict_titles(security_db, &mut conflicts);
            return Ok(SyncResult::Conflict(conflicts));
        }

        let remote_meta = self.validate_remote_meta(security_db)?;

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

        self.export_all_entries(security_db, &report.unreadable_ids)?;

        self.finalize("vault: sync")?;

        let meta_hash = self
            .vault_file
            .meta_file_hash()
            .map_err(|e| SyncError::Other(e.to_string()))?;
        security_db
            .refresh_anchor(&meta_hash, remote_meta.dek_version)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        if !report.unreadable_ids.is_empty() {
            return Ok(SyncResult::UnreadableEntries {
                reason: self.unreadable_reason(security_db)?,
                ids: report.unreadable_ids,
            });
        }

        Ok(SyncResult::Success)
    }

    pub fn force_push(&self, security_db: &SecurityDbState) -> Result<SyncResult, SyncError> {
        {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());

            if !git.is_init() {
                return Err(SyncError::NotInitialized);
            }

            if !git.has_remote()? {
                return Err(SyncError::NoRemoteConfigured);
            }
        }

        self.export_all_entries(security_db, &[])?;
        self.refresh_anchor(security_db)?;
        self.finalize_force("vault: sync")?;
        Ok(SyncResult::Success)
    }
}
