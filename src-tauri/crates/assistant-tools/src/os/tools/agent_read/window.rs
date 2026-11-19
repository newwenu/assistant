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

/// 输出窗口填满后，继续「只计数不渲染」的解码字节预算。
///
/// 作用是把 `totalLines` 尽量算准（普通大小文件都能在此预算内读到 EOF），
/// 同时给超大文件的额外 IO 封顶。按**字节**而非字符计量：同一份文本在不同
/// 编码下字符数与字节数的比例可达数倍，字符口径不可控（I57 §3.2）。
pub const EOF_GRACE_BYTES: usize = 8 * 1024 * 1024;

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
    /// 本次输出中**加了截断标记**的行数（只数渲染过的行，`truncatedLines <= returnedLines`）。
    pub truncated_lines: usize,
    /// 是否因**字节**预算耗尽而停（`truncated` 的一个成因）。
    /// 提示的四态按 `total_lines` / `window_full` 归因，本字段只作事实记录与验收断言。
    #[allow(dead_code)]
    pub stopped_by_bytes: bool,
    /// 停止时输出窗口是否已达上限：行窗口满，或下一条行的整行字节本身就超过
    /// 字节上限（再多行也塞不进）。为 `false` 表示「窗口未满也停」（I57 §4.1 第 5 条），
    /// 截断提示据此在 S2 / S3 之间二选一。
    pub window_full: bool,
    /// 实际从磁盘读入的字节数（含被跳过行与宽限段），供 S3 文案归因与验收断言。
    pub scanned_bytes: usize,
    pub info: DecodedInfo,
}

/// 逐行渲染口：行格式的唯一拼装处（I57 §0.3 P1）。
///
/// `Scanner` 只负责计量与预算判定，行号前缀、分隔符、截断标记、换行都由 sink 拼；
/// 换一个 sink 就能换行格式（I58 的 `super_read` 复用同一份计量与预算）。
pub(crate) trait LineSink {
    /// 渲染一行。`line_bytes` 是该行进入预算的字节数（前缀 + 可见段 + 标记 + 换行）。
    fn emit(&mut self, number: u64, visible: &str, truncated: bool, line_bytes: usize);

    /// 丢弃已写入的内容。
    ///
    /// UTF-8 乐观路径在后续块发现非法字节时会重猜编码并**从头重扫**；sink 在扫描之外
    /// 持有，重扫会复用同一个 sink，因此必须显式清空首轮（错误编码下）写入的行。
    /// 无状态 sink 也要实现本方法，避免「不实现就静默保留脏行」。
    fn reset(&mut self);
}

/// 默认 sink：复刻 `read` 的既有行格式 `{number:>6}\t{visible}` + 截断标记。
#[derive(Default)]
pub(crate) struct TextSink {
    rendered: String,
}

impl LineSink for TextSink {
    fn emit(&mut self, number: u64, visible: &str, truncated: bool, _line_bytes: usize) {
        let _ = write!(
            self.rendered,
            "{:>width$}{}",
            number,
            LINE_NUMBER_SEPARATOR,
            width = LINE_NUMBER_WIDTH
        );
        self.rendered.push_str(visible);
        if truncated {
            self.rendered.push_str(LINE_TRUNCATED_MARKER);
        }
        self.rendered.push('\n');
    }

    fn reset(&mut self) {
        self.rendered.clear();
    }
}

/// 按行窗口流式读取：内存只与块大小、单行上限、输出上限相关，与文件大小无关。
///
/// 编码按「BOM → 显式 `encoding` → 二进制判定 → UTF-8 逐块校验 → 猜测回退」的顺序
/// 判定（上级 §14.3）；判定一次后锁定，后续块由同一个解码器续解，跨块的多字节字符
/// 由解码器内部缓冲保证正确。
pub fn scan(request: &WindowRequest<'_>) -> Result<WindowOutcome, String> {
    let mut sink = TextSink::default();
    let mut outcome = scan_into(request, &mut sink)?;
    outcome.rendered = sink.rendered;
    Ok(outcome)
}

