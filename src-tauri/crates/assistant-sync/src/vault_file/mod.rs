pub mod types;

use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use assistant_security::types::{VaultCategory, VaultEntryImport, VaultEntrySnapshot};
use assistant_security::vault_crypto::types::{CipherAlgorithm, MasterKey, VaultPlaintext};
use assistant_security::vault_crypto::VaultCrypto;

use crate::types::SyncError;
use types::{
    BlobCustomField, EmergencyDataFile, EntryBlob, EntryFile, ImportedEntry, VaultCryptoMeta,
    VaultMetaFile,
};

type HmacSha256 = Hmac<Sha256>;

const ENTRIES_DIR: &str = "entries";
const VAULT_META_FILE: &str = ".vault-meta";
const VAULT_META_VERSION: u32 = 1;

pub struct VaultFileManager {
    vault_dir: PathBuf,
}

impl VaultFileManager {
    pub fn new(vault_dir: PathBuf) -> Self {
        Self { vault_dir }
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub fn entries_dir(&self) -> PathBuf {
        self.vault_dir.join(ENTRIES_DIR)
    }

    pub fn ensure_structure(&self) -> Result<(), SyncError> {
        fs::create_dir_all(self.entries_dir())?;
        Ok(())
    }

    pub fn entry_file_path(&self, entry_id: &str) -> PathBuf {
        self.entries_dir().join(format!("{}.enc", entry_id))
    }

    pub fn export_entry(
        &self,
        master_key: &MasterKey,
        snapshot: &VaultEntrySnapshot,
        force: bool,
    ) -> Result<(), SyncError> {
        self.ensure_structure()?;

        let path = self.entry_file_path(&snapshot.id);
        if !force {
            if let Some(existing) = read_entry_file(&path) {
                if existing.revision == snapshot.revision {
                    return Ok(());
                }
            }
        }

        let blob = EntryBlob {
            title: snapshot.title.clone(),
            username: snapshot.username.clone(),
            url: snapshot.url.clone(),
            password: snapshot.password.clone(),
            notes: snapshot.notes.clone(),
            custom_fields: snapshot
                .custom_fields
                .iter()
                .map(|cf| BlobCustomField {
                    name: cf.name.clone(),
                    value: cf.value.clone(),
                    is_secret: cf.is_protected,
                })
                .collect(),
            totp_secret: snapshot.totp_secret.clone(),
            totp_digits: snapshot.totp_digits,
            totp_period: snapshot.totp_period,
            tags: snapshot.tags.clone(),
            is_favorite: snapshot.is_favorite,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        };

        let blob_json = serde_json::to_vec(&blob).map_err(|e| SyncError::Other(e.to_string()))?;
        let plaintext = VaultPlaintext::from(blob_json);
        let entry_key = VaultCrypto::derive_entry_key(master_key, &snapshot.id);
        let encrypted =
            VaultCrypto::encrypt_field(&plaintext, entry_key.expose(), CipherAlgorithm::default())
                .map_err(|e| SyncError::Other(e.to_string()))?;

        let file = EntryFile {
            id: snapshot.id.clone(),
            category: snapshot.category.to_string(),
            revision: snapshot.revision,
            encrypted_blob: encrypted,
        };

        let json =
            serde_json::to_string_pretty(&file).map_err(|e| SyncError::Other(e.to_string()))?;
        write_atomically(&path, json.as_bytes())
    }

    pub fn import_entry(
        &self,
        master_key: &MasterKey,
        entry_id: &str,
    ) -> Result<ImportedEntry, SyncError> {
        let path = self.entry_file_path(entry_id);
        if !path.exists() {
            return Err(SyncError::Other(format!(
                "Entry file not found: {}",
                entry_id
            )));
        }

        let content = fs::read_to_string(&path)?;
        self.decode_entry_file(master_key, &content)
    }

    pub fn decode_entry_file(
        &self,
        master_key: &MasterKey,
        content: &str,
    ) -> Result<ImportedEntry, SyncError> {
        let file: EntryFile =
            serde_json::from_str(content).map_err(|e| SyncError::Other(e.to_string()))?;

        let entry_key = VaultCrypto::derive_entry_key(master_key, &file.id);
        let plaintext = VaultCrypto::decrypt_field(&file.encrypted_blob, entry_key.expose())
            .map_err(|e| SyncError::HistoryDecryptFailed(e.to_string()))?;
        let blob: EntryBlob = serde_json::from_slice(plaintext.expose())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(ImportedEntry {
            id: file.id,
            category: file.category,
            revision: file.revision,
            blob,
        })
    }

    pub fn read_all_entries(
        &self,
        master_key: &MasterKey,
    ) -> Result<(Vec<ImportedEntry>, Vec<String>), SyncError> {
        let ids = self.list_entry_files()?;
        let mut entries = Vec::with_capacity(ids.len());
        let mut unreadable_ids = Vec::new();
        for id in ids {
            match self.import_entry(master_key, &id) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    log::warn!("Skipping unreadable vault entry file {}: {}", id, e);
                    unreadable_ids.push(id);
                }
            }
        }
        Ok((entries, unreadable_ids))
    }

    pub fn remove_entry_file(&self, entry_id: &str) -> Result<(), SyncError> {
        let path = self.entry_file_path(entry_id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn list_entry_files(&self) -> Result<Vec<String>, SyncError> {
        let entries_dir = self.entries_dir();
        if !entries_dir.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        for entry in fs::read_dir(&entries_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(id) = name_str.strip_suffix(".enc") {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn export_vault_meta(
        &self,
        master_key: &MasterKey,
        crypto: Option<VaultCryptoMeta>,
        emergency: Option<EmergencyDataFile>,
    ) -> Result<(), SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        let existing = fs::read_to_string(&path).ok();
        let existing_meta = existing
            .as_deref()
            .and_then(|content| serde_json::from_str::<VaultMetaFile>(content).ok());

        let exported_at = existing_meta
            .as_ref()
            .map(|meta| meta.exported_at)
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        let meta = sign_vault_meta(
            master_key,
            VaultMetaFile {
                version: VAULT_META_VERSION,
                exported_at,
                crypto,
                emergency,
                integrity: String::new(),
            },
        );

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        if existing.as_deref() == Some(json.as_str()) {
            return Ok(());
        }

        let meta = sign_vault_meta(
            master_key,
            VaultMetaFile {
                exported_at: chrono::Utc::now().timestamp(),
                ..meta
            },
        );
        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        fs::create_dir_all(&self.vault_dir)?;
        write_atomically(&path, json.as_bytes())
    }

    pub fn read_vault_meta(&self) -> Result<VaultMetaFile, SyncError> {
        let content = fs::read_to_string(self.vault_dir.join(VAULT_META_FILE))?;
        serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))
    }

    pub fn verify_vault_meta_integrity(
        &self,
        master_key: &MasterKey,
    ) -> Result<IntegrityResult, SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        if !path.exists() {
            return Ok(IntegrityResult {
                valid: false,
                message: ".vault-meta file not found".to_string(),
            });
        }

        let content = fs::read_to_string(&path)?;
        let meta: VaultMetaFile =
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?;

        if meta.integrity.is_empty() {
            return Ok(IntegrityResult {
                valid: false,
                message: ".vault-meta is unsigned (integrity field empty)".to_string(),
            });
        }

        let expected = compute_vault_meta_hmac(master_key, &meta);
        let actual = match B64.decode(&meta.integrity) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Ok(IntegrityResult {
                    valid: false,
                    message: format!(".vault-meta integrity field is not valid base64: {}", e),
                })
            }
        };

        if expected == actual {
            Ok(IntegrityResult {
                valid: true,
                message: "Integrity verified".to_string(),
            })
        } else {
            Ok(IntegrityResult {
                valid: false,
                message: "HMAC signature mismatch — vault meta may be tampered".to_string(),
            })
        }
    }

    pub fn read_emergency_data(&self) -> Result<Option<EmergencyDataFile>, SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        if !path.exists() {
            return Ok(None);
        }
        Ok(self.read_vault_meta()?.emergency)
    }

    pub fn write_emergency_data(
        &self,
        master_key: &MasterKey,
        emergency: &EmergencyDataFile,
    ) -> Result<(), SyncError> {
        let existing = self.read_vault_meta().ok();
        let crypto = existing.as_ref().and_then(|m| m.crypto.clone());
        let version = existing
            .as_ref()
            .map(|m| m.version)
            .unwrap_or(VAULT_META_VERSION);

        let mut meta = VaultMetaFile {
            version,
            exported_at: chrono::Utc::now().timestamp(),
            crypto,
            emergency: Some(emergency.clone()),
            integrity: String::new(),
        };
        let hmac = compute_vault_meta_hmac(master_key, &meta);
        meta.integrity = B64.encode(hmac);

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        write_atomically(&self.vault_dir.join(VAULT_META_FILE), json.as_bytes())
    }

    pub fn clear_emergency_data(&self, master_key: &MasterKey) -> Result<(), SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        if !path.exists() {
            return Ok(());
        }
        let mut meta = self.read_vault_meta()?;
        if meta.emergency.is_none() {
            return Ok(());
        }
        meta.emergency = None;
        meta.exported_at = chrono::Utc::now().timestamp();
        meta.integrity = String::new();
        let hmac = compute_vault_meta_hmac(master_key, &meta);
        meta.integrity = B64.encode(hmac);

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        write_atomically(&path, json.as_bytes())
    }

    pub fn rewrite_vault_meta_with_new_key(&self, master_key: &MasterKey) -> Result<(), SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        if !path.exists() {
            return Ok(());
        }
        let mut meta = self.read_vault_meta()?;
        meta.integrity = String::new();
        let hmac = compute_vault_meta_hmac(master_key, &meta);
        meta.integrity = B64.encode(hmac);

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        write_atomically(&path, json.as_bytes())
    }
}

