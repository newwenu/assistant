use std::fs;
use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sha2::{Digest, Sha256};

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::derive_meta_key;
use assistant_crypto::vault_crypto::types::Dek;
use assistant_sync::types::SyncError;

use crate::types::PlainEntry;
use crate::vault_file::types::{ManifestEntry, VaultManifest};
use crate::vault_file::{decode_entry_content, VaultFileManager};

use super::decode_paper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverySkip {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RecoveryExport {
    pub entries: Vec<PlainEntry>,
    pub skipped: Vec<RecoverySkip>,
}

pub struct RecoverySession {
    dek: Dek,
    entries_dir: PathBuf,
    manifest: VaultManifest,
    dek_version: u32,
}

impl RecoverySession {
    pub fn dek_version(&self) -> u32 {
        self.dek_version
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.manifest
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect()
    }

    pub fn export_all(&self) -> Result<RecoveryExport, VaultError> {
        let mut entries = Vec::with_capacity(self.manifest.entries.len());
        let mut skipped = Vec::new();
        for manifest_entry in &self.manifest.entries {
            match self.export_one(manifest_entry) {
                Ok(entry) => entries.push(entry),
                Err(VaultError::VaultManifestMismatch) => {
                    return Err(VaultError::VaultManifestMismatch);
                }
                Err(err) => skipped.push(RecoverySkip {
                    id: manifest_entry.id.clone(),
                    reason: err.to_string(),
                }),
            }
        }
        Ok(RecoveryExport { entries, skipped })
    }

    fn export_one(&self, manifest_entry: &ManifestEntry) -> Result<PlainEntry, VaultError> {
        if !is_safe_entry_id(&manifest_entry.id) {
            return Err(VaultError::VaultManifestMismatch);
        }

        let path = self.entries_dir.join(format!("{}.enc", manifest_entry.id));
        let bytes = fs::read(&path)?;

        let actual_sha = B64.encode(Sha256::digest(&bytes));
        if actual_sha != manifest_entry.sha256 {
            return Err(VaultError::VaultManifestMismatch);
        }

        let content = String::from_utf8(bytes).map_err(|_| {
            VaultError::EntryCorrupted(format!(
                "entry file {} is not valid utf-8",
                manifest_entry.id
            ))
        })?;

        let imported =
            decode_entry_content(&self.dek, &content).map_err(sync_error_to_vault_error)?;

        Ok(PlainEntry {
            id: imported.id,
            category: imported.category,
            blob: imported.blob,
        })
    }
}

pub fn open_recovery_session(
    repo: &VaultFileManager,
    raw_key: &str,
) -> Result<RecoverySession, VaultError> {
    let dek = decode_paper(raw_key)?;
    let meta = repo.read_vault_meta_v2()?;

    let meta_key = derive_meta_key(&dek);
    let integrity = repo.verify_meta_integrity(&meta_key)?;
    if !integrity.valid {
        return Err(VaultError::InvalidRecoveryKeyOrMetaTampered);
    }

    repo.verify_manifest()?;

    Ok(RecoverySession {
        dek,
        entries_dir: repo.entries_dir(),
        manifest: meta.manifest.clone(),
        dek_version: meta.dek_version,
    })
}

fn is_safe_entry_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains("..")
        && !id.contains('\0')
}

fn sync_error_to_vault_error(err: SyncError) -> VaultError {
    match err {
        SyncError::Io(msg) => VaultError::Io(std::io::Error::other(msg)),
        SyncError::EntryCorrupted(msg) => VaultError::EntryCorrupted(msg),
        SyncError::UnsupportedEntryFileVersion(msg) => VaultError::VaultCorrupted(msg),
        SyncError::VaultKeyMismatch(msg) => VaultError::Crypto(msg),
        other => VaultError::Other(other.to_string()),
    }
}
