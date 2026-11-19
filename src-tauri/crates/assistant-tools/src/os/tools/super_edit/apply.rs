//! 三段流式写回（I58 §3.3）。
//!
//! `head[0..start)` + `replacement` + `tail[end..EOF)` 顺序写进 temp file，再 `sync_all` +
//! rename 落盘。区间外的字节**逐字节原样拷贝**：BOM 留在 head 里，编码只作用于
//! `replacement` 段，因此没有整文件体积闸门，也不需要整篇转码。

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};

use crate::os::agent_read::file_revision;
use crate::os::agent_write::MAX_TRANSCODE_BYTES;

/// 一段拷贝的读缓冲：大文件也只用这么点内存。
const COPY_CHUNK_BYTES: usize = 64 * 1024;

/// 写回结果：`bytes_written` 是写回后的文件总字节数（head + replacement + tail）。
#[derive(Debug)]
pub(crate) struct WriteOutcome {
    pub bytes_written: u64,
    pub revision: String,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
}

/// 一次写回的完整参数：区间 + 替换文本 + 文件编码口径 + 定位时的修订号。
///
/// 打包成结构体而非位置参数，避免「哪个 `bool` 是 `crlf`」这类调用点歧义。
pub(crate) struct WriteRequest<'a> {
    pub start: u64,
    pub end: u64,
    pub replacement: &'a str,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub crlf: bool,
    pub expected_revision: &'a str,
}

/// 把 `replacement` 写进 `[start, end)`，区间外原样保留。必须在 `spawn_blocking` 内调用。
pub(crate) fn apply_range(path: &Path, request: WriteRequest<'_>) -> Result<WriteOutcome, String> {
    let WriteRequest {
        start,
        end,
        replacement,
        encoding,
        has_bom,
        crlf,
        expected_revision,
    } = request;
    let path_text = path.display().to_string();

    // F10：定位期间文件被外部改写，`expected` 校验的已经不是同一个文件。
    let revision = file_revision(path)?;
    if revision != expected_revision {
        return Err(format!(
            "'{}' changed between locating the range and writing it (revision {} -> {}); call \
             super_read again and retry",
            path_text, expected_revision, revision
        ));
    }

    let file_bytes = std::fs::metadata(path)
        .map_err(|error| format!("cannot stat '{}': {}", path_text, error))?
        .len();
    if end > file_bytes || start > end {
        return Err(format!(
            "'{}' shrank while being edited (target range ends at byte {}, file is {} bytes); \
             call super_read again and retry",
            path_text, end, file_bytes
        ));
    }

    let segment_text = normalize_newlines(replacement, crlf);
    let segment = encode_segment(&segment_text, encoding)?;
    if encoding != UTF_8 && segment.len() > MAX_TRANSCODE_BYTES {
        // F12：转码闸门只看替换段，不看整个文件。
        return Err(format!(
            "'replacement' is {} bytes after transcoding to {}, over the {} byte cap; shorten \
             the replacement text",
            segment.len(),
            encoding.name(),
            MAX_TRANSCODE_BYTES
        ));
    }

    let mut source = std::fs::File::open(path)
        .map_err(|error| format!("cannot read '{}': {}", path_text, error))?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("'{}' has no parent directory", path_text))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "create temp file in '{}' failed: {}",
            parent.display(),
            error
        )
    })?;

    copy_range(&mut source, temp.as_file_mut(), 0, start, &path_text)?;
    temp.write_all(&segment)
        .map_err(|error| format!("write temp file failed: {}", error))?;
    copy_range(&mut source, temp.as_file_mut(), end, file_bytes, &path_text)?;

    // Windows 的 rename 要求目标没有打开的句柄（Rust 的 `File` 不带 `FILE_SHARE_DELETE`），
    // 所以必须在落盘前关掉源文件，否则 `persist` 会报 access denied。
    drop(source);

    temp.as_file()
        .sync_all()
        .map_err(|error| format!("sync temp file failed: {}", error))?;
    temp.persist(path)
        .map_err(|error| format!("persist '{}' failed: {}", path_text, error))?;

    Ok(WriteOutcome {
        bytes_written: start + segment.len() as u64 + (file_bytes - end),
        revision: file_revision(path)?,
        encoding,
        has_bom,
    })
}

