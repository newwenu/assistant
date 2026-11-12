use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};
use std::time::SystemTime;

use crate::types::{RepositoryState, SyncError};

#[cfg(not(windows))]
pub const BIN_NAME: &str = "git";
#[cfg(windows)]
pub const BIN_NAME: &str = "git.exe";

const GIT_FETCH_HEAD_FILE: &str = ".git/FETCH_HEAD";
const GIT_EXIT_STATUS_NOT_FOUND: i32 = 1;

pub fn git_init(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["init", "-q"], false)?;
    disable_eol_conversion(repo)
}

pub fn git_clone(repo: &Path, url: &str, path: &str, quiet: bool) -> Result<(), SyncError> {
    let mut args = vec![
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.safecrlf=false",
        "clone",
        "-q",
    ];
    if !quiet {
        args.push("--progress");
    }
    args.extend_from_slice(&[url, path]);
    git(repo, &args, true)?;
    disable_eol_conversion(Path::new(path))
}

fn disable_eol_conversion(repo: &Path) -> Result<(), SyncError> {
    git(repo, ["config", "core.autocrlf", "false"], false)?;
    git(repo, ["config", "core.safecrlf", "false"], false)
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

pub fn git_unmerged_paths(repo: &Path) -> Result<Vec<String>, SyncError> {
    Ok(
        git_stdout_ok(repo, ["diff", "--name-only", "--diff-filter=U"], false)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.to_string())
            .collect(),
    )
}

pub fn git_conflict_stage_hashes(repo: &Path, path: &str) -> Result<(String, String), SyncError> {
    let output = git_stdout_ok(repo, ["ls-files", "-u", "--", path], false)?;
    let mut local = String::new();
    let mut remote = String::new();

    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let _mode = parts.next();
        let hash = parts.next().unwrap_or_default();
        let stage = parts.next().unwrap_or_default();
        match stage {
            "2" => local = hash.to_string(),
            "3" => remote = hash.to_string(),
            _ => {}
        }
    }

    Ok((local, remote))
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

pub fn git_abort(repo: &Path, state: RepositoryState) -> Result<(), SyncError> {
    let sub = match state {
        RepositoryState::Merge => "merge",
        RepositoryState::Rebase
        | RepositoryState::RebaseInteractive
        | RepositoryState::RebaseMerge => "rebase",
        RepositoryState::CherryPick => "cherry-pick",
        RepositoryState::Revert => "revert",
        RepositoryState::Apply | RepositoryState::Am => "am",
        _ => {
            return Err(SyncError::GitCli(
                "Repository is not in an abortable state".to_string(),
            ))
        }
    };
    git(repo, [sub, "--abort"], false)
}

pub fn git_checkout_ours(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["checkout", "--ours", path], false)
}

pub fn git_checkout_theirs(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["checkout", "--theirs", path], false)
}

pub fn git_checkout_detached(repo: &Path, commit: &str) -> Result<(), SyncError> {
    git(repo, ["checkout", "--detach", commit], false)
}

/// 返回 `(作者, 提交时间)`，其中提交时间为 **unix 毫秒**。
///
/// Git 的 `%ct` 只有秒精度，此处解析后统一 ×1000 对齐 crate 内的毫秒约定，
/// 因此毫秒值低三位恒为 `000`。
pub fn git_last_commit_info(
    repo: &Path,
    reference: Option<&str>,
    path: &str,
) -> Result<(String, i64), SyncError> {
    let mut args = vec![
        "log".to_string(),
        "-1".to_string(),
        "--format=%an%x1f%ct".to_string(),
    ];
    if let Some(reference) = reference {
        args.push(reference.to_string());
    }
    args.push("--".to_string());
    args.push(path.to_string());

    let output = git_output(repo, &args, false)?;
    if !output.status.success() {
        return Err(SyncError::Other(format!(
            "git log failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    let mut parts = line.split('\u{1f}');
    let author = parts.next().unwrap_or("").trim().to_string();
    let timestamp = parts
        .next()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(0);

    if author.is_empty() {
        return Err(SyncError::Other(format!(
            "No commit history found for {}",
            path
        )));
    }
    Ok((author, timestamp * 1000))
}

pub struct RawLogEntry {
    pub hash: String,
    /// unix 毫秒（由 Git `%ct` 秒值 ×1000，低三位恒为 `000`）
    pub timestamp: i64,
    pub message: String,
    pub paths: Vec<String>,
}

pub fn git_log(
    repo: &Path,
    path: Option<&str>,
    limit: usize,
) -> Result<Vec<RawLogEntry>, SyncError> {
    let mut args = vec![
        "log".to_string(),
        format!("-n{}", limit),
        "--format=%H%x1f%ct%x1f%s".to_string(),
        "--name-only".to_string(),
    ];
    if let Some(path) = path {
        args.push("--".to_string());
        args.push(path.to_string());
    }

    let output = git_output(repo, &args, false)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("does not have any commits") {
            return Ok(Vec::new());
        }
        return Err(SyncError::GitFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: sanitize_git_stderr(repo, &stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<RawLogEntry> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains('\u{1f}') {
            let mut parts = trimmed.split('\u{1f}');
            let hash = parts.next().unwrap_or_default().to_string();
            let timestamp = parts.next().unwrap_or("0").parse::<i64>().unwrap_or(0) * 1000;
            let message = parts.next().unwrap_or_default().to_string();
            entries.push(RawLogEntry {
                hash,
                timestamp,
                message,
                paths: Vec::new(),
            });
        } else if let Some(last) = entries.last_mut() {
            last.paths.push(trimmed.to_string());
        }
    }
    Ok(entries)
}

pub fn git_show_file(repo: &Path, commit: &str, path: &str) -> Result<String, SyncError> {
    git_stdout_ok(repo, ["show", &format!("{}:{}", commit, path)], false)
}

pub fn git_add(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["add", path], false)
}

pub(crate) fn git<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<(), SyncError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo, args, connects_remote)?;
    cmd_assert_status(&output, repo)
}

/// 返回 `(本地领先, 远程领先)`，对应 `git rev-list --left-right --count A...B`。
pub fn git_rev_list_left_right_count(repo: &Path, range: &str) -> Result<(u32, u32), SyncError> {
    let out = git_stdout_ok(repo, ["rev-list", "--left-right", "--count", range], false)?;
    let mut parts = out.split_whitespace();
    match (
        parts.next().and_then(|v| v.parse::<u32>().ok()),
        parts.next().and_then(|v| v.parse::<u32>().ok()),
    ) {
        (Some(left), Some(right)) => Ok((left, right)),
        _ => Err(SyncError::GitCli(format!(
            "git rev-list returned unexpected output: {out}"
        ))),
    }
}

const MAX_GIT_STDERR_CHARS: usize = 2000;

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
        cmd_assert_status(&output, repo)?;
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_stdout_ok<I, S>(repo: &Path, args: I, connects_remote: bool) -> Result<String, SyncError>
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

fn cmd_assert_status(output: &Output, repo: &Path) -> Result<(), SyncError> {
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
