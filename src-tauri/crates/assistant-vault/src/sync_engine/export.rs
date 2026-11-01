use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, MasterKey};
use assistant_sync::types::SyncError;

use crate::security_db::SecurityDbState;
use crate::types::VaultCryptoInfo;
use crate::vault_file::types::{Argon2Params, VaultCryptoMeta};
use crate::vault_file::IntegrityResult;

use super::SyncEngine;

impl SyncEngine {
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

    pub(super) fn write_signed_vault_meta(
        &self,
        security_db: &SecurityDbState,
    ) -> Result<(), SyncError> {
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

    pub fn verify_vault_meta_integrity(
        &self,
        master_key: &MasterKey,
    ) -> Result<IntegrityResult, SyncError> {
        self.vault_file.verify_vault_meta_integrity(master_key)
    }
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
