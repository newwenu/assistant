use std::path::Path;
use std::time::SystemTime;

use crate::types::SyncError;

use super::plumbing::{git_output, sanitize_git_stderr};

const GIT_FETCH_HEAD_FILE: &str = ".git/FETCH_HEAD";

pub fn git_last_pull_time(repo: &Path) -> Result<SystemTime, SyncError> {
    repo.join(GIT_FETCH_HEAD_FILE)
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| SyncError::Io(e.to_string()))
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
