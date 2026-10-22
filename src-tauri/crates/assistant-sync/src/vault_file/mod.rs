pub mod types;

use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use assistant_security::vault_crypto::types::MasterKey;
use assistant_security::vault_crypto::VaultCrypto;

use crate::types::{DeletedEntry, DeletedIndex, SyncError, VaultMeta};
use types::{
    DeletedEntryRecord, DeletedIndexFile, EmergencyDataFile, EncryptedEntryBlob, VaultMetaFile,
};

type HmacSha256 = Hmac<Sha256>;

const ENTRIES_DIR: &str = "entries";
const DELETIONS_DIR: &str = "deletions";
const DELETED_JSON: &str = "deleted.json";
const VAULT_META_FILE: &str = ".vault-meta";

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

    pub fn ensure_structure(&self) -> Result<(), SyncError> {
        fs::create_dir_all(self.vault_dir.join(ENTRIES_DIR))?;
        fs::create_dir_all(self.vault_dir.join(DELETIONS_DIR))?;
        Ok(())
    }

    pub fn export_entry(
        &self,
        master_key: &MasterKey,
        entry_id: &str,
        category: &str,
        title: &str,
        username: Option<&str>,
        url: Option<&str>,
        encrypted_password: Option<&str>,
        encrypted_notes: Option<&str>,
        encrypted_totp_secret: Option<&str>,
        encrypted_custom_fields: Option<&str>,
        tags: &[String],
        is_favorite: bool,
        revision: u64,
        created_at: i64,
        updated_at: i64,
    ) -> Result<(), SyncError> {
        let entry_key = VaultCrypto::derive_entry_key(master_key, entry_id);

        let blob = EncryptedEntryBlob {
            id: entry_id.to_string(),
            category: category.to_string(),
            title: title.to_string(),
            username: username.map(|s| s.to_string()),
            url: url.map(|s| s.to_string()),
            encrypted_password: encrypted_password.map(|s| s.to_string()),
            encrypted_notes: encrypted_notes.map(|s| s.to_string()),
            encrypted_totp_secret: encrypted_totp_secret.map(|s| s.to_string()),
            encrypted_custom_fields: encrypted_custom_fields.map(|s| s.to_string()),
            tags: tags.to_vec(),
            is_favorite,
            revision,
            created_at,
            updated_at,
        };

        let json = serde_json::to_vec(&blob).map_err(|e| SyncError::Other(e.to_string()))?;

        let plaintext = assistant_security::vault_crypto::types::VaultPlaintext::from(json);
        let encrypted =
            VaultCrypto::encrypt_field(&plaintext, entry_key.expose(), Default::default())
                .map_err(|e| SyncError::Other(e.to_string()))?;

        let file_path = self
            .vault_dir
            .join(ENTRIES_DIR)
            .join(format!("{}.enc", entry_id));
        fs::write(&file_path, encrypted.as_bytes())?;

        Ok(())
    }

    pub fn import_entry(
        &self,
        master_key: &MasterKey,
        entry_id: &str,
    ) -> Result<EncryptedEntryBlob, SyncError> {
        let file_path = self
            .vault_dir
            .join(ENTRIES_DIR)
            .join(format!("{}.enc", entry_id));

        if !file_path.exists() {
            return Err(SyncError::Other(format!(
                "Entry file not found: {}",
                entry_id
            )));
        }

        let encrypted = fs::read_to_string(&file_path)?;
        let entry_key = VaultCrypto::derive_entry_key(master_key, entry_id);

        let plaintext = VaultCrypto::decrypt_field(&encrypted, entry_key.expose())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        let blob: EncryptedEntryBlob = serde_json::from_slice(plaintext.expose())
            .map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(blob)
    }

    pub fn remove_entry_file(&self, entry_id: &str) -> Result<(), SyncError> {
        let file_path = self
            .vault_dir
            .join(ENTRIES_DIR)
            .join(format!("{}.enc", entry_id));
        if file_path.exists() {
            fs::remove_file(&file_path)?;
        }
        Ok(())
    }

    pub fn list_entry_files(&self) -> Result<Vec<String>, SyncError> {
        let entries_dir = self.vault_dir.join(ENTRIES_DIR);
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
        Ok(ids)
    }

    pub fn write_vault_meta(
        &self,
        master_key: &MasterKey,
        version: u32,
        cipher_algorithm: &str,
        entry_count: u32,
        last_sync_at: Option<i64>,
        emergency: Option<&EmergencyDataFile>,
    ) -> Result<(), SyncError> {
        let meta = VaultMetaFile {
            version,
            cipher_algorithm: cipher_algorithm.to_string(),
            entry_count,
            last_sync_at,
            emergency: emergency.cloned(),
            hmac: String::new(),
        };

        let hmac = compute_vault_meta_hmac(master_key, &meta);

        let mut meta = meta;
        meta.hmac = B64.encode(hmac);

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        fs::write(self.vault_dir.join(VAULT_META_FILE), json)?;

        Ok(())
    }

    pub fn read_vault_meta(&self, master_key: &MasterKey) -> Result<VaultMeta, SyncError> {
        let content = fs::read_to_string(self.vault_dir.join(VAULT_META_FILE))?;
        let meta: VaultMetaFile =
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?;

        let expected_hmac = compute_vault_meta_hmac(master_key, &meta);
        let actual_hmac = B64
            .decode(&meta.hmac)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        if expected_hmac != actual_hmac {
            return Err(SyncError::HmacFailed);
        }

        Ok(VaultMeta {
            version: meta.version,
            cipher_algorithm: meta.cipher_algorithm,
            entry_count: meta.entry_count,
            last_sync_at: meta.last_sync_at,
            hmac: meta.hmac,
        })
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

        let expected_hmac = compute_vault_meta_hmac(master_key, &meta);
        let actual_hmac = B64
            .decode(&meta.hmac)
            .map_err(|e| SyncError::Other(e.to_string()))?;

        if expected_hmac == actual_hmac {
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
        let content = fs::read_to_string(&path)?;
        let meta: VaultMetaFile =
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?;
        Ok(meta.emergency)
    }

    pub fn write_emergency_data(
        &self,
        master_key: &MasterKey,
        emergency: &EmergencyDataFile,
    ) -> Result<(), SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        let mut meta = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?
        } else {
            VaultMetaFile {
                version: 1,
                cipher_algorithm: "aes256Gcm".to_string(),
                entry_count: 0,
                last_sync_at: None,
                emergency: None,
                hmac: String::new(),
            }
        };

        meta.emergency = Some(emergency.clone());
        let hmac = compute_vault_meta_hmac(master_key, &meta);
        meta.hmac = B64.encode(hmac);

        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn clear_emergency_data(&self) -> Result<(), SyncError> {
        let path = self.vault_dir.join(VAULT_META_FILE);
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&path)?;
        let mut meta: VaultMetaFile =
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?;
        if meta.emergency.is_none() {
            return Ok(());
        }
        meta.emergency = None;
        meta.hmac = String::new();
        let json =
            serde_json::to_string_pretty(&meta).map_err(|e| SyncError::Other(e.to_string()))?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn write_deleted_index(&self, deleted: &DeletedIndex) -> Result<(), SyncError> {
        let file = DeletedIndexFile {
            entries: deleted
                .entries
                .iter()
                .map(|d| DeletedEntryRecord {
                    id: d.id.clone(),
                    deleted_at: d.deleted_at,
                })
                .collect(),
        };

        let json =
            serde_json::to_string_pretty(&file).map_err(|e| SyncError::Other(e.to_string()))?;
        fs::write(self.vault_dir.join(DELETIONS_DIR).join(DELETED_JSON), json)?;
        Ok(())
    }

    pub fn read_deleted_index(&self) -> Result<DeletedIndex, SyncError> {
        let path = self.vault_dir.join(DELETIONS_DIR).join(DELETED_JSON);
        if !path.exists() {
            return Ok(DeletedIndex::default());
        }

        let content = fs::read_to_string(&path)?;
        let file: DeletedIndexFile =
            serde_json::from_str(&content).map_err(|e| SyncError::Other(e.to_string()))?;

        Ok(DeletedIndex {
            entries: file
                .entries
                .into_iter()
                .map(|d| DeletedEntry {
                    id: d.id,
                    deleted_at: d.deleted_at,
                })
                .collect(),
        })
    }

    pub fn add_deleted_entry(&self, id: &str, deleted_at: i64) -> Result<(), SyncError> {
        let mut index = self.read_deleted_index()?;
        index.entries.push(DeletedEntry {
            id: id.to_string(),
            deleted_at,
        });
        self.write_deleted_index(&index)
    }

    pub fn remove_deleted_entry(&self, id: &str) -> Result<(), SyncError> {
        let mut index = self.read_deleted_index()?;
        index.entries.retain(|e| e.id != id);
        self.write_deleted_index(&index)
    }
}

fn compute_hmac(key: &[u8; 32], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_vault_meta_hmac(master_key: &MasterKey, meta: &VaultMetaFile) -> Vec<u8> {
    let mut meta_for_hmac = meta.clone();
    meta_for_hmac.hmac = String::new();
    let content = serde_json::to_vec(&meta_for_hmac).unwrap_or_default();
    compute_hmac(master_key.expose(), &content)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityResult {
    pub valid: bool,
    pub message: String,
}
