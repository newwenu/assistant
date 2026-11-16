use std::io::Write as _;
use std::path::Path;

use super::encoding::{build_payload, TargetEncoding};
use super::lines::count_lines;
use super::{too_large_message, MAX_TRANSCODE_BYTES, MAX_WRITE_BYTES};

/// 一次写入的结果。`unchanged` 为真时不写盘，其余字段描述写盘后的最终文件。
pub(super) struct WriteOutcome {
    pub(super) created: bool,
    pub(super) appended: bool,
    pub(super) unchanged: bool,
    pub(super) bytes: usize,
    pub(super) lines: usize,
}

/// 同步写入，必须在 `spawn_blocking` 内调用。
///
/// 顺序固定为「读现有内容 → 过五道闸 → 判等 / 拼装 → 建父目录 → 临时文件 →
/// `sync_all` → rename」：先读是为了在不可读时提前拒绝覆盖，而不是写完才发现目标被占用。
pub(super) fn write_atomically(
    path: &Path,
    content: &str,
    append: bool,
    target: TargetEncoding,
    source: Option<&'static encoding_rs::Encoding>,
    has_receipt: bool,
) -> Result<(WriteOutcome, TargetEncoding), String> {
    let existing = read_existing(path)?;
    let created = existing.is_none();
    let existing_len = existing.as_ref().map_or(0, Vec::len);

    if !created && !target.is_plain_utf8() && !has_receipt {
        return Err(format!(
            "'{}' exists but has not been read in this session; read it first so its encoding is \
             known before writing it as {} (transcoding an unread file can corrupt its content)",
            path.display(),
            target.name()
        ));
    }

    // G3 只按「要转码的输入」计：覆盖场景现有内容整篇被替换，不计入上限。
    if !target.is_plain_utf8() {
        let transcoded = content.len() + if append { existing_len } else { 0 };
        if transcoded > MAX_TRANSCODE_BYTES {
            return Err(format!(
                "transcoded write is {} bytes ({} new + {} kept by append), exceeding the {} \
                 byte limit for transcoded writes",
                transcoded,
                content.len(),
                transcoded - content.len(),
                MAX_TRANSCODE_BYTES
            ));
        }
    }

    let payload = build_payload(path, existing.as_deref(), content, append, target, source)?;

    if let Some(bytes) = &existing {
        if bytes == &payload {
            return Ok((
                WriteOutcome {
                    created: false,
                    appended: false,
                    unchanged: true,
                    bytes: bytes.len(),
                    lines: count_lines(bytes),
                },
                target,
            ));
        }
    }

    if payload.len() > MAX_WRITE_BYTES {
        return Err(too_large_message(payload.len()));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create parent directory '{}': {}",
            parent.display(),
            error
        )
    })?;

    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "failed to create a temporary file in '{}': {}",
            parent.display(),
            error
        )
    })?;
    temp.write_all(&payload)
        .map_err(|error| format!("failed to write '{}': {}", path.display(), error))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("failed to flush '{}': {}", path.display(), error))?;
    temp.persist(path)
        .map_err(|error| format!("failed to replace '{}': {}", path.display(), error.error))?;

    Ok((
        WriteOutcome {
            created,
            appended: append && !created,
            unchanged: false,
            bytes: payload.len(),
            lines: count_lines(&payload),
        },
        target,
    ))
}

/// 读取现有文件内容：不存在返回 `None`。
///
/// 已存在但读不到（权限 / 被占用）一律拒绝：无法确认现有内容时覆盖不可逆。
fn read_existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to stat '{}': {}", path.display(), error)),
    };

    if metadata.is_dir() {
        return Err(format!(
            "'{}' is a directory; write targets files only",
            path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("'{}' is not a regular file", path.display()));
    }

    std::fs::read(path).map(Some).map_err(|error| {
        format!(
            "'{}' exists but cannot be read ({}); refusing to overwrite a file whose current \
             content cannot be confirmed",
            path.display(),
            error
        )
    })
}
