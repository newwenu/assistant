use std::path::Path;
use std::time::SystemTime;

use crate::types::{RemoteStatus, RemoteStatusErrorKind, SyncError};

use super::{git, GIT_PULL_OUTDATED};

pub(super) fn set_status_error(
    status: &mut RemoteStatus,
    repo: &Path,
    kind: RemoteStatusErrorKind,
    error: impl std::fmt::Display,
) {
    status.error_kind = Some(kind);
    status.error_detail = Some(git::sanitize_git_stderr(repo, &error.to_string()));
}

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn safe_need_to_push(repo: &Path) -> bool {
    match need_to_push(repo) {
        Ok(push) => push,
        Err(err) => {
            log::warn!(
                "Failed to test if local branch differs from remote, assuming push needed: {err}"
            );
            true
        }
    }
}

fn need_to_push(repo: &Path) -> Result<bool, SyncError> {
    let last_pulled = git::git_last_pull_time(repo)?;
    if last_pulled.elapsed().unwrap_or(GIT_PULL_OUTDATED) > GIT_PULL_OUTDATED {
        return Ok(true);
    }

    let branch = git::git_current_branch(repo)?;
    let upstream = match git::git_branch_upstream(repo, &branch)? {
        Some(u) => u,
        None => return Ok(true),
    };

    Ok(git::git_ref_hash(repo, &branch)? != git::git_ref_hash(repo, &upstream)?)
}
