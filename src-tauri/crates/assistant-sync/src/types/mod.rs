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
pub enum SyncResult {
    Success,
    UpToDate,
    Conflict(Vec<ConflictFile>),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub local_hash: String,
    pub remote_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    pub remote_url: Option<String>,
    pub remote_name: String,
    pub branch: String,
    pub ssh_key_path: Option<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            remote_url: None,
            remote_name: "origin".to_string(),
            branch: "main".to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultMeta {
    pub version: u32,
    pub cipher_algorithm: String,
    pub entry_count: u32,
    pub last_sync_at: Option<i64>,
    pub hmac: String,
}

impl VaultMeta {
    pub const CURRENT_VERSION: u32 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedEntryFile {
    pub id: String,
    pub encrypted_data: String,
    pub revision: u64,
    pub updated_at: i64,
}