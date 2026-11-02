use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum SyncActionDto {
    Pull,
    Push,
    Sync,
    ForcePush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum RemoteModeDto {
    Add,
    Set,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum RepositoryStateDto {
    Clean,
    Merge,
    Rebase,
    RebaseInteractive,
    RebaseMerge,
    Apply,
    CherryPick,
    Revert,
    Bisect,
    Am,
    Mailbox,
    Stash,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum ReadynessDto {
    NoSync,
    RepoState(RepositoryStateDto),
    NoRemote,
    Dirty,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct PendingRemovalEntryDto {
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct PendingRemovalsDto {
    pub entries: Vec<PendingRemovalEntryDto>,
    pub total_local: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum UnreadableReasonDto {
    KeyMismatch,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct UnreadableEntriesDto {
    pub ids: Vec<String>,
    pub reason: UnreadableReasonDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum SyncResultDto {
    Success,
    NoRemote,
    Conflict(Vec<ConflictFileDto>),
    PendingRemovals(PendingRemovalsDto),
    UnreadableEntries(UnreadableEntriesDto),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct ConflictFileDto {
    pub path: String,
    pub local_hash: String,
    pub remote_hash: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct SyncConfigDto {
    pub remote_url: Option<String>,
    pub remote_name: String,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum ConflictResolutionDto {
    UseLocal,
    UseRemote,
    MergeBoth,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct HistoryCommitDto {
    pub hash: String,
    pub timestamp: i64,
    pub message: String,
    pub entry_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct EntryPreviewFieldDto {
    pub name: String,
    pub value: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct EntryPreviewDto {
    pub id: String,
    pub commit: String,
    pub commit_author: Option<String>,
    pub commit_time: Option<i64>,
    pub category: String,
    pub revision: u64,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<EntryPreviewFieldDto>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum RemoteStatusErrorKindDto {
    NoRemote,
    NoUpstream,
    FetchFailed,
    CompareFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RemoteStatusDto {
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub fetched: bool,
    pub checked_at: i64,
    pub error_kind: Option<RemoteStatusErrorKindDto>,
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct ConflictPreviewDto {
    pub entry_id: String,
    pub local: Option<EntryPreviewDto>,
    pub remote: Option<EntryPreviewDto>,
    pub local_error: Option<String>,
    pub remote_error: Option<String>,
    pub diff_fields: Vec<String>,
}
