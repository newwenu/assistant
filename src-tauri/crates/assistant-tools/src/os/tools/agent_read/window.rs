use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use encoding_rs::{Encoding, UTF_8};

use super::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, LINE_NUMBER_SEPARATOR, LINE_NUMBER_WIDTH,
    LINE_TRUNCATED_MARKER, MAX_LINE_CHARS,
};
use crate::os::text_encoding::{
    guess_encoding, DecodeError, DecodedInfo, EncodingSource, BINARY_PROBE_BYTES,
};

/// 单次从磁盘读入的字节数。
///
/// 取 256KB 而不是「一次读完」：峰值内存与文件大小解耦，且与二进制探测窗口同量级。
pub const CHUNK_BYTES: usize = 256 * 1024;

/// 输出窗口填满后，继续「只计数不渲染」的解码字符预算。
///
/// 作用是把 `totalLines` 尽量算准（普通大小文件都能在此预算内读到 EOF），
/// 同时给超大文件的额外 IO 封顶。
pub const EOF_GRACE_CHARS: usize = 1024 * 1024;

/// 猜测编码时的采样上限：不为判定把大文件整个再读一遍。
pub const GUESS_SAMPLE_BYTES: usize = 1024 * 1024;

/// 一次分块扫描的入参。`offset` 为 1-based 起始行。
pub struct WindowRequest<'a> {
    pub path: &'a Path,
    pub offset: u64,
    pub limit: Option<usize>,
    pub encoding: Option<&'static Encoding>,
}

/// 分块扫描的结果。`rendered` 只含输出窗口，不含截断提示与编码 footer。
pub struct WindowOutcome {
    pub rendered: String,
    pub returned_lines: usize,
    /// `rendered` 的字节数（等于 `rendered.len()`）。
    pub rendered_bytes: usize,
    /// 从 `offset` 到 EOF 全部渲染时应有的大致字节数；未读到 EOF 时为已扫描部分的合计。
    pub total_rendered_bytes: usize,
    /// 精确总行数；`None` 表示输出窗口填满后仍未读到 EOF。
    pub total_lines: Option<u64>,
    pub truncated: bool,
    pub info: DecodedInfo,
}

/// 按行窗口流式读取：内存只与块大小、单行上限、输出上限相关，与文件大小无关。
///
/// 编码按「BOM → 显式 `encoding` → 二进制判定 → UTF-8 逐块校验 → 猜测回退」的顺序
/// 判定（上级 §14.3）；判定一次后锁定，后续块由同一个解码器续解，跨块的多字节字符
/// 由解码器内部缓冲保证正确。
pub fn scan(request: &WindowRequest<'_>) -> Result<WindowOutcome, String> {
    let head = read_head(request.path)?;

    match plan_from_head(&head, request.encoding) {
        Ok(Plan::Locked {
            encoding,
            source,
            has_bom,
            skip,
        }) => {
            let mut scanner = Scanner::new(request, encoding, source, has_bom, skip);
            scanner
                .run(request.path)
                .map_err(|error| scan_error(request.path, error))?;
            Ok(scanner.finish())
        }
        Ok(Plan::Utf8) => {
            let mut scanner = Scanner::new(request, UTF_8, EncodingSource::Utf8, false, 0);
            match scanner.run(request.path) {
                Ok(()) => Ok(scanner.finish()),
                Err(ScanError::Fallback) => {
                    // 头部合法、后续块非法：按整文件采样重猜一次，再从头流式解码。
                    let sample = read_sample(request.path, GUESS_SAMPLE_BYTES)?;
                    let (encoding, is_confident) = guess_encoding(&sample);
                    if !is_confident {
                        return Err(format!(
                            "'{}': {}",
                            request.path.display(),
                            DecodeError::Undetectable
                        ));
                    }
                    let mut retry =
                        Scanner::new(request, encoding, EncodingSource::Guess, false, 0);
                    retry
                        .run(request.path)
                        .map_err(|error| scan_error(request.path, error))?;
                    Ok(retry.finish())
                }
                Err(error) => Err(scan_error(request.path, error)),
            }
        }
        Err(error) => Err(format!("'{}': {}", request.path.display(), error)),
    }
}

/// 头部结论。`skip` 是解码起点（BOM 已被剥离）。
enum Plan {
    Locked {
        encoding: &'static Encoding,
        source: EncodingSource,
        has_bom: bool,
        skip: u64,
    },
    /// UTF-8 乐观路径：逐块校验，遇到非法块再回退到猜测。
    Utf8,
}

