//! 行索引格式：行起点表、落盘、载入校验（I58 §3.5）。
//!
//! 本模块属 B 私有公共层，`super_read` 与 `super_edit` 共用（I58 §7.2 / G18）：
//! 「读能定位的行」与「写能定位的区间」由同一份起点表定义，不存在两份判据。
//!
//! 实现独立：本模块不消费 `agent_read` 的 `LineSink` / `Scanner`，只对齐
//! 「行数与切行判据」这一互操作契约（I58 §0.3）。扫描按**原始字节**找行尾，
//! 不解码整行，因此建索引的峰值内存与文件大小、行长都解耦。
//!
//! 索引文件格式（小端）：
//!
//! ```text
//! magic "AIDX1" | lineCount u64 | fileBytes u64 | revisionLen u32 | revision utf8
//! offsets: (lineCount + 1) * u64   // offsets[i] = 第 i+1 行的起始字节偏移
//! ```
//!
//! `offsets[lineCount]` 是文件末尾的哨兵：末行有换行时等于 `fileBytes`，
//! 末行无换行时也等于 `fileBytes`（该行占 `[offsets[lineCount-1], fileBytes)`）。

#![allow(dead_code)]

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};
use sha2::{Digest, Sha256};

use crate::os::text_encoding::{DecodeError, EncodingSource, BINARY_PROBE_BYTES};

/// 索引文件头魔数（含版本）。格式变更必须换版本号，`load` 据此判废重建。
pub(crate) const MAGIC: &[u8; 5] = b"AIDX1";

/// 索引文件容量闸门（I58 §3.5 G11）：单文件索引上限 64 MiB。
pub(crate) const MAX_INDEX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// 建索引时的分块大小；与 `read` 同量级，峰值内存与文件大小解耦。
const CHUNK_BYTES: usize = 256 * 1024;

/// 文件头固定段：魔数(5) + 行数(8) + 文件字节(8) + revision 长度(4)。
const HEADER_FIXED_BYTES: u64 = 5 + 8 + 8 + 4;

/// 单个行起点占用的字节数。
const OFFSET_BYTES: u64 = 8;

/// 新建索引的全部事实：元信息 + 行起点表。
pub(crate) struct BuiltIndex {
    pub line_count: u64,
    pub file_bytes: u64,
    pub revision: String,
    pub encoding: &'static Encoding,
    pub has_bom: bool,
    pub source: EncodingSource,
    pub offsets: Vec<u64>,
}

/// 建索引：扫描源文件、判定编码口径、按容量闸门收口。
///
/// 编码判定与 `read` 同序（BOM → 显式 `encoding` → 二进制探测 → UTF-8），
/// 差别只在**不校验 UTF-8 合法性**：索引只记字节偏移，非法字节留给读取阶段报错。
pub(crate) fn build(
    path: &Path,
    revision: &str,
    explicit: Option<&'static Encoding>,
) -> Result<BuiltIndex, String> {
    let head = read_head(path)?;
    let (encoding, has_bom, bom_len, source) = detect_encoding(&head, explicit)
        .map_err(|error| format!("'{}': {}", path.display(), error))?;

    let file_bytes = std::fs::metadata(path)
        .map_err(|error| format!("failed to stat '{}': {}", path.display(), error))?
        .len();

    let mut offsets = scan_offsets(path, bom_len, line_break(encoding))?;
    // 末行无换行时扫描拿不到它之后的边界，补上文件末尾哨兵；
    // 以换行结尾时最后一项已经是 `file_bytes`，此时它只作哨兵、不算作空行。
    if offsets.last() != Some(&file_bytes) {
        offsets.push(file_bytes);
    }
    let line_count = offsets.len() as u64 - 1;

    let index_bytes = index_file_bytes(line_count, revision.len() as u64);
    if index_bytes > MAX_INDEX_FILE_BYTES {
        return Err(format!(
            "'{}': index would need {} bytes for {} lines, over the {} byte cap; \
             the file is too large to index",
            path.display(),
            index_bytes,
            line_count,
            MAX_INDEX_FILE_BYTES
        ));
    }

    Ok(BuiltIndex {
        line_count,
        file_bytes,
        revision: revision.to_string(),
        encoding,
        has_bom,
        source,
        offsets,
    })
}

