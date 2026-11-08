#![cfg(feature = "git-tests")]

mod sync_common;

use std::path::Path;

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_sync::types::{
    ConflictResolution, Readyness, SyncError, SyncResult, UnreadableReason,
};
use assistant_vault::dto::{VaultErrorCode, VaultErrorDto};
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::sync_engine::SyncEngine;
use sync_common::{
    bare_repo, cleanup, commit_repo, engine, git_repo, local_entry, open_db, open_db_with_password,
    remote_snapshot as snapshot, temp_db_path, write_remote, OTHER_PASSWORD, PASSWORD,
};

#[test]
fn import_from_repo_inserts_then_updates_newer_remote_revision() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    write_remote(
        &engine,
        &db,
        &[
            snapshot("e1", "First", 1, 1_000),
            snapshot("e2", "Second", 1, 1_000),
        ],
    );

    let summary = engine.import_from_repo(&db, &[]).unwrap();

    assert_eq!(summary.summary.total, 2, "both remote entries must be seen");
    assert_eq!(summary.summary.inserted, 2);
    assert_eq!(summary.summary.updated, 0);
    assert_eq!(summary.summary.skipped, 0);
    assert_eq!(db.list_entry_ids(false).unwrap().len(), 2);
    assert_eq!(db.get_entry("e1").unwrap().title, "First");

    write_remote(&engine, &db, &[snapshot("e1", "Remote v2", 2, 2_000)]);
    let summary = engine.import_from_repo(&db, &["e1".to_string()]).unwrap();

    assert_eq!(summary.summary.updated, 1, "newer revision must overwrite");
    assert_eq!(summary.summary.inserted, 0);
    assert_eq!(db.get_entry("e1").unwrap().title, "Remote v2");
    cleanup(&db_path);
}

#[test]
fn import_reports_removed_ids_without_touching_local_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    let local = db
        .create_entry_tx(
            local_entry("Local only"),
            CipherAlgorithm::Aes256Gcm,
            |_, _| Ok(()),
        )
        .unwrap();

    let guard = db.get_master_key().unwrap();
    engine
        .vault_file_manager()
        .export_vault_meta(guard.as_ref().unwrap(), None, None)
        .unwrap();
    drop(guard);

    let report = engine
        .import_from_repo(&db, std::slice::from_ref(&local.id))
        .unwrap();

    assert_eq!(report.removed_ids, vec![local.id.clone()]);
    assert!(report.unreadable_ids.is_empty());
    assert_eq!(db.list_entry_ids(false).unwrap(), vec![local.id]);
    cleanup(&db_path);
}

#[test]
fn history_list_and_restore_revert_entry_to_old_commit() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    write_remote(&engine, &db, &[snapshot("e1", "First", 1, 1_000)]);
    engine.init(&db).unwrap();

    write_remote(&engine, &db, &[snapshot("e1", "Second", 2, 2_000)]);
    engine.finalize("vault: update e1").unwrap();

    let commits = engine.history_list(Some("e1"), 10).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].message, "vault: update e1");
    assert_eq!(commits[0].entry_id.as_deref(), Some("e1"));

    let old = commits[1].hash.clone();
    let preview = engine.history_preview(&db, &old, "e1").unwrap();
    assert_eq!(preview.title, "First");
    assert_eq!(preview.password.as_deref(), Some("•••"));
    assert_eq!(preview.custom_fields[0].value, "1234");

    let restored = engine.history_restore(&db, &old, "e1").unwrap();
    assert_eq!(restored.title, "First");
    assert!(restored.revision >= 2);
    assert_eq!(db.get_entry("e1").unwrap().title, "First");
    cleanup(&db_path);
}

