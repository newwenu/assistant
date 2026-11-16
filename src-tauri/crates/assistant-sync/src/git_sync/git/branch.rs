use std::path::Path;

use crate::types::SyncError;

use super::plumbing::{
    cmd_assert_status, git, git_output, git_stdout_ok, git_stdout_ok_or, GIT_EXIT_STATUS_NOT_FOUND,
};

pub fn git_current_branch(repo: &Path) -> Result<String, SyncError> {
    let branch = git_stdout_ok(repo, ["rev-parse", "--abbrev-ref", "HEAD"], false)?;
    if branch.is_empty() || branch.contains('\n') {
        return Err(SyncError::GitCli(
            "git returned invalid branch name".to_string(),
        ));
    }
    Ok(branch)
}

pub fn git_config_branch_remote(repo: &Path, branch: &str) -> Result<Option<String>, SyncError> {
    let mut remote = git_stdout_ok_or(
        repo,
        ["config", "--get", &format!("branch.{branch}.remote")],
        false,
        GIT_EXIT_STATUS_NOT_FOUND,
    )?;

    if remote.is_empty() {
        remote = git_stdout_ok_or(
            repo,
            ["config", "--get", "remote.pushDefault"],
            false,
            GIT_EXIT_STATUS_NOT_FOUND,
        )?;
    }

    if remote.is_empty() {
        return Ok(None);
    }
    Ok(Some(remote))
}

pub fn git_config_branch_set_remote(
    repo: &Path,
    branch: &str,
    remote: &str,
) -> Result<(), SyncError> {
    git_stdout_ok(
        repo,
        ["config", &format!("branch.{branch}.remote"), remote],
        false,
    )
    .map(|_| ())
}

pub fn git_branch_remote(repo: &Path) -> Result<Vec<String>, SyncError> {
    Ok(git_stdout_ok(repo, ["branch", "-r", "--no-color"], false)?
        .lines()
        .map(|r| {
            match r.strip_prefix("* ") {
                Some(s) => s,
                None => r,
            }
            .to_string()
        })
        .collect())
}

pub fn git_branch_upstream(repo: &Path, reference: &str) -> Result<Option<String>, SyncError> {
    let output = git_output(
        repo,
        [
            "rev-parse",
            "--abbrev-ref",
            &format!("{}@{{upstream}}", reference),
        ],
        false,
    )?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("fatal: no upstream configured for branch") {
        return Ok(None);
    }

    cmd_assert_status(&output, repo)?;

    let upstream = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if upstream.is_empty() {
        return Ok(None);
    }
    Ok(Some(upstream))
}

pub fn git_branch_set_upstream(
    repo: &Path,
    reference: Option<&str>,
    upstream: &str,
) -> Result<(), SyncError> {
    let mut args = vec!["branch", "--set-upstream-to", upstream];
    if let Some(reference) = reference {
        args.push(reference);
    }
    git(repo, &args, false)
}

pub fn git_ref_hash(repo: &Path, reference: &str) -> Result<String, SyncError> {
    let hash = git_stdout_ok(repo, ["rev-parse", reference], false)?;
    let valid_len = hash.len() == 40 || hash.len() == 64;
    if !valid_len || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(SyncError::GitCli(format!(
            "git returned invalid hash: {}",
            hash
        )));
    }
    Ok(hash)
}
