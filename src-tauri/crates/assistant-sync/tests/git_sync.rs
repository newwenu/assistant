#![cfg(feature = "git-tests")]

mod common;

use std::path::PathBuf;
use std::process::Command;

use assistant_sync::git_sync::git::{
    git_current_branch, git_fetch, git_ls_remote, git_ref_hash, sanitize_git_stderr,
};
use assistant_sync::git_sync::ssh::resolve_persist_hosts;
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{Readyness, RemoteStatusErrorKind, SyncConfig, SyncError};
use common::{bare_repo, git_env};

fn sync(repo: PathBuf) -> GitSync {
    GitSync::new(repo, SyncConfig::default())
}

#[test]
fn readyness_is_no_sync_then_no_remote_after_init() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));

    assert_eq!(git.readyness().unwrap(), Readyness::NoSync);

    git.init().unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::NoRemote);
}

#[test]
fn dirty_after_change_and_ready_after_commit() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();
    git.add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();

    std::fs::create_dir_all(git.path().join("entries")).unwrap();
    std::fs::write(git.path().join("entries").join("a.enc"), b"payload").unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Dirty);

    git.commit_all("vault: add a", false).unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Ready);
}

#[test]
fn dirty_without_remote_reports_no_remote() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();

    std::fs::write(git.path().join("local.enc"), b"x").unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::NoRemote);
}

#[test]
fn ref_hash_is_full_hex_digest() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();

    let hash = git_ref_hash(git.path(), "HEAD").unwrap();
    assert!(hash.len() == 40 || hash.len() == 64, "unexpected: {hash}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn push_then_clone_roundtrip() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());

    let source = sync(dir.path().join("source"));
    source.init().unwrap();
    std::fs::write(source.path().join("entries.enc"), b"vault-data").unwrap();
    source.commit_all("vault: seed", false).unwrap();
    source
        .add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();

    let branch = assistant_sync::git_sync::git::git_current_branch(source.path()).unwrap();
    source.push(Some(&branch), Some("origin")).unwrap();

    let target = sync(dir.path().join("target"));
    target.clone(remote.to_str().unwrap(), true).unwrap();

    assert!(target.is_init());
    assert_eq!(target.readyness().unwrap(), Readyness::Ready);
    assert_eq!(
        std::fs::read_to_string(target.path().join("entries.enc")).unwrap(),
        "vault-data"
    );
}

#[test]
fn finalize_without_remote_only_commits() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();

    std::fs::write(git.path().join("local.enc"), b"x").unwrap();
    git.finalize("vault: local commit").unwrap();

    assert_eq!(git.readyness().unwrap(), Readyness::NoRemote);
    assert!(!git.has_remote().unwrap());
}

#[test]
fn ref_hash_accepts_sha256_repository() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("vault");
    std::fs::create_dir_all(&repo).unwrap();

    let status = Command::new("git")
        .args(["init", "-q", "--object-format=sha256"])
        .arg(&repo)
        .status()
        .unwrap();
    if !status.success() {
        return;
    }

    let git = sync(repo);
    std::fs::write(git.path().join("a.enc"), b"x").unwrap();
    git.commit_all("vault: sha256 repo", false).unwrap();

    let hash = git_ref_hash(git.path(), "HEAD").unwrap();
    assert_eq!(hash.len(), 64, "unexpected: {hash}");
}

#[test]
fn resolve_persist_hosts_falls_back_to_defaults_and_normalizes() {
    let defaults = vec!["github.com".to_string(), "gitlab.com".to_string()];
    assert_eq!(resolve_persist_hosts(&[]), defaults);
    assert_eq!(resolve_persist_hosts(&["   ".to_string()]), defaults);
    assert_eq!(
        resolve_persist_hosts(&[" Example.COM ".to_string(), "GitLab.com".to_string()]),
        vec!["example.com".to_string(), "gitlab.com".to_string()]
    );
}

#[test]
fn finalize_pushes_only_when_local_branch_differs_from_upstream() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();
    git.add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();
    git.finalize("vault: base").unwrap();

    let branch = assistant_sync::git_sync::git::git_current_branch(git.path()).unwrap();
    let remote_ref = format!("refs/heads/{branch}");
    let local_head = git_ref_hash(git.path(), "HEAD").unwrap();
    assert_eq!(
        local_head,
        git_ref_hash(&remote, &remote_ref).unwrap(),
        "finalize must push the initial commit"
    );

    git.finalize("vault: noop").unwrap();
    assert_eq!(
        git_ref_hash(git.path(), "HEAD").unwrap(),
        local_head,
        "finalize without local changes must not commit"
    );
    assert_eq!(
        git_ref_hash(&remote, &remote_ref).unwrap(),
        local_head,
        "finalize without divergence must not push again"
    );

    std::fs::create_dir_all(git.path().join("entries")).unwrap();
    std::fs::write(git.path().join("entries/e1.enc"), b"{}").unwrap();
    git.finalize("vault: add e1").unwrap();

    let new_head = git_ref_hash(git.path(), "HEAD").unwrap();
    assert_ne!(new_head, local_head);
    assert_eq!(
        git_ref_hash(&remote, &remote_ref).unwrap(),
        new_head,
        "finalize must push a new local commit"
    );
}

