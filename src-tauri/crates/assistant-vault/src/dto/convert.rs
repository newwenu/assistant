use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::totp::{ParsedOtpAuthSpec, TotpAlgorithm};
use assistant_sync::types::{
    ConflictFile, ConflictPreview, ConflictResolution, EntryPreview, EntryPreviewField,
    HistoryCommit, PendingRemovalEntry, PendingRemovals, Readyness, RemoteStatus,
    RemoteStatusErrorKind, RepositoryState, SyncConfig, SyncError, SyncResult, UnreadableReason,
};

use super::error::{VaultErrorCode, VaultErrorDto};
use super::hibp::HibpAuditEntryDto;
use super::sync::{
    ConflictFileDto, ConflictPreviewDto, ConflictResolutionDto, EntryPreviewDto,
    EntryPreviewFieldDto, HistoryCommitDto, PendingRemovalEntryDto, PendingRemovalsDto,
    ReadynessDto, RemoteStatusDto, RemoteStatusErrorKindDto, RepositoryStateDto, SyncConfigDto,
    SyncResultDto, UnreadableEntriesDto, UnreadableReasonDto,
};
use super::totp::{ParsedOtpAuthDto, TotpAlgorithmDto};
use super::{VaultEntryDetail, VaultEntrySummary};
use crate::types::VaultEntrySnapshot;

impl From<RepositoryState> for RepositoryStateDto {
    fn from(v: RepositoryState) -> Self {
        match v {
            RepositoryState::Clean => Self::Clean,
            RepositoryState::Merge => Self::Merge,
            RepositoryState::Rebase => Self::Rebase,
            RepositoryState::RebaseInteractive => Self::RebaseInteractive,
            RepositoryState::RebaseMerge => Self::RebaseMerge,
            RepositoryState::Apply => Self::Apply,
            RepositoryState::CherryPick => Self::CherryPick,
            RepositoryState::Revert => Self::Revert,
            RepositoryState::Bisect => Self::Bisect,
            RepositoryState::Am => Self::Am,
            RepositoryState::Mailbox => Self::Mailbox,
            RepositoryState::Stash => Self::Stash,
            RepositoryState::Unknown => Self::Unknown,
        }
    }
}

impl From<Readyness> for ReadynessDto {
    fn from(v: Readyness) -> Self {
        match v {
            Readyness::NoSync => Self::NoSync,
            Readyness::RepoState(state) => Self::RepoState(RepositoryStateDto::from(state)),
            Readyness::NoRemote => Self::NoRemote,
            Readyness::Dirty => Self::Dirty,
            Readyness::Ready => Self::Ready,
        }
    }
}

impl From<RemoteStatusErrorKind> for RemoteStatusErrorKindDto {
    fn from(v: RemoteStatusErrorKind) -> Self {
        match v {
            RemoteStatusErrorKind::NoRemote => Self::NoRemote,
            RemoteStatusErrorKind::NoUpstream => Self::NoUpstream,
            RemoteStatusErrorKind::FetchFailed => Self::FetchFailed,
            RemoteStatusErrorKind::CompareFailed => Self::CompareFailed,
        }
    }
}

impl From<RemoteStatus> for RemoteStatusDto {
    fn from(v: RemoteStatus) -> Self {
        let RemoteStatus {
            upstream,
            ahead,
            behind,
            fetched,
            checked_at,
            error_kind,
            error_detail,
        } = v;
        Self {
            upstream,
            ahead,
            behind,
            fetched,
            checked_at,
            error_kind: error_kind.map(RemoteStatusErrorKindDto::from),
            error_detail,
        }
    }
}

impl From<PendingRemovalEntry> for PendingRemovalEntryDto {
    fn from(v: PendingRemovalEntry) -> Self {
        let PendingRemovalEntry { id, title } = v;
        Self { id, title }
    }
}

impl From<PendingRemovals> for PendingRemovalsDto {
    fn from(v: PendingRemovals) -> Self {
        let PendingRemovals {
            entries,
            total_local,
        } = v;
        Self {
            entries: entries
                .into_iter()
                .map(PendingRemovalEntryDto::from)
                .collect(),
            total_local,
        }
    }
}

impl From<UnreadableReason> for UnreadableReasonDto {
    fn from(v: UnreadableReason) -> Self {
        match v {
            UnreadableReason::KeyMismatch => Self::KeyMismatch,
            UnreadableReason::Corrupt => Self::Corrupt,
        }
    }
}

impl TryFrom<SyncResult> for SyncResultDto {
    type Error = VaultErrorDto;