fn plan_from_head(head: &[u8], explicit: Option<&'static Encoding>) -> Result<Plan, DecodeError> {
    if let Some((encoding, bom_len)) = Encoding::for_bom(head) {
        return Ok(Plan::Locked {
            encoding,
            source: EncodingSource::Bom,
            has_bom: true,
            skip: bom_len as u64,
        });
    }

    if let Some(encoding) = explicit {
        return Ok(Plan::Locked {
            encoding,
            source: EncodingSource::Explicit,
            has_bom: false,
            skip: 0,
        });
    }

    if head.contains(&0) {
        return Err(DecodeError::Binary);
    }

    Ok(Plan::Utf8)
}

/// 解码过程中的失败。`Fallback` 只出现在 UTF-8 乐观路径，表示需要重猜编码。
enum ScanError {
    Decode(DecodeError),
    Io(std::io::Error),
    Fallback,
}

impl From<std::io::Error> for ScanError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn scan_error(path: &Path, error: ScanError) -> String {
    match error {
        ScanError::Decode(inner) => format!("'{}': {}", path.display(), inner),
        ScanError::Io(inner) => format!("failed to read '{}': {}", path.display(), inner),
        ScanError::Fallback => format!(
            "'{}': content is not valid UTF-8 and the encoding could not be guessed",
            path.display()
        ),
    }
}

fn read_head(path: &Path) -> Result<Vec<u8>, String> {
    read_sample(path, BINARY_PROBE_BYTES)
}

fn read_sample(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    let mut buffer = Vec::with_capacity(max_bytes);
    Read::take(file, max_bytes as u64)
        .read_to_end(&mut buffer)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;
    Ok(buffer)
}

fn open_at(path: &Path, skip: u64) -> Result<File, ScanError> {
    let mut file = File::open(path)?;
    if skip > 0 {
        file.seek(SeekFrom::Start(skip))?;
    }
    Ok(file)
}

/// 读一块；`true` 表示已到文件末尾（本次读满则为 `false`，下一次读空才判 EOF）。
fn read_chunk(file: &mut File) -> Result<(Vec<u8>, bool), ScanError> {
    let mut chunk = Vec::with_capacity(CHUNK_BYTES);
    let read = Read::take(&mut *file, CHUNK_BYTES as u64).read_to_end(&mut chunk)?;
    Ok((chunk, read < CHUNK_BYTES))
}

/// 增量解码器：锁定编码后跨块续解，块尾不完整序列由解码器内部保留。
struct Decoder {
    encoding: &'static Encoding,
    inner: Inner,
}

enum Inner {
    /// 非 UTF-8：`encoding_rs` 的流式解码器。
    Locked(encoding_rs::Decoder),
    /// UTF-8：逐块 `from_utf8` 校验，块尾不完整序列留在 `carry`。
    Utf8 { carry: Vec<u8> },
}

impl Decoder {
    fn new(encoding: &'static Encoding) -> Self {
        let inner = if encoding == UTF_8 {
            Inner::Utf8 { carry: Vec::new() }
        } else {
            Inner::Locked(encoding.new_decoder_without_bom_handling())
        };
        Self { encoding, inner }
    }

    fn next(&mut self, chunk: &[u8], eof: bool) -> Result<String, ScanError> {
        match &mut self.inner {
            Inner::Locked(decoder) => {
                let capacity = decoder
                    .max_utf8_buffer_length(chunk.len())
                    .unwrap_or(chunk.len() * 3);
                let mut text = String::with_capacity(capacity);
                let (_, read, had_errors) = decoder.decode_to_string(chunk, &mut text, eof);
                if had_errors || read < chunk.len() {
                    return Err(ScanError::Decode(DecodeError::Lossy {
                        encoding_name: self.encoding.name(),
                    }));
                }
                Ok(text)
            }
            Inner::Utf8 { carry } => {
                let mut combined = std::mem::take(carry);
                combined.extend_from_slice(chunk);
                match std::str::from_utf8(&combined) {
                    Ok(text) => Ok(text.to_owned()),
                    Err(error) => {
                        let valid = error.valid_up_to();
                        let head = std::str::from_utf8(&combined[..valid])
                            .expect("valid_up_to is a char boundary")
                            .to_owned();
                        // 只有「末尾不完整」才值得把尾巴留到下一块；真的非法字节直接回退。
                        if error.error_len().is_none() && !eof {
                            *carry = combined[valid..].to_vec();
                            Ok(head)
                        } else {
                            Err(ScanError::Fallback)
                        }
                    }
                }
            }
        }
    }
}

struct Scanner<'a> {
    request: &'a WindowRequest<'a>,
    info: DecodedInfo,
    skip: u64,

    line_cap: usize,
    line_cap_is_output_cap: bool,
    byte_cap: usize,

    line_no: u64,
    current: String,
    current_chars: usize,
    current_truncated: bool,

    rendered: String,
    rendered_lines: usize,
    rendered_bytes: usize,
    total_rendered_bytes: usize,

    stopped: bool,
    stopped_by_bytes: bool,
    grace_chars: usize,
    reached_eof: bool,
}

