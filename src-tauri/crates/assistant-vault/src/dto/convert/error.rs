use assistant_crypto::error::VaultError;
use assistant_sync::types::SyncError;

use crate::dto::error::{VaultErrorCode, VaultErrorDto};

impl From<VaultError> for VaultErrorDto {
    fn from(v: VaultError) -> Self {
        let code = match &v {
            VaultError::Locked => VaultErrorCode::Locked,
            VaultError::AlreadyUnlocked => VaultErrorCode::AlreadyUnlocked,
            VaultError::InvalidPassword => VaultErrorCode::InvalidPassword,
            VaultError::NotInitialized => VaultErrorCode::NotInitialized,
            VaultError::AlreadyExists => VaultErrorCode::AlreadyExists,
            VaultError::RateLimited => VaultErrorCode::RateLimited,
            VaultError::Crypto(_) => VaultErrorCode::Crypto,
            VaultError::Database(_) => VaultErrorCode::Database,
            VaultError::NotFound(_) => VaultErrorCode::NotFound,
            VaultError::EntryCorrupted(_) => VaultErrorCode::EntryCorrupted,
            VaultError::Validation(_) => VaultErrorCode::Validation,
            VaultError::Clipboard(_) => VaultErrorCode::Clipboard,
            VaultError::Io(_) => VaultErrorCode::Io,
            VaultError::BiometricUnavailable => VaultErrorCode::BiometricUnavailable,
            VaultError::BiometricCancelled => VaultErrorCode::BiometricCancelled,
            VaultError::BiometricFailed(_) => VaultErrorCode::BiometricFailed,
            VaultError::BiometricReset => VaultErrorCode::BiometricReset,
            VaultError::InvalidKeyFormat(_) => VaultErrorCode::InvalidKeyFormat,
            VaultError::InvalidRecoveryKeyFormat => VaultErrorCode::InvalidRecoveryKeyFormat,
            VaultError::InvalidRecoveryKeyChecksum => VaultErrorCode::InvalidRecoveryKeyChecksum,
            VaultError::InvalidRecoveryKeyOrMetaTampered => {
                VaultErrorCode::InvalidRecoveryKeyOrMetaTampered
            }
            VaultError::VaultMetaIntegrityFailed => VaultErrorCode::VaultMetaIntegrityFailed,
            VaultError::UnsupportedVaultVersion => VaultErrorCode::UnsupportedVaultVersion,
            VaultError::VaultVersionTooNew => VaultErrorCode::VaultVersionTooNew,
            VaultError::LocalDbKeyMismatch => VaultErrorCode::LocalDbKeyMismatch,
            VaultError::VaultRollbackDetected => VaultErrorCode::VaultRollbackDetected,
            VaultError::VaultManifestMismatch => VaultErrorCode::VaultManifestMismatch,
            VaultError::VaultCorrupted(_) => VaultErrorCode::VaultCorrupted,
            VaultError::Other(_) => VaultErrorCode::Internal,
        };
        let message = match &v {
            VaultError::Crypto(_) => "Cryptographic operation failed".to_string(),
            VaultError::Database(_) => "Vault database error".to_string(),
            VaultError::InvalidKeyFormat(_) => "Invalid SSH key format".to_string(),
            VaultError::VaultCorrupted(m) => m.clone(),
            other => other.to_string(),
        };
        VaultErrorDto::new(code, message)
    }
}

impl From<SyncError> for VaultErrorDto {
    fn from(v: SyncError) -> Self {
        let code = match &v {
            SyncError::Io(_) => VaultErrorCode::Internal,
            SyncError::GitCli(_) => VaultErrorCode::GitUnavailable,
            SyncError::GitFailed { .. } => VaultErrorCode::GitUnavailable,
            SyncError::NotInitialized => VaultErrorCode::RepoNotInitialized,
            SyncError::NoRemoteConfigured => VaultErrorCode::NoRemoteConfigured,
            SyncError::NoUpstreamConfigured => VaultErrorCode::NoUpstream,
            SyncError::VaultLocked => VaultErrorCode::Locked,
            SyncError::Conflict(_) => VaultErrorCode::SyncConflict,
            SyncError::HmacFailed => VaultErrorCode::VaultMetaIntegrityFailed,
            SyncError::VaultMetaIntegrityFailed(_) => VaultErrorCode::VaultMetaIntegrityFailed,
            SyncError::HistoryDecryptFailed(_) => VaultErrorCode::HistoryDecryptFailed,
            SyncError::VaultKeyMismatch(_) => VaultErrorCode::KeyMismatch,
            SyncError::VaultDekVersionAhead { .. } => VaultErrorCode::VaultDekVersionAhead,
            SyncError::VaultDekVersionBehind { .. } => VaultErrorCode::VaultDekVersionBehind,
            SyncError::VaultManifestMismatch => VaultErrorCode::VaultManifestMismatch,
            SyncError::UnsupportedVaultVersion => VaultErrorCode::UnsupportedVaultVersion,
            SyncError::VaultCorrupted(_) => VaultErrorCode::VaultCorrupted,
            SyncError::HistoryDekUnavailable(_) => VaultErrorCode::HistoryDekUnavailable,
            SyncError::RepositoryContainsForbiddenFile(_) => {
                VaultErrorCode::RepositoryContainsForbiddenFile
            }
            SyncError::UnsupportedEntryFileVersion(_) | SyncError::EntryCorrupted(_) => {
                VaultErrorCode::Internal
            }
            SyncError::Other(_) => VaultErrorCode::Internal,
        };
        let message = assistant_sync::redact::redact_url_userinfo(&v.to_string());
        VaultErrorDto::new(code, message)
    }
}