#[test]
fn conflict_preview_decrypts_both_sides_and_blocks_history_restore() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);

    let source = engine(&dir.path().join("source"));
    write_remote(&source, &db, &[snapshot("e1", "Base", 1, 1_000)]);
    source.init(&db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    write_remote(&target, &db, &[snapshot("e1", "Remote side", 2, 2_000)]);
    commit_repo(&target, "vault: update e1");
    target.finalize("vault: update e1").unwrap();

    write_remote(&source, &db, &[snapshot("e1", "Local side", 2, 3_000)]);
    commit_repo(&source, "vault: update e1");

    let result = source.pull(&db).unwrap();
    assert!(
        matches!(result, SyncResult::Conflict(_)),
        "pull must report a conflict"
    );

    let preview = source.conflict_preview(&db, "entries/e1.enc").unwrap();
    assert_eq!(preview.entry_id, "e1");
    assert!(preview.local_error.is_none());
    assert!(preview.remote_error.is_none());
    let local = preview.local.as_ref().unwrap();
    let remote_preview = preview.remote.as_ref().unwrap();
    assert_eq!(local.title, "Local side");
    assert_eq!(remote_preview.title, "Remote side");
    assert_eq!(local.password.as_deref(), Some("•••"));
    assert_eq!(remote_preview.password.as_deref(), Some("•••"));
    assert!(local.commit_author.is_some());
    assert!(remote_preview.commit_author.is_some());
    assert!(preview.diff_fields.iter().any(|f| f == "title"));

    let revealed = source
        .reveal_conflict_field(&db, "entries/e1.enc", "remote", "password")
        .unwrap();
    assert_eq!(revealed, "remote-pw");

    let commits = source.history_list(Some("e1"), 5).unwrap();
    let err = source
        .history_restore(&db, &commits[0].hash, "e1")
        .unwrap_err();
    assert!(
        matches!(err, SyncError::Other(ref msg) if msg.contains("Repository is in")),
        "history restore must be refused while conflicts are unresolved"
    );
    cleanup(&db_path);
}

#[test]
fn history_restore_rejects_version_encrypted_with_other_master_key() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let other_db_path = temp_db_path();
    let db = open_db(&db_path);
    let other_db = open_db_with_password(&other_db_path, OTHER_PASSWORD);
    let engine = engine(dir.path());

    write_remote(&engine, &db, &[snapshot("e1", "First", 1, 1_000)]);
    engine.init(&db).unwrap();

    {
        let guard = other_db.get_master_key().unwrap();
        engine
            .vault_file_manager()
            .export_entry(
                guard.as_ref().unwrap(),
                &snapshot("e1", "Other key", 2, 2_000),
                false,
            )
            .unwrap();
    }
    commit_repo(&engine, "vault: update e1");

    let commits = engine.history_list(Some("e1"), 1).unwrap();
    let err = engine
        .history_restore(&db, &commits[0].hash, "e1")
        .unwrap_err();
    assert!(matches!(err, SyncError::HistoryDecryptFailed(_)));
    assert!(db.list_entry_ids(false).unwrap().is_empty());
    cleanup(&db_path);
    cleanup(&other_db_path);
}

#[test]
fn import_aborts_when_vault_meta_tampered() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    write_remote(&engine, &db, &[snapshot("e1", "First", 1, 1_000)]);

    let meta_path = engine.vault_file_manager().vault_dir().join(".vault-meta");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    meta["version"] = serde_json::json!(99);
    std::fs::write(&meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

    let err = engine
        .import_from_repo(&db, &["e1".to_string()])
        .unwrap_err();

    assert!(matches!(err, SyncError::VaultMetaIntegrityFailed(_)));
    assert!(db.list_entry_ids(false).unwrap().is_empty());
    cleanup(&db_path);
}

#[test]
fn import_skips_entry_deleted_locally() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    write_remote(&engine, &db, &[snapshot("e1", "Remote", 1, 1_000)]);
    engine.import_from_repo(&db, &[]).unwrap();
    db.delete_entry("e1").unwrap();

    write_remote(&engine, &db, &[snapshot("e1", "Remote again", 5, 5_000)]);
    let summary = engine.import_from_repo(&db, &["e1".to_string()]).unwrap();

    assert_eq!(summary.summary.skipped, 1);
    assert_eq!(summary.summary.inserted, 0);
    assert!(db.get_entry("e1").is_err());
    cleanup(&db_path);
}

#[test]
fn purge_removes_entry_files_before_db_rows_so_entries_never_resurrect() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    write_remote(&engine, &db, &[snapshot("p1", "Purge me", 1, 1_000)]);
    engine.import_from_repo(&db, &[]).unwrap();
    assert_eq!(
        engine.vault_file_manager().list_entry_files().unwrap(),
        vec!["p1"]
    );

    db.delete_entry("p1").unwrap();
    let ids = db.list_deleted_entry_ids().unwrap();
    assert_eq!(ids, vec!["p1".to_string()]);

    for id in &ids {
        engine.vault_file_manager().remove_entry_file(id).unwrap();
    }
    assert_eq!(db.purge_deleted().unwrap(), 1);

    assert!(!engine.vault_file_manager().entry_file_path("p1").exists());
    assert!(engine
        .vault_file_manager()
        .list_entry_files()
        .unwrap()
        .is_empty());
    assert!(db.list_entry_ids(true).unwrap().is_empty());

    let summary = engine.import_from_repo(&db, &[]).unwrap();
    assert_eq!(summary.summary.inserted, 0);
    assert!(db.get_entry("p1").is_err());
    cleanup(&db_path);
}

