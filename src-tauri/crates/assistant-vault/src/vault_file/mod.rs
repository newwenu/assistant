pub mod types;

use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::types::{VaultCategory, VaultEntryImport, VaultEntrySnapshot};
use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek, MetaKey, VaultPlaintext};
use assistant_crypto::vault_crypto::{derive_entry_key, entry_file_aad};

pub use crate::dto::integrity::IntegrityResult;

use assistant_sync::types::SyncError;
use types::{
    BlobCustomField, EntryBlobV2, EntryFileV2, ImportedEntry, ManifestEntry, TotpBlob,
    VaultManifest, VaultMetaFileV2, ENTRY_FILE_VERSION_V2,
};

const ENTRIES_DIR: &str = "entries";
const VAULT_META_FILE: &str = ".vault-meta";
pub const VAULT_META_FILE_NAME: &str = VAULT_META_FILE;
const GITIGNORE_FILE: &str = ".gitignore";
const GIT_DIR: &str = ".git";
const TEMP_SUFFIX: &str = ".tmp";

const MINIMAL_GITIGNORE: &str = "\
security.db
security.db-wal
security.db-shm
backups/
.audit/
vault-config.json
recovery/
*.tmp
";

type HmacSha256 = Hmac<Sha256>;

pub type CipherProvider = Box<dyn Fn() -> CipherAlgorithm + Send + Sync>;

pub struct VaultFileManager {
    vault_dir: PathBuf,
    cipher: CipherProvider,
}

impl VaultFileManager {
    pub fn new(vault_dir: PathBuf, cipher: CipherProvider) -> Self {
        Self { vault_dir, cipher }
    }