/// 索引文件的总字节数：容量闸门与落盘断言共用同一口径。
pub(crate) fn index_file_bytes(line_count: u64, revision_len: u64) -> u64 {
    HEADER_FIXED_BYTES + revision_len + (line_count + 1) * OFFSET_BYTES
}

/// 落盘：写临时文件再改名，保证读到的索引要么完整要么不存在。
///
/// 只在 `load` 判废后调用，因此**总是覆盖**同名文件：残留的损坏索引正是要替换的对象。
pub(crate) fn store(
    index_root: &Path,
    source: &Path,
    built: &BuiltIndex,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(index_root)
        .map_err(|error| format!("failed to create '{}': {}", index_root.display(), error))?;

    let target = index_root.join(index_file_name(source, &built.revision));
    let staging = target.with_extension("idx.tmp");
    let written = write_index(&staging, built);
    if let Err(error) = written {
        let _ = std::fs::remove_file(&staging);
        return Err(error);
    }

    // Windows 的 `rename` 不覆盖已存在目标：先删再改。索引是缓存，中断只会导致重建。
    if target.is_file() {
        let _ = std::fs::remove_file(&target);
    }
    std::fs::rename(&staging, &target).map_err(|error| {
        let _ = std::fs::remove_file(&staging);
        format!(
            "failed to move index into '{}': {}",
            target.display(),
            error
        )
    })?;
    Ok(target)
}

/// 载入并校验索引：任何一项与当前文件事实不符都判废（返回 `None`，由调用方重建）。
///
/// 判废而不是报错：索引是缓存，过期是常态（文件被外部改写、格式升级），
/// 只有「源文件读不了」才是业务错误。
pub(crate) fn load(
    index_root: &Path,
    source: &Path,
    revision: &str,
    file_bytes: u64,
) -> Result<Option<LoadedIndex>, String> {
    let path = index_root.join(index_file_name(source, revision));
    if !path.is_file() {
        return Ok(None);
    }
    let mut file = File::open(&path)
        .map_err(|error| format!("failed to open '{}': {}", path.display(), error))?;

    let mut magic = [0u8; 5];
    if file.read_exact(&mut magic).is_err() || &magic != MAGIC {
        return Ok(None);
    }
    let line_count = read_u64(&mut file)?;
    let stored_file_bytes = read_u64(&mut file)?;
    let revision_len = read_u32(&mut file)? as u64;

    if stored_file_bytes != file_bytes {
        return Ok(None);
    }

    let mut revision_bytes = vec![0u8; revision_len as usize];
    if file.read_exact(&mut revision_bytes).is_err() {
        return Ok(None);
    }
    let stored_revision = match String::from_utf8(revision_bytes) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    if stored_revision != revision {
        return Ok(None);
    }

    let header_bytes = index_file_bytes(line_count, revision_len) - (line_count + 1) * OFFSET_BYTES;
    let expected_bytes = header_bytes + (line_count + 1) * OFFSET_BYTES;
    let actual_bytes = std::fs::metadata(&path)
        .map_err(|error| format!("failed to stat '{}': {}", path.display(), error))?
        .len();
    if actual_bytes != expected_bytes {
        return Ok(None);
    }

    Ok(Some(LoadedIndex {
        file,
        line_count,
        file_bytes: stored_file_bytes,
        header_bytes,
    }))
}

/// 已校验的索引句柄：按行窗口读起点表，内存只与请求窗口相关。
pub(crate) struct LoadedIndex {
    file: File,
    pub line_count: u64,
    pub file_bytes: u64,
    header_bytes: u64,
}

impl LoadedIndex {
    /// 读 `first_line`（1-based）起 `count` 行的起点，返回 `count + 1` 个偏移
    /// （最后一个是末行之后的哨兵）。`count` 超出末尾时按实际行数收口。
    pub(crate) fn offsets_range(
        &mut self,
        first_line: u64,
        count: u64,
    ) -> Result<Vec<u64>, String> {
        if first_line == 0 {
            return Err("'startLine' must be >= 1".to_string());
        }
        if first_line > self.line_count {
            return Err(format!(
                "line {} is past the end of the file ({} lines)",
                first_line, self.line_count
            ));
        }

        let last_line = first_line
            .checked_add(count)
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| "line window overflows".to_string())?
            .min(self.line_count);

