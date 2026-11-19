//! 行地址 → 字节区间（I58 §3.3）。
//!
//! 索引只负责「行起点表」，本模块负责把「行 + 列」推进成半开字节区间 `[start, end)`：
//! - 行起点由 [`super_index::format`] 给出，行的真实性由 [`super_index::proof::read_line`] 自证；
//! - 列推进复用 [`super_index::proof`] 的字符边界表，不在这里另写一份；
//! - 两端自证与 `super_read` 完全同一判据，失败即 `StaleIndex`，由调用方重建后重试一次。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use encoding_rs::Encoding;

use crate::os::text_encoding::EncodingSource;
use crate::os::tools::super_index::{format, proof};
use proof::{ColumnUnit, IndexError, LinePosition};

/// 目标区间上限：区间要整段解码驻留以比对 `expected`（I58 §3.3 / L14）。
pub(crate) const MAX_EDIT_RANGE_BYTES: u64 = 16 * 1024 * 1024;

/// 行尾风格采样上限：只回答「这篇文件是否用 CRLF」，不需要读全文件。
const CRLF_SAMPLE_BYTES: u64 = 64 * 1024;

/// 定位失败。`Index` 类交给调用方重建后重试，`Business` 类重试也不会好。
pub(crate) enum LocateError {
    /// 行起点自证失败 / 编码需重猜（F5 的边界判据也走这里，文案由 `IndexError` 给）。
    Index(IndexError),
    /// 地址或参数本身不可用（F9 / F11）。
    Business(String),
}

impl LocateError {
    pub(crate) fn message(self, path: &Path) -> String {
        match self {
            Self::Index(error) => error.message(path),
            Self::Business(message) => message,
        }
    }
}

/// 一次调用的行地址（列口径已归一）。
///
/// `end_column: None` 即「到行尾」：`toEndOfLine` 在入参解析时已归一成这一种形态，
/// 因此定位侧不再区分两种写法。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Address {
    pub start_line: u64,
    pub start_column: u64,
    pub end_line: u64,
    pub end_column: Option<u64>,
    pub unit: ColumnUnit,
}

/// 定位结果：字节区间 + 现区间文本 + 行尾风格。
pub(crate) struct Located {
    pub start: u64,
    pub end: u64,
    pub text: String,
    pub total_lines: u64,
    pub crlf: bool,
}

/// 把行地址推进成字节区间，并在 `[start, end)` 内解码出「现区间文本」。
pub(crate) fn locate(
    path: &Path,
    offsets: &[u64],
    encoding: &'static Encoding,
    source: EncodingSource,
    address: &Address,
) -> Result<Located, LocateError> {
    let total_lines = offsets.len() as u64 - 1;
    if total_lines == 0 {
        return Err(LocateError::Business(format!(
            "'{}' has no lines to edit",
            path.display()
        )));
    }
    if address.start_line > total_lines {
        return Err(LocateError::Business(format!(
            "'startLine' {} is past the end of the file ({} lines)",
            address.start_line, total_lines
        )));
    }
    if address.end_line > total_lines {
        return Err(LocateError::Business(format!(
            "'endLine' {} is past the end of the file ({} lines)",
            address.end_line, total_lines
        )));
    }
    if address.end_line < address.start_line {
        return Err(LocateError::Business(format!(
            "'endLine' {} must not be before 'startLine' {}",
            address.end_line, address.start_line
        )));
    }

    let pattern = format::line_break(encoding).pattern();
    let mut file = File::open(path).map_err(|error| {
        LocateError::Index(IndexError::Io {
            path: path.display().to_string(),
            error: error.to_string(),
        })
    })?;

    let (start_raw, start_text) = read_and_decode(
        &mut file,
        offsets,
        address.start_line,
        total_lines,
        path,
        encoding,
        source,
        pattern,
    )?;
    let start_char = column_to_char_index(
        &start_text,
        &start_raw,
        encoding,
        address.start_column,
        address.unit,
        address.start_line,
        "startColumn",
    )?;
    let start = offsets[(address.start_line - 1) as usize]
        + proof::char_index_to_byte(&start_text, &start_raw, encoding, start_char) as u64;

    let (end_raw, end_text) = if address.end_line == address.start_line {
        (start_raw, start_text)
    } else {
        read_and_decode(
            &mut file,
            offsets,
            address.end_line,
            total_lines,
            path,
            encoding,
            source,
            pattern,
        )?
    };

    let end_char = match address.end_column {
        Some(column) => column_to_char_index(
            &end_text,
            &end_raw,
            encoding,
            column,
            address.unit,
            address.end_line,
            "endColumn",
        )?,
        // `endColumn` 缺省与 `toEndOfLine` 同义：结束位置取该行行尾（不含行尾符）。
        None => end_text.chars().count(),
    };
    let end = offsets[(address.end_line - 1) as usize]
        + proof::char_index_to_byte(&end_text, &end_raw, encoding, end_char) as u64;

    if address.end_line == address.start_line && end < start {
        return Err(LocateError::Business(format!(
            "'endColumn' resolves before 'startColumn' on line {} (byte {} < {})",
            address.start_line, end, start
        )));
    }

    let range_bytes = end - start;
    if range_bytes > MAX_EDIT_RANGE_BYTES {
        // F11：只报区间口径，不提示改用 `edit`——那是另一种语义。
        return Err(LocateError::Business(format!(
            "the target range is {} bytes, over the {} byte cap; split the edit into several \
             calls over smaller ranges",
            range_bytes, MAX_EDIT_RANGE_BYTES
        )));
    }

    let mut raw = vec![0u8; range_bytes as usize];
    if range_bytes > 0 {
        file.seek(SeekFrom::Start(start)).map_err(|error| {
            LocateError::Index(IndexError::Io {
                path: path.display().to_string(),
                error: error.to_string(),
            })
        })?;
        file.read_exact(&mut raw).map_err(|error| {
            LocateError::Index(IndexError::Io {
                path: path.display().to_string(),
                error: error.to_string(),
            })
        })?;
    }
    let text = proof::decode_line(&raw, encoding, source, address.start_line)
        .map_err(LocateError::Index)?;

    Ok(Located {
        start,
        end,
        text,
        total_lines,
        crlf: uses_crlf(&mut file, path, pattern).map_err(LocateError::Index)?,
    })
}

