use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::time::SystemTime;

use crate::types::{RepositoryState, SyncError};

#[cfg(not(windows))]
pub const BIN_NAME: &str = "git";
#[cfg(windows)]
pub const BIN_NAME: &str = "git.exe";

const GIT_FETCH_HEAD_FILE: &str = ".git/FETCH_HEAD";
const GIT_EXIT_STATUS_NOT_FOUND: i32 = 1;

pub fn git_init(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["init", "-q"], false)
}

pub fn git_clone(repo: &Path, url: &str, path: &str, quiet: bool) -> Result<(), SyncError> {
    let mut args = vec!["clone", "-q"];
    if !quiet {
        args.push("--progress");
    }
    args.extend_from_slice(&[url, path]);
    git(repo, &args, true)
}

pub fn git_add_all(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["add", "."], false)
}

pub fn git_commit(repo: &Path, msg: &str, commit_empty: bool) -> Result<(), SyncError> {
    if !commit_empty && !git_has_changes(repo)? {
        return Ok(());
    }
    let mut args = vec!["commit", "-q", "--no-edit", "-m", msg];
    if commit_empty {
        args.push("--allow-empty");
    }
    git(repo, &args, false)
}

pub fn git_reset_hard(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["reset", "--hard", "-q"], false)
}

pub fn git_status(repo: &Path, short: bool) -> Result<String, SyncError> {
    let mut args = vec!["status"];
    if short {
        args.push("--short");
    }
    git_stdout_ok(repo, &args, false)
}

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

pub fn git_has_changes(repo: &Path) -> Result<bool, SyncError> {
    Ok(!git_stdout_ok(repo, ["status", "-s"], false)?.is_empty())
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

    cmd_assert_status(output.status)?;

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
    if hash.len() != 40 {
        return Err(SyncError::GitCli(format!(
            "git returned invalid hash: {}",
            hash
        )));
    }
    Ok(hash)
}

pub fn git_last_pull_time(repo: &Path) -> Result<SystemTime, SyncError> {
    repo.join(GIT_FETCH_HEAD_FILE)
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| SyncError::Io(e.to_string()))
}

pub fn git_state(repo: &Path) -> Result<RepositoryState, SyncError> {
    let state_str = git_stdout_ok_or(
        repo,
        ["rev-parse", "--git-dir"],
        false,
        GIT_EXIT_STATUS_NOT_FOUND,
    )?;

    let git_dir = repo.join(&state_str);

    if git_dir.join("MERGE_HEAD").exists() {
        return Ok(RepositoryState::Merge);
    }
    if git_dir.join("rebase-merge").exists() {
        return Ok(RepositoryState::RebaseInteractive);
    }
    if git_dir.join("rebase-apply").exists() {
        if git_dir.join("rebase-apply/applying").exists() {
            return Ok(RepositoryState::Apply);
        }
        return Ok(RepositoryState::Rebase);
    }
    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Ok(RepositoryState::CherryPick);
    }
    if git_dir.join("REVERT_HEAD").exists() {
        return Ok(RepositoryState::Revert);
    }
    if git_dir.join("BISECT_LOG").exists() {
        return Ok(RepositoryState::Bisect);
    }
    if git_dir.join("am").exists() {
        return Ok(RepositoryState::Am);
    }
    if git_dir.join("MERGE_MSG").exists() && git_dir.join("rebase-apply").exists() {
        return Ok(RepositoryState::RebaseMerge);
    }

    Ok(RepositoryState::Clean)
}

pub fn git_checkout_ours(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["checkout", "--ours", path], false)
}

pub fn git_checkout_theirs(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["checkout", "--theirs", path], false)
}

pub fn git_add(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["add", path], false)
}

pub(crate) fn git<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<(), SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    cmd_assert_status(
        cmd_git(args, repo, connects_remote)
            .status()
            .map_err(|e| SyncError::Io(e.to_string()))?,
    )
}

fn git_output<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<Output, SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    cmd_git(args, repo, connects_remote)
        .output()
        .map_err(|e| SyncError::Io(e.to_string()))
}

fn git_stdout_ok_or<I, S>(
    repo: &Path,
    args: I,
    connects_remote: bool,
    allow_exit_code: i32,
) -> Result<String, SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo, args, connects_remote)?;
    if output.status.code() != Some(allow_exit_code) {
        cmd_assert_status(output.status)?;
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_stdout_ok<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<String, SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo, args, connects_remote)?;
    cmd_assert_status(output.status)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn cmd_git<I, S>(args: I, dir: &Path, connects_remote: bool) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(BIN_NAME);
    cmd.arg("-C");
    cmd.arg(dir);
    cmd.current_dir(dir);
    cmd.env("GIT_TERMINAL_PROMPT", "0");

    if connects_remote {
        cmd.arg("-c").arg("http.timeout=30");
        cmd.arg("-c").arg("http.lowSpeedLimit=1000");
        cmd.arg("-c").arg("http.lowSpeedTime=60");
        crate::git_sync::ssh::configure_ssh_persist(&mut cmd);
    }

    cmd.args(args);
    cmd
}

fn cmd_assert_status(status: ExitStatus) -> Result<(), SyncError> {
    if !status.success() {
        return Err(SyncError::GitStatus(status.code().unwrap_or(-1)));
    }
    Ok(())
}