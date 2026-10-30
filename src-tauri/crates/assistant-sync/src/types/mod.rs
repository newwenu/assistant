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

    #[error("No remote configured")]
    NoRemoteConfigured,

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

impl std::fmt::Display for Readyness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSync => f.write_str("no sync configured"),
            Self::Dirty => f.write_str("uncommitted changes"),
            Self::Ready => f.write_str("ready"),
            Self::RepoState(state) => write!(f, "repository busy with {state}"),
        }
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

impl RepositoryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::RebaseInteractive => "rebase-interactive",
            Self::RebaseMerge => "rebase-merge",
            Self::Apply => "apply",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
            Self::Bisect => "bisect",
            Self::Am => "am",
            Self::Mailbox => "mailbox",
            Self::Stash => "stash",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for RepositoryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
    /// unix 毫秒
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
    /// unix 毫秒（由 Git `%ct` 秒值 ×1000，低三位恒为 `000`）
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
