use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum VaultErrorCode {
    Locked,
    InvalidPassword,
    NotInitialized,
    AlreadyExists,
    RateLimited,
    Crypto,
    NotFound,
    EntryCorrupted,
    Validation,
    BiometricUnavailable,
    BiometricCancelled,
    BiometricFailed,
    BiometricReset,
    InvalidKeyFormat,
    InvalidRecoveryKeyFormat,
    InvalidRecoveryKeyChecksum,
    InvalidRecoveryKeyOrMetaTampered,
    VaultMetaIntegrityFailed,
    UnsupportedVaultVersion,
    VaultVersionTooNew,
    LocalDbKeyMismatch,
    VaultRollbackDetected,
    VaultManifestMismatch,
    VaultDekVersionAhead,
    VaultDekVersionBehind,
    RepositoryContainsForbiddenFile,
    HistoryDekUnavailable,
    VaultCorrupted,
    Cancelled,
    RepoNotInitialized,
    NoRemoteConfigured,
    NoUpstream,
    SyncConflict,
    GitUnavailable,
    KeyMismatch,
    HistoryDecryptFailed,
    Internal,
}

impl VaultErrorCode {
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::RateLimited | Self::GitUnavailable)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct VaultErrorDto {
    pub code: VaultErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl VaultErrorDto {
    pub fn new(code: VaultErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: code.is_retryable(),
        }
    }
}
