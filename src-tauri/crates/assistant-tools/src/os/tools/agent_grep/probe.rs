use globset::GlobSet;
use std::path::Path;
use std::time::Instant;

use crate::os::text_encoding::BINARY_PROBE_BYTES;
use crate::os::walk;

use super::outcome::NonUtf8Probe;
use super::search::{decide_file, FileDecision};
use super::PROBE_FILE_LIMIT;

/// 逐文件读前 8KB，统计「非 UTF-8 且非二进制」文件数（上级 §14.7 冷路径）。
///
/// 复用单个缓冲；有 BOM 的文件已可搜（`bom_sniffing`），合法 UTF-8 的文件本来就搜过了，
/// 两者都不计入。达到 [`PROBE_FILE_LIMIT`] 或 deadline 即停并标记 `truncated`。
pub(super) fn probe_non_utf8(
    base: &Path,
    include: Option<&GlobSet>,
    deadline: Instant,
) -> NonUtf8Probe {
    let mut probe = NonUtf8Probe {
        files: 0,
        truncated: false,
    };
    let mut buffer = Vec::with_capacity(BINARY_PROBE_BYTES);
    let mut checked = 0usize;

    for entry in walk::builder(base).build() {
        let Ok(entry) = entry else {
            continue;
        };
        if !matches!(decide_file(&entry, base, include), FileDecision::Search(_)) {
            continue;
        }
        if checked >= PROBE_FILE_LIMIT {
            probe.truncated = true;
            break;
        }
        if Instant::now() >= deadline {
            probe.truncated = true;
            break;
        }
        checked += 1;

        buffer.clear();
        let read = read_prefix(entry.path(), &mut buffer);
        if read == PrefixKind::NonUtf8 {
            probe.files += 1;
        }
    }

    probe
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrefixKind {
    /// 合法 UTF-8，或只是被 8KB 窗口截断（尾部半个字符）。
    Utf8,
    /// 有 BOM：`bom_sniffing` 已能搜。
    Bom,
    /// 含 `0x00`：二进制。
    Binary,
    /// 既非 UTF-8 也非二进制：`read` 会要求显式 `encoding`。
    NonUtf8,
}

fn read_prefix(path: &Path, buffer: &mut Vec<u8>) -> PrefixKind {
    use std::io::Read as _;

    let Ok(file) = std::fs::File::open(path) else {
        return PrefixKind::Utf8;
    };
    match std::io::BufReader::new(file)
        .take(BINARY_PROBE_BYTES as u64)
        .read_to_end(buffer)
    {
        Ok(_) => classify_prefix(buffer),
        Err(_) => PrefixKind::Utf8,
    }
}

fn classify_prefix(prefix: &[u8]) -> PrefixKind {
    if encoding_rs::Encoding::for_bom(prefix).is_some() {
        return PrefixKind::Bom;
    }
    if prefix.contains(&0) {
        return PrefixKind::Binary;
    }
    match std::str::from_utf8(prefix) {
        Ok(_) => PrefixKind::Utf8,
        // `error_len == None` 是「读到一半」而非「非法字节」，不能算非 UTF-8。
        Err(error) if error.error_len().is_none() => PrefixKind::Utf8,
        Err(_) => PrefixKind::NonUtf8,
    }
}
