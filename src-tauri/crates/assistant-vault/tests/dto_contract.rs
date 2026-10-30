use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::totp::TotpResult;
use assistant_sync::types::{
    EntryPreview, EntryPreviewField, Readyness, RepositoryState, SyncConfig, SyncError, SyncResult,
};
use assistant_vault::dto::{
    ConflictFileDto, DisasterRecoveryResultDto, EntryPreviewDto, HibpAuditEntryDto,
    PendingRemovalsDto, ReadynessDto, RemoteModeDto, RepositoryStateDto, SshKeyInfoDto,
    SshKeyMetaDto, SyncActionDto, SyncConfigDto, SyncResultDto, TotpResultDto,
    UnreadableEntriesDto, UnreadableReasonDto, VaultErrorCode, VaultErrorDto,
};
use serde_json::{json, to_value};

fn sample_sync_config() -> SyncConfig {
    SyncConfig {
        remote_url: Some("https://example.invalid/synthetic-vault.git".into()),
        remote_name: "origin".into(),
        ssh_key_path: Some("synthetic/ssh/key".into()),
    }
}

fn sample_totp_result() -> TotpResult {
    TotpResult {
        code: "000000".into(),
        time_remaining: 12,
    }
}

fn sample_entry_preview(category: &str) -> EntryPreview {
    EntryPreview {
        id: "synthetic-entry".into(),
        commit: "synthetic-commit".into(),
        commit_author: Some("synthetic-author".into()),
        commit_time: Some(1_700_000_000_000),
        category: category.into(),
        revision: 3,
        title: "synthetic-title".into(),
        username: Some("synthetic-user".into()),
        url: Some("https://example.invalid".into()),
        password: Some("synthetic-password".into()),
        notes: Some("synthetic-notes".into()),
        totp_secret: Some("SYNTHETICSECRET".into()),
        custom_fields: vec![EntryPreviewField {
            name: "synthetic-field".into(),
            value: "synthetic-value".into(),
            is_secret: true,
        }],
        tags: vec!["synthetic-tag".into()],
        is_favorite: false,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
    }
}

#[test]
fn sync_config_dto_is_one_to_one() {
    let domain = sample_sync_config();
    let dto = SyncConfigDto::from(domain.clone());
    assert_eq!(
        to_value(&domain).unwrap(),
        to_value(&dto).unwrap(),
        "SyncConfigDto must not change the wire format"
    );
}

#[test]
fn ssh_key_info_dto_wire_is_frozen() {
    let dto = SshKeyInfoDto {
        fingerprint: "SHA256:synthetic-fingerprint".into(),
        key_type: "ssh-ed25519".into(),
        comment: "synthetic".into(),
        bits: 256,
        added_by_vault: true,
    };
    assert_eq!(
        to_value(&dto).unwrap(),
        json!({
            "fingerprint": "SHA256:synthetic-fingerprint",
            "keyType": "ssh-ed25519",
            "comment": "synthetic",
            "bits": 256,
            "addedByVault": true,
        }),
        "SshKeyInfoDto wire shape is frozen here; equivalence with the security crate type \
         is asserted in cmd layer where both types are visible (orphan rule forbids From here)"
    );
}

#[test]
fn ssh_key_meta_dto_wire_is_frozen() {
    let dto = SshKeyMetaDto {
        key_type: "ssh-ed25519".into(),
        bits: 256,
        fingerprint: "SHA256:synthetic-fingerprint".into(),
        comment: "synthetic".into(),
        public_key: "ssh-ed25519 SYNTHETICKEY synthetic".into(),
    };
    assert_eq!(
        to_value(&dto).unwrap(),
        json!({
            "keyType": "ssh-ed25519",
            "bits": 256,
            "fingerprint": "SHA256:synthetic-fingerprint",
            "comment": "synthetic",
            "publicKey": "ssh-ed25519 SYNTHETICKEY synthetic",
        }),
        "SshKeyMetaDto wire shape is frozen here; equivalence with the security crate type \
         is asserted in cmd layer where both types are visible (orphan rule forbids From here)"
    );
}

#[test]
fn totp_result_dto_is_one_to_one() {
    let domain = sample_totp_result();
    let dto = TotpResultDto::from(domain.clone());
    assert_eq!(
        to_value(&domain).unwrap(),
        to_value(&dto).unwrap(),
        "TotpResultDto must not change the wire format"
    );
}

#[test]
fn readyness_dto_is_one_to_one() {
    for state in [
        Readyness::NoSync,
        Readyness::Dirty,
        Readyness::Ready,
        Readyness::RepoState(RepositoryState::RebaseInteractive),
    ] {
        let dto = ReadynessDto::from(state);
        assert_eq!(
            to_value(state).unwrap(),
            to_value(dto).unwrap(),
            "ReadynessDto must not change the wire format for {state}"
        );
    }
}

