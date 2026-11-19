use std::io::Write as _;
use std::path::{Path, PathBuf};

use encoding_rs::{Encoding, UTF_8};

use crate::context::ToolUseContext;
use crate::os::agent_read::file_revision;
use crate::os::agent_write::MAX_TRANSCODE_BYTES;
use crate::os::path;
use crate::os::text_encoding::{detect_and_decode, encode_text, writes_bom};

use super::locate::{apply_hits, effective_replacement, locate};
use super::report::{ambiguous_message, diff_text, not_found_message};
use super::text::line_number_at;
use super::EditRequest;

/// `edit` 的整文件体积上限：匹配、行尾归一、整篇转码写回都需要整文件文本。
///
/// `read` 已改为分块流式（`read` §3.4），不再受此限制；`grep` 有同值的单文件闸门，
/// 但两者语义不同（`grep` 跳过并计数，`edit` 直接拒绝）。
pub const MAX_WHOLE_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// `edit` 的体积超限文案：只说 `edit` 的约束，不提 `read` / `grep`。
///
/// 提到「`read` 能读」会让调用方以为换个工具就能编辑；`edit` 必须整文件驻留，
/// 超限文件的唯一出路是拆分（工具集内没有外部转码手段，I38c §4.2）。
pub fn file_too_large_message(path: &Path, bytes: u64) -> String {
    format!(
        "'{}' is {} bytes, exceeding the {} byte limit for edit; edit needs the whole file in \
         memory (matching, line-ending normalization, transcoding), so split it into smaller files",
        path.display(),
        bytes,
        MAX_WHOLE_FILE_BYTES
    )
}

/// 一次编辑的结果。行号均以替换前的文件为准，`match_count` 为实际替换次数。
pub(super) struct EditOutcome {
    pub(super) start_line: usize,
    pub(super) old_end_line: usize,
    pub(super) new_end_line: usize,
    pub(super) match_count: usize,
    pub(super) tier: &'static str,
    pub(super) diff: String,
}

/// 同步探测目标：解析路径、确认是普通文件、取当前修订号。必须在 `spawn_blocking` 内调用。
pub(super) fn probe_target(
    request: &EditRequest,
    context: &ToolUseContext,
) -> Result<(PathBuf, String), String> {
    let resolved = path::resolve(&request.path, context)?;
    let metadata = match std::fs::metadata(&resolved) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "'{}' does not exist; create it with the write tool first",
                resolved.display()
            ))
        }
        Err(error) => return Err(format!("cannot access '{}': {}", resolved.display(), error)),
    };
    if metadata.is_dir() {
        return Err(format!(
            "'{}' is a directory; edit works on files only",
            resolved.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("'{}' is not a regular file", resolved.display()));
    }
    // `edit` 独有的闸门：`read` 已改为分块流式，不再有整文件上限。
    if metadata.len() > MAX_WHOLE_FILE_BYTES {
        return Err(file_too_large_message(&resolved, metadata.len()));
    }
    let revision = file_revision(&resolved)?;
    Ok((resolved, revision))
}

/// 一次编辑的写回结果：报告数据 + 实际落盘的编码状态。
pub(super) struct EditWrite {
    pub(super) outcome: EditOutcome,
    pub(super) revision: String,
    pub(super) encoding: &'static Encoding,
    pub(super) has_bom: bool,
}