    fn try_from(v: SyncResult) -> Result<Self, Self::Error> {
        Ok(match v {
            SyncResult::Success => Self::Success,
            SyncResult::NoRemote => Self::NoRemote,
            SyncResult::Conflict(files) => {
                Self::Conflict(files.into_iter().map(ConflictFileDto::from).collect())
            }
            SyncResult::PendingRemovals(pending) => {
                Self::PendingRemovals(PendingRemovalsDto::from(pending))
            }
            SyncResult::UnreadableEntries { ids, reason } => {
                Self::UnreadableEntries(UnreadableEntriesDto {
                    ids,
                    reason: UnreadableReasonDto::from(reason),
                })
            }
            SyncResult::Error(message) => {
                return Err(VaultErrorDto::new(VaultErrorCode::Internal, message));
            }
        })
    }
}

impl From<ConflictFile> for ConflictFileDto {
    fn from(v: ConflictFile) -> Self {
        let ConflictFile {
            path,
            local_hash,
            remote_hash,
            title,
        } = v;
        Self {
            path,
            local_hash,
            remote_hash,
            title,
        }
    }
}

impl From<SyncConfig> for SyncConfigDto {
    fn from(v: SyncConfig) -> Self {
        let SyncConfig {
            remote_url,
            remote_name,
            ssh_key_path,
        } = v;
        Self {
            remote_url,
            remote_name,
            ssh_key_path,
        }
    }
}

impl From<SyncConfigDto> for SyncConfig {
    fn from(v: SyncConfigDto) -> Self {
        let SyncConfigDto {
            remote_url,
            remote_name,
            ssh_key_path,
        } = v;
        Self {
            remote_url,
            remote_name,
            ssh_key_path,
        }
    }
}

impl From<ConflictResolution> for ConflictResolutionDto {
    fn from(v: ConflictResolution) -> Self {
        match v {
            ConflictResolution::UseLocal => Self::UseLocal,
            ConflictResolution::UseRemote => Self::UseRemote,
            ConflictResolution::MergeBoth => Self::MergeBoth,
            ConflictResolution::Skip => Self::Skip,
        }
    }
}

impl From<ConflictResolutionDto> for ConflictResolution {
    fn from(v: ConflictResolutionDto) -> Self {
        match v {
            ConflictResolutionDto::UseLocal => Self::UseLocal,
            ConflictResolutionDto::UseRemote => Self::UseRemote,
            ConflictResolutionDto::MergeBoth => Self::MergeBoth,
            ConflictResolutionDto::Skip => Self::Skip,
        }
    }
}

impl From<HistoryCommit> for HistoryCommitDto {
    fn from(v: HistoryCommit) -> Self {
        let HistoryCommit {
            hash,
            timestamp,
            message,
            entry_id,
        } = v;
        Self {
            hash,
            timestamp,
            message,
            entry_id,
        }
    }
}

impl From<EntryPreviewField> for EntryPreviewFieldDto {
    fn from(v: EntryPreviewField) -> Self {
        let EntryPreviewField {
            name,
            value,
            is_secret,
        } = v;
        Self {
            name,
            value,
            is_secret,
        }
    }
}

impl From<EntryPreview> for EntryPreviewDto {
    fn from(v: EntryPreview) -> Self {
        let EntryPreview {
            id,
            commit,
            commit_author,
            commit_time,
            category,
            revision,
            title,
            username,
            url,
            password,
            notes,
            totp_secret,
            custom_fields,
            tags,
            is_favorite,
            created_at,
            updated_at,
        } = v;
        Self {
            id,
            commit,
            commit_author,
            commit_time,
            category,
            revision,
            title,
            username,
            url,
            password,
            notes,
            totp_secret,
            custom_fields: custom_fields
                .into_iter()
                .map(EntryPreviewFieldDto::from)
                .collect(),
            tags,
            is_favorite,
            created_at,
            updated_at,
        }
    }
}

impl From<ConflictPreview> for ConflictPreviewDto {
    fn from(v: ConflictPreview) -> Self {
        let ConflictPreview {
            entry_id,
            local,
            remote,
            local_error,
            remote_error,
            diff_fields,
        } = v;
        Self {
            entry_id,
            local: local.map(EntryPreviewDto::from),
            remote: remote.map(EntryPreviewDto::from),
            local_error,
            remote_error,
            diff_fields,
        }
    }
}

impl From<TotpAlgorithm> for TotpAlgorithmDto {
    fn from(v: TotpAlgorithm) -> Self {
        match v {
            TotpAlgorithm::Sha1 => Self::Sha1,
            TotpAlgorithm::Sha256 => Self::Sha256,
            TotpAlgorithm::Sha512 => Self::Sha512,
        }
    }
}

