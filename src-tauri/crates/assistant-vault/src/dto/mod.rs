use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod audit;
mod convert;
pub mod error;
pub mod hibp;
pub mod integrity;
pub mod lifecycle;
pub mod recovery;
pub mod ssh;
pub mod sync;
pub mod totp;

pub use crate::shared::{CustomFieldInput, VaultCategory};
pub use audit::{AuditLogEntry, StrengthResult, StrengthWarning};
pub use error::{VaultErrorCode, VaultErrorDto};
pub use hibp::HibpAuditEntryDto;
pub use integrity::IntegrityResult;
pub use lifecycle::LockReasonDto;
pub use recovery::{
    ExportResultDto, PaperErrorPosDto, RecoveryBackupDto, RecoveryEntryDto, RecoveryExportDto,
    RecoveryKeyDto, RecoverySkipDto,
};
pub use ssh::{SshKeyInfoDto, SshKeyMetaDto};
pub use sync::{
    ConflictFileDto, ConflictPreviewDto, ConflictResolutionDto, EntryPreviewDto,
    EntryPreviewFieldDto, HistoryCommitDto, PendingRemovalEntryDto, PendingRemovalsDto,
    ReadynessDto, RemoteModeDto, RemoteStatusDto, RemoteStatusErrorKindDto, RepositoryStateDto,
    SyncActionDto, SyncConfigDto, SyncResultDto, UnreadableEntriesDto, UnreadableReasonDto,
};
pub use totp::{ParsedOtpAuthDto, TotpAlgorithmDto, TotpCodeDto, TotpInputDto};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "vault.contract.ts")]
pub struct VaultEntryCreate {
    pub category: VaultCategory,
    pub title: String,
    #[ts(optional = nullable)]
    pub username: Option<String>,
    #[ts(optional = nullable)]
    pub url: Option<String>,
    #[ts(optional = nullable)]
    pub password: Option<String>,
    #[ts(optional = nullable)]
    pub notes: Option<String>,
    #[ts(optional = nullable)]
    pub totp: Option<TotpInputDto>,
    pub tags: Vec<String>,
    pub custom_fields: Vec<CustomFieldInput>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "vault.contract.ts")]
pub struct VaultEntryUpdate {
    pub category: VaultCategory,
    pub title: String,
    #[ts(optional = nullable)]
    pub username: Option<String>,
    #[ts(optional = nullable)]
    pub url: Option<String>,
    #[ts(optional = nullable)]
    pub password: Option<String>,
    #[ts(optional = nullable)]
    pub notes: Option<String>,
    #[ts(optional = nullable)]
    pub totp: Option<TotpInputDto>,
    pub tags: Vec<String>,
    pub custom_fields: Vec<CustomFieldInput>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
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

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct VaultEntryDetail {
    pub id: String,
    pub category: VaultCategory,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub has_password: bool,
    pub has_notes: bool,
    pub has_totp: bool,
    pub totp_digits: u32,
    pub totp_period: u32,
    pub totp_algorithm: TotpAlgorithmDto,
    pub custom_field_names: Vec<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RevealResult {
    pub value: String,
    pub expires_at: i64,
}
