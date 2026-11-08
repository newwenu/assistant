use assistant_crypto::vault_crypto::types::MasterKey;
use assistant_sync::types::SyncError;

use crate::recovery_key::EmergencyData;
use crate::types::VaultEntryExport;
use crate::vault_file::types::EmergencyDataFile;

use super::SyncEngine;

impl SyncEngine {
    pub fn write_emergency_data(
        &self,
        master_key: &MasterKey,
        emergency: &EmergencyData,
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

    pub fn rewrite_vault_meta_with_new_key(&self, master_key: &MasterKey) -> Result<(), SyncError> {
        self.vault_file.rewrite_vault_meta_with_new_key(master_key)
    }

    pub fn disaster_recovery_export(
        &self,
        master_key: &MasterKey,
    ) -> Result<(Vec<VaultEntryExport>, Vec<String>), SyncError> {
        let (entries, skipped_ids) = self.vault_file.read_all_entries(master_key)?;
        let exports = entries
            .into_iter()
            .map(|entry| {
                let imported = entry.into_import();
                let totp = imported
                    .totp_secret
                    .as_deref()
                    .and_then(crate::shared::totp::TotpConfig::from_stored);
                VaultEntryExport {
                    category: imported.category,
                    title: imported.title,
                    username: imported.username,
                    url: imported.url,
                    password: imported.password,
                    notes: imported.notes,
                    totp_secret: totp
                        .as_ref()
                        .and_then(|config| config.secret().to_str().ok())
                        .map(String::from),
                    totp_digits: totp.as_ref().map(|config| config.digits()).unwrap_or(6),
                    totp_period: totp.as_ref().map(|config| config.period()).unwrap_or(30),
                    totp_algorithm: totp
                        .as_ref()
                        .map(|config| config.algorithm().as_str().to_string())
                        .unwrap_or_else(|| "sha1".to_string()),
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