/// 同步编辑并写回。必须在 `spawn_blocking` 内调用。
///
/// `encoding` / `has_bom` 来自 `read` 的回执（上级 §14.8）：写侧只消费、不重判，
/// 因此同一文件不会在 `read` 与 `edit` 上得出两种编码结论。回执只作为解码闸门——
/// 字节与回执编码不符时解码失败即拒绝；BOM 按回执 `has_bom` 写回（§14.2 唯一发现点）。
///
/// `escalation` 只影响定位失败时的诊断文案（是否附「可申请按行地址替换」的指引），
/// 不影响写入行为（I58 §3.6）。
pub(super) fn apply_edit(
    path: &Path,
    request: &EditRequest,
    encoding: &'static Encoding,
    has_bom: bool,
    escalation: bool,
) -> Result<EditWrite, String> {
    let path_text = path.display().to_string();
    let raw =
        std::fs::read(path).map_err(|error| format!("cannot read '{}': {}", path_text, error))?;
    let decoded = detect_and_decode(&raw, Some(encoding))
        .map_err(|error| format!("'{}': {}", path_text, error))?;
    let write_encoding = decoded.encoding;
    let write_has_bom = writes_bom(write_encoding, has_bom || decoded.has_bom);
    let text = decoded.text;

    let uses_crlf = text.contains("\r\n");
    let content = if uses_crlf {
        text.replace("\r\n", "\n")
    } else {
        text
    };

    let located = match locate(&content, &request.old_string, &request.new_string) {
        Some(located) => located,
        None => {
            return Err(not_found_message(
                &path_text,
                &content,
                &request.old_string,
                escalation,
            ))
        }
    };

    if located.hits.len() > 1 && !request.replace_all {
        return Err(ambiguous_message(&path_text, &content, &located.hits));
    }

    let changed = located.hits.iter().any(|hit| {
        &content[hit.0..hit.1] != effective_replacement(&content, *hit, &located).as_str()
    });
    if !changed {
        let start_line = line_number_at(&content, located.hits[0].0);
        return Err(format!(
            "'old_string' at line {} of '{}' already equals 'new_string'; nothing to change",
            start_line, path_text
        ));
    }

    let updated = apply_hits(&content, &located.hits, &located);
    let payload = if uses_crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    let bytes = encode_payload(&path_text, &payload, write_encoding, write_has_bom)?;
    persist_atomically(path, &bytes)?;
    let revision = file_revision(path)?;

    let first = located.hits[0];
    let start_line = line_number_at(&content, first.0);
    let replacement = effective_replacement(&content, first, &located);
    let old_lines = content[first.0..first.1].lines().count().max(1);
    let new_lines = replacement.lines().count().max(1);

    Ok(EditWrite {
        outcome: EditOutcome {
            start_line,
            old_end_line: start_line + old_lines - 1,
            new_end_line: start_line + new_lines - 1,
            match_count: located.hits.len(),
            tier: located.tier,
            diff: diff_text(&content[first.0..first.1], &replacement),
        },
        revision,
        encoding: write_encoding,
        has_bom: write_has_bom,
    })
}

/// 编码写回（G2 / G3 / G5）。
///
/// UTF-8 无 BOM 走直写，与 P5 行为逐字节一致；其余编码整体重编码，有损即拒绝
/// （禁止 `?` 替换式写回），且受转码上限约束（§14.6 G3 / OQ7）。
fn encode_payload(
    path_text: &str,
    payload: &str,
    encoding: &'static Encoding,
    has_bom: bool,
) -> Result<Vec<u8>, String> {
    if encoding == UTF_8 && !has_bom {
        return Ok(payload.as_bytes().to_vec());
    }

    let bytes = encode_text(payload, encoding, has_bom)
        .map_err(|error| format!("'{}': {}", path_text, error))?;

    if bytes.len() > MAX_TRANSCODE_BYTES {
        return Err(format!(
            "transcoded write is {} bytes, exceeding the {} byte limit for transcoded writes",
            bytes.len(),
            MAX_TRANSCODE_BYTES
        ));
    }

    Ok(bytes)
}

fn persist_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", path.display()))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "create temp file in '{}' failed: {}",
            parent.display(),
            error
        )
    })?;
    temp.write_all(bytes)
        .map_err(|error| format!("write temp file failed: {}", error))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("sync temp file failed: {}", error))?;
    temp.persist(path)
        .map_err(|error| format!("persist '{}' failed: {}", path.display(), error))?;
    Ok(())
}