impl From<ParsedOtpAuthSpec> for ParsedOtpAuthDto {
    fn from(v: ParsedOtpAuthSpec) -> Self {
        let ParsedOtpAuthSpec {
            secret,
            digits,
            period,
            algorithm,
            issuer,
            account,
        } = v;
        Self {
            secret,
            digits,
            period,
            algorithm: TotpAlgorithmDto::from(algorithm),
            issuer,
            account,
        }
    }
}

impl From<VaultError> for VaultErrorDto {
    fn from(v: VaultError) -> Self {
        let code = match &v {
            VaultError::Locked => VaultErrorCode::Locked,
            VaultError::AlreadyUnlocked => VaultErrorCode::Internal,
            VaultError::InvalidPassword => VaultErrorCode::InvalidPassword,
            VaultError::NotInitialized => VaultErrorCode::NotInitialized,
            VaultError::AlreadyExists => VaultErrorCode::AlreadyExists,
            VaultError::RateLimited => VaultErrorCode::RateLimited,
            VaultError::Crypto(_) => VaultErrorCode::Crypto,
            VaultError::Database(_) => VaultErrorCode::Internal,
            VaultError::NotFound(_) => VaultErrorCode::NotFound,
            VaultError::Validation(_) => VaultErrorCode::Validation,
            VaultError::Clipboard(_) => VaultErrorCode::Internal,
            VaultError::Io(_) => VaultErrorCode::Internal,
            VaultError::BiometricUnavailable => VaultErrorCode::BiometricUnavailable,
            VaultError::BiometricCancelled => VaultErrorCode::BiometricCancelled,
            VaultError::BiometricFailed(_) => VaultErrorCode::BiometricFailed,
            VaultError::BiometricReset => VaultErrorCode::BiometricReset,
            VaultError::InvalidKeyFormat(_) => VaultErrorCode::InvalidKeyFormat,
            VaultError::InvalidRecoveryKey => VaultErrorCode::InvalidRecoveryKey,
            VaultError::RecoveryKeyNotSet => VaultErrorCode::RecoveryKeyNotSet,
            VaultError::VaultMetaIntegrityFailed => VaultErrorCode::VaultMetaIntegrityFailed,
            VaultError::Other(_) => VaultErrorCode::Internal,
        };
        let message = match &v {
            VaultError::Crypto(_) => "Cryptographic operation failed".to_string(),
            VaultError::Database(_) => "Vault database error".to_string(),
            VaultError::InvalidKeyFormat(_) => "Invalid SSH key format".to_string(),
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
            SyncError::Other(_) => VaultErrorCode::Internal,
        };
        VaultErrorDto::new(code, v.to_string())
    }
}

impl From<(String, u32)> for HibpAuditEntryDto {
    fn from(v: (String, u32)) -> Self {
        let (entry_id, match_count) = v;
        Self {
            entry_id,
            match_count,
        }
    }
}

impl From<&VaultEntrySnapshot> for VaultEntryDetail {
    fn from(v: &VaultEntrySnapshot) -> Self {
        let totp = v
            .totp_secret
            .as_deref()
            .and_then(crate::shared::totp::TotpConfig::from_stored);
        let totp_algorithm = TotpAlgorithmDto::from(
            totp.as_ref()
                .map(|config| config.algorithm())
                .unwrap_or_default(),
        );
        Self {
            id: v.id.clone(),
            category: v.category,
            title: v.title.clone(),
            username: v.username.clone(),
            url: v.url.clone(),
            has_password: v.password.is_some(),
            has_notes: v.notes.is_some(),
            has_totp: totp.is_some(),
            totp_digits: totp.as_ref().map(|config| config.digits()).unwrap_or(6),
            totp_period: totp.as_ref().map(|config| config.period()).unwrap_or(30),
            totp_algorithm,
            custom_field_names: v
                .custom_fields
                .iter()
                .map(|field| field.name.clone())
                .collect(),
            tags: v.tags.clone(),
            is_favorite: v.is_favorite,
            revision: v.revision,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl From<&VaultEntrySnapshot> for VaultEntrySummary {
    fn from(v: &VaultEntrySnapshot) -> Self {
        Self {
            id: v.id.clone(),
            category: v.category,
            title: v.title.clone(),
            username: v.username.clone(),
            url: v.url.clone(),
            tags: v.tags.clone(),
            is_favorite: v.is_favorite,
            updated_at: v.updated_at,
        }
    }
}
