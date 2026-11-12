mod common;
mod sync_common;

use std::path::Path;

use assistant_vault::plaintext_export::{
    validate_export_target, verify_export_token, write_private_file, EXPORT_CONFIRM_TOKEN,
};
use assistant_vault::sync_engine::mask_entry_preview;
use assistant_vault::types::VaultError;
use assistant_vault::vault_file::types::{BlobCustomField, EntryBlobV2, TotpBlob};

fn tmp_files_in(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".tmp"))
        .count()
}

#[test]
fn export_rejects_missing_or_wrong_confirm_token() {
    assert!(
        matches!(verify_export_token(""), Err(VaultError::Validation(_))),
        "空 token 必须被拒绝"
    );
    assert!(
        matches!(
            verify_export_token("export-plaintext"),
            Err(VaultError::Validation(_))
        ),
        "大小写不符的 token 必须被拒绝"
    );
    assert!(
        matches!(
            verify_export_token("EXPORT"),
            Err(VaultError::Validation(_))
        ),
        "截断 token 必须被拒绝"
    );
    assert!(verify_export_token(EXPORT_CONFIRM_TOKEN).is_ok());
}

#[test]
fn export_target_rejects_vault_directories() {
    let dir = tempfile::tempdir().unwrap();
    let security = dir.path().join("security");
    let repo = security.join("vault");
    std::fs::create_dir_all(&repo).unwrap();

    let in_security = security.join("export.json");
    let in_repo = repo.join("export.json");
    let forbidden = [security.as_path(), repo.as_path()];

    assert!(
        matches!(
            validate_export_target(&in_security, &forbidden),
            Err(VaultError::Validation(_))
        ),
        "导出目标不得落在 security/"
    );
    assert!(
        matches!(
            validate_export_target(&in_repo, &forbidden),
            Err(VaultError::Validation(_))
        ),
        "导出目标不得落在仓库目录"
    );
}

#[test]
fn export_target_rejects_existing_relative_and_parentless_paths() {
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("already.json");
    std::fs::write(&existing, "[]").unwrap();

    assert!(
        matches!(
            validate_export_target(&existing, &[]),
            Err(VaultError::Validation(_))
        ),
        "已存在的目标必须拒绝，禁止静默覆盖"
    );

    assert!(
        matches!(
            validate_export_target(Path::new("relative.json"), &[]),
            Err(VaultError::Validation(_))
        ),
        "相对路径必须拒绝"
    );

    let missing_parent = dir.path().join("no-such-dir").join("export.json");
    assert!(
        matches!(
            validate_export_target(&missing_parent, &[]),
            Err(VaultError::Validation(_))
        ),
        "父目录不存在必须拒绝"
    );
}

#[test]
fn export_target_accepts_new_file_in_writable_dir() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("vault-export.json");
    let resolved = validate_export_target(&target, &[]).unwrap();
    assert_eq!(resolved, target);
}

#[test]
fn export_file_is_created_with_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("vault-export.json");
    write_private_file(&target, b"[]").unwrap();

    assert!(target.is_file(), "导出文件必须就位");
    assert_eq!(std::fs::read(&target).unwrap(), b"[]");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "导出文件必须是 0600");
    }
}

#[test]
fn export_failure_leaves_no_partial_target_or_tmp() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("blocked");
    std::fs::create_dir(&target).unwrap();

    let result = write_private_file(&target, b"secret");
    assert!(result.is_err(), "覆盖目录必须失败");
    assert!(target.is_dir(), "目标必须保持原样");
    assert_eq!(tmp_files_in(dir.path()), 0, "失败后不得残留 .tmp");
}

#[test]
fn conflict_preview_masks_notes_and_secrets() {
    let mut blob = EntryBlobV2::new("Example");
    blob.username = Some("user".to_string());
    blob.url = Some("https://example.com".to_string());
    blob.password = Some("plain-password".to_string());
    blob.notes = Some("recovery codes: 1234 5678".to_string());
    blob.totp = Some(TotpBlob {
        secret: "JBSWY3DPEHPK3PXP".to_string(),
        digits: 6,
        period: 30,
        algorithm: "sha1".to_string(),
    });
    blob.custom_fields = vec![
        BlobCustomField {
            name: "PIN".to_string(),
            value: "1234".to_string(),
            is_secret: true,
        },
        BlobCustomField {
            name: "Note".to_string(),
            value: "public".to_string(),
            is_secret: false,
        },
    ];

    let preview = mask_entry_preview("commit", "e1".to_string(), "login".to_string(), 1, blob);

    assert_eq!(preview.title, "Example");
    assert_eq!(preview.username.as_deref(), Some("user"));
    assert_eq!(preview.password.as_deref(), Some("•••"));
    assert_eq!(preview.notes.as_deref(), Some("•••"), "notes 必须脱敏");
    assert_eq!(preview.totp_secret.as_deref(), Some("•••"));
    assert_eq!(preview.custom_fields[0].value, "•••");
    assert_eq!(preview.custom_fields[1].value, "public");
}

#[cfg(feature = "git-tests")]
#[test]
fn import_writes_entry_files_and_commits_them() {
    use std::process::Command;

    use assistant_crypto::vault_crypto::types::CipherAlgorithm;

    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = sync_common::temp_db_path();
    let db = sync_common::open_db(&db_path);
    let engine = sync_common::engine(dir.path());
    sync_common::seed_meta(&engine, &db);
    engine.init(&db).unwrap();

    let entries = vec![
        sync_common::local_entry("Imported A"),
        sync_common::local_entry("Imported B"),
    ];
    let exported: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());

    let count = db
        .import_entries(entries, CipherAlgorithm::Aes256Gcm, |mk, snapshot| {
            exported.borrow_mut().push(snapshot.id.clone());
            engine
                .vault_file_manager()
                .export_entry(mk, snapshot)
                .map_err(|e| VaultError::Other(e.to_string()))
        })
        .unwrap();
    assert_eq!(count, 2, "两条新条目都必须导入");

    engine.finalize("vault: import 2 entries").unwrap();

    let vault_dir = engine.vault_file_manager().vault_dir().to_path_buf();
    for id in exported.borrow().iter() {
        let path = vault_dir.join(format!("entries/{id}.enc"));
        assert!(path.exists(), "导入后必须存在仓库条目文件: {id}");
    }

    let tracked = Command::new("git")
        .arg("-C")
        .arg(&vault_dir)
        .args(["ls-files", "entries"])
        .output()
        .unwrap();
    assert!(tracked.status.success());
    let tracked_files = String::from_utf8_lossy(&tracked.stdout);
    for id in exported.borrow().iter() {
        assert!(
            tracked_files.contains(&format!("entries/{id}.enc")),
            "条目文件必须已提交到 git: {id}"
        );
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(&vault_dir)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "导入提交后工作区必须干净"
    );

    sync_common::cleanup(&db_path);
}
