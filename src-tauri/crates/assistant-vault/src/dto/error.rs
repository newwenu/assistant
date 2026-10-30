use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VaultErrorCode {
    Locked,
    InvalidPassword,
    NotInitialized,
    AlreadyExists,
    RateLimited,
    Crypto,
    NotFound,
    Validation,
    BiometricUnavailable,
    BiometricCancelled,
    BiometricFailed,
    BiometricReset,
    InvalidKeyFormat,
    InvalidRecoveryKey,
    RecoveryKeyNotSet,
    VaultMetaIntegrityFailed,
    Cancelled,
    RepoNotInitialized,
    NoRemoteConfigured,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