#[test]
fn pull_stages_all_removals_then_cancel_keeps_db_and_confirm_soft_deletes() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);

    let source = engine(&dir.path().join("source"));
    write_remote(
        &source,
        &db,
        &[
            snapshot("e1", "First", 1, 1_000),
            snapshot("e2", "Second", 1, 1_000),
        ],
    );
    source.init(&db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();
    git_repo(source.vault_file_manager().vault_dir().to_path_buf())
        .push(None, None)
        .unwrap();

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    target.import_from_repo(&db, &[]).unwrap();
    assert_eq!(db.list_entry_ids(false).unwrap().len(), 2);

    source.vault_file_manager().remove_entry_file("e1").unwrap();
    source.vault_file_manager().remove_entry_file("e2").unwrap();
    commit_repo(&source, "vault: delete all");
    git_repo(source.vault_file_manager().vault_dir().to_path_buf())
        .push(None, None)
        .unwrap();

    let pending = match target.pull(&db).unwrap() {
        SyncResult::PendingRemovals(pending) => pending,
        other => panic!("expected PendingRemovals, got {:?}", other),
    };
    let mut staged: Vec<String> = pending.entries.iter().map(|e| e.id.clone()).collect();
    staged.sort();
    assert_eq!(staged, vec!["e1".to_string(), "e2".to_string()]);
    assert!(pending
        .entries
        .iter()
        .any(|e| e.id == "e2" && e.title.as_deref() == Some("Second")));
    assert_eq!(pending.total_local, 2);
    assert_eq!(
        db.list_entry_ids(false).unwrap().len(),
        2,
        "staging must not delete local rows"
    );
    assert!(db.list_deleted_entry_ids().unwrap().is_empty());

    assert!(matches!(
        target.sync_now(&db).unwrap(),
        SyncResult::PendingRemovals(_)
    ));
    assert_eq!(
        db.list_entry_ids(false).unwrap().len(),
        2,
        "cancel must leave local rows untouched"
    );
    assert!(db.list_deleted_entry_ids().unwrap().is_empty());
    assert!(target.pending_removals().is_some());
    assert!(!target
        .vault_file_manager()
        .vault_dir()
        .join("entries/e1.enc")
        .exists());

    let deleted = target.confirm_removals(&db, &["e2".to_string()]).unwrap();
    assert_eq!(deleted, 1);
    assert!(target.pending_removals().is_none());
    assert_eq!(
        db.list_entry_ids(false).unwrap(),
        vec!["e1".to_string()],
        "unconfirmed removal must keep its row"
    );
    assert_eq!(db.list_deleted_entry_ids().unwrap(), vec!["e2".to_string()]);
    assert!(!target
        .vault_file_manager()
        .vault_dir()
        .join("entries/e2.enc")
        .exists());
    cleanup(&db_path);
}

#[test]
fn unreadable_entry_reported_by_pull_and_sync_now_without_overwriting() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);

    let source = engine(&dir.path().join("source"));
    write_remote(
        &source,
        &db,
        &[
            snapshot("e1", "First", 1, 1_000),
            snapshot("e2", "Second", 1, 1_000),
        ],
    );
    source.init(&db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();
    git_repo(source.vault_file_manager().vault_dir().to_path_buf())
        .push(None, None)
        .unwrap();

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    target.import_from_repo(&db, &[]).unwrap();

    let broken = target
        .vault_file_manager()
        .vault_dir()
        .join("entries/e2.enc");
    let corrupted = b"not a valid entry file".to_vec();
    std::fs::write(&broken, &corrupted).unwrap();

    match target.pull(&db).unwrap() {
        SyncResult::UnreadableEntries { ids, reason } => {
            assert_eq!(
                ids,
                vec!["e2".to_string()],
                "pull must name the corrupt entry"
            );
            assert_eq!(reason, UnreadableReason::Corrupt);
        }
        other => panic!("expected UnreadableEntries from pull, got {:?}", other),
    }
    assert_eq!(
        std::fs::read(&broken).unwrap(),
        corrupted,
        "pull must not overwrite the corrupt file"
    );

    match target.sync_now(&db).unwrap() {
        SyncResult::UnreadableEntries { ids, reason } => {
            assert_eq!(
                ids,
                vec!["e2".to_string()],
                "sync_now must name the corrupt entry"
            );
            assert_eq!(reason, UnreadableReason::Corrupt);
        }
        other => panic!("expected UnreadableEntries from sync_now, got {:?}", other),
    }
    assert_eq!(
        std::fs::read(&broken).unwrap(),
        corrupted,
        "sync_now must not export over the corrupt file"
    );
    cleanup(&db_path);
}

