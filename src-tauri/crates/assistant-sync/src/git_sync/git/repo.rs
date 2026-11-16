use std::path::Path;

use crate::types::{RepositoryState, SyncError};

use super::plumbing::{git, git_stdout_ok, git_stdout_ok_or, GIT_EXIT_STATUS_NOT_FOUND};

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

pub fn git_has_changes(repo: &Path) -> Result<bool, SyncError> {
    Ok(!git_stdout_ok(repo, ["status", "-s"], false)?.is_empty())
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

pub fn git_show_file(repo: &Path, commit: &str, path: &str) -> Result<String, SyncError> {
    git_stdout_ok(repo, ["show", &format!("{}:{}", commit, path)], false)
}

pub fn git_add(repo: &Path, path: &str) -> Result<(), SyncError> {
    git(repo, ["add", path], false)
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