        let first_index = first_line - 1;
        let offsets = last_line - first_line + 2;
        let position = self.header_bytes + first_index * OFFSET_BYTES;

        self.file
            .seek(SeekFrom::Start(position))
            .map_err(|error| format!("failed to seek index: {}", error))?;

        let mut buffer = vec![0u8; (offsets as usize) * OFFSET_BYTES as usize];
        self.file
            .read_exact(&mut buffer)
            .map_err(|error| format!("failed to read index: {}", error))?;

        Ok(buffer
            .as_chunks::<{ OFFSET_BYTES as usize }>()
            .0
            .iter()
            .map(|chunk| u64::from_le_bytes(*chunk))
            .collect())
    }

    /// 读整张起点表（`line_count + 1` 项），供内存缓存持有（I58 §4）。
    ///
    /// 落盘索引仍是「按窗口切片读」；只有把索引提升到内存缓存时才整表读一次，
    /// 内存占用与文件大小无关、只与行数相关（每行 8 字节）。
    pub(crate) fn all_offsets(&mut self) -> Result<Vec<u64>, String> {
        if self.line_count == 0 {
            // 空文件只有哨兵一项（起点 0），没有可切的行窗口。
            return Ok(vec![0]);
        }
        let line_count = self.line_count;
        self.offsets_range(1, line_count)
    }
}

/// 索引文件路径：内存缓存淘汰时据此删除对应落盘文件（I58 §4）。
pub(crate) fn index_file(index_root: &Path, source: &Path, revision: &str) -> PathBuf {
    index_root.join(index_file_name(source, revision))
}

/// 索引文件名：绝对路径的 sha2 摘要（大小写归一，Windows 路径不区分大小写）
/// + 净化后的 revision（`len:nanos` 的 `:` 换成 `_`）。
fn index_file_name(source: &Path, revision: &str) -> String {
    let digest = hex::encode(Sha256::digest(
        source.to_string_lossy().to_lowercase().as_bytes(),
    ));
    let mut safe_revision = String::with_capacity(revision.len());
    for character in revision.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
            safe_revision.push(character);
        } else {
            safe_revision.push('_');
        }
    }
    format!("{}_{}.idx", digest, safe_revision)
}

fn write_index(path: &Path, built: &BuiltIndex) -> Result<(), String> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("failed to create '{}': {}", path.display(), error))?;
    let mut writer = BufWriter::new(file);

    writer
        .write_all(MAGIC)
        .and_then(|_| writer.write_all(&built.line_count.to_le_bytes()))
        .and_then(|_| writer.write_all(&built.file_bytes.to_le_bytes()))
        .and_then(|_| writer.write_all(&(built.revision.len() as u32).to_le_bytes()))
        .and_then(|_| writer.write_all(built.revision.as_bytes()))
        .and_then(|_| {
            for offset in &built.offsets {
                writer.write_all(&offset.to_le_bytes())?;
            }
            Ok(())
        })
        .and_then(|_| writer.flush())
        .map_err(|error| format!("failed to write '{}': {}", path.display(), error))
}

fn read_u64(file: &mut File) -> Result<u64, String> {
    let mut bytes = [0u8; 8];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("failed to read index header: {}", error))?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_u32(file: &mut File) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("failed to read index header: {}", error))?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_head(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    let mut buffer = Vec::with_capacity(BINARY_PROBE_BYTES);
    Read::take(file, BINARY_PROBE_BYTES as u64)
        .read_to_end(&mut buffer)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    Ok(buffer)
}

/// 读取阶段的编码口径：与建索引用同一个判定，保证「索引的行起点」与「解码口径」一致。
///
/// 返回值第二项是内容起始偏移（BOM 已剥离），第三项是来源（决定 `[encoding]` 后缀）。
pub(crate) fn detect(
    path: &Path,
    explicit: Option<&'static Encoding>,
) -> Result<(&'static Encoding, bool, u64, EncodingSource), String> {
    let head = read_head(path)?;
    detect_encoding(&head, explicit).map_err(|error| format!("'{}': {}", path.display(), error))
}

