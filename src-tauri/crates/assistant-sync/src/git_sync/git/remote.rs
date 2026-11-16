use std::path::Path;

use crate::types::SyncError;

use super::plumbing::{git, git_output, git_stdout_ok, sanitize_git_stderr};

pub fn git_push(
    repo: &Path,
    set_branch: Option<&str>,
    set_upstream: Option<&str>,
) -> Result<(), SyncError> {
    let mut args = vec!["push", "-q"];
    if let Some(upstream) = set_upstream {
        args.extend_from_slice(&["--set-upstream", upstream]);
    }
    if let Some(branch) = set_branch {
        args.push(branch);
    }
    git(repo, &args, true)
}

pub fn git_pull(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["pull", "-q"], true)
}

pub fn git_fetch(repo: &Path, reference: Option<&str>) -> Result<(), SyncError> {
    let mut args = vec!["fetch", "-q"];
    if let Some(reference) = reference {
        args.push(reference);
    }
    git(repo, &args, true)
}

pub fn git_has_remote(repo: &Path) -> Result<bool, SyncError> {
    Ok(!git_stdout_ok(repo, ["remote"], false)?.is_empty())
}

pub fn git_remote(repo: &Path) -> Result<Vec<String>, SyncError> {
    Ok(git_stdout_ok(repo, ["remote"], false)?
        .lines()
        .map(|r| r.to_string())
        .collect())
}

pub fn git_remote_get_url(repo: &Path, remote: &str) -> Result<String, SyncError> {
    git_stdout_ok(repo, ["remote", "get-url", remote], false)
}

pub fn git_remote_add(repo: &Path, remote: &str, url: &str) -> Result<(), SyncError> {
    git(repo, ["remote", "add", remote, url], false)
}

pub fn git_remote_remove(repo: &Path, remote: &str) -> Result<(), SyncError> {
    git(repo, ["remote", "remove", remote], false)
}

pub fn git_ls_remote(repo: &Path, url: &str) -> Result<String, SyncError> {
    let output = git_output(repo, ["ls-remote", "--heads", url], true)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(SyncError::GitCli(if stderr.is_empty() {
            format!(
                "ls-remote exited with status {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            sanitize_git_stderr(repo, &stderr)
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
