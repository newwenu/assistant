use assistant_sync::types::{
    ConflictFile, ConflictPreview, ConflictResolution, EntryPreview, HistoryCommit, Readyness,
    SyncError,
};

use crate::security_db::SecurityDbState;
use crate::types::VaultEntrySnapshot;
use crate::vault_file::types::ImportedEntry;

use super::preview::{
    attach_conflict_titles, diff_entry_fields, entry_rel_path, mask_entry_preview, parse_entry_id,
};
use super::SyncEngine;

impl SyncEngine {
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
        if path == crate::vault_file::VAULT_META_FILE_NAME {
            return Err(SyncError::Other(
                ".vault-meta conflicts are never auto-merged; resolve manually".to_string(),
            ));
        }

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

        self.finish_resolution(security_db)
    }

    pub fn resolve_meta_conflict(
        &self,
        security_db: &SecurityDbState,
        use_remote: bool,
    ) -> Result<(), SyncError> {
        let path = crate::vault_file::VAULT_META_FILE_NAME;
        {
            let git = self.git_sync.lock().unwrap_or_else(|e| e.into_inner());
            if use_remote {
                git.resolve_conflict_remote(path)?;
            } else {
                git.resolve_conflict_local(path)?;
            }
        }

        self.finish_resolution(security_db)
    }

    fn finish_resolution(&self, security_db: &SecurityDbState) -> Result<(), SyncError> {
        if !self.conflicts(security_db)?.is_empty() {
            return Ok(());
        }

        self.validate_meta_identity(security_db)?;

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;
        let meta = crate::lifecycle::sync_write_meta(&self.vault_file, dek)
            .map_err(|e| SyncError::Other(e.to_string()))?;
        drop(dek_guard);

        self.advance_anchor(security_db, &meta)?;
        self.finalize("vault: resolve conflict")?;

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

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        self.vault_file.export_entry(dek, &snapshot)?;
        Ok(())
    }

    fn decode_entry_content(
        &self,
        security_db: &SecurityDbState,
        content: &str,
    ) -> Result<ImportedEntry, SyncError> {
        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        self.vault_file
            .decode_entry_file(dek, content)
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
            .force_import_entry_tx(
                entry,
                self.vault_file.cipher_algorithm(),
                |dek, snapshot| {
                    vault_file
                        .export_entry(dek, snapshot)
                        .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))
                },
            )
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
            .create_remote_copy_tx(
                entry,
                self.vault_file.cipher_algorithm(),
                |dek, snapshot| {
                    vault_file
                        .export_entry(dek, snapshot)
                        .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))
                },
            )
            .map_err(|e| SyncError::Other(e.to_string()))
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

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        let decode = |content: Option<&String>| -> (Option<ImportedEntry>, Option<String>) {
            let Some(content) = content else {
                return (None, None);
            };
            match self.vault_file.decode_entry_file(dek, content) {
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

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;
        let imported = self.vault_file.decode_entry_file(dek, &content)?;
        let blob = imported.blob;

        match field {
            "password" => blob.password,
            "totpSecret" => blob.totp.map(|totp| totp.secret),
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
                |dek, snap| {
                    vault_file
                        .export_entry(dek, snap)
                        .map_err(|e| assistant_crypto::error::VaultError::Other(e.to_string()))?;
                    crate::lifecycle::sync_write_meta(vault_file, dek).map(|_| ())
                },
            )
            .map_err(|e| SyncError::Other(e.to_string()))?;

        self.refresh_anchor(security_db)?;
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

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        // rotate_dek 后旧 dek 不保留，历史文件声明的 dekVersion 与当前会话不一致时
        // 必然不可读，必须返回 `HistoryDekUnavailable` 而不是伪装成解密失败。
        let current_dek_version = dek_guard.dek_version().ok_or(SyncError::VaultLocked)?;
        let declared = crate::vault_file::peek_entry_dek_version(&content)?;
        if declared != current_dek_version {
            log::debug!(
                "history entry {} in commit {} uses retired dekVersion {} (current {})",
                entry_id,
                commit,
                declared,
                current_dek_version
            );
            return Err(SyncError::HistoryDekUnavailable(declared));
        }

        self.vault_file
            .decode_entry_file(dek, &content)
            .map_err(|e| SyncError::HistoryDecryptFailed(e.to_string()))
    }
}
