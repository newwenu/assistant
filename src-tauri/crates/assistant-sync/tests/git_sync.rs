#![cfg(feature = "git-tests")]

mod common;

use std::path::PathBuf;
use std::process::Command;

use assistant_sync::git_sync::git::git_ref_hash;
use assistant_sync::git_sync::ssh::resolve_persist_hosts;
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{Readyness, SyncConfig};
use common::{bare_repo, git_env};

fn sync(repo: PathBuf) -> GitSync {
    GitSync::new(repo, SyncConfig::default())
}

#[test]
fn readyness_is_no_sync_before_init_and_ready_after() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));

    assert_eq!(git.readyness().unwrap(), Readyness::NoSync);

    git.init().unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Ready);
}

#[test]
fn dirty_after_change_and_ready_after_commit() {
    git_env();
    let dir = tempfile::tempdir().unwrap();
    let git = sync(dir.path().join("vault"));
    git.init().unwrap();

    std::fs::create_dir_all(git.path().join("entries")).unwrap();
    std::fs::write(git.path().join("entries").join("a.enc"), b"payload").unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Dirty);

    git.commit_all("vault: add a", false).unwrap();
    assert_eq!(git.readyness().unwrap(), Readyness::Ready);
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

    assert_eq!(git.readyness().unwrap(), Readyness::Ready);
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
