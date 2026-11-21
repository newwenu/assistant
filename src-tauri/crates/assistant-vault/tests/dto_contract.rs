use assistant_crypto::error::VaultError;
use assistant_sync::types::{
    EntryPreview, EntryPreviewField, Readyness, RemoteStatus, RemoteStatusErrorKind,
    RepositoryState, SyncConfig, SyncError, SyncResult,
};
use assistant_vault::conf::VaultConf;
use assistant_vault::dto::{
    AuditLogEntry, ConflictFileDto, EntryPreviewDto, HibpAuditEntryDto, LockReasonDto,
    PaperErrorPosDto, PendingRemovalsDto, ReadynessDto, RecoveryBackupDto, RecoveryEntryDto,
    RecoveryExportDto, RecoveryKeyDto, RecoverySkipDto, RemoteModeDto, RemoteStatusDto,
    RemoteStatusErrorKindDto, RepositoryStateDto, SshKeyInfoDto, SshKeyMetaDto, SyncActionDto,
    SyncConfigDto, SyncResultDto, TotpCodeDto, UnreadableEntriesDto, UnreadableReasonDto,
    VaultCipherAlgorithmDto, VaultConfDto, VaultErrorCode, VaultErrorDto,
};
use assistant_vault::security_db::LockReason;
use serde_json::{json, to_value};
use std::collections::BTreeSet;

