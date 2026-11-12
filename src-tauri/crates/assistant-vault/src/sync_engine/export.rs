use assistant_crypto::vault_crypto::types::MetaKey;
use assistant_sync::types::SyncError;

use crate::security_db::SecurityDbState;
use crate::vault_file::IntegrityResult;

use super::SyncEngine;

impl SyncEngine {
    pub fn export_all_entries(
        &self,
        security_db: &SecurityDbState,
        skip_ids: &[String],
    ) -> Result<u32, SyncError> {
        let snapshots = security_db
            .export_all_snapshots()
            .map_err(|e| SyncError::Other(e.to_string()))?;
        let deleted_ids = security_db
            .list_deleted_entry_ids()
            .map_err(|e| SyncError::Other(e.to_string()))?;

        self.vault_file.ensure_structure()?;
        self.vault_file.check_repository_layout()?;

        let dek_guard = security_db.get_dek().map_err(|_| SyncError::VaultLocked)?;
        let dek = dek_guard.as_ref().ok_or(SyncError::VaultLocked)?;

        let mut exported = 0u32;
        for snapshot in &snapshots {
            if skip_ids.iter().any(|id| id == &snapshot.id) {
                continue;
            }
            self.vault_file.export_entry(dek, snapshot)?;
            exported += 1;
        }

        for id in &deleted_ids {
            if skip_ids.iter().any(|skip| skip == id) {
                continue;
            }
            self.vault_file.remove_entry_file(id)?;
        }

        crate::lifecycle::sync_write_meta(&self.vault_file, dek)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(exported)
    }

    pub fn verify_meta_integrity(&self, meta_key: &MetaKey) -> Result<IntegrityResult, SyncError> {
        self.vault_file
            .verify_meta_integrity(meta_key)
            .map_err(|e| SyncError::Other(e.to_string()))
    }
}