#[test]
fn entry_preview_dto_is_one_to_one() {
    let domain = sample_entry_preview("unlisted");
    let dto = EntryPreviewDto::from(domain.clone());
    assert_eq!(
        to_value(&domain).unwrap(),
        to_value(&dto).unwrap(),
        "EntryPreviewDto must not change the wire format"
    );
}

#[test]
fn entry_preview_category_passes_unknown_value_through() {
    let dto = EntryPreviewDto::from(sample_entry_preview("unlisted"));
    let value = to_value(&dto).unwrap();
    assert_eq!(
        value["category"],
        json!("unlisted"),
        "category must stay a free string, not a narrowed enum"
    );
}

#[test]
fn sync_action_dto_wire_is_frozen() {
    assert_eq!(to_value(SyncActionDto::Pull).unwrap(), json!("pull"));
    assert_eq!(to_value(SyncActionDto::Push).unwrap(), json!("push"));
    assert_eq!(to_value(SyncActionDto::Sync).unwrap(), json!("sync"));
    assert_eq!(
        to_value(SyncActionDto::ForcePush).unwrap(),
        json!("forcePush")
    );
}

#[test]
fn remote_mode_dto_wire_is_frozen() {
    assert_eq!(to_value(RemoteModeDto::Add).unwrap(), json!("add"));
    assert_eq!(to_value(RemoteModeDto::Set).unwrap(), json!("set"));
}

#[test]
fn disaster_recovery_result_dto_wire_is_frozen() {
    let dto = DisasterRecoveryResultDto {
        file_path: "synthetic/export.json".into(),
        entry_count: 2,
        integrity_warning: true,
        skipped_ids: vec!["synthetic-skipped".into()],
    };
    assert_eq!(
        to_value(&dto).unwrap(),
        json!({
            "filePath": "synthetic/export.json",
            "entryCount": 2,
            "integrityWarning": true,
            "skippedIds": ["synthetic-skipped"],
        })
    );
}

#[test]
fn hibp_audit_entry_dto_wire_is_structured() {
    let dto = HibpAuditEntryDto::from(("synthetic-entry".to_string(), 4u32));
    assert_eq!(
        to_value(&dto).unwrap(),
        json!({ "entryId": "synthetic-entry", "matchCount": 4 }),
        "tuple payload must become a named object"
    );
}

#[test]
fn sync_result_dto_has_exactly_five_variants() {
    assert_eq!(to_value(SyncResultDto::Success).unwrap(), json!("success"));
    assert_eq!(
        to_value(SyncResultDto::UpToDate).unwrap(),
        json!("upToDate")
    );
    assert_eq!(
        to_value(SyncResultDto::Conflict(Vec::<ConflictFileDto>::new())).unwrap(),
        json!({ "conflict": [] })
    );
    assert_eq!(
        to_value(SyncResultDto::PendingRemovals(PendingRemovalsDto {
            entries: vec![],
            total_local: 0,
        }))
        .unwrap(),
        json!({ "pendingRemovals": { "entries": [], "totalLocal": 0 } })
    );
    assert_eq!(
        to_value(SyncResultDto::UnreadableEntries(UnreadableEntriesDto {
            ids: vec!["synthetic-entry".into()],
            reason: UnreadableReasonDto::Corrupt,
        }))
        .unwrap(),
        json!({ "unreadableEntries": { "ids": ["synthetic-entry"], "reason": "corrupt" } })
    );
}

#[test]
fn sync_result_error_is_not_a_wire_variant() {
    let converted = SyncResultDto::try_from(SyncResult::Error("synthetic".into()));
    assert!(
        converted.is_err(),
        "domain SyncResult::Error must leave the success channel"
    );
}

