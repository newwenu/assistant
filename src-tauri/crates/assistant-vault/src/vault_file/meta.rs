use std::fs;
use std::path::PathBuf;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::MetaKey;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use super::types::{ManifestEntry, VaultManifest, VaultMetaFileV2};
use super::{write_atomically, IntegrityResult, VaultFileManager, VAULT_META_FILE};

type HmacSha256 = Hmac<Sha256>;

impl VaultFileManager {
    pub fn meta_path(&self) -> PathBuf {
        self.vault_dir().join(VAULT_META_FILE)
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
        fs::create_dir_all(self.vault_dir())?;
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