impl ImportedEntry {
    pub fn into_import(self) -> VaultEntryImport {
        VaultEntryImport {
            id: self.id,
            category: VaultCategory::parse(&self.category).unwrap_or_default(),
            revision: self.revision,
            title: self.blob.title,
            username: self.blob.username,
            url: self.blob.url,
            password: self.blob.password,
            notes: self.blob.notes,
            totp_secret: self.blob.totp_secret,
            totp_digits: self.blob.totp_digits,
            totp_period: self.blob.totp_period,
            custom_fields: self
                .blob
                .custom_fields
                .into_iter()
                .map(|cf| assistant_security::types::CustomFieldInput {
                    name: cf.name,
                    value: cf.value,
                    is_protected: cf.is_secret,
                })
                .collect(),
            tags: self.blob.tags,
            is_favorite: self.blob.is_favorite,
            created_at: self.blob.created_at,
            updated_at: self.blob.updated_at,
        }
    }
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), SyncError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn read_entry_file(path: &Path) -> Option<EntryFile> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn sign_vault_meta(master_key: &MasterKey, mut meta: VaultMetaFile) -> VaultMetaFile {
    meta.integrity = String::new();
    meta.integrity = B64.encode(compute_vault_meta_hmac(master_key, &meta));
    meta
}

fn compute_hmac(key: &[u8; 32], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_vault_meta_hmac(master_key: &MasterKey, meta: &VaultMetaFile) -> Vec<u8> {
    let mut meta_for_hmac = meta.clone();
    meta_for_hmac.integrity = String::new();
    let content = serde_json::to_vec(&meta_for_hmac).unwrap_or_default();
    compute_hmac(master_key.expose(), &content)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityResult {
    pub valid: bool,
    pub message: String,
}