/// 读一行的原始字节并解码；`read_line` 同时完成行起点自证（与 `super_read` 同一判据）。
#[allow(clippy::too_many_arguments)]
fn read_and_decode(
    file: &mut File,
    offsets: &[u64],
    number: u64,
    total_lines: u64,
    path: &Path,
    encoding: &'static Encoding,
    source: EncodingSource,
    pattern: &[u8],
) -> Result<(Vec<u8>, String), LocateError> {
    let index = (number - 1) as usize;
    let raw = proof::read_line(
        file,
        offsets[index],
        offsets[index + 1],
        LinePosition {
            number,
            is_first: number == 1,
            is_last: number == total_lines,
        },
        path,
        pattern,
    )
    .map_err(LocateError::Index)?;
    let text = proof::decode_line(&raw, encoding, source, number).map_err(LocateError::Index)?;
    Ok((raw, text))
}

/// 1-based 列 → 0-based 字符下标。字符口径与 `super_read` 同上限（F6 在解析期已挡），
/// 字节口径的「切在字符中间」直接复用 `proof::byte_column_to_char_start`（F5）。
fn column_to_char_index(
    line: &str,
    raw: &[u8],
    encoding: &'static Encoding,
    column: u64,
    unit: ColumnUnit,
    number: u64,
    label: &str,
) -> Result<usize, LocateError> {
    let chars = line.chars().count();
    match unit {
        ColumnUnit::Char => {
            let index = column.saturating_sub(1) as usize;
            if index > chars {
                return Err(LocateError::Business(format!(
                    "'{}' {} is past the end of line {} ({} characters)",
                    label, column, number, chars
                )));
            }
            Ok(index)
        }
        ColumnUnit::Byte => {
            let offset = column.saturating_sub(1);
            if offset > raw.len() as u64 {
                return Err(LocateError::Business(format!(
                    "'{}' {} is past the end of line {} ({} bytes)",
                    label,
                    column,
                    number,
                    raw.len()
                )));
            }
            proof::byte_column_to_char_start(line, raw, encoding, offset, number)
                .map_err(LocateError::Index)
        }
    }
}

/// 这篇文件是否用 CRLF：只采样文件头，返回「replacement 是否该把 `\n` 展开成 `\r\n`」。
///
/// 与 `edit` 的「CRLF 文件保持 CRLF」对齐；区间外字节一律原样拷贝，所以这里只影响
/// `replacement` 自身的行尾风格（I58 §3.3 / L13）。
fn uses_crlf(file: &mut File, path: &Path, pattern: &[u8]) -> Result<bool, IndexError> {
    let io_error = |error: std::io::Error| IndexError::Io {
        path: path.display().to_string(),
        error: error.to_string(),
    };
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut head = vec![0u8; CRLF_SAMPLE_BYTES as usize];
    let read = file.read(&mut head).map_err(io_error)?;
    let head = &head[..read];

    // CR 与行尾符同一编码：UTF-16 的 CR 是双字节，不能直接拼 ASCII 的 `\r`。
    let mut crlf = Vec::with_capacity(pattern.len() + 2);
    if pattern == [0x0A, 0x00] {
        crlf.extend_from_slice(&[0x0D, 0x00]);
    } else if pattern == [0x00, 0x0A] {
        crlf.extend_from_slice(&[0x00, 0x0D]);
    } else {
        crlf.push(b'\r');
    }
    crlf.extend_from_slice(pattern);

    Ok(head
        .windows(crlf.len())
        .any(|window| window == crlf.as_slice()))
}
