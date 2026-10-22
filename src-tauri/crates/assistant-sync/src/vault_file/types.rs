use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedEntryBlob {
    pub id: String,
    pub category: String,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub encrypted_password: Option<String>,
    pub encrypted_notes: Option<String>,
    pub encrypted_totp_secret: Option<String>,
    pub encrypted_custom_fields: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMetaFile {
    pub version: u32,
    pub cipher_algorithm: String,
    pub entry_count: u32,
    pub last_sync_at: Option<i64>,
    pub emergency: Option<EmergencyDataFile>,
    pub hmac: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyDataFile {
    pub token: String,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedIndexFile {
    pub entries: Vec<DeletedEntryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedEntryRecord {
    pub id: String,
    pub deleted_at: i64,
}

impl Default for DeletedIndexFile {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
