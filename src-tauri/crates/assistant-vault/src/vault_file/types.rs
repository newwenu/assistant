use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryFileV2 {
    pub version: u32,
    pub id: String,
    pub category: String,
    pub revision: i64,
    pub dek_version: u32,
    pub cipher: String,
    pub encrypted_blob: String,
}

pub const ENTRY_FILE_VERSION_V2: u32 = 2;
pub const ENTRY_BLOB_SCHEMA_V1: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryBlobV2 {
    pub schema: u32,
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
    pub totp: Option<TotpBlob>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl EntryBlobV2 {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            schema: ENTRY_BLOB_SCHEMA_V1,
            title: title.into(),
            username: None,
            url: None,
            password: None,
            notes: None,
            custom_fields: Vec::new(),
            totp: None,
            tags: Vec::new(),
            is_favorite: false,
            created_at: 0,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpBlob {
    pub secret: String,
    #[serde(default = "default_totp_digits")]
    pub digits: u32,
    #[serde(default = "default_totp_period")]
    pub period: u32,
    #[serde(default = "default_totp_algorithm")]
    pub algorithm: String,
}

impl TotpBlob {
    /// 从本机库存的 TOTP payload（JSON 字符串）还原；非法 payload 返回 `None`。
    pub fn from_stored_payload(stored: &str) -> Option<Self> {
        let config = crate::shared::totp::TotpConfig::from_stored(stored)?;
        let secret = config.secret().to_str().ok()?.to_string();
        Some(Self {
            secret,
            digits: config.digits(),
            period: config.period(),
            algorithm: config.algorithm().as_str().to_string(),
        })
    }

    pub fn to_stored_payload(&self) -> String {
        let algorithm = self.algorithm.parse().unwrap_or_default();
        crate::shared::totp::TotpConfig::from_parts(
            &self.secret,
            self.digits,
            self.period,
            algorithm,
        )
        .to_stored()
    }
}

fn default_totp_digits() -> u32 {
    6
}

fn default_totp_period() -> u32 {
    30
}

fn default_totp_algorithm() -> String {
    "sha1".to_string()
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
    pub dek_version: u32,
    pub blob: EntryBlobV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMetaFileV2 {
    pub version: u32,
    pub vault_id: String,
    pub dek_version: u32,
    pub cipher: String,
    pub slots: Vec<VaultSlot>,
    pub manifest: VaultManifest,
    #[serde(default)]
    pub recovery_backup: Option<RecoveryBackup>,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSlot {
    pub id: String,
    #[serde(rename = "type")]
    pub slot_type: String,
    pub payload_kind: String,
    pub kdf: KdfParams,
    pub wrapped_dek: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub algo: String,
    pub salt: String,
    pub m: u32,
    pub t: u32,
    pub p: u32,
    pub v: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultManifest {
    pub count: u32,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    pub sha256: String,
    pub dek_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryBackup {
    pub dek_version: u32,
    pub confirmed_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMetaIntegrityView<'a> {
    pub version: u32,
    pub vault_id: &'a str,
    pub dek_version: u32,
    pub cipher: &'a str,
    pub slots: &'a [VaultSlot],
    pub manifest: &'a VaultManifest,
    pub recovery_backup: Option<&'a RecoveryBackup>,
}

impl VaultMetaFileV2 {
    pub fn integrity_view(&self) -> VaultMetaIntegrityView<'_> {
        VaultMetaIntegrityView {
            version: self.version,
            vault_id: &self.vault_id,
            dek_version: self.dek_version,
            cipher: &self.cipher,
            slots: &self.slots,
            manifest: &self.manifest,
            recovery_backup: self.recovery_backup.as_ref(),
        }
    }
}
