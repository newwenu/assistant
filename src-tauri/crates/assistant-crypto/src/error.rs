#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,
    #[error("Vault is already unlocked")]
    AlreadyUnlocked,
    #[error("Invalid master password")]
    InvalidPassword,
    #[error("Vault not initialized")]
    NotInitialized,
    #[error("Vault already exists")]
    AlreadyExists,
    #[error("Rate limited")]
    RateLimited,
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Entry not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Biometric unavailable")]
    BiometricUnavailable,
    #[error("Biometric cancelled")]
    BiometricCancelled,
    #[error("Biometric failed: {0}")]
    BiometricFailed(String),
    #[error("Biometric credential reset; re-enrollment required")]
    BiometricReset,
    #[error("Invalid SSH key format: {0}")]
    InvalidKeyFormat(String),
    #[error("Invalid recovery key")]
    InvalidRecoveryKey,
    #[error("Recovery key not set")]
    RecoveryKeyNotSet,
    #[error("Vault meta integrity check failed")]
    VaultMetaIntegrityFailed,
    #[error("{0}")]
    Other(String),
}

impl From<String> for VaultError {
    fn from(s: String) -> Self {
        VaultError::Database(s)
    }
}
