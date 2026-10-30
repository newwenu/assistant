use std::fmt;

use serde::{Deserialize, Serialize};

use assistant_crypto::vault_crypto::types::CipherAlgorithm;

pub use assistant_crypto::error::VaultError;

pub use crate::shared::*;

#[derive(Debug, Clone)]
pub struct VaultEntrySnapshot {
    pub id: String,
    pub category: VaultCategory,
    pub revision: u64,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_digits: u32,
    pub totp_period: u32,
    pub custom_fields: Vec<CustomFieldInput>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryImport {
    pub id: String,
    pub category: VaultCategory,
    pub revision: u64,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_digits: u32,
    pub totp_period: u32,
    pub custom_fields: Vec<CustomFieldInput>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    Inserted,
    Updated,
    SkippedLocallyDeleted,
    SkippedStale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCryptoInfo {
    pub master_key_salt: [u8; 32],
    pub hash_salt: [u8; 32],
    pub argon2_m: u32,
    pub argon2_t: u32,
    pub argon2_p: u32,
    pub default_cipher: CipherAlgorithm,
}

pub struct VaultEntryDecrypted {
    pub id: String,
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<assistant_crypto::vault_crypto::types::VaultPlaintext>,
    pub notes: Option<assistant_crypto::vault_crypto::types::VaultPlaintext>,
    pub totp_secret: Option<assistant_crypto::vault_crypto::types::VaultPlaintext>,
    pub custom_fields: Vec<DecryptedCustomField>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl fmt::Debug for VaultEntryDecrypted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultEntryDecrypted")
            .field("id", &self.id)
            .field("category", &self.category)
            .field("title", &self.title)
            .field("username", &self.username)
            .field("url", &self.url)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("notes", &self.notes.as_ref().map(|_| "[REDACTED]"))
            .field(
                "totp_secret",
                &self.totp_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("custom_fields", &self.custom_fields.len())
            .field("tags", &self.tags)
            .field("is_favorite", &self.is_favorite)
            .field("revision", &self.revision)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Debug)]
pub struct DecryptedCustomField {
    pub name: String,
    pub value: assistant_crypto::vault_crypto::types::VaultPlaintext,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryExport {
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    pub tags: Vec<String>,
    pub custom_fields: Vec<CustomFieldInput>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntryCsvRow {
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    #[serde(
        serialize_with = "serialize_tags_csv",
        deserialize_with = "deserialize_tags_csv",
        default
    )]
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

fn serialize_tags_csv<S: serde::Serializer>(tags: &[String], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&tags.join(","))
}

fn deserialize_tags_csv<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let raw = String::deserialize(d)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

impl From<VaultEntryExport> for VaultEntryCsvRow {
    fn from(e: VaultEntryExport) -> Self {
        Self {
            category: e.category,
            title: e.title,
            username: e.username,
            url: e.url,
            password: e.password,
            notes: e.notes,
            totp_secret: e.totp_secret,
            tags: e.tags,
            is_favorite: e.is_favorite,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

impl From<VaultEntryCsvRow> for crate::dto::VaultEntryCreate {
    fn from(r: VaultEntryCsvRow) -> Self {
        Self {
            category: r.category,
            title: r.title,
            username: r.username,
            url: r.url,
            password: r.password,
            notes: r.notes,
            totp_secret: r.totp_secret,
            tags: r.tags,
            custom_fields: Vec::new(),
            is_favorite: r.is_favorite,
        }
    }
}