/// 顺序拷贝 `[from, to)`：`seek` 到起点后按固定块读，不整段驻留。
fn copy_range(
    source: &mut std::fs::File,
    target: &mut std::fs::File,
    from: u64,
    to: u64,
    path_text: &str,
) -> Result<(), String> {
    if to == from {
        return Ok(());
    }
    source
        .seek(SeekFrom::Start(from))
        .map_err(|error| format!("cannot seek '{}': {}", path_text, error))?;
    let mut remaining = to - from;
    let mut buffer = vec![0u8; COPY_CHUNK_BYTES];
    while remaining > 0 {
        let chunk = remaining.min(COPY_CHUNK_BYTES as u64) as usize;
        source
            .read_exact(&mut buffer[..chunk])
            .map_err(|error| format!("cannot read '{}': {}", path_text, error))?;
        target
            .write_all(&buffer[..chunk])
            .map_err(|error| format!("write temp file failed: {}", error))?;
        remaining -= chunk as u64;
    }
    Ok(())
}

/// 与 `edit` 的「CRLF 文件保持 CRLF」对齐：只调整 `replacement` 自身的行尾，
/// 区间外字节不参与（I58 §3.3 / L13）。
fn normalize_newlines(replacement: &str, crlf: bool) -> String {
    if !crlf {
        return replacement.to_string();
    }
    let lf_only = replacement.replace("\r\n", "\n");
    lf_only.replace('\n', "\r\n")
}

/// `replacement` 段编码。**不加 BOM**：BOM 属于 head，已原样留在文件里。
fn encode_segment(text: &str, encoding: &'static Encoding) -> Result<Vec<u8>, String> {
    if encoding == UTF_8 {
        return Ok(text.as_bytes().to_vec());
    }
    if encoding == UTF_16LE || encoding == UTF_16BE {
        let little_endian = encoding == UTF_16LE;
        let mut out = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            let bytes = if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            };
            out.extend_from_slice(&bytes);
        }
        return Ok(out);
    }

    let (encoded, used, had_errors) = encoding.encode(text);
    if had_errors {
        return Err(format!(
            "'replacement' cannot be represented in {} without loss; pick characters the file's \
             encoding can store",
            used.name()
        ));
    }
    Ok(encoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::GBK;

    fn fixture(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("doc.txt");
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    /// UTF-8 无 BOM、LF 的常规请求：各用例只覆盖自己关心的字段。
    fn request<'a>(
        start: u64,
        end: u64,
        replacement: &'a str,
        expected_revision: &'a str,
    ) -> WriteRequest<'a> {
        WriteRequest {
            start,
            end,
            replacement,
            encoding: UTF_8,
            has_bom: false,
            crlf: false,
            expected_revision,
        }
    }

    /// F10：定位与写回之间文件被改写（修订号变了）→ 拒绝写入，而不是按旧区间硬写。
    #[test]
    fn a_revision_drift_between_locate_and_write_is_refused() {
        let (_dir, path) = fixture("alpha\nbeta\n");
        let error = apply_range(
            &path,
            request(0, 5, "ALPHA", "revision-that-no-longer-holds"),
        )
        .expect_err("修订号不符必须失败");

        assert!(error.contains("changed between locating"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\nbeta\n");
    }

    /// F12：转码闸门只约束 `replacement` 段，且给出实测字节数。
    #[test]
    fn a_transcoded_replacement_over_the_cap_is_refused() {
        let (_dir, path) = fixture("alpha\n");
        let revision = file_revision(&path).expect("revision");
        // GBK 下每个汉字 2 字节：60 万个即 1.2 MB，越过 1 MiB 闸门。
        let replacement = "中".repeat(600_000);
        let mut request = request(0, 5, &replacement, &revision);
        request.encoding = GBK;

        let error = apply_range(&path, request).expect_err("转码后超限必须失败");

        assert!(error.contains("after transcoding"), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\n");
    }

    /// 三段写的口径：`bytes_written` 是写回后的文件总字节数，`\n` 按文件风格展开。
    #[test]
    fn crlf_files_get_crlf_replacements() {
        let (_dir, path) = fixture("one\r\ntwo\r\n");
        let revision = file_revision(&path).expect("revision");

        // 偏移 5 = 第一行行尾符之后（`one\r\n` 占 0..5），插在第二行行首。
        let mut request = request(5, 5, "a\nb\n", &revision);
        request.crlf = true;
        let outcome = apply_range(&path, request).expect("apply");

        assert_eq!(std::fs::read(&path).unwrap(), b"one\r\na\r\nb\r\ntwo\r\n");
        assert_eq!(outcome.bytes_written, 16);
    }
}
