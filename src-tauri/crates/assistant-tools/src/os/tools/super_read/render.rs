//! `super_read` 的自持渲染器（I58 §3.2）。
//!
//! 与 `agent_read` 的边界是**契约对齐、实现独立**（I58 §0.3）：
//!
//! - 对齐的是**格式字面量**：行号宽度、行号与正文的分隔符、截断标记，直接引用
//!   `agent_read` 的常量，因此两边不可能各写一个 `6` / `\t` 而漂移（G12）。
//! - 独立的是**实现**：行号前缀、可见段、截断判定、列推进、字节预算全部在本模块
//!   内重新实现，不消费 `LineSink` / `Scanner` / `scan_into`。行数据来自行索引的
//!   起点表（`super_index::format`），不来自顺序扫描。
//!
//! 行起点自证、单行解码与列推进来自 B 私有公共层 `super_index::proof`（§7.2 / G18）：
//! 本模块不复制判据，只以 `RenderError` 别名沿用既有命名。
//!
//! 计量口径与 `read` 一致（I57 §3.2）：进入预算的是**整行**字节（含被列窗口切掉的
//! 尾部），`rendered_bytes` 只累计可见段。

use std::fmt::Write as _;
use std::fs::File;
use std::path::Path;

use encoding_rs::Encoding;

use crate::os::agent_read::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, LINE_NUMBER_SEPARATOR, LINE_NUMBER_WIDTH,
    LINE_TRUNCATED_MARKER,
};
use crate::os::text_encoding::EncodingSource;
use crate::os::tools::super_index::{format, proof};

pub(crate) use proof::ColumnUnit;
pub(crate) use proof::IndexError as RenderError;
pub(crate) use proof::MAX_COLUMN_CHARS;

/// 每行宽度下限：再小就没有翻页意义，反而把行拆碎。
pub(crate) const MIN_LINE_WINDOW_CHARS: usize = 200;
/// 每行宽度上限：单行可见段的 CPU 与内存上界。
pub(crate) const MAX_LINE_WINDOW_CHARS: usize = 20_000;

/// 一次渲染的全部入参。
#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowSpec {
    /// 1-based 起始行。
    pub offset: u64,
    /// 调用方要求的行数；`None` 表示「按输出上限」。
    pub limit: Option<usize>,
    /// 1-based 行内起始列。
    pub column: u64,
    pub unit: ColumnUnit,
    /// 每行可见段的字符上限。
    pub max_line_chars: usize,
}

/// 渲染结果：正文只含输出窗口，不含提示与 footer。
#[derive(Debug)]
pub(crate) struct RenderedWindow {
    pub text: String,
    pub returned_lines: usize,
    pub rendered_bytes: usize,
    /// 本次输出中加了截断标记的行数。
    pub truncated_lines: usize,
    /// 因列推进而整行没有可见内容的行数（行在 `column` 之前就结束了）。
    pub lines_beyond_column: usize,
    pub truncated: bool,
    pub window_full: bool,
}

/// 本次输出最多渲染多少行：调用方 `limit` 与输出上限取小。
///
/// 与 `read` 同一条规则；`offsets_range` 取起点表的行数必须与它一致，
/// 否则会多读一段起点却渲染不了（白白放大单次 IO）。
pub(crate) fn line_cap(limit: Option<usize>) -> usize {
    limit.map_or(DEFAULT_MAX_LINES, |value| value.min(DEFAULT_MAX_LINES))
}

