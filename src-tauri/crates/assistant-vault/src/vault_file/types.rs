use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryFile {
    pub id: String,
    pub category: String,
    pub revision: i64,
    pub encrypted_blob: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryBlob {
    pub title: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub custom_fields: Vec<BlobCustomField>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default = "default_totp_digits")]
    pub totp_digits: u32,
    #[serde(default = "default_totp_period")]
    pub totp_period: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl EntryBlob {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            username: None,
            url: None,
            password: None,
            notes: None,
            custom_fields: Vec::new(),
            totp_secret: None,
            totp_digits: default_totp_digits(),
            totp_period: default_totp_period(),
            tags: Vec::new(),
            is_favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }
}

fn default_totp_digits() -> u32 {
    6
}

fn default_totp_period() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobCustomField {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub is_secret: bool,
}

#[derive(Debug, Clone)]
pub struct ImportedEntry {
    pub id: String,
    pub category: String,
    pub revision: i64,
    pub blob: EntryBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMetaFile {
    pub version: u32,
    /// unix **秒**。对齐 spec-4 §11.6 与 VAULT-DESIGN 示例值，属跨设备文件契约，
    /// 与本项目本地时间体系的毫秒约定不同，勿改。
    #[serde(default)]
    pub exported_at: i64,
    #[serde(default)]
    pub crypto: Option<VaultCryptoMeta>,
    #[serde(default)]
    pub emergency: Option<EmergencyDataFile>,
    #[serde(default)]
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCryptoMeta {
    pub master_key_salt: String,
    pub hash_salt: String,
    pub argon2_params: Argon2Params,
    pub default_cipher: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Argon2Params {
    pub m: u32,
    pub t: u32,
    pub p: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyDataFile {
    pub token: String,
    pub salt: String,
}
