use serde::{Deserialize, Serialize};

pub mod audit;
pub mod integrity;

pub use crate::shared::{CustomFieldInput, VaultCategory};
pub use audit::{AuditLogEntry, StrengthResult, StrengthWarning};
pub use integrity::IntegrityResult;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealResult {
    pub value: String,
    pub expires_at: i64,
}
