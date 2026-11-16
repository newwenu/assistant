use std::fs;
use std::path::PathBuf;

use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek, VaultPlaintext};
use assistant_crypto::vault_crypto::{derive_entry_key, entry_file_aad};
use assistant_sync::types::SyncError;

use crate::types::{VaultCategory, VaultEntryImport, VaultEntrySnapshot};

use super::types::{
    BlobCustomField, EntryBlobV2, EntryFileV2, ImportedEntry, TotpBlob, ENTRY_FILE_VERSION_V2,
};
use super::{write_atomically, VaultFileManager};

impl VaultFileManager {
    pub fn export_entry(&self, dek: &Dek, snapshot: &VaultEntrySnapshot) -> Result<(), SyncError> {
        self.ensure_structure()?;

        let path = self.entry_file_path(&snapshot.id);
        if let Some(existing) = read_entry_file(&path) {
            if existing.revision == snapshot.revision
                && existing.dek_version == snapshot.dek_version
            {
                return Ok(());
            }
        }

        let blob = EntryBlobV2 {
            schema: super::types::ENTRY_BLOB_SCHEMA_V1,
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
            totp: snapshot
                .totp_secret
                .as_deref()
                .and_then(TotpBlob::from_stored_payload),
            tags: snapshot.tags.clone(),
            is_favorite: snapshot.is_favorite,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        };

        let blob_json = serde_json::to_vec(&blob).map_err(|e| SyncError::Other(e.to_string()))?;
        let plaintext = VaultPlaintext::from(blob_json);
        let entry_key = derive_entry_key(dek, &snapshot.id);
        let cipher = self.cipher_algorithm();
        let aad = entry_file_aad(
            &snapshot.id,
            &snapshot.category.to_string(),
            snapshot.revision,
            snapshot.dek_version,
        );
        let encrypted = assistant_crypto::vault_crypto::encrypt_entry_blob(
            &entry_key, &plaintext, cipher, &aad,
        )
        .map_err(|e| SyncError::Other(e.to_string()))?;

        let file = EntryFileV2 {
            version: ENTRY_FILE_VERSION_V2,
            id: snapshot.id.clone(),
            category: snapshot.category.to_string(),
            revision: snapshot.revision,
            dek_version: snapshot.dek_version,
            cipher: crate::lifecycle::cipher_wire_name(cipher).to_string(),
            encrypted_blob: encrypted,
        };

        let json =
            serde_json::to_string_pretty(&file).map_err(|e| SyncError::Other(e.to_string()))?;
        write_atomically(&path, json.as_bytes())
    }

    pub fn import_entry(&self, dek: &Dek, entry_id: &str) -> Result<ImportedEntry, SyncError> {
        let path = self.entry_file_path(entry_id);
        if !path.exists() {
            return Err(SyncError::Other(format!(
                "Entry file not found: {}",
                entry_id
            )));
        }

        let content = fs::read_to_string(&path)?;
        let entry = self.decode_entry_file(dek, &content)?;
        if entry.id != entry_id {
            return Err(SyncError::EntryCorrupted(format!(
                "entry file id mismatch: file name {}, content {}",
                entry_id, entry.id
            )));
        }
        Ok(entry)
    }

    pub fn decode_entry_file(&self, dek: &Dek, content: &str) -> Result<ImportedEntry, SyncError> {
        decode_entry_content(dek, content)
    }

    /// `rotate_dek` 全量重加密：先用 `dek_old` 解开每条 `.enc`，再用 `dek_new` 重写。
    ///
    /// 约束：全部条目先写入 `<id>.tmp` 暂存，**全部成功后才批量 rename**；
    /// 任一失败则清理暂存文件并返回错误，原文件一个字节都不动。
    pub fn reencrypt_entries(
        &self,
        dek_old: &Dek,
        dek_new: &Dek,
        dek_version_new: u32,
    ) -> Result<usize, SyncError> {
        self.ensure_structure()?;
        let ids = self.list_entry_files()?;
        let entries_dir = self.entries_dir();
        let cipher = self.cipher_algorithm();

        let staged = (|| -> Result<Vec<(PathBuf, PathBuf)>, SyncError> {
            let mut staged = Vec::with_capacity(ids.len());
            for id in &ids {
                let final_path = self.entry_file_path(id);
                let content = fs::read_to_string(&final_path)?;
                let imported = decode_entry_content(dek_old, &content)?;

                let blob_json = serde_json::to_vec(&imported.blob)
                    .map_err(|e| SyncError::Other(e.to_string()))?;
                let entry_key = derive_entry_key(dek_new, &imported.id);
                let aad = entry_file_aad(
                    &imported.id,
                    &imported.category,
                    imported.revision,
                    dek_version_new,
                );
                let encrypted = assistant_crypto::vault_crypto::encrypt_entry_blob(
                    &entry_key,
                    &VaultPlaintext::from(blob_json),
                    cipher,
                    &aad,
                )
                .map_err(|e| SyncError::Other(e.to_string()))?;

                let file = EntryFileV2 {
                    version: ENTRY_FILE_VERSION_V2,
                    id: imported.id.clone(),
                    category: imported.category.clone(),
                    revision: imported.revision,
                    dek_version: dek_version_new,
                    cipher: crate::lifecycle::cipher_wire_name(cipher).to_string(),
                    encrypted_blob: encrypted,
                };
                let json = serde_json::to_string_pretty(&file)
                    .map_err(|e| SyncError::Other(e.to_string()))?;

                let staging_path = entries_dir.join(format!("{}.tmp", imported.id));
                fs::write(&staging_path, json.as_bytes())?;
                staged.push((staging_path, final_path));
            }
            Ok(staged)
        })();

        let staged = match staged {
            Ok(staged) => staged,
            Err(err) => {
                for id in &ids {
                    let staging_path = entries_dir.join(format!("{}.tmp", id));
                    if staging_path.exists() {
                        let _ = fs::remove_file(&staging_path);
                    }
                }
                return Err(err);
            }
        };

        let mut first_err: Option<SyncError> = None;
        for (staging_path, final_path) in &staged {
            if let Err(e) = fs::rename(staging_path, final_path) {
                log::error!(
                    "Failed to replace re-encrypted entry {}: {}",
                    final_path.display(),
                    e
                );
                if first_err.is_none() {
                    first_err = Some(SyncError::Io(e.to_string()));
                }
            }
        }
        match first_err {
            Some(err) => Err(err),
            None => Ok(staged.len()),
        }
    }

