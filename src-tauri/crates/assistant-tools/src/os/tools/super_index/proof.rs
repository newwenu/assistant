//! 行起点自证、单行解码与列推进（I58 §4 / §3.2 F5·F6）。
//!
//! 起点表的定义是 `offsets[i] = 紧跟第 i 行行尾符之后`，因此：
//!
//! 1. 非首行的起点前 `pattern.len()` 字节必然是行尾符本身；
//! 2. 本行内容里不得再出现行尾符；
//! 3. 非全文件末行的区间必须以行尾符结束。
//!
//! 三条合起来就是完备判据：`start` 是真实行首、`start` 之后第一个行尾符恰好落在
//! `end` 前，故 `[start, end)` 正是第 `number` 行的真实范围。任一条不成立即
//! `StaleIndex`，由调用方重建索引后重试——代价只是每行多读 1–2 字节与一次比较，
//! syscall 数不变，却对「谁改了文件」一视同仁（工具集内、外部进程、手工编辑都覆盖）。
//!
//! 本模块的错误类型只定义一次：`super_read/render.rs` 以 `RenderError` 别名沿用，
//! `super_edit/locate.rs` 直接用 `IndexError`，新增失败码不会只加在一侧（§7.2）。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use encoding_rs::{Encoding, UTF_8};

use crate::os::text_encoding::{DecodeError, EncodingSource};

/// `unit = "char"` 时 `column` 的上限（I58 §3.2 F6）。
///
/// 这是 CPU 上界：字符口径要逐字符推进，20 万字符是「还能接受」的量级；
/// 更深的列必须改用 `unit = "byte"`，按字节定位不需要逐字符解码。
pub(crate) const MAX_COLUMN_CHARS: u64 = 200_000;

/// `column` 的计量单位（I58 §3.2）：读取与编辑共用同一口径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnUnit {
    /// 解码后的字符（1-based）。
    Char,
    /// 行内容起始（BOM 已剥离）的原始字节偏移（1-based）。
    Byte,
}

impl ColumnUnit {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Char => "char",
            Self::Byte => "byte",
        }
    }
}

/// 定位与渲染的失败。全部是业务错误，不允许静默降级成「少读几行」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IndexError {
    /// 头部未锁定编码（乐观 UTF-8）而窗口内出现非法字节：调用方需重猜编码后重试。
    Utf8Fallback {
        line: u64,
    },
    /// 行起点自证失败：起点表与文件的真实换行位置不符（I58 §4）。
    ///
    /// 调用方应重建索引后重试一次；重建后仍失败说明文件正在被写，按 F4 报错。
    StaleIndex {
        line: u64,
    },
    /// 已锁定编码下仍无法无损解码。
    Undecodable {
        line: u64,
        error: DecodeError,
    },
    /// `unit = "byte"` 的列切在多字节字符中间（F5）。
    NotOnBoundary {
        line: u64,
        requested: u64,
        nearest: u64,
    },
    Io {
        path: String,
        error: String,
    },
}

impl IndexError {
    /// 面向模型的文案（`path` 只在 IO 之外由调用方补齐）。
    pub(crate) fn message(&self, path: &Path) -> String {
        match self {
            Self::Utf8Fallback { line } => format!(
                "'{}': line {} is not valid UTF-8 and no encoding was given",
                path.display(),
                line
            ),
            Self::Undecodable { line, error } => {
                format!("'{}': line {}: {}", path.display(), line, error)
            }
            Self::NotOnBoundary {
                line,
                requested,
                nearest,
            } => format!(
                "'column' {} (unit = \"byte\") falls inside a multi-byte character on line {}; \
                 the nearest character boundary is {}",
                requested, line, nearest
            ),
            Self::Io { path, error } => format!("failed to read '{}': {}", path, error),
            Self::StaleIndex { line } => format!(
                "'{}': line {} does not start where the index says; the file changed while it \
                 was being read",
                path.display(),
                line
            ),
        }
    }
}

/// 一行在文件中的位置：自证需要知道「首行前面没有行尾符可借」与「末行允许没有行尾符」。
pub(crate) struct LinePosition {
    pub number: u64,
    pub is_first: bool,
    pub is_last: bool,
}

