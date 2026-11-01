use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use assistant_sync::types::{
    ImportReport, ImportSummary, PendingRemovalEntry, PendingRemovals, SyncError, SyncResult,
    UnreadableReason,
};

use crate::security_db::SecurityDbState;
use crate::types::{ImportOutcome, VaultEntryImport};

use super::preview::{attach_conflict_titles, collect_conflicts};
use super::SyncEngine;

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
}