#[test]
fn pull_reports_key_mismatch_and_keeps_remote_ciphertext() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);

    let other_path = temp_db_path();
    let other = open_db_with_password(&other_path, OTHER_PASSWORD);

    let source = engine(&dir.path().join("source"));
    write_remote(&source, &other, &[snapshot("e1", "First", 1, 1_000)]);
    source.init(&other).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();
    git_repo(source.vault_file_manager().vault_dir().to_path_buf())
        .push(None, None)
        .unwrap();

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    let entry_path = target
        .vault_file_manager()
        .vault_dir()
        .join("entries/e1.enc");
    let before = std::fs::read(&entry_path).unwrap();

    let err = target.pull(&db).unwrap_err();
    assert!(
        matches!(err, SyncError::VaultKeyMismatch(_)),
        "expected VaultKeyMismatch, got {:?}",
        err
    );
    assert_eq!(std::fs::read(&entry_path).unwrap(), before);
    cleanup(&db_path);
    cleanup(&other_path);
}

fn stage_conflict(dir: &Path, db: &SecurityDbState) -> SyncEngine {
    let remote = bare_repo(dir);
    let source = engine(&dir.join("source"));
    write_remote(&source, db, &[snapshot("e1", "Base", 1, 1_000)]);
    source.init(db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();

    let target = engine(&dir.join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    write_remote(&target, db, &[snapshot("e1", "Remote side", 2, 2_000)]);
    commit_repo(&target, "vault: update e1");
    target.finalize("vault: update e1").unwrap();

    write_remote(&source, db, &[snapshot("e1", "Local side", 2, 3_000)]);
    commit_repo(&source, "vault: update e1");
    source.import_from_repo(db, &[]).unwrap();
    assert!(matches!(source.pull(db).unwrap(), SyncResult::Conflict(_)));
    source
}

#[test]
fn resolved_use_remote_does_not_regress_on_next_sync() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let source = stage_conflict(dir.path(), &db);

    source
        .resolve_conflict(&db, "entries/e1.enc", ConflictResolution::UseRemote, None)
        .unwrap();
    assert_eq!(db.get_entry("e1").unwrap().title, "Remote side");
    let remaining = source.conflicts(&db).unwrap();
    assert!(remaining.is_empty(), "remaining conflicts: {:?}", remaining);

    assert!(matches!(
        source.sync_now(&db).unwrap(),
        SyncResult::Success | SyncResult::NoRemote
    ));
    assert_eq!(db.get_entry("e1").unwrap().title, "Remote side");

    let content = std::fs::read_to_string(
        source
            .vault_file_manager()
            .vault_dir()
            .join("entries/e1.enc"),
    )
    .unwrap();
    let guard = db.get_master_key().unwrap();
    let decoded = source
        .vault_file_manager()
        .decode_entry_file(guard.as_ref().unwrap(), &content)
        .unwrap();
    assert_eq!(decoded.blob.title, "Remote side");
    cleanup(&db_path);
}

#[test]
fn resolved_merge_both_creates_remote_copy_entry() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let source = stage_conflict(dir.path(), &db);

    source
        .resolve_conflict(
            &db,
            "entries/e1.enc",
            ConflictResolution::MergeBoth,
            Some(" (remote)"),
        )
        .unwrap();

    let ids = db.list_entry_ids(false).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(db.get_entry("e1").unwrap().title, "Local side");
    let copy_id = ids.iter().find(|id| id.as_str() != "e1").unwrap();
    assert_eq!(db.get_entry(copy_id).unwrap().title, "Remote side (remote)");
    assert!(source.conflicts(&db).unwrap().is_empty());
    cleanup(&db_path);
}