fn sample_sync_config() -> SyncConfig {
    SyncConfig {
        remote_url: Some("https://example.invalid/synthetic-vault.git".into()),
        remote_name: "origin".into(),
        ssh_key_path: Some("synthetic/ssh/key".into()),
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
fn audit_log_entry_wire_always_has_entry_id() {
    let dto = AuditLogEntry {
        id: "jsonl-1-2-3".into(),
        event_type: "vault_unlock".into(),
        entry_id: None,
        detail: None,
        timestamp: 1_700_000_000_000,
    };
    let value = to_value(&dto).unwrap();
    assert_eq!(
        value,
        json!({
            "id": "jsonl-1-2-3",
            "eventType": "vault_unlock",
            "entryId": null,
            "detail": null,
            "timestamp": 1_700_000_000_000i64,
        }),
        "AuditLogEntry must keep every key present; entryId is nullable but never omitted"
    );
}

#[test]
fn totp_code_dto_wire_format() {
    let dto = TotpCodeDto {
        code: "000000".into(),
        time_remaining: 12,
        period: 30,
    };
    let value = to_value(&dto).unwrap();
    assert_eq!(value["code"], "000000");
    assert_eq!(value["timeRemaining"], 12);
    assert_eq!(value["period"], 30);
}

#[test]
fn lock_reason_dto_wire_is_frozen() {
    let pairs = [
        (LockReasonDto::User, "user"),
        (LockReasonDto::AutoLock, "autoLock"),
        (LockReasonDto::ScreenLock, "screenLock"),
        (LockReasonDto::AppExit, "appExit"),
        (LockReasonDto::Reset, "reset"),
        (LockReasonDto::Panic, "panic"),
    ];
    for (dto, wire) in pairs {
        assert_eq!(to_value(dto).unwrap(), json!(wire));
    }
}

#[test]
fn lock_reason_dto_maps_one_to_one_to_domain() {
    let pairs = [
        (LockReasonDto::User, LockReason::User, "user"),
        (LockReasonDto::AutoLock, LockReason::AutoLock, "auto_lock"),
        (
            LockReasonDto::ScreenLock,
            LockReason::ScreenLock,
            "screen_lock",
        ),
        (LockReasonDto::AppExit, LockReason::AppExit, "app_exit"),
        (LockReasonDto::Reset, LockReason::Reset, "reset"),
        (LockReasonDto::Panic, LockReason::Panic, "panic"),
    ];
    for (dto, expected, detail) in pairs {
        assert_eq!(LockReason::from(dto), expected);
        assert_eq!(LockReason::from(dto).as_str(), detail);
    }
}

#[test]
fn readyness_dto_is_one_to_one() {
    for state in [
        Readyness::NoSync,
        Readyness::NoRemote,
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
fn remote_status_dto_serializes_camel_case() {
    let domain = RemoteStatus {
        upstream: Some("origin/main".into()),
        ahead: Some(2),
        behind: Some(3),
        fetched: true,
        checked_at: 1_700_000_000_000,
        error_kind: Some(RemoteStatusErrorKind::FetchFailed),
        error_detail: Some("sanitized detail".into()),
    };
    let dto = RemoteStatusDto::from(domain.clone());
    assert_eq!(
        to_value(&domain).unwrap(),
        to_value(&dto).unwrap(),
        "RemoteStatusDto must not change the wire format"
    );
    let value = to_value(&dto).unwrap();
    assert_eq!(value["checkedAt"], json!(1_700_000_000_000i64));
    assert_eq!(value["errorKind"], json!("fetchFailed"));
}

#[test]
fn remote_status_error_kind_wire_is_frozen() {
    assert_eq!(
        to_value(RemoteStatusErrorKindDto::NoRemote).unwrap(),
        json!("noRemote")
    );
    assert_eq!(
        to_value(RemoteStatusErrorKindDto::NoUpstream).unwrap(),
        json!("noUpstream")
    );
    assert_eq!(
        to_value(RemoteStatusErrorKindDto::FetchFailed).unwrap(),
        json!("fetchFailed")
    );
    assert_eq!(
        to_value(RemoteStatusErrorKindDto::CompareFailed).unwrap(),
        json!("compareFailed")
    );
}

#[test]
fn git_errors_map_to_expected_codes() {
    let dto = VaultErrorDto::from(SyncError::GitFailed {
        code: 128,
        stderr: "synthetic".into(),
    });
    assert_eq!(dto.code, VaultErrorCode::GitUnavailable);
    assert!(dto.retryable);

    let dto = VaultErrorDto::from(SyncError::NoUpstreamConfigured);
    assert_eq!(dto.code, VaultErrorCode::NoUpstream);
    assert!(!dto.retryable);
}

#[test]
fn t_s12_sync_error_message_never_contains_remote_credentials() {
    let dto = VaultErrorDto::from(SyncError::GitFailed {
        code: 128,
        stderr:
            "fatal: unable to access 'https://alice:secret-token@git.example.com/repo.git/': 403"
                .into(),
    });
    assert_eq!(dto.code, VaultErrorCode::GitUnavailable);
    assert!(
        !dto.message.contains("alice"),
        "username leaked: {}",
        dto.message
    );
    assert!(
        !dto.message.contains("secret-token"),
        "token leaked: {}",
        dto.message
    );
    assert!(dto.message.contains("git.example.com/repo.git/"));

    let dto = VaultErrorDto::from(SyncError::GitCli(
        "git ls-remote https://token@git.example.com/repo.git failed".into(),
    ));
    assert!(
        !dto.message.contains("token@"),
        "userinfo leaked: {}",
        dto.message
    );
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
fn recovery_dto_wire_is_frozen() {
    let key = RecoveryKeyDto {
        paper_key: "0123456789ABCDEFGHJKMNPQRSTVWXYZ01234567".into(),
        groups: vec!["01234567".into(); 5],
        dek_version: 3,
        created_at: 1_700_000_000_000,
    };
    assert_eq!(
        to_value(&key).unwrap(),
        json!({
            "paperKey": "0123456789ABCDEFGHJKMNPQRSTVWXYZ01234567",
            "groups": ["01234567", "01234567", "01234567", "01234567", "01234567"],
            "dekVersion": 3,
            "createdAt": 1_700_000_000_000i64,
        })
    );

    let export = RecoveryExportDto {
        entries: vec![RecoveryEntryDto {
            id: "synthetic-entry".into(),
            category: "login".into(),
            blob: json!({ "title": "synthetic" }),
        }],
        skipped: vec![RecoverySkipDto {
            id: "synthetic-skipped".into(),
            reason: "decrypt failed".into(),
        }],
        exported_at: 1_700_000_000_000,
        dek_version: 3,
    };
    assert_eq!(
        to_value(&export).unwrap(),
        json!({
            "entries": [{
                "id": "synthetic-entry",
                "category": "login",
                "blob": { "title": "synthetic" },
            }],
            "skipped": [{ "id": "synthetic-skipped", "reason": "decrypt failed" }],
            "exportedAt": 1_700_000_000_000i64,
            "dekVersion": 3,
        })
    );

    let pos = PaperErrorPosDto { group: 3, index: 5 };
    assert_eq!(to_value(pos).unwrap(), json!({ "group": 3, "index": 5 }));

    let backup = RecoveryBackupDto {
        dek_version: 3,
        confirmed_at: 1_700_000_000_000,
    };
    assert_eq!(
        to_value(&backup).unwrap(),
        json!({ "dekVersion": 3, "confirmedAt": 1_700_000_000_000i64 })
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
        to_value(SyncResultDto::NoRemote).unwrap(),
        json!("noRemote")
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
        (VaultErrorCode::EntryCorrupted, "entryCorrupted"),
        (VaultErrorCode::Validation, "validation"),
        (VaultErrorCode::BiometricUnavailable, "biometricUnavailable"),
        (VaultErrorCode::BiometricCancelled, "biometricCancelled"),
        (VaultErrorCode::BiometricFailed, "biometricFailed"),
        (VaultErrorCode::BiometricReset, "biometricReset"),
        (VaultErrorCode::InvalidKeyFormat, "invalidKeyFormat"),
        (
            VaultErrorCode::InvalidRecoveryKeyFormat,
            "invalidRecoveryKeyFormat",
        ),
        (
            VaultErrorCode::InvalidRecoveryKeyChecksum,
            "invalidRecoveryKeyChecksum",
        ),
        (
            VaultErrorCode::InvalidRecoveryKeyOrMetaTampered,
            "invalidRecoveryKeyOrMetaTampered",
        ),
        (
            VaultErrorCode::VaultMetaIntegrityFailed,
            "vaultMetaIntegrityFailed",
        ),
        (
            VaultErrorCode::UnsupportedVaultVersion,
            "unsupportedVaultVersion",
        ),
        (VaultErrorCode::VaultVersionTooNew, "vaultVersionTooNew"),
        (VaultErrorCode::LocalDbKeyMismatch, "localDbKeyMismatch"),
        (
            VaultErrorCode::VaultRollbackDetected,
            "vaultRollbackDetected",
        ),
        (
            VaultErrorCode::VaultManifestMismatch,
            "vaultManifestMismatch",
        ),
        (VaultErrorCode::VaultDekVersionAhead, "vaultDekVersionAhead"),
        (
            VaultErrorCode::VaultDekVersionBehind,
            "vaultDekVersionBehind",
        ),
        (
            VaultErrorCode::RepositoryContainsForbiddenFile,
            "repositoryContainsForbiddenFile",
        ),
        (
            VaultErrorCode::HistoryDekUnavailable,
            "historyDekUnavailable",
        ),
        (VaultErrorCode::VaultCorrupted, "vaultCorrupted"),
        (VaultErrorCode::Cancelled, "cancelled"),
        (VaultErrorCode::RepoNotInitialized, "repoNotInitialized"),
        (VaultErrorCode::NoRemoteConfigured, "noRemoteConfigured"),
        (VaultErrorCode::NoUpstream, "noUpstream"),
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

fn frontend_error_codes() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../src/vault/core/display/vaultError.ts");
    let text = std::fs::read_to_string(&path).expect("vaultError.ts must be readable");
    let body = text
        .split_once("VAULT_ERROR_CODE_FLAGS")
        .and_then(|(_, rest)| rest.split_once('{'))
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(body, _)| body)
        .expect("VAULT_ERROR_CODE_FLAGS object must exist in vaultError.ts");
    body.lines()
        .filter_map(|line| line.trim().strip_suffix(": true,"))
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect()
}

/// 从生成契约读错误码全集：`vaultError.ts` 的镜像表必须覆盖 Rust 侧全部变体，
/// 而生成物由 `npm run contract:gen` 从 `VaultErrorCode` 派生，比手写清单更不易过期。
fn generated_error_codes() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../src/vault/core/contract/vault.contract.ts");
    let text = std::fs::read_to_string(&path).expect("vault.contract.ts must be readable");
    let union = text
        .split_once("export type VaultErrorCode = ")
        .and_then(|(_, rest)| rest.split_once(';'))
        .map(|(body, _)| body)
        .expect("VaultErrorCode union must exist in vault.contract.ts");
    union
        .split('|')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

#[test]
fn vault_error_codes_match_frontend_array() {
    let generated = generated_error_codes();
    let frontend = frontend_error_codes();
    assert_eq!(
        generated.iter().collect::<std::collections::BTreeSet<_>>(),
        frontend.iter().collect::<std::collections::BTreeSet<_>>(),
        "T-D1: 生成契约 VaultErrorCode 与 vaultError.ts 的 VAULT_ERROR_CODE_FLAGS 必须双向一致"
    );
}

#[test]
fn precise_codes_are_not_collapsed_to_internal() {
    assert_eq!(
        VaultErrorDto::from(VaultError::Database("x".into())).code,
        VaultErrorCode::Database
    );
    assert_eq!(
        VaultErrorDto::from(VaultError::Io(std::io::Error::other("x"))).code,
        VaultErrorCode::Io
    );
    assert_eq!(
        VaultErrorDto::from(VaultError::Clipboard("x".into())).code,
        VaultErrorCode::Clipboard
    );
    assert_eq!(
        VaultErrorDto::from(VaultError::AlreadyUnlocked).code,
        VaultErrorCode::AlreadyUnlocked
    );

    for code in [
        VaultErrorCode::Database,
        VaultErrorCode::Io,
        VaultErrorCode::Clipboard,
        VaultErrorCode::AlreadyUnlocked,
    ] {
        assert!(!code.is_retryable(), "{code:?} must not be retryable");
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
    assert_eq!(dto.code, VaultErrorCode::Database);
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

fn snake_from_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn object_keys(value: serde_json::Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("serialized config must be a JSON object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn vault_conf_dto_covers_every_disk_key() {
    let disk_keys: Vec<&str> = vec![
        "vault_auto_lock_minutes",
        "vault_lock_on_screen_lock",
        "vault_clipboard_clear_seconds",
        "vault_cipher_algorithm",
        "vault_hibp_auto_audit",
        "vault_sync_remote_url",
        "vault_ssh_auto_add",
        "vault_ssh_use_pageant",
        "vault_ssh_use_openssh_agent",
        "vault_ssh_auth_sock_override",
        "vault_sync_last_at",
        "vault_sync_ssh_hosts",
    ];
    let wire_keys: Vec<&str> = vec![
        "vaultAutoLockMinutes",
        "vaultLockOnScreenLock",
        "vaultClipboardClearSeconds",
        "vaultCipherAlgorithm",
        "vaultHibpAutoAudit",
        "vaultSyncRemoteUrl",
        "vaultSshAutoAdd",
        "vaultSshUsePageant",
        "vaultSshUseOpensshAgent",
        "vaultSshAuthSockOverride",
        "vaultSyncLastAt",
        "vaultSyncSshHosts",
    ];

    let conf = VaultConf::new();
    let actual_disk = object_keys(to_value(&conf).unwrap());
    let actual_wire = object_keys(to_value(VaultConfDto::from(conf)).unwrap());

    assert_eq!(
        actual_disk,
        disk_keys.iter().map(|k| k.to_string()).collect(),
        "VaultConf 的磁盘键集合已变化，迁移前后的配置文件将不再互通"
    );
    assert_eq!(
        actual_wire,
        wire_keys.iter().map(|k| k.to_string()).collect(),
        "VaultConfDto 的 wire 键集合已变化，前端契约将失配"
    );
    assert_eq!(
        actual_wire
            .iter()
            .map(|key| snake_from_camel(key))
            .collect::<BTreeSet<String>>(),
        actual_disk,
        "VaultConfDto 与 VaultConf 的字段集合必须一一对应"
    );
}

#[test]
fn vault_cipher_algorithm_wire_is_frozen() {
    assert_eq!(
        to_value(VaultCipherAlgorithmDto::from(
            assistant_vault::conf::VaultCipherAlgorithm::Aes256Gcm
        ))
        .unwrap(),
        json!("aes256Gcm")
    );
    assert_eq!(
        to_value(VaultCipherAlgorithmDto::from(
            assistant_vault::conf::VaultCipherAlgorithm::Chacha20Poly1305
        ))
        .unwrap(),
        json!("chacha20Poly1305")
    );
}