/// 编码口径：返回 `(编码, 是否有 BOM, 内容起始偏移, 来源)`。
///
/// 与 `read` 的 `plan_from_head` 同序，但**不做 UTF-8 合法性校验**：
/// 建索引只看行尾字节，非法 UTF-8 在读取单行时才会报错。
fn detect_encoding(
    head: &[u8],
    explicit: Option<&'static Encoding>,
) -> Result<(&'static Encoding, bool, u64, EncodingSource), DecodeError> {
    if let Some((encoding, bom_len)) = Encoding::for_bom(head) {
        return Ok((encoding, true, bom_len as u64, EncodingSource::Bom));
    }
    if let Some(encoding) = explicit {
        return Ok((encoding, false, 0, EncodingSource::Explicit));
    }
    if head.contains(&0) {
        return Err(DecodeError::Binary);
    }
    Ok((UTF_8, false, 0, EncodingSource::Utf8))
}

/// 行尾字节模式：UTF-16 的换行是双字节，其余编码都是单个 `0x0A`。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineBreak {
    Single,
    Utf16Le,
    Utf16Be,
}

impl LineBreak {
    pub(crate) fn pattern(self) -> &'static [u8] {
        match self {
            Self::Single => b"\n",
            Self::Utf16Le => &[0x0A, 0x00],
            Self::Utf16Be => &[0x00, 0x0A],
        }
    }
}

pub(crate) fn line_break(encoding: &'static Encoding) -> LineBreak {
    if encoding == UTF_16LE {
        LineBreak::Utf16Le
    } else if encoding == UTF_16BE {
        LineBreak::Utf16Be
    } else {
        LineBreak::Single
    }
}

/// 分块扫描行尾，返回全部行起点（首项是内容起始，末项是文件末尾哨兵）。
///
/// 跨块的行尾模式由 `carry` 拼接：模式最多 2 字节，所以只需保留上一块末尾 1 字节。
fn scan_offsets(path: &Path, start: u64, line_break: LineBreak) -> Result<Vec<u64>, String> {
    let pattern = line_break.pattern();
    let mut file = File::open(path)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    if start > 0 {
        file.seek(SeekFrom::Start(start))
            .map_err(|error| format!("failed to seek '{}': {}", path.display(), error))?;
    }

    let mut offsets = vec![start];
    let mut carry = Vec::new();
    let mut base = start;

    loop {
        let mut chunk = Vec::with_capacity(CHUNK_BYTES);
        let read = Read::take(&mut file, CHUNK_BYTES as u64)
            .read_to_end(&mut chunk)
            .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;

        if chunk.is_empty() {
            break;
        }

        let mut buffer = std::mem::take(&mut carry);
        buffer.extend_from_slice(&chunk);

        let mut cursor = 0usize;
        while let Some(index) = find_pattern(&buffer[cursor..], pattern) {
            let at = cursor + index;
            offsets.push(base + (at + pattern.len()) as u64);
            cursor = at + pattern.len();
        }

        // 保留末尾可能构成跨块模式的前缀；已匹配过的部分不再重复扫描。
        let keep = pattern.len().saturating_sub(1);
        let tail_start = buffer.len().saturating_sub(keep);
        base += tail_start as u64;
        carry = buffer[tail_start..].to_vec();

        if read < CHUNK_BYTES {
            break;
        }
    }

    Ok(offsets)
}