/// 读一行的原始字节并剥掉行尾，同时用**行起点自证**校验起点表（I58 §4）。
pub(crate) fn read_line(
    file: &mut File,
    start: u64,
    end: u64,
    position: LinePosition,
    path: &Path,
    pattern: &[u8],
) -> Result<Vec<u8>, IndexError> {
    let LinePosition {
        number,
        is_first,
        is_last,
    } = position;
    let io_error = |error: std::io::Error| IndexError::Io {
        path: path.display().to_string(),
        error: error.to_string(),
    };
    let stale = || IndexError::StaleIndex { line: number };

    // 首行之前是 BOM 或文件头，没有行尾符可断言，只读本行区间。
    let read_start = if is_first {
        start
    } else {
        start.saturating_sub(pattern.len() as u64)
    };
    let length = (end - read_start) as usize;
    let mut buffer = vec![0u8; length];
    file.seek(SeekFrom::Start(read_start)).map_err(io_error)?;
    file.read_exact(&mut buffer).map_err(io_error)?;

    let body_start = if is_first {
        0
    } else {
        if !buffer.starts_with(pattern) {
            return Err(stale());
        }
        pattern.len()
    };

    let body = &buffer[body_start..];
    let ends_with_break = body.ends_with(pattern);
    let content = if ends_with_break {
        &body[..body.len() - pattern.len()]
    } else {
        if !is_last {
            return Err(stale());
        }
        body
    };

    // 只在按行尾模式对齐的位置找：UTF-16 的双字节行尾在奇数字节处出现只是巧合字节。
    if content.chunks(pattern.len()).any(|chunk| chunk == pattern) {
        return Err(stale());
    }

    Ok(content.to_vec())
}

/// 单行解码。`read` 的乐观 UTF-8 路径在窗口内遇到非法字节时回退重猜，其余编码直接报错。
pub(crate) fn decode_line(
    raw: &[u8],
    encoding: &'static Encoding,
    source: EncodingSource,
    number: u64,
) -> Result<String, IndexError> {
    if encoding == UTF_8 {
        return std::str::from_utf8(raw)
            .map(str::to_owned)
            .map_err(|_| match source {
                EncodingSource::Utf8 => IndexError::Utf8Fallback { line: number },
                _ => IndexError::Undecodable {
                    line: number,
                    error: DecodeError::Lossy {
                        encoding_name: encoding.name(),
                    },
                },
            });
    }
    let (text, had_errors) = encoding.decode_without_bom_handling(raw);
    if had_errors {
        return Err(IndexError::Undecodable {
            line: number,
            error: DecodeError::Lossy {
                encoding_name: encoding.name(),
            },
        });
    }
    Ok(text.into_owned())
}

/// 把 `unit = "byte"` 的列（0-based 原始字节偏移）换成字符下标。
///
/// 越界按「行尾之后」处理（可见段为空），不报错：列比行还深是合法用法，
/// 只是这一行没有可显示的内容。**切在字符中间**才报 F5。
pub(crate) fn byte_column_to_char_start(
    line: &str,
    raw: &[u8],
    encoding: &'static Encoding,
    offset: u64,
    number: u64,
) -> Result<usize, IndexError> {
    let offset = offset as usize;
    if offset == 0 {
        return Ok(0);
    }
    let total_chars = line.chars().count();
    if offset >= raw.len() {
        return Ok(total_chars);
    }

    if encoding == UTF_8 {
        if line.is_char_boundary(offset) {
            return Ok(line[..offset].chars().count());
        }
        let mut low = offset;
        while low > 0 && !line.is_char_boundary(low) {
            low -= 1;
        }
        let mut high = offset;
        while high < line.len() && !line.is_char_boundary(high) {
            high += 1;
        }
        let nearest = if offset - low <= high - offset {
            low
        } else {
            high
        };
        return Err(IndexError::NotOnBoundary {
            line: number,
            requested: offset as u64 + 1,
            nearest: nearest as u64 + 1,
        });
    }

    // 非 UTF-8：原始字节偏移与解码字符不是一一对应，只能按「逐字节喂解码器」建边界表。
    let boundaries = decode_boundaries(raw, encoding);
    if boundaries[offset] > boundaries[offset - 1] {
        return Ok(boundaries[offset]);
    }
    let mut low = offset;
    while low > 0 && boundaries[low] == boundaries[low - 1] {
        low -= 1;
    }
    let mut high = offset;
    while high < raw.len() && boundaries[high + 1] == boundaries[high] {
        high += 1;
    }
    let nearest = if offset - low <= high - offset {
        low
    } else {
        high
    };
    Err(IndexError::NotOnBoundary {
        line: number,
        requested: offset as u64 + 1,
        nearest: nearest as u64 + 1,
    })
}

