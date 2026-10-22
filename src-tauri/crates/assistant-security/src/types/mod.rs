use std::fmt;

use serde::{Deserialize, Serialize};

impl From<String> for VaultError {
    fn from(s: String) -> Self {
        VaultError::Database(s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,
    #[error("Vault is already unlocked")]
    AlreadyUnlocked,
    #[error("Invalid master password")]
    InvalidPassword,
    #[error("Vault not initialized")]
    NotInitialized,
    #[error("Vault already exists")]
    AlreadyExists,
    #[error("Rate limited")]
    RateLimited,
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Entry not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Biometric unavailable")]
    BiometricUnavailable,
    #[error("Biometric cancelled")]
    BiometricCancelled,
    #[error("Biometric failed: {0}")]
    BiometricFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VaultCategory {
    Login,
    Note,
    Card,
    Identity,
    SshKey,
    Custom,
}

impl Default for VaultCategory {
    fn default() -> Self {
        Self::Login
    }
}

impl fmt::Display for VaultCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login => write!(f, "login"),
            Self::Note => write!(f, "note"),
            Self::Card => write!(f, "card"),
            Self::Identity => write!(f, "identity"),
            Self::SshKey => write!(f, "sshKey"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl VaultCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "login" => Some(Self::Login),
            "note" => Some(Self::Note),
            "card" => Some(Self::Card),
            "identity" => Some(Self::Identity),
            "sshKey" => Some(Self::SshKey),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldInput {
    pub name: String,
    pub value: String,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryCreate {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryUpdate {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntrySummary {
    pub id: String,
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryDetail {
    pub id: String,
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub has_password: bool,
    pub has_notes: bool,
    pub has_totp: bool,
    pub custom_field_names: Vec<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct VaultEntryDecrypted {
    pub id: String,
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<crate::vault_crypto::types::VaultPlaintext>,
    pub notes: Option<crate::vault_crypto::types::VaultPlaintext>,
    pub totp_secret: Option<crate::vault_crypto::types::VaultPlaintext>,
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
    pub value: crate::vault_crypto::types::VaultPlaintext,
    pub is_protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealResult {
    pub value: String,
    pub expires_at: i64,
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
