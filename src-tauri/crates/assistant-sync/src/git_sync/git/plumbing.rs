use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use crate::types::SyncError;

#[cfg(not(windows))]
pub const BIN_NAME: &str = "git";
#[cfg(windows)]
pub const BIN_NAME: &str = "git.exe";

pub(super) const GIT_EXIT_STATUS_NOT_FOUND: i32 = 1;

const MAX_GIT_STDERR_CHARS: usize = 2000;

pub(crate) fn git<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<(), SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo, args, connects_remote)?;
    cmd_assert_status(&output, repo)
}

/// Git stderr 脱敏：URL userinfo → 去掉 `user:token@`，仓库绝对路径 → `<vault>`，并截断。
pub fn sanitize_git_stderr(repo: &Path, raw: &str) -> String {
    let text = match repo.to_str() {
        Some(path) => {
            let trimmed = path.trim_end_matches(['/', '\\']);
            if trimmed.is_empty() {
                raw.trim().to_string()
            } else {
                [trimmed.to_string(), trimmed.replace('\\', "/")]
                    .into_iter()
                    .fold(raw.trim().to_string(), |text, variant| {
                        text.replace(&variant, "<vault>")
                    })
            }
        }
        None => raw.trim().to_string(),
    };
    crate::redact::redact_url_userinfo(&text)
        .chars()
        .take(MAX_GIT_STDERR_CHARS)
        .collect()
}

pub(super) fn git_output<I, S>(
    repo: &Path,
    args: I,
    connects_remote: bool,
) -> Result<Output, SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    cmd_git(args, repo, connects_remote)
        .output()
        .map_err(|e| SyncError::Io(e.to_string()))
}

pub(super) fn git_stdout_ok_or<I, S>(
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
        cmd_assert_status(&output, repo)?;
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn git_stdout_ok<I, S>(
    repo: &Path,
    args: I,
    connects_remote: bool,
) -> Result<String, SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo, args, connects_remote)?;
    cmd_assert_status(&output, repo)?;
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
    cmd.env("GCM_INTERACTIVE", "never");

    if connects_remote {
        cmd.arg("-c").arg("http.timeout=30");
        cmd.arg("-c").arg("http.lowSpeedLimit=1000");
        cmd.arg("-c").arg("http.lowSpeedTime=60");
        crate::git_sync::ssh::configure_ssh_persist(&mut cmd, dir);
    }

    cmd.args(args);
    cmd
}

pub(super) fn cmd_assert_status(output: &Output, repo: &Path) -> Result<(), SyncError> {
    if !output.status.success() {
        return Err(SyncError::GitFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: sanitize_git_stderr(repo, &String::from_utf8_lossy(&output.stderr)),
        });
    }
    Ok(())
}

#[cfg(feature = "git-tests")]
#[doc(hidden)]
pub fn cmd_git_for_test<I, S>(args: I, dir: &Path, connects_remote: bool) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    cmd_git(args, dir, connects_remote)
}