    pub(crate) fn cipher_algorithm(&self) -> CipherAlgorithm {
        (self.cipher)()
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

    pub fn ensure_gitignore(&self) -> Result<(), SyncError> {
        fs::create_dir_all(&self.vault_dir)?;
        let path = self.vault_dir.join(GITIGNORE_FILE);
        if !path.exists() {
            fs::write(&path, MINIMAL_GITIGNORE)?;
        }
        Ok(())
    }

    pub fn clean_temp_files(&self) -> Result<u32, SyncError> {
        let mut removed = 0u32;
        for dir in [self.vault_dir.clone(), self.entries_dir()] {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(TEMP_SUFFIX) {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    pub fn check_repository_layout(&self) -> Result<(), SyncError> {
        if !self.vault_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.vault_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if name == VAULT_META_FILE || name == GITIGNORE_FILE || name == GIT_DIR {
                continue;
            }

            if name == ENTRIES_DIR {
                if !entry.file_type()?.is_dir() {
                    return Err(SyncError::RepositoryContainsForbiddenFile(name));
                }
                for inner in fs::read_dir(entry.path())? {
                    let inner = inner?;
                    let inner_name = inner.file_name().to_string_lossy().to_string();
                    if !inner.file_type()?.is_file() || !inner_name.ends_with(".enc") {
                        return Err(SyncError::RepositoryContainsForbiddenFile(format!(
                            "{}/{}",
                            ENTRIES_DIR, inner_name
                        )));
                    }
                }
                continue;
            }

            return Err(SyncError::RepositoryContainsForbiddenFile(name));
        }

        Ok(())
    }

    pub fn entry_file_path(&self, entry_id: &str) -> PathBuf {
        self.entries_dir().join(format!("{}.enc", entry_id))
    }

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
            schema: types::ENTRY_BLOB_SCHEMA_V1,
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

impl VaultFileManager {
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

    pub fn meta_path(&self) -> PathBuf {
        self.vault_dir.join(VAULT_META_FILE)
    }

    pub fn read_vault_meta_v2(&self) -> Result<VaultMetaFileV2, VaultError> {
        let path = self.meta_path();
        if !path.exists() {
            return Err(VaultError::NotInitialized);
        }

        let content = fs::read_to_string(&path)?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|_| VaultError::VaultCorrupted("unparsable .vault-meta".to_string()))?;

        reject_v1_meta(&value)?;

        let meta: VaultMetaFileV2 = serde_json::from_value(value)
            .map_err(|e| VaultError::VaultCorrupted(format!("invalid .vault-meta: {}", e)))?;

        validate_meta_v2(&meta)?;
        Ok(meta)
    }

    pub fn write_vault_meta_v2(
        &self,
        meta: &mut VaultMetaFileV2,
        meta_key: &MetaKey,
    ) -> Result<(), VaultError> {
        meta.integrity = String::new();
        meta.integrity = B64.encode(compute_meta_integrity(meta_key, meta));

        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| VaultError::VaultCorrupted(format!("serialize .vault-meta: {}", e)))?;
        fs::create_dir_all(&self.vault_dir)?;
        write_atomically(&self.meta_path(), json.as_bytes())
            .map_err(|e| VaultError::Io(std::io::Error::other(e.to_string())))
    }

    pub fn compute_manifest(&self, dek_version: u32) -> Result<VaultManifest, VaultError> {
        let ids = self
            .list_entry_files()
            .map_err(|e| VaultError::Io(std::io::Error::other(e.to_string())))?;
        let mut entries = Vec::with_capacity(ids.len());
        for id in ids {
            let path = self.entry_file_path(&id);
            let bytes = fs::read(&path)?;
            entries.push(ManifestEntry {
                id,
                sha256: B64.encode(Sha256::digest(&bytes)),
                dek_version,
            });
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        let count = u32::try_from(entries.len())
            .map_err(|_| VaultError::VaultCorrupted("manifest entry count overflow".to_string()))?;
        Ok(VaultManifest { count, entries })
    }

    pub fn meta_file_hash(&self) -> Result<String, VaultError> {
        let bytes = fs::read(self.meta_path())?;
        Ok(hex::encode(Sha256::digest(&bytes)))
    }

    pub fn verify_manifest(&self) -> Result<(), VaultError> {
        let meta = self.read_vault_meta_v2()?;
        let expected = self.compute_manifest(meta.dek_version)?;
        if expected.count != meta.manifest.count || expected.entries != meta.manifest.entries {
            return Err(VaultError::VaultManifestMismatch);
        }
        Ok(())
    }

    pub fn verify_meta_integrity(&self, meta_key: &MetaKey) -> Result<IntegrityResult, VaultError> {
        let path = self.meta_path();
        if !path.exists() {
            return Ok(IntegrityResult {
                valid: false,
                message: ".vault-meta file not found".to_string(),
            });
        }

        let meta = match self.read_vault_meta_v2() {
            Ok(meta) => meta,
            Err(VaultError::VaultCorrupted(message)) => {
                return Ok(IntegrityResult {
                    valid: false,
                    message,
                })
            }
            Err(e) => return Err(e),
        };

        if meta.integrity.is_empty() {
            return Ok(IntegrityResult {
                valid: false,
                message: ".vault-meta is unsigned (integrity field empty)".to_string(),
            });
        }

        let expected = compute_meta_integrity(meta_key, &meta);
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

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), SyncError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn canonical_meta_bytes(meta: &VaultMetaFileV2) -> Result<Vec<u8>, VaultError> {
    serde_json::to_vec(&meta.integrity_view())
        .map_err(|e| VaultError::VaultCorrupted(format!("canonical meta serialization: {}", e)))
}

fn compute_hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_meta_integrity(meta_key: &MetaKey, meta: &VaultMetaFileV2) -> Vec<u8> {
    let canonical = canonical_meta_bytes(meta).expect("canonical meta serialization is infallible");
    compute_hmac(meta_key.expose(), &canonical)
}

fn reject_v1_meta(value: &serde_json::Value) -> Result<(), VaultError> {
    if let Some(version) = value.get("version") {
        return match version.as_u64() {
            Some(1) => Err(VaultError::UnsupportedVaultVersion),
            Some(2) => Ok(()),
            Some(newer) if newer > 2 => Err(VaultError::VaultVersionTooNew),
            _ => Err(VaultError::VaultCorrupted(format!(
                "unexpected .vault-meta version: {}",
                version
            ))),
        };
    }

    let has_master_key_salt = value
        .get("crypto")
        .and_then(|crypto| crypto.get("masterKeySalt"))
        .is_some();
    let has_emergency = value.get("emergency").is_some();

    if has_master_key_salt || has_emergency {
        return Err(VaultError::UnsupportedVaultVersion);
    }

    Err(VaultError::VaultCorrupted(
        "unparsable .vault-meta".to_string(),
    ))
}

fn validate_meta_v2(meta: &VaultMetaFileV2) -> Result<(), VaultError> {
    let corrupt = VaultError::VaultCorrupted;

    if meta.version != 2 {
        return Err(corrupt(format!(
            "unsupported .vault-meta version {}",
            meta.version
        )));
    }
    if !is_lower_hex_32(&meta.vault_id) {
        return Err(corrupt(
            "vaultId must be 32 lowercase hex chars".to_string(),
        ));
    }
    if meta.dek_version == 0 {
        return Err(corrupt("dekVersion must be >= 1".to_string()));
    }
    if meta.cipher != "aes256Gcm" && meta.cipher != "chacha20Poly1305" {
        return Err(corrupt(format!("unsupported cipher: {}", meta.cipher)));
    }
    if meta.slots.len() != 1 {
        return Err(corrupt(format!(
            "slots must contain exactly 1 slot, got {}",
            meta.slots.len()
        )));
    }

    let slot = &meta.slots[0];
    if slot.slot_type != "password" {
        return Err(corrupt(format!(
            "unsupported slot type: {}",
            slot.slot_type
        )));
    }
    if slot.payload_kind != "dek" {
        return Err(corrupt(format!(
            "unsupported slot payloadKind: {}",
            slot.payload_kind
        )));
    }
    if slot.kdf.algo != "argon2id" {
        return Err(corrupt(format!("unsupported kdf algo: {}", slot.kdf.algo)));
    }
    if !is_b64_len(&slot.kdf.salt, 16) {
        return Err(corrupt("slot kdf salt must be b64(16)".to_string()));
    }
    if !is_b64_len(&slot.wrapped_dek, 48) {
        return Err(corrupt("slot wrappedDek must be b64(48)".to_string()));
    }
    if !is_slot_id(&slot.id) {
        return Err(corrupt(format!("invalid slot id: {}", slot.id)));
    }
    if meta.manifest.count as usize != meta.manifest.entries.len() {
        return Err(corrupt("manifest count mismatch".to_string()));
    }
    if meta
        .manifest
        .entries
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(corrupt("manifest entries must be sorted by id".to_string()));
    }
    if let Some(recovery) = &meta.recovery_backup {
        if recovery.dek_version > meta.dek_version {
            return Err(corrupt(
                "recoveryBackup dekVersion exceeds current dekVersion".to_string(),
            ));
        }
    }

    Ok(())
}

fn is_lower_hex_32(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_b64_len(value: &str, expected: usize) -> bool {
    B64.decode(value)
        .map(|bytes| bytes.len() == expected)
        .unwrap_or(false)
}

fn is_slot_id(value: &str) -> bool {
    let Some(n) = value.strip_prefix("pwd-") else {
        return false;
    };
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match n.parse::<u32>() {
        Ok(parsed) if parsed >= 1 => format!("pwd-{}", parsed) == value,
        _ => false,
    }
}

fn read_entry_file(path: &Path) -> Option<EntryFileV2> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn other_cipher(cipher: CipherAlgorithm) -> Option<CipherAlgorithm> {
    match cipher {
        CipherAlgorithm::Aes256Gcm => Some(CipherAlgorithm::ChaCha20Poly1305),
        CipherAlgorithm::ChaCha20Poly1305 => Some(CipherAlgorithm::Aes256Gcm),
    }
}