impl<'a> Scanner<'a> {
    fn new(
        request: &'a WindowRequest<'a>,
        encoding: &'static Encoding,
        source: EncodingSource,
        has_bom: bool,
        skip: u64,
    ) -> Self {
        let limit = request.limit;
        Self {
            request,
            info: DecodedInfo {
                encoding,
                has_bom,
                source,
            },
            skip,
            // 行上限同时受 `limit` 与输出上限约束：`limit` 更大时仍按输出上限收口。
            line_cap: limit.map_or(DEFAULT_MAX_LINES, |value| value.min(DEFAULT_MAX_LINES)),
            line_cap_is_output_cap: limit.is_none_or(|value| value > DEFAULT_MAX_LINES),
            byte_cap: DEFAULT_MAX_BYTES,
            line_no: 0,
            current: String::new(),
            current_chars: 0,
            current_truncated: false,
            rendered: String::new(),
            rendered_lines: 0,
            rendered_bytes: 0,
            total_rendered_bytes: 0,
            stopped: false,
            stopped_by_bytes: false,
            grace_chars: 0,
            reached_eof: false,
        }
    }

    fn run(&mut self, path: &Path) -> Result<(), ScanError> {
        let mut file = open_at(path, self.skip)?;
        let mut decoder = Decoder::new(self.info.encoding);

        loop {
            if self.grace_chars >= EOF_GRACE_CHARS {
                break;
            }
            let (chunk, eof) = read_chunk(&mut file)?;
            let text = decoder.next(&chunk, eof)?;
            self.feed(&text);
            if eof {
                self.reached_eof = true;
                break;
            }
        }

        // 末尾没有换行符的最后一行也要计入（空内容表示文件恰好以换行结束）。
        if self.reached_eof && !self.current.is_empty() {
            self.flush_line();
        }
        Ok(())
    }

    fn feed(&mut self, text: &str) {
        let mut rest = text;
        while let Some(position) = rest.find('\n') {
            let (line, tail) = rest.split_at(position + 1);
            self.append(&line[..position]);
            self.flush_line();
            rest = tail;
        }
        if !rest.is_empty() {
            self.append(rest);
        }
    }

    /// 累积当前行，超过 [`MAX_LINE_CHARS`] 后停止累积并置截断标记（内存不随行长增长）。
    fn append(&mut self, text: &str) {
        if text.is_empty() || self.current_truncated {
            return;
        }
        let remaining = MAX_LINE_CHARS.saturating_sub(self.current_chars);
        let mut taken = 0usize;
        let mut end = text.len();
        for (index, _) in text.char_indices() {
            if taken == remaining {
                end = index;
                break;
            }
            taken += 1;
        }
        self.current.push_str(&text[..end]);
        self.current_chars += taken;
        if end < text.len() {
            self.current_truncated = true;
        }
    }

    fn flush_line(&mut self) {
        self.line_no += 1;
        let number = self.line_no;
        let content = std::mem::take(&mut self.current);
        let content_chars = self.current_chars;
        let content_truncated = self.current_truncated;
        self.current_chars = 0;
        self.current_truncated = false;

        if number < self.request.offset {
            return;
        }

        let extra = if content_truncated {
            LINE_TRUNCATED_MARKER.len()
        } else {
            0
        };
        let line_bytes = prefix_bytes(number) + content.len() + extra + 1;

        if !self.stopped {
            if self.rendered_lines >= self.line_cap {
                self.stopped = true;
            } else if self.rendered_lines > 0 && self.rendered_bytes + line_bytes > self.byte_cap {
                self.stopped = true;
                self.stopped_by_bytes = true;
            }
        }

        if self.stopped {
            // 只计数不渲染：让 `totalLines` 尽量算准，内存不增长。
            self.total_rendered_bytes += line_bytes;
            self.grace_chars += content_chars + 1;
            return;
        }

        let _ = write!(
            self.rendered,
            "{:>width$}{}",
            number,
            LINE_NUMBER_SEPARATOR,
            width = LINE_NUMBER_WIDTH
        );
        self.rendered.push_str(&content);
        if content_truncated {
            self.rendered.push_str(LINE_TRUNCATED_MARKER);
        }
        self.rendered.push('\n');
        self.rendered_lines += 1;
        self.rendered_bytes += line_bytes;
        self.total_rendered_bytes += line_bytes;
    }

    fn finish(self) -> WindowOutcome {
        let truncated = self.stopped && (self.stopped_by_bytes || self.line_cap_is_output_cap);
        WindowOutcome {
            rendered: self.rendered,
            returned_lines: self.rendered_lines,
            rendered_bytes: self.rendered_bytes,
            total_rendered_bytes: self.total_rendered_bytes,
            total_lines: self.reached_eof.then_some(self.line_no),
            truncated,
            info: self.info,
        }
    }
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