fn find_pattern(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::tools::agent_read::window::{scan, WindowRequest};

    fn fixture(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    fn reference_offsets(content: &[u8], start: u64) -> Vec<u64> {
        let mut offsets = vec![start];
        for (index, byte) in content.iter().enumerate().skip(start as usize) {
            if *byte == b'\n' {
                offsets.push(index as u64 + 1);
            }
        }
        if offsets.last() != Some(&(content.len() as u64)) {
            offsets.push(content.len() as u64);
        }
        offsets
    }

    fn build_for(path: &Path) -> BuiltIndex {
        let revision = "7:11";
        build(path, revision, None).expect("build index")
    }

    #[test]
    fn offsets_match_a_byte_wise_line_split() {
        let content = b"alpha\r\nbeta\r\ngamma\n";
        let (_dir, path) = fixture("crlf.txt", content);

        let built = build_for(&path);
        assert_eq!(built.offsets, reference_offsets(content, 0));
        assert_eq!(built.line_count, 3);
        assert_eq!(built.file_bytes, content.len() as u64);
    }

    #[test]
    fn a_final_line_without_a_newline_still_gets_an_offset() {
        let content = b"alpha\nbeta";
        let (_dir, path) = fixture("tail.txt", content);

        let built = build_for(&path);
        assert_eq!(built.offsets, reference_offsets(content, 0));
        assert_eq!(built.line_count, 2);
    }

    #[test]
    fn a_file_ending_with_a_newline_has_no_extra_empty_line() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("trailing.txt", content);

        let built = build_for(&path);
        assert_eq!(built.line_count, 2);
        assert_eq!(
            *built.offsets.last().expect("sentinel"),
            content.len() as u64
        );
    }

    #[test]
    fn an_empty_file_has_no_lines() {
        let (_dir, path) = fixture("empty.txt", b"");

        let built = build_for(&path);
        assert_eq!(built.line_count, 0);
        assert_eq!(built.offsets, vec![0]);
        assert_eq!(built.file_bytes, 0);
    }

    #[test]
    fn a_bom_only_file_has_no_lines_and_skips_the_bom() {
        let content = b"\xEF\xBB\xBF";
        let (_dir, path) = fixture("bom.txt", content);

        let built = build_for(&path);
        assert_eq!(built.line_count, 0);
        assert_eq!(built.offsets, vec![3]);
        assert!(built.has_bom);
        assert_eq!(built.source, EncodingSource::Bom);
    }

    #[test]
    fn a_bom_is_not_counted_as_part_of_the_first_line() {
        let content = b"\xEF\xBB\xBFalpha\nbeta\n";
        let (_dir, path) = fixture("bom-lines.txt", content);

        let built = build_for(&path);
        assert_eq!(built.offsets, reference_offsets(content, 3));
        assert_eq!(built.line_count, 2);
    }

    #[test]
    fn gbk_content_is_split_on_the_single_byte_newline() {
        let content: &[u8] = b"\xC3\xC3\xC2\xD6\n\xCB\xC1\xC4\xC8\n";
        let (_dir, path) = fixture("gbk.txt", content);

        let built = build(&path, "1:2", Some(encoding_rs::GBK)).expect("build index");
        assert_eq!(built.offsets, reference_offsets(content, 0));
        assert_eq!(built.line_count, 2);
        assert_eq!(built.source, EncodingSource::Explicit);
        assert!(!built.has_bom);
    }

    #[test]
    fn utf16_le_newlines_are_two_bytes() {
        let text = "alpha\nbeta\n";
        let content: Vec<u8> = text
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        let (_dir, path) = fixture("utf16le.txt", content.as_slice());

        let built = build(&path, "1:2", Some(encoding_rs::UTF_16LE)).expect("build index");
        assert_eq!(built.line_count, 2);
        assert_eq!(built.offsets, vec![0, 12, 22]);
        assert_eq!(built.source, EncodingSource::Explicit);
        assert_eq!(built.file_bytes, content.len() as u64);
    }

    #[test]
    fn the_index_line_count_matches_reads_total_lines() {
        let cases: &[(&str, &[u8])] = &[
            ("plain", b"alpha\nbeta\ngamma\n"),
            ("crlf", b"alpha\r\nbeta\r\n"),
            ("no-tail", b"alpha\nbeta"),
            ("blank-lines", b"alpha\n\n\ngamma\n"),
            ("bom", b"\xEF\xBB\xBFalpha\nbeta\n"),
        ];

        for (name, content) in cases {
            let (_dir, path) = fixture(&format!("{name}.txt"), content);
            let built = build_for(&path);

            let request = WindowRequest {
                path: &path,
                offset: 1,
                limit: None,
                encoding: None,
            };
            let outcome = scan(&request).expect("read");
            assert_eq!(
                outcome.total_lines,
                Some(built.line_count),
                "{name}: index and read disagree on the line count"
            );
        }
    }

    #[test]
    fn a_stored_index_loads_back_with_the_same_offsets() {
        let content = b"alpha\nbeta\ngamma\n";
        let (_dir, path) = fixture("roundtrip.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        let index_file = store(root.path(), &path, &built).expect("store index");

        let mut loaded = load(root.path(), &path, &built.revision, built.file_bytes)
            .expect("load index")
            .expect("index is valid");

        assert_eq!(loaded.line_count, built.line_count);
        assert_eq!(loaded.file_bytes, built.file_bytes);
        let offsets = loaded.offsets_range(1, built.line_count).expect("offsets");
        assert_eq!(offsets, built.offsets);
        assert_eq!(
            std::fs::metadata(&index_file).expect("stat").len(),
            index_file_bytes(built.line_count, built.revision.len() as u64)
        );
    }

    #[test]
    fn a_changed_file_size_invalidates_the_index() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("changed.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        store(root.path(), &path, &built).expect("store index");

        let loaded = load(root.path(), &path, &built.revision, built.file_bytes + 1).expect("load");
        assert!(
            loaded.is_none(),
            "stale fileBytes must invalidate the index"
        );
    }

    #[test]
    fn a_changed_revision_invalidates_the_index() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("revision.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        store(root.path(), &path, &built).expect("store index");

        let loaded = load(root.path(), &path, "99:99", built.file_bytes).expect("load");
        assert!(loaded.is_none(), "stale revision must invalidate the index");
    }

    #[test]
    fn a_corrupted_magic_invalidates_the_index() {
        let content = b"alpha\n";
        let (_dir, path) = fixture("corrupt.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        let index_file = store(root.path(), &path, &built).expect("store index");
        let mut bytes = std::fs::read(&index_file).expect("read index");
        bytes[0] = b'X';
        std::fs::write(&index_file, &bytes).expect("write index");

        let loaded = load(root.path(), &path, &built.revision, built.file_bytes).expect("load");
        assert!(loaded.is_none(), "a broken magic must invalidate the index");
    }

    #[test]
    fn a_truncated_index_invalidates_itself() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("truncated.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        let index_file = store(root.path(), &path, &built).expect("store index");
        let bytes = std::fs::read(&index_file).expect("read index");
        std::fs::write(&index_file, &bytes[..bytes.len() - 8]).expect("truncate index");

        let loaded = load(root.path(), &path, &built.revision, built.file_bytes).expect("load");
        assert!(loaded.is_none(), "a short index must be rejected");
    }

    #[test]
    fn offsets_range_clamps_to_the_last_line() {
        let content = b"alpha\nbeta\ngamma\n";
        let (_dir, path) = fixture("clamp.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        store(root.path(), &path, &built).expect("store index");
        let mut loaded = load(root.path(), &path, &built.revision, built.file_bytes)
            .expect("load")
            .expect("valid");

        let offsets = loaded.offsets_range(2, 100).expect("offsets");
        assert_eq!(offsets, vec![6, 11, 17]);
    }

    #[test]
    fn offsets_range_rejects_a_start_past_the_end() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("past-end.txt", content);
        let built = build_for(&path);

        let root = tempfile::tempdir().expect("index root");
        store(root.path(), &path, &built).expect("store index");
        let mut loaded = load(root.path(), &path, &built.revision, built.file_bytes)
            .expect("load")
            .expect("valid");

        assert!(loaded.offsets_range(3, 1).is_err());
        assert!(loaded.offsets_range(0, 1).is_err());
    }

    #[test]
    fn the_capacity_gate_is_measured_in_bytes() {
        assert_eq!(MAX_INDEX_FILE_BYTES, 64 * 1024 * 1024);

        let max_lines = (MAX_INDEX_FILE_BYTES - HEADER_FIXED_BYTES) / OFFSET_BYTES - 1;
        assert!(index_file_bytes(max_lines, 0) <= MAX_INDEX_FILE_BYTES);
        assert!(index_file_bytes(max_lines + 1, 0) > MAX_INDEX_FILE_BYTES);
    }

    #[test]
    fn binary_files_are_rejected() {
        let content = b"alpha\x00beta\n";
        let (_dir, path) = fixture("binary.bin", content);

        let error = match build(&path, "1:1", None) {
            Err(error) => error,
            Ok(_) => panic!("binary must be rejected"),
        };
        assert!(error.contains("binary"), "unexpected error: {error}");
    }

    #[test]
    fn a_cross_chunk_newline_is_found_exactly_once() {
        let mut content = vec![b'a'; CHUNK_BYTES - 1];
        content.push(b'\n');
        content.extend_from_slice(b"beta\n");
        let (_dir, path) = fixture("chunk-boundary.txt", content.as_slice());

        let built = build_for(&path);
        assert_eq!(built.offsets, reference_offsets(content.as_slice(), 0));
        assert_eq!(built.line_count, 2);
    }
}