/// 按行索引的起点表渲染输出窗口。
///
/// `offsets` 是 `format::LoadedIndex::offsets_range` 的返回值：`n + 1` 项，
/// 末项是最后一行之后的哨兵。`source` 决定「UTF-8 非法字节是回退重猜还是报错」。
/// `total_lines` 只用于自证：起点表的末行允许没有行尾符，其余行必须有。
pub(crate) fn render_window(
    path: &Path,
    encoding: &'static Encoding,
    source: EncodingSource,
    spec: &WindowSpec,
    offsets: &[u64],
    total_lines: u64,
) -> Result<RenderedWindow, RenderError> {
    // 行上限与 `read` 同一规则：`limit` 更大时仍按输出上限收口，
    // 且只有「上限本身就是输出上限」时才把「还有行没读完」报成截断（I57 §4.2）。
    let line_cap = line_cap(spec.limit);
    let line_cap_is_output_cap = spec.limit.is_none_or(|value| value > DEFAULT_MAX_LINES);
    let byte_cap = DEFAULT_MAX_BYTES;
    let pattern = format::line_break(encoding).pattern();

    let mut file = File::open(path).map_err(|error| RenderError::Io {
        path: path.display().to_string(),
        error: error.to_string(),
    })?;

    let mut text = String::new();
    let mut rendered_lines = 0usize;
    let mut rendered_bytes = 0usize;
    let mut budget_bytes = 0usize;
    let mut truncated_lines = 0usize;
    let mut lines_beyond_column = 0usize;
    let mut stopped = false;
    let mut stopped_by_bytes = false;
    let mut window_full = false;

    for (index, pair) in offsets.windows(2).enumerate() {
        let number = spec.offset + index as u64;

        // 行窗口已满：后面的行不会再渲染，连解码都不做（索引已给出精确总行数）。
        if rendered_lines >= line_cap {
            stopped = true;
            window_full = true;
            break;
        }

        let raw = proof::read_line(
            &mut file,
            pair[0],
            pair[1],
            proof::LinePosition {
                number,
                is_first: number == 1,
                is_last: number == total_lines,
            },
            path,
            pattern,
        )?;
        let line = proof::decode_line(&raw, encoding, source, number)?;
        let total_chars = line.chars().count();

        // 预算只关心「这一行整行有多大」与「带不带标记」。标记按**整行口径**判定
        // （列推进只会让可见段更短，不会让标记消失），与 `read` 的计量一致。
        let full_truncated = total_chars > spec.max_line_chars;
        let prefix = prefix_bytes(number);
        let marker_bytes = if full_truncated {
            LINE_TRUNCATED_MARKER.len()
        } else {
            0
        };
        let line_budget_bytes = prefix + line.len() + marker_bytes + 1;

        // 首个渲染行不受预算限制：单行就超过输出上限时仍要给出一行可见内容，
        // 否则「大文件的第一行」会得到空输出（I57 §3.2 的既有规则）。
        if rendered_lines > 0 && budget_bytes.saturating_add(line_budget_bytes) > byte_cap {
            stopped = true;
            stopped_by_bytes = true;
            window_full = line_budget_bytes > byte_cap;
            break;
        }

        let char_start = match spec.unit {
            ColumnUnit::Char => ((spec.column - 1) as usize).min(total_chars),
            ColumnUnit::Byte => {
                proof::byte_column_to_char_start(&line, &raw, encoding, spec.column - 1, number)?
            }
        };
        let visible: String = line
            .chars()
            .skip(char_start)
            .take(spec.max_line_chars)
            .collect();
        let truncated = total_chars > char_start.saturating_add(spec.max_line_chars);
        let beyond_column = spec.column > 1 && char_start >= total_chars;

        let marker_bytes = if truncated {
            LINE_TRUNCATED_MARKER.len()
        } else {
            0
        };
        let line_bytes = prefix + visible.len() + marker_bytes + 1;

        let _ = write!(
            text,
            "{:>width$}{}",
            number,
            LINE_NUMBER_SEPARATOR,
            width = LINE_NUMBER_WIDTH
        );
        text.push_str(&visible);
        if truncated {
            text.push_str(LINE_TRUNCATED_MARKER);
        }
        text.push('\n');

        rendered_lines += 1;
        rendered_bytes += line_bytes;
        budget_bytes += line_budget_bytes;
        if truncated {
            truncated_lines += 1;
        }
        if beyond_column {
            lines_beyond_column += 1;
        }
    }

    let truncated = stopped && (stopped_by_bytes || line_cap_is_output_cap);
    Ok(RenderedWindow {
        text,
        returned_lines: rendered_lines,
        rendered_bytes,
        truncated_lines,
        lines_beyond_column,
        truncated,
        window_full,
    })
}