#[test]
fn conflict_preview_reports_only_the_undecryptable_side() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);

    let other_path = temp_db_path();
    let other = open_db_with_password(&other_path, OTHER_PASSWORD);

    let source = engine(&dir.path().join("source"));
    write_remote(&source, &db, &[snapshot("e1", "Base", 1, 1_000)]);
    source.init(&db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();
    source.finalize("vault: base").unwrap();

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    let other_key = other.get_master_key().unwrap();
    target
        .vault_file_manager()
        .export_entry(
            other_key.as_ref().unwrap(),
            &snapshot("e1", "Remote side", 2, 2_000),
            false,
        )
        .unwrap();
    drop(other_key);
    let guard = db.get_master_key().unwrap();
    target
        .vault_file_manager()
        .export_vault_meta(guard.as_ref().unwrap(), None, None)
        .unwrap();
    drop(guard);
    commit_repo(&target, "vault: update e1");
    target.finalize("vault: update e1").unwrap();

    write_remote(&source, &db, &[snapshot("e1", "Local side", 2, 3_000)]);
    commit_repo(&source, "vault: update e1");
    assert!(matches!(source.pull(&db).unwrap(), SyncResult::Conflict(_)));

    let preview = source.conflict_preview(&db, "entries/e1.enc").unwrap();
    assert!(preview.local_error.is_none());
    assert!(preview.remote_error.is_some());
    assert!(preview.local.is_some());
    assert!(preview.remote.is_none());
    cleanup(&db_path);
    cleanup(&other_path);
}

#[test]
fn force_export_rewrites_entries_after_master_password_change() {
    let _rekey = sync_common::rekey_guard();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    let created = db
        .create_entry_tx(local_entry("Rekey"), CipherAlgorithm::Aes256Gcm, |_, _| {
            Ok(())
        })
        .unwrap();
    let path = engine
        .vault_file_manager()
        .vault_dir()
        .join(format!("entries/{}.enc", created.id));

    engine.export_all_entries(&db, false, &[]).unwrap();
    let before = std::fs::read(&path).unwrap();

    engine.export_all_entries(&db, false, &[]).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "unchanged revision must be skipped"
    );

    db.change_master_password(PASSWORD, "N3w#P@ssw0rd!2026", CipherAlgorithm::Aes256Gcm)
        .unwrap();
    engine.export_all_entries(&db, false, &[]).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "non-force export must skip entries whose revision did not change"
    );

    engine.export_all_entries(&db, true, &[]).unwrap();
    assert_ne!(
        std::fs::read(&path).unwrap(),
        before,
        "force export must rewrite ciphertext with the new master key"
    );
    cleanup(&db_path);
}

#[test]
fn push_bootstraps_upstream_and_commits_dirty_changes() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let source = engine(&dir.path().join("source"));

    let created = db
        .create_entry_tx(local_entry("Pushed"), CipherAlgorithm::Aes256Gcm, |_, _| {
            Ok(())
        })
        .unwrap();

    source.init(&db).unwrap();
    source
        .ensure_remote("origin", remote.to_str().unwrap())
        .unwrap();

    assert!(matches!(source.push(&db).unwrap(), SyncResult::Success));

    let upstream = assistant_sync::git_sync::git::git_branch_upstream(
        source.vault_file_manager().vault_dir(),
        "HEAD",
    )
    .unwrap();
    assert!(
        upstream.is_some(),
        "push must bootstrap the upstream branch on first use"
    );

    let target = engine(&dir.path().join("target"));
    target.clone_repo(remote.to_str().unwrap()).unwrap();
    assert_eq!(
        target.vault_file_manager().list_entry_files().unwrap(),
        vec![created.id.clone()],
        "push must export and commit local DB entries before pushing"
    );
    cleanup(&db_path);
}

#[test]
fn sync_now_without_remote_commits_local_changes() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());

    let created = db
        .create_entry_tx(
            local_entry("Local only"),
            CipherAlgorithm::Aes256Gcm,
            |_, _| Ok(()),
        )
        .unwrap();

    engine.init(&db).unwrap();
    assert!(matches!(
        engine.sync_now(&db).unwrap(),
        SyncResult::NoRemote
    ));
    assert_eq!(engine.readyness().unwrap(), Readyness::NoRemote);
    assert!(
        engine
            .vault_file_manager()
            .entry_file_path(&created.id)
            .exists(),
        "sync without a remote must still export and commit local changes"
    );
    cleanup(&db_path);
}

#[test]
fn pull_failure_is_not_conflict() {
    sync_common::git_env();
    let dir = tempfile::tempdir().unwrap();
    let db_path = temp_db_path();
    let db = open_db(&db_path);
    let engine = engine(dir.path());
    engine.init(&db).unwrap();

    let missing = dir.path().join("missing.git");
    engine
        .ensure_remote("origin", missing.to_str().unwrap())
        .unwrap();

    let err = engine.pull(&db).unwrap_err();
    assert!(
        matches!(err, SyncError::GitFailed { .. }),
        "network/credential failures must not be reported as conflicts, got {err:?}"
    );
    let dto = VaultErrorDto::from(err);
    assert_eq!(dto.code, VaultErrorCode::GitUnavailable);
    cleanup(&db_path);
}
