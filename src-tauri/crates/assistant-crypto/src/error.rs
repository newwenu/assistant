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
    #[error("Entry corrupted: {0}")]
    EntryCorrupted(String),
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
    #[error("Invalid recovery key format")]
    InvalidRecoveryKeyFormat,
    #[error("Recovery key checksum mismatch")]
    InvalidRecoveryKeyChecksum,
    #[error("Recovery key or vault meta tampered")]
    InvalidRecoveryKeyOrMetaTampered,
    #[error("Vault meta integrity check failed")]
    VaultMetaIntegrityFailed,
    #[error("Unsupported vault version; reset required")]
    UnsupportedVaultVersion,
    #[error("Vault version is newer than this application; upgrade required")]
    VaultVersionTooNew,
    #[error("Local vault database key mismatch")]
    LocalDbKeyMismatch,
    #[error("Vault rollback detected")]
    VaultRollbackDetected,
    #[error("Vault manifest mismatch")]
    VaultManifestMismatch,
    #[error("Vault meta is corrupted or unparsable")]
    VaultCorrupted(String),
    #[error("{0}")]
    Other(String),
}

impl From<String> for VaultError {
    fn from(s: String) -> Self {
        VaultError::Database(s)
    }
}