#[test]
fn vault_error_code_strings_are_frozen() {
    let codes = [
        (VaultErrorCode::Locked, "locked"),
        (VaultErrorCode::InvalidPassword, "invalidPassword"),
        (VaultErrorCode::NotInitialized, "notInitialized"),
        (VaultErrorCode::AlreadyExists, "alreadyExists"),
        (VaultErrorCode::RateLimited, "rateLimited"),
        (VaultErrorCode::Crypto, "crypto"),
        (VaultErrorCode::NotFound, "notFound"),
        (VaultErrorCode::Validation, "validation"),
        (VaultErrorCode::BiometricUnavailable, "biometricUnavailable"),
        (VaultErrorCode::BiometricCancelled, "biometricCancelled"),
        (VaultErrorCode::BiometricFailed, "biometricFailed"),
        (VaultErrorCode::BiometricReset, "biometricReset"),
        (VaultErrorCode::InvalidKeyFormat, "invalidKeyFormat"),
        (VaultErrorCode::InvalidRecoveryKey, "invalidRecoveryKey"),
        (VaultErrorCode::RecoveryKeyNotSet, "recoveryKeyNotSet"),
        (
            VaultErrorCode::VaultMetaIntegrityFailed,
            "vaultMetaIntegrityFailed",
        ),
        (VaultErrorCode::Cancelled, "cancelled"),
        (VaultErrorCode::RepoNotInitialized, "repoNotInitialized"),
        (VaultErrorCode::NoRemoteConfigured, "noRemoteConfigured"),
        (VaultErrorCode::SyncConflict, "syncConflict"),
        (VaultErrorCode::GitUnavailable, "gitUnavailable"),
        (VaultErrorCode::KeyMismatch, "keyMismatch"),
        (VaultErrorCode::HistoryDecryptFailed, "historyDecryptFailed"),
        (VaultErrorCode::Internal, "internal"),
    ];
    for (code, expected) in codes {
        assert_eq!(
            to_value(code).unwrap(),
            json!(expected),
            "wire code for {code:?} must stay stable"
        );
    }
}

#[test]
fn retryable_is_derived_from_code_only() {
    assert!(VaultErrorCode::RateLimited.is_retryable());
    assert!(VaultErrorCode::GitUnavailable.is_retryable());
    assert!(!VaultErrorCode::Internal.is_retryable());
    assert!(!VaultErrorCode::KeyMismatch.is_retryable());

    let dto = VaultErrorDto::from(VaultError::RateLimited);
    assert_eq!(dto.code, VaultErrorCode::RateLimited);
    assert!(dto.retryable);

    let dto = VaultErrorDto::from(SyncError::GitCli("synthetic".into()));
    assert_eq!(dto.code, VaultErrorCode::GitUnavailable);
    assert!(dto.retryable);

    let dto = VaultErrorDto::from(VaultError::InvalidPassword);
    assert_eq!(dto.code, VaultErrorCode::InvalidPassword);
    assert!(!dto.retryable);
}

#[test]
fn error_message_does_not_carry_crypto_payload() {
    let dto = VaultErrorDto::from(VaultError::Crypto("synthetic-secret-material".into()));
    assert_eq!(dto.code, VaultErrorCode::Crypto);
    assert!(
        !dto.message.contains("synthetic-secret-material"),
        "Crypto payload must not reach the wire"
    );

    let dto = VaultErrorDto::from(VaultError::Database("synthetic-db-path".into()));
    assert_eq!(dto.code, VaultErrorCode::Internal);
    assert!(
        !dto.message.contains("synthetic-db-path"),
        "Database payload must not reach the wire"
    );

    let dto = VaultErrorDto::from(VaultError::InvalidKeyFormat("synthetic-key".into()));
    assert_eq!(dto.code, VaultErrorCode::InvalidKeyFormat);
    assert!(
        !dto.message.contains("synthetic-key"),
        "InvalidKeyFormat payload must not reach the wire"
    );
}

#[test]
fn error_dto_wire_shape_is_code_message_retryable() {
    let dto = VaultErrorDto::from(VaultError::Locked);
    let value = to_value(&dto).unwrap();
    assert_eq!(
        value,
        json!({
            "code": "locked",
            "message": "Vault is locked",
            "retryable": false,
        })
    );
}

#[test]
fn enum_payload_shapes_serialize_as_expected() {
    assert_eq!(to_value(RepositoryStateDto::Clean).unwrap(), json!("clean"));
    assert_eq!(
        to_value(ReadynessDto::NoSync).unwrap(),
        json!("noSync"),
        "bare variant must stay a plain string"
    );
    assert_eq!(
        to_value(ReadynessDto::RepoState(RepositoryStateDto::Clean)).unwrap(),
        json!({ "repoState": "clean" }),
        "newtype variant must stay a single-key object"
    );
    assert_eq!(
        to_value(UnreadableReasonDto::KeyMismatch).unwrap(),
        json!("keyMismatch")
    );
}

#[test]
fn option_fields_serialize_as_null() {
    let dto = SyncConfigDto {
        remote_url: None,
        remote_name: "origin".into(),
        ssh_key_path: None,
    };
    let value = to_value(&dto).unwrap();
    assert_eq!(
        value,
        json!({
            "remoteUrl": null,
            "remoteName": "origin",
            "sshKeyPath": null,
        }),
        "Option must serialize as null, never be skipped"
    );
}