/// 行号前缀的字节数：定宽 6 列，行号超过 6 位时自然变长（与渲染格式一致）。
fn prefix_bytes(number: u64) -> usize {
    let mut digits = 1;
    let mut value = number;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits.max(LINE_NUMBER_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::UTF_8;
    use std::path::PathBuf;

    fn fixture(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    fn offsets_of(content: &[u8], start: usize) -> Vec<u64> {
        let mut offsets = vec![start as u64];
        for (index, byte) in content.iter().enumerate().skip(start) {
            if *byte == b'\n' {
                offsets.push(index as u64 + 1);
            }
        }
        if offsets.last() != Some(&(content.len() as u64)) {
            offsets.push(content.len() as u64);
        }
        offsets
    }

    fn spec(offset: u64, limit: Option<usize>) -> WindowSpec {
        WindowSpec {
            offset,
            limit,
            column: 1,
            unit: ColumnUnit::Char,
            max_line_chars: 2000,
        }
    }

    fn render(content: &[u8], spec: &WindowSpec) -> RenderedWindow {
        let (_dir, path) = fixture("sample.txt", content);
        let offsets = offsets_of(content, 0);
        let total_lines = offsets.len() as u64 - 1;
        render_window(
            &path,
            UTF_8,
            EncodingSource::Utf8,
            spec,
            &offsets,
            total_lines,
        )
        .expect("render")
    }

    #[test]
    fn the_line_format_matches_the_read_literal() {
        let window = render(b"alpha\nbeta\n", &spec(1, None));
        let expected = format!(
            "{:>width$}\talpha\n{:>width$}\tbeta\n",
            1,
            2,
            width = LINE_NUMBER_WIDTH
        );
        assert_eq!(window.text, expected);
    }

    #[test]
    fn a_crlf_keeps_the_carriage_return_visible() {
        let window = render(b"alpha\r\nbeta\r\n", &spec(1, None));
        assert!(window.text.contains("alpha\r\n"));
    }

    #[test]
    fn a_long_line_is_cut_and_marked() {
        let long = "x".repeat(2100);
        let content = format!("{}\n", long);
        let window = render(content.as_bytes(), &spec(1, None));
        assert_eq!(window.truncated_lines, 1);
        assert!(window
            .text
            .ends_with(&format!("{}\n", LINE_TRUNCATED_MARKER)));
        assert!(window.text.contains(&"x".repeat(2000)));
    }

    #[test]
    fn an_empty_file_renders_nothing() {
        let window = render(b"", &spec(1, None));
        assert_eq!(window.returned_lines, 0);
        assert_eq!(window.rendered_bytes, 0);
    }

    #[test]
    fn an_explicit_limit_below_the_output_cap_is_not_truncation() {
        let content = b"a\nb\nc\n";
        let window = render(content, &spec(1, Some(2)));
        assert_eq!(window.returned_lines, 2);
        assert!(!window.truncated);
    }

    #[test]
    fn a_window_at_the_output_cap_is_truncation() {
        // 文件必须真的超过输出上限，否则「返回了全部行」本就不该算截断。
        let content = "a\n".repeat(DEFAULT_MAX_LINES + 5);
        let window = render(content.as_bytes(), &spec(1, Some(DEFAULT_MAX_LINES + 1)));
        assert_eq!(window.returned_lines, DEFAULT_MAX_LINES);
        assert!(window.truncated);
    }

    #[test]
    fn char_and_byte_columns_agree_on_ascii() {
        let content = b"abcdef\n";
        let mut char_spec = spec(1, None);
        char_spec.column = 3;
        let mut byte_spec = char_spec;
        byte_spec.unit = ColumnUnit::Byte;

        let by_char = render(content, &char_spec);
        let by_byte = render(content, &byte_spec);
        assert_eq!(by_char.text, by_byte.text);
        assert!(by_char.text.contains("\tcd"));
    }

    #[test]
    fn a_byte_column_inside_a_multibyte_char_is_f5() {
        let content = "aé\n".as_bytes();
        let mut byte_spec = spec(1, None);
        byte_spec.column = 3;
        byte_spec.unit = ColumnUnit::Byte;

        let (_dir, path) = fixture("multibyte.txt", content);
        let offsets = offsets_of(content, 0);
        let total_lines = offsets.len() as u64 - 1;
        let error = render_window(
            &path,
            UTF_8,
            EncodingSource::Utf8,
            &byte_spec,
            &offsets,
            total_lines,
        )
        .expect_err("byte column inside a char must fail");
        match error {
            RenderError::NotOnBoundary { nearest, .. } => assert_eq!(nearest, 2),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn a_bom_is_not_part_of_the_first_line() {
        let content = b"\xEF\xBB\xBFalpha\nbeta\n";
        let (_dir, path) = fixture("bom.txt", content);
        let offsets = offsets_of(content, 3);
        let total_lines = offsets.len() as u64 - 1;
        let window = render_window(
            &path,
            UTF_8,
            EncodingSource::Bom,
            &spec(1, None),
            &offsets,
            total_lines,
        )
        .expect("render");
        assert!(window.text.starts_with(&format!(
            "{:>width$}\talpha",
            1,
            width = LINE_NUMBER_WIDTH
        )));
        assert_eq!(window.returned_lines, 2);
    }

    #[test]
    fn a_line_shorter_than_the_column_counts_as_beyond() {
        let content = b"ab\nabcdef\n";
        let mut spec = spec(1, None);
        spec.column = 4;
        let window = render(content, &spec);
        assert_eq!(window.lines_beyond_column, 1);
        assert!(window.text.contains("\tdef\n"));
    }

    #[test]
    fn invalid_utf8_without_an_explicit_encoding_requests_a_fallback() {
        let content = b"\xff\xfe\x00\x00\n";
        let (_dir, path) = fixture("invalid.txt", content);
        let offsets = offsets_of(content, 0);
        let total_lines = offsets.len() as u64 - 1;
        let error = render_window(
            &path,
            UTF_8,
            EncodingSource::Utf8,
            &spec(1, None),
            &offsets,
            total_lines,
        )
        .expect_err("invalid utf8 must request a fallback");
        assert!(matches!(error, RenderError::Utf8Fallback { line: 1 }));
    }

    #[test]
    fn a_gbk_line_decodes_through_the_explicit_encoding() {
        let content = "中文\n".as_bytes().to_vec();
        let mut raw = Vec::new();
        let (encoded, _, _) = encoding_rs::GBK.encode("中文\n");
        raw.extend_from_slice(&encoded);
        let (_dir, path) = fixture("gbk.txt", &raw);
        let offsets = offsets_of(&raw, 0);
        let total_lines = offsets.len() as u64 - 1;
        let window = render_window(
            &path,
            encoding_rs::GBK,
            EncodingSource::Explicit,
            &spec(1, None),
            &offsets,
            total_lines,
        )
        .expect("render");
        assert!(window.text.contains("中文"));
        assert_eq!(content.len(), "中文\n".len());
    }
}
