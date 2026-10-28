use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Git CLI error: {0}")]
    GitCli(String),

    #[error("Git operation exited with non-zero status code: {0}")]
    GitStatus(i32),

    #[error("Sync not initialized")]
    NotInitialized,

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Conflict detected: {0}")]
    Conflict(String),

    #[error("HMAC verification failed")]
    HmacFailed,

    #[error("Vault meta integrity check failed: {0}")]
    VaultMetaIntegrityFailed(String),

    #[error("Entry version cannot be decrypted with the current master key: {0}")]
    HistoryDecryptFailed(String),

    #[error("Remote repository uses a different master key (another device changed the master password): {0}")]
    VaultKeyMismatch(String),

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Readyness {
    NoSync,
    RepoState(RepositoryState),
    Dirty,
    Ready,
}

impl Readyness {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryState {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRemovalEntry {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRemovals {
    pub entries: Vec<PendingRemovalEntry>,
    pub total_local: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnreadableReason {
    KeyMismatch,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncResult {
    Success,
    UpToDate,
    Conflict(Vec<ConflictFile>),
    PendingRemovals(PendingRemovals),
    UnreadableEntries {
        ids: Vec<String>,
        reason: UnreadableReason,
    },
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub local_hash: String,
    pub remote_hash: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    pub remote_url: Option<String>,
    pub remote_name: String,
    pub ssh_key_path: Option<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            remote_url: None,
            remote_name: "origin".to_string(),
            ssh_key_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolution {
    UseLocal,
    UseRemote,
    MergeBoth,
    Skip,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub total: u32,
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub summary: ImportSummary,
    pub removed_ids: Vec<String>,
    pub unreadable_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCommit {
    pub hash: String,
    pub timestamp: i64,
    pub message: String,
    pub entry_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPreviewField {
    pub name: String,
    pub value: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPreview {
    pub entry_id: String,
    pub local: Option<EntryPreview>,
    pub remote: Option<EntryPreview>,
    #[serde(default)]
    pub local_error: Option<String>,
    #[serde(default)]
    pub remote_error: Option<String>,
    #[serde(default)]
    pub diff_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPreview {
    pub id: String,
    pub commit: String,
    #[serde(default)]
    pub commit_author: Option<String>,
    #[serde(default)]
    pub commit_time: Option<i64>,
    pub category: String,
    pub revision: u64,
    pub title: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<EntryPreviewField>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