    pub fn read_all_entries(
        &self,
        dek: &Dek,
    ) -> Result<(Vec<ImportedEntry>, Vec<String>), SyncError> {
        let ids = self.list_entry_files()?;
        let mut entries = Vec::with_capacity(ids.len());
        let mut unreadable_ids = Vec::new();
        for id in ids {
            match self.import_entry(dek, &id) {
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
        if path.exists() {
            return Err(SyncError::Other(format!(
                "Entry file still present after removal: {}",
                entry_id
            )));
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
}

/// 只读取条目文件声明的 `dekVersion`，不触碰密文。
/// 供历史预览判定「旧代条目已不可读」使用（sync-crypto-spec §2.6）。
pub fn peek_entry_dek_version(content: &str) -> Result<u32, SyncError> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| SyncError::EntryCorrupted(format!("unparsable entry file: {}", e)))?;
    value
        .get("dekVersion")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            SyncError::EntryCorrupted("entry file has no valid dekVersion field".to_string())
        })
}

pub fn decode_entry_content(dek: &Dek, content: &str) -> Result<ImportedEntry, SyncError> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| SyncError::EntryCorrupted(format!("unparsable entry file: {}", e)))?;

    match value.get("version").and_then(|v| v.as_u64()) {
        Some(version) if version == ENTRY_FILE_VERSION_V2 as u64 => {}
        Some(version) => {
            return Err(SyncError::UnsupportedEntryFileVersion(format!(
                "entry file version {}",
                version
            )))
        }
        None => {
            return Err(SyncError::UnsupportedEntryFileVersion(
                "entry file has no version field (v1 format)".to_string(),
            ))
        }
    }

    let file: EntryFileV2 = serde_json::from_value(value)
        .map_err(|e| SyncError::EntryCorrupted(format!("invalid entry file: {}", e)))?;

    if file.revision < 0 {
        return Err(SyncError::EntryCorrupted(format!(
            "negative revision {} in entry file {}",
            file.revision, file.id
        )));
    }
    if file.dek_version == 0 {
        return Err(SyncError::EntryCorrupted(format!(
            "invalid dekVersion 0 in entry file {}",
            file.id
        )));
    }
    let category = VaultCategory::parse(&file.category).ok_or_else(|| {
        SyncError::EntryCorrupted(format!(
            "unknown category {} in entry file {}",
            file.category, file.id
        ))
    })?;

    let declared = crate::lifecycle::cipher_from_wire_name(&file.cipher)
        .map_err(|e| SyncError::EntryCorrupted(e.to_string()))?;

    let entry_key = derive_entry_key(dek, &file.id);
    let aad = entry_file_aad(
        &file.id,
        &category.to_string(),
        file.revision,
        file.dek_version,
    );

    let plaintext = match assistant_crypto::vault_crypto::decrypt_entry_blob(
        &entry_key,
        &file.encrypted_blob,
        declared,
        &aad,
    ) {
        Ok(plaintext) => plaintext,
        Err(e) => {
            if let Some(actual) = other_cipher(declared) {
                if assistant_crypto::vault_crypto::decrypt_entry_blob(
                    &entry_key,
                    &file.encrypted_blob,
                    actual,
                    &aad,
                )
                .is_ok()
                {
                    return Err(SyncError::EntryCorrupted(format!(
                        "entry file {} declares cipher {} but blob uses {}",
                        file.id,
                        file.cipher,
                        crate::lifecycle::cipher_wire_name(actual)
                    )));
                }
            }
            return Err(SyncError::EntryCorrupted(format!(
                "entry file {} decrypt failed: {}",
                file.id, e
            )));
        }
    };

    let blob: EntryBlobV2 = serde_json::from_slice(plaintext.expose())
        .map_err(|e| SyncError::EntryCorrupted(format!("invalid entry blob: {}", e)))?;

    Ok(ImportedEntry {
        id: file.id,
        category: file.category,
        revision: file.revision,
        dek_version: file.dek_version,
        blob,
    })
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
            totp_secret: self.blob.totp.map(|totp| totp.to_stored_payload()),
            custom_fields: self
                .blob
                .custom_fields
                .into_iter()
                .map(|cf| crate::types::CustomFieldInput {
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

fn read_entry_file(path: &std::path::Path) -> Option<EntryFileV2> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn other_cipher(cipher: CipherAlgorithm) -> Option<CipherAlgorithm> {
    match cipher {
        CipherAlgorithm::Aes256Gcm => Some(CipherAlgorithm::ChaCha20Poly1305),
        CipherAlgorithm::ChaCha20Poly1305 => Some(CipherAlgorithm::Aes256Gcm),
    }
}