/// 字符下标（0-based，可等于字符总数）→ 行内原始字节偏移。
///
/// 与 [`byte_column_to_char_start`] 互为逆运算：读取只要「列 → 可见段起点」，
/// 编辑还要把字符边界换回**字节区间**（I58 §3.3），所以逆运算也归本模块，
/// 避免 `super_edit` 自己再写一份边界表而与自证判据漂移。
pub(crate) fn char_index_to_byte(
    line: &str,
    raw: &[u8],
    encoding: &'static Encoding,
    char_index: usize,
) -> usize {
    if encoding == UTF_8 {
        return line
            .char_indices()
            .nth(char_index)
            .map_or(line.len(), |(byte, _)| byte);
    }

    // `boundaries[k]` = 喂入前 k 字节后产出的字符数；第 `c` 个字符（0-based）的起点
    // 就是「产出字符数首次达到 c」的那个字节位置。
    let boundaries = decode_boundaries(raw, encoding);
    boundaries
        .iter()
        .position(|&count| count >= char_index)
        .unwrap_or(raw.len())
}

/// 原始字节偏移 → 已解码字符数。`boundaries[k]` 是「喂入前 k 字节后产出的字符数」，
/// `boundaries[k] > boundaries[k-1]` 即「k 是字符边界」。
fn decode_boundaries(raw: &[u8], encoding: &'static Encoding) -> Vec<usize> {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let mut buffer = String::with_capacity(raw.len());
    let mut boundaries = Vec::with_capacity(raw.len() + 1);
    let mut chars = 0usize;
    boundaries.push(0);
    for (index, byte) in raw.iter().enumerate() {
        buffer.clear();
        let (_, _, _) = decoder.decode_to_string(
            std::slice::from_ref(byte),
            &mut buffer,
            index + 1 == raw.len(),
        );
        chars += buffer.chars().count();
        boundaries.push(chars);
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    #[test]
    fn a_start_that_is_not_after_a_line_break_is_stale() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("stale.txt", content);
        let mut file = File::open(&path).expect("open");

        let position = LinePosition {
            number: 2,
            is_first: false,
            is_last: false,
        };
        // 起点取在 `beta` 中间（第 7 字节），其前一字节不是行尾符。
        let error = read_line(&mut file, 7, 11, position, &path, b"\n").expect_err("stale");
        assert!(matches!(error, IndexError::StaleIndex { line: 2 }));
    }

    #[test]
    fn the_first_line_has_no_prefix_to_prove() {
        let content = b"alpha\nbeta\n";
        let (_dir, path) = fixture("first.txt", content);
        let mut file = File::open(&path).expect("open");

        let position = LinePosition {
            number: 1,
            is_first: true,
            is_last: false,
        };
        let raw = read_line(&mut file, 0, 6, position, &path, b"\n").expect("first line");
        assert_eq!(raw, b"alpha");
    }

    #[test]
    fn a_line_with_an_extra_break_inside_is_stale() {
        // 起点表把第 2 行报成 `be\nta`：区间两端都合规，但内容里还有行尾符。
        let content = b"alpha\nbe\nta\n";
        let (_dir, path) = fixture("inner-break.txt", content);
        let mut file = File::open(&path).expect("open");

        let position = LinePosition {
            number: 2,
            is_first: false,
            is_last: true,
        };
        let error = read_line(&mut file, 6, 12, position, &path, b"\n").expect_err("stale");
        assert!(matches!(error, IndexError::StaleIndex { line: 2 }));
    }

    #[test]
    fn a_byte_column_inside_a_character_is_rejected() {
        let text = "你好";
        let raw = text.as_bytes();
        let error =
            byte_column_to_char_start(text, raw, UTF_8, 1, 1).expect_err("inside a character");
        assert!(matches!(error, IndexError::NotOnBoundary { nearest, .. } if nearest == 1));
    }

    #[test]
    fn a_byte_column_past_the_line_end_is_the_empty_tail() {
        let text = "你好";
        let raw = text.as_bytes();
        assert_eq!(
            byte_column_to_char_start(text, raw, UTF_8, 99, 1).expect("past end"),
            2
        );
    }

    #[test]
    fn an_optimistic_utf8_line_asks_for_a_guess() {
        let error = decode_line(b"alpha\xFF", UTF_8, EncodingSource::Utf8, 1).expect_err("invalid");
        assert!(matches!(error, IndexError::Utf8Fallback { line: 1 }));
    }
}
