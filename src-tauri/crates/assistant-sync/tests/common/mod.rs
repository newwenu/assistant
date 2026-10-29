#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn git_env() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        std::env::set_var("GIT_AUTHOR_NAME", "Vault Test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "vault-test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Vault Test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "vault-test@example.com");
    });
}

pub fn bare_repo(dir: &Path) -> PathBuf {
    git_env();
    let path = dir.join("remote.git");
    let status = Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    path
}