#[test]
fn detect_conflicts_reports_both_sides() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());

    let source = sync(dir.path().join("source"));
    source.init().unwrap();
    std::fs::write(source.path().join("entry.enc"), b"base").unwrap();
    source.commit_all("vault: base", false).unwrap();
    source
        .add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();
    let branch = assistant_sync::git_sync::git::git_current_branch(source.path()).unwrap();
    source.push(Some(&branch), Some("origin")).unwrap();

    let target = sync(dir.path().join("target"));
    target.clone(remote.to_str().unwrap(), true).unwrap();
    std::fs::write(target.path().join("entry.enc"), b"remote-side").unwrap();
    target.commit_all("vault: remote", false).unwrap();
    target.push(None, None).unwrap();

    std::fs::write(source.path().join("entry.enc"), b"local-side").unwrap();
    source.commit_all("vault: local", false).unwrap();

    assert!(source.pull().is_err());

    let conflicts = source.detect_conflicts().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "entry.enc");
    assert!(!conflicts[0].local_hash.is_empty());
    assert!(!conflicts[0].remote_hash.is_empty());
    assert_ne!(conflicts[0].local_hash, conflicts[0].remote_hash);
}

#[test]
fn readyness_no_remote_then_ready_after_remote_added() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let git = sync(dir.path().join("vault"));

    git.init().unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::NoRemote);

    git.add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Ready);
}

fn diverged_repos(dir: &std::path::Path) -> (GitSync, String) {
    let remote = bare_repo(dir);

    let source = sync(dir.join("source"));
    source.init().unwrap();
    std::fs::write(source.path().join("base.enc"), b"base").unwrap();
    source.commit_all("vault: base", false).unwrap();
    source
        .add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();
    let branch = git_current_branch(source.path()).unwrap();
    source.push(Some(&branch), Some("origin")).unwrap();

    let target = sync(dir.join("target"));
    target.clone(remote.to_str().unwrap(), true).unwrap();
    std::fs::write(target.path().join("remote.enc"), b"remote").unwrap();
    target.commit_all("vault: remote", false).unwrap();
    target.push(None, None).unwrap();

    std::fs::write(source.path().join("local.enc"), b"local").unwrap();
    source.commit_all("vault: local", false).unwrap();

    (source, branch)
}

#[test]
fn remote_status_ahead_behind_after_divergence() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let (source, branch) = diverged_repos(dir.path());

    let status = source.remote_status(true).unwrap();
    let expected = format!("origin/{branch}");
    assert_eq!(status.upstream.as_deref(), Some(expected.as_str()));
    assert_eq!(status.ahead, Some(1));
    assert_eq!(status.behind, Some(1));
    assert!(status.fetched);
    assert!(status.error_kind.is_none());
}

#[test]
fn remote_status_offline_uses_existing_refs() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let (source, _branch) = diverged_repos(dir.path());

    let online = source.remote_status(true).unwrap();
    let offline = source.remote_status(false).unwrap();
    assert!(!offline.fetched);
    assert_eq!(offline.ahead, online.ahead);
    assert_eq!(offline.behind, online.behind);
    assert!(offline.error_kind.is_none());
}

#[test]
fn remote_status_no_upstream_reports_error() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let remote = bare_repo(dir.path());
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();
    git.add_remote_url("origin", remote.to_str().unwrap())
        .unwrap();

    let status = git.remote_status(false).unwrap();
    assert!(status.upstream.is_none());
    assert!(status.ahead.is_none());
    assert!(status.behind.is_none());
    assert_eq!(status.error_kind, Some(RemoteStatusErrorKind::NoUpstream));
}

#[test]
fn git_failed_carries_stderr() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();

    let missing = dir.path().join("missing-remote.git");
    let err = git_fetch(git.path(), Some(missing.to_str().unwrap())).unwrap_err();
    match err {
        SyncError::GitFailed { code, stderr } => {
            assert_ne!(code, 0);
            assert!(!stderr.is_empty(), "stderr must be captured");
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
}

#[test]
fn sanitize_git_stderr_redacts_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("vault");
    let raw = "fatal: unable to access 'https://alice:secret@git.example.com/repo.git/': bad";
    let sanitized = sanitize_git_stderr(&repo, raw);
    assert!(!sanitized.contains("alice"));
    assert!(!sanitized.contains("secret"));
    assert!(sanitized.contains("https://git.example.com/repo.git/"));
}

#[test]
fn t_s12_ls_remote_failure_never_leaks_credentials() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("vault");
    std::fs::create_dir_all(&repo).unwrap();
    let url = "https://alice:secret-token@127.0.0.1:1/vault.git";

    let error = git_ls_remote(&repo, url).unwrap_err();
    let message = error.to_string();
    assert!(!message.contains("alice"), "username leaked: {message}");
    assert!(!message.contains("secret-token"), "token leaked: {message}");
}

#[test]
fn sanitize_git_stderr_replaces_repo_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("vault");
    let raw = format!("fatal: not a git repository: {}", repo.display());
    let sanitized = sanitize_git_stderr(&repo, &raw);
    assert!(sanitized.contains("<vault>"));
    assert!(!sanitized.contains(repo.to_str().unwrap()));
}

#[test]
fn sanitize_git_stderr_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let sanitized = sanitize_git_stderr(dir.path(), &"x".repeat(3000));
    assert_eq!(sanitized.chars().count(), 2000);
}