/// 用指定 sink 渲染：行格式由 sink 决定，计量与预算判定不变。
///
/// 默认 sink 之外的 sink 只负责格式，`rendered` 恒为空（正文由调用方从自己的 sink 取）。
pub(crate) fn scan_into(
    request: &WindowRequest<'_>,
    sink: &mut dyn LineSink,
) -> Result<WindowOutcome, String> {
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
                .run(request.path, sink)
                .map_err(|error| scan_error(request.path, error))?;
            Ok(scanner.finish())
        }
        Ok(Plan::Utf8) => {
            let mut scanner = Scanner::new(request, UTF_8, EncodingSource::Utf8, false, 0);
            match scanner.run(request.path, sink) {
                Ok(()) => Ok(scanner.finish()),
                Err(ScanError::Fallback) => {
                    // 头部合法、后续块非法：按整文件采样重猜一次，再从头流式解码。
                    // 首轮已写进 sink 的行作废（它们来自错误编码）。
                    sink.reset();
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
                        .run(request.path, sink)
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
    /// 当前行**整行**的解码字节（含被 `MAX_LINE_CHARS` 切掉的尾部）。
    current_line_bytes: usize,

    rendered_lines: usize,
    rendered_bytes: usize,
    total_rendered_bytes: usize,
    /// 渲染过的行里带截断标记的行数（`stopped` 之后不再计数，故恒 `<= rendered_lines`）。
    truncated_lines: usize,
    /// 已渲染行的**整行**预算合计（§3.2 的 `line_budget_bytes` 累加器），只进预算、不出文案。
    budget_bytes: usize,

    stopped: bool,
    stopped_by_bytes: bool,
    window_full: bool,
    grace_bytes: usize,
    scanned_bytes: usize,
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
            current_line_bytes: 0,
            rendered_lines: 0,
            rendered_bytes: 0,
            total_rendered_bytes: 0,
            truncated_lines: 0,
            budget_bytes: 0,
            stopped: false,
            stopped_by_bytes: false,
            window_full: false,
            grace_bytes: 0,
            scanned_bytes: 0,
            reached_eof: false,
        }
    }

    fn run(&mut self, path: &Path, sink: &mut dyn LineSink) -> Result<(), ScanError> {
        let mut file = open_at(path, self.skip)?;
        let mut decoder = Decoder::new(self.info.encoding);

        loop {
            if self.grace_bytes >= EOF_GRACE_BYTES {
                break;
            }
            let (chunk, eof) = read_chunk(&mut file)?;
            self.scanned_bytes += chunk.len();
            let text = decoder.next(&chunk, eof)?;
            self.feed(&text, sink);
            if eof {
                self.reached_eof = true;
                break;
            }
        }

        // 末尾没有换行符的最后一行也要计入（空内容表示文件恰好以换行结束）。
        if self.reached_eof && !self.current.is_empty() {
            self.flush_line(sink);
        }
        Ok(())
    }

    fn feed(&mut self, text: &str, sink: &mut dyn LineSink) {
        let mut rest = text;
        while let Some(position) = rest.find('\n') {
            let (line, tail) = rest.split_at(position + 1);
            self.append(&line[..position]);
            self.flush_line(sink);
            rest = tail;
        }
        if !rest.is_empty() {
            self.append(rest);
        }
    }

    /// 累积当前行，超过 [`MAX_LINE_CHARS`] 后停止累积并置截断标记（内存不随行长增长）。
    ///
    /// 可见段最多 `MAX_LINE_CHARS` 个字符，但**整行字节**照常累计（§3.2）：被切掉的
    /// 尾部同样占输出预算，否则一条 4 MiB 的行在预算里只值 2000 字符（N5 的根因）。
    fn append(&mut self, text: &str) {
        self.current_line_bytes += text.len();
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

    fn flush_line(&mut self, sink: &mut dyn LineSink) {
        self.line_no += 1;
        let number = self.line_no;
        let content = std::mem::take(&mut self.current);
        let content_truncated = self.current_truncated;
        let full_line_bytes = self.current_line_bytes;
        self.current_chars = 0;
        self.current_truncated = false;
        self.current_line_bytes = 0;

        if number < self.request.offset {
            return;
        }

        let marker_bytes = if content_truncated {
            LINE_TRUNCATED_MARKER.len()
        } else {
            0
        };
        // 供 `data` 与提示使用：只含可见段。
        let line_bytes = prefix_bytes(number) + content.len() + marker_bytes + 1;
        // 只进预算：整行字节（含被切掉的尾部）。
        let line_budget_bytes = prefix_bytes(number) + full_line_bytes + marker_bytes + 1;

        if !self.stopped {
            if self.rendered_lines >= self.line_cap {
                self.stopped = true;
                self.window_full = true;
            } else if self.rendered_lines > 0
                && self.budget_bytes + line_budget_bytes > self.byte_cap
            {
                self.stopped = true;
                self.stopped_by_bytes = true;
                // 这一行本身就超过字节上限 → 任何行都塞不进，窗口已满；
                // 否则只是「累加到这一步放不下」，窗口未满也停（§4.1 第 5 条）。
                self.window_full = line_budget_bytes > self.byte_cap;
            }
        }

        if self.stopped {
            // 只计数不渲染：让 `totalLines` 尽量算准，内存不增长。
            self.total_rendered_bytes += line_bytes;
            self.grace_bytes += line_budget_bytes;
            return;
        }

        sink.emit(number, &content, content_truncated, line_bytes);
        self.rendered_lines += 1;
        self.rendered_bytes += line_bytes;
        self.total_rendered_bytes += line_bytes;
        self.budget_bytes += line_budget_bytes;
        if content_truncated {
            self.truncated_lines += 1;
        }
    }

    fn finish(self) -> WindowOutcome {
        let truncated = self.stopped && (self.stopped_by_bytes || self.line_cap_is_output_cap);
        WindowOutcome {
            rendered: String::new(),
            returned_lines: self.rendered_lines,
            rendered_bytes: self.rendered_bytes,
            total_rendered_bytes: self.total_rendered_bytes,
            total_lines: self.reached_eof.then_some(self.line_no),
            truncated,
            truncated_lines: self.truncated_lines,
            stopped_by_bytes: self.stopped_by_bytes,
            window_full: self.window_full,
            scanned_bytes: self.scanned_bytes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 记录每一次 `emit` 的四项事实：行号、可见段、截断标记、行字节。
    #[derive(Default)]
    struct RecordingSink {
        emissions: Vec<(u64, String, bool, usize)>,
    }

    impl LineSink for RecordingSink {
        fn emit(&mut self, number: u64, visible: &str, truncated: bool, line_bytes: usize) {
            self.emissions
                .push((number, visible.to_string(), truncated, line_bytes));
        }

        fn reset(&mut self) {
            self.emissions.clear();
        }
    }

    fn fixture(name: &str, content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    fn request<'a>(path: &'a Path, offset: u64, limit: Option<usize>) -> WindowRequest<'a> {
        WindowRequest {
            path,
            offset,
            limit,
            encoding: None,
        }
    }

    #[test]
    fn grace_budget_is_measured_in_bytes() {
        assert_eq!(EOF_GRACE_BYTES, 8 * 1024 * 1024);
    }

    #[test]
    fn default_sink_reproduces_the_line_format() {
        let mut sink = TextSink::default();
        sink.emit(1, "alpha", false, 6 + 5 + 1);
        sink.emit(12, "beta", true, 6 + 4 + LINE_TRUNCATED_MARKER.len() + 1);
        sink.emit(1234567, "gamma", false, 7 + 5 + 1);
        assert_eq!(
            sink.rendered,
            "     1\talpha\n    12\tbeta [truncated]\n1234567\tgamma\n"
        );
    }

    #[test]
    fn scanner_emits_every_line_through_the_sink() {
        let (_dir, path) = fixture("lines.txt", "alpha\nbeta\n");
        let request = request(&path, 1, None);

        let mut sink = RecordingSink::default();
        let outcome = scan_into(&request, &mut sink).expect("scan");

        assert!(
            outcome.rendered.is_empty(),
            "正文只能由 sink 产出，扫描核心不得自行拼接"
        );
        assert_eq!(outcome.returned_lines, 2);
        assert_eq!(sink.emissions.len(), 2);
        assert_eq!(
            sink.emissions[0],
            (1, "alpha".to_string(), false, 6 + 5 + 1)
        );
        assert_eq!(sink.emissions[1], (2, "beta".to_string(), false, 6 + 4 + 1));

        let mut render = TextSink::default();
        for (number, visible, truncated, line_bytes) in &sink.emissions {
            render.emit(*number, visible.as_str(), *truncated, *line_bytes);
        }
        let default_outcome = scan(&request).expect("scan");
        assert_eq!(render.rendered, default_outcome.rendered);
        let emitted_bytes: usize = sink.emissions.iter().map(|line| line.3).sum();
        assert_eq!(default_outcome.rendered_bytes, emitted_bytes);
    }

    #[test]
    fn text_sink_reset_discards_lines_from_a_failed_pass() {
        let mut sink = TextSink::default();
        sink.emit(1, "garbage", false, 8);
        sink.reset();
        assert!(sink.rendered.is_empty(), "编码回退重扫前必须清空首轮的脏行");
        sink.emit(1, "alpha", false, 6 + 5 + 1);
        assert_eq!(sink.rendered, "     1\talpha\n");
    }

    #[test]
    fn truncated_line_is_flagged_through_the_sink() {
        let content = format!("{}\n", "a".repeat(MAX_LINE_CHARS + 1));
        let (_dir, path) = fixture("long.txt", &content);
        let request = request(&path, 1, None);

        let mut sink = RecordingSink::default();
        scan_into(&request, &mut sink).expect("scan");

        assert_eq!(sink.emissions.len(), 1);
        let (number, visible, truncated, line_bytes) = &sink.emissions[0];
        assert_eq!(*number, 1);
        assert_eq!(visible.chars().count(), MAX_LINE_CHARS);
        assert!(*truncated, "超长行的截断事实必须由 sink 拿到");
        assert_eq!(
            *line_bytes,
            6 + MAX_LINE_CHARS + LINE_TRUNCATED_MARKER.len() + 1
        );
    }

    /// 一行 `bytes` 个 `a`（单字节字符），跨块也不产生额外字节。
    fn wide_line(bytes: usize) -> String {
        "a".repeat(bytes)
    }

    fn fixture_of_lines(
        name: &str,
        lines: usize,
        line_bytes: usize,
    ) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(name);
        let line = wide_line(line_bytes);
        let mut content = String::with_capacity(lines * (line_bytes + 1));
        for _ in 0..lines {
            content.push_str(&line);
            content.push('\n');
        }
        std::fs::write(&path, content).expect("write fixture");
        (dir, path)
    }

    fn scanner<'a>(request: &'a WindowRequest<'a>) -> Scanner<'a> {
        Scanner::new(request, UTF_8, EncodingSource::Utf8, false, 0)
    }

    /// 跑完一次扫描：`ScanError` 不带 `Debug`，统一转成可读失败信息。
    fn drive(scanner: &mut Scanner<'_>, path: &Path) -> RecordingSink {
        let mut sink = RecordingSink::default();
        scanner
            .run(path, &mut sink)
            .unwrap_or_else(|error| panic!("scan failed: {}", scan_error(path, error)));
        sink
    }

    #[test]
    fn append_charges_the_whole_line_to_the_budget() {
        let line_bytes = 4 * 1024 * 1024;
        let (_dir, path) = fixture("wide.txt", &format!("{}\ntail\n", wide_line(line_bytes)));
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        // 整行 4 MiB 进预算，而不是可见段（2000 字符）。
        assert_eq!(
            scanner.budget_bytes,
            prefix_bytes(1) + line_bytes + LINE_TRUNCATED_MARKER.len() + 1
        );
        assert_eq!(
            scanner.rendered_bytes,
            6 + MAX_LINE_CHARS + LINE_TRUNCATED_MARKER.len() + 1
        );

        let outcome = scanner.finish();
        assert_eq!(outcome.returned_lines, 1);
        assert!(outcome.stopped_by_bytes, "整行字节超预算必须触发停止");
        assert!(
            !outcome.window_full,
            "下一行本身很小、只是累加放不下 → 窗口未满也停"
        );
        assert!(outcome.truncated);
    }

    #[test]
    fn budget_stop_before_line_cap_is_reported_as_window_full() {
        let line_bytes = 4 * 1024 * 1024;
        let (_dir, path) = fixture_of_lines("wide-five.txt", 5, line_bytes);
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        let scanned = scanner.scanned_bytes;
        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, 1);
        assert_eq!(outcome.total_lines, None, "宽限耗尽时总行数未知");
        assert!(outcome.stopped_by_bytes);
        assert!(outcome.window_full, "单行整行字节已超上限 → 窗口已满");
        assert!(outcome.truncated);
        assert!(
            scanned <= EOF_GRACE_BYTES + line_bytes + CHUNK_BYTES,
            "扫描必须在宽限预算内收口: {scanned}"
        );
        assert!(scanned < 5 * line_bytes, "不得为算总数把文件整个读完");
    }

    #[test]
    fn small_lines_stop_on_budget_before_the_line_cap() {
        let line_bytes = 100 * 1024;
        let lines = 120;
        let (_dir, path) = fixture_of_lines("many-mid.txt", lines, line_bytes);
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(
            outcome.returned_lines, 10,
            "预算只能容纳 10 条整行（1 MiB / 100 KB）"
        );
        assert_eq!(outcome.total_lines, None);
        assert!(outcome.stopped_by_bytes);
        assert!(
            !outcome.window_full,
            "行数远未到 limit，窗口未满也要停（N5 的洞）"
        );
        assert!(outcome.truncated);
    }

    #[test]
    fn skipped_lines_before_offset_do_not_consume_the_budget() {
        let line_bytes = 4 * 1024 * 1024;
        let (_dir, path) = fixture_of_lines("wide-three.txt", 3, line_bytes);
        let request = request(&path, 2, None);
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, 1);
        assert_eq!(outcome.total_lines, Some(3), "停止后宽限足够读到 EOF");
        assert!(outcome.stopped_by_bytes);
        assert!(outcome.window_full);
        assert!(outcome.truncated);
    }

    #[test]
    fn single_wide_line_is_cut_and_scan_stays_within_budget() {
        let line_bytes = 1024 * 1024;
        let (_dir, path) = fixture("one-wide.txt", &format!("{}\n", wide_line(line_bytes)));
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let sink = drive(&mut scanner, &path);

        let scanned = scanner.scanned_bytes;
        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, 1);
        assert_eq!(outcome.total_lines, Some(1), "单行文件必须读到 EOF");
        assert!(!outcome.truncated, "窗口取完了行，只有行本身超宽");
        assert!(!outcome.stopped_by_bytes);
        assert_eq!(sink.emissions.len(), 1);
        assert!(sink.emissions[0].2, "超宽行必须带截断标记");
        assert_eq!(sink.emissions[0].1.chars().count(), MAX_LINE_CHARS);
        assert!(
            scanned <= EOF_GRACE_BYTES + CHUNK_BYTES,
            "单行文件的扫描量必须等于文件本身: {scanned}"
        );
    }

    #[test]
    fn bom_only_file_reports_zero_lines() {
        let (_dir, path) = fixture("bom-only.txt", "\u{feff}");
        let request = request(&path, 1, None);
        // 走公开入口：BOM 剥离发生在头部判定里，不经 `Scanner::new` 直连。
        let outcome = scan(&request).expect("scan");

        assert!(outcome.rendered.is_empty(), "BOM 剥离后没有正文");
        assert_eq!(outcome.returned_lines, 0);
        assert_eq!(outcome.total_lines, Some(0));
        assert!(!outcome.truncated);
    }

    #[test]
    fn trailing_newline_does_not_add_an_empty_line() {
        let (_dir, path) = fixture("trailing.txt", "alpha\nbeta\n");
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(sink.emissions.len(), 2);
        assert_eq!(outcome.total_lines, Some(2));
    }

    #[test]
    fn line_cap_stop_still_counts_the_total() {
        let (_dir, path) = fixture_of_lines("plain.txt", 2500, 10);
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, DEFAULT_MAX_LINES);
        assert_eq!(outcome.total_lines, Some(2500), "普通大小文件仍要算准总数");
        assert!(outcome.window_full);
        assert!(outcome.truncated);
        assert!(!outcome.stopped_by_bytes);
    }

    /// `truncated_lines` 只数**渲染过**的行：停止之后被跳过的行不再计入。
    #[test]
    fn truncated_lines_counts_only_rendered_lines() {
        let line_bytes = 4 * 1024 * 1024;
        let (_dir, path) = fixture_of_lines("wide-three.txt", 3, line_bytes);
        let request = request(&path, 1, Some(1));
        let mut scanner = scanner(&request);
        let _sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, 1);
        assert_eq!(
            outcome.truncated_lines, 1,
            "只有渲染过的那一行被切；后面两行没进输出也不计数"
        );
        assert!(outcome.truncated_lines <= outcome.returned_lines);
    }

    /// 混合文件：`truncated_lines` 是返回行数的子集，不是行总数。
    #[test]
    fn truncated_lines_is_a_subset_of_returned_lines() {
        let (_dir, path) = fixture("mixed.txt", &format!("short\n{}\ntail\n", wide_line(5000)));
        let request = request(&path, 1, None);
        let mut scanner = scanner(&request);
        let sink = drive(&mut scanner, &path);

        let outcome = scanner.finish();

        assert_eq!(outcome.returned_lines, 3);
        assert_eq!(outcome.truncated_lines, 1);
        assert_eq!(
            sink.emissions.iter().filter(|line| line.2).count(),
            outcome.truncated_lines,
            "计数必须与 sink 收到的截断标记数一致"
        );
        assert!(
            !outcome.truncated,
            "窗口取完了行，`truncated` 与 `truncatedLines` 正交"
        );
    }
}
