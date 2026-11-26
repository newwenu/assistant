use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;

use super::preview::PREVIEW_CHARS;
use super::PersistedRef;

/// stdout 落盘上限：触达后继续读取并丢弃（只计数），不杀进程。
pub const MAX_SPILL_BYTES: usize = 60 * 1024 * 1024;

/// stderr 内存环形窗口（字符）：stderr 独占保留区，避免被 stdout 挤掉错误末行。
pub const STDERR_RING_CHARS: usize = 2 * 1024 * 1024;

/// stdout 触达上限后写入文件的标记行（内容事实，唯一允许写进文件的标记）。
pub const SPILL_TRUNCATED_MARKER: &str = "[truncated: output exceeded 60 MiB; total unknown]";

/// 码页探测窗口（字节）：首 4 KiB 判定一次（I56 §8.3）。
pub const ENCODING_PROBE_BYTES: usize = 4096;

/// 落盘文件的编码口径：探测结论的承载，由 `finish` 回传给 `run.rs`（I56 §8.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEncoding {
    /// 原始字节直写（合法 UTF-8；Unix 上非 UTF-8 流也归入此类，非法字节计 `U+FFFD`）。
    Utf8,
    /// Windows 控制台 OEM 码页：流式解码后以 UTF-8 写盘。
    Oem(u32),
}

impl OutputEncoding {
    /// wire 口径标签：`utf8` / `oem<cp>`（如 `oem936`）。
    pub fn label(self) -> String {
        match self {
            OutputEncoding::Utf8 => "utf8".to_string(),
            OutputEncoding::Oem(code_page) => format!("oem{code_page}"),
        }
    }
}

/// 输出流选择：决定字节进落盘文件还是 stderr 环形窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// 流式落盘 sink：同步签名，调用方是 blocking 池里的管道读取循环（无法 `await`）。
///
/// 并发写入由持有方串行化（`Arc<Mutex<Option<Box<dyn ToolResultSink>>>>`）：
/// 临界区只有一次 `write`，内部不取其它锁、不做跨进程等待，因此不会与「读管道」互相卡死。
pub trait ToolResultSink: Send {
    /// 逐块写入原始字节；`stream` 决定落盘区还是 stderr 环形窗口。
    ///
    /// **永不因超限返回 `Err`**：超限后只计数（stdout 丢弃 / stderr 环形覆盖），
    /// 仍继续消费管道——否则子进程写阻塞，读管道的一方会等一个永不到来的 EOF。
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String>;

    /// 同步收尾（追加 stderr 段 / 删空文件 / 解码尾窗口），由调用方置于 `spawn_blocking`。
    fn finish(self: Box<Self>) -> Result<SpilledOutput, String>;
}

/// 流式落盘的收尾结果：引用、尾窗口与全流计数。
#[derive(Debug, Clone)]
pub struct SpilledOutput {
    /// 空输出为 `None`（文件已删除）。
    pub reference: Option<PersistedRef>,
    /// 文件末 `PREVIEW_CHARS` 字符（`total_chars` 不足时即全文；
    /// `truncated` 时以 `[truncated: …]` 结尾）。
    pub preview: String,
    /// 已写入文件的字符数（`truncated` 时为截至上限的计数）。
    pub total_chars: usize,
    /// 子进程总产出字节（精确，含被丢弃部分）。
    pub output_bytes: usize,
    /// 子进程总产出行数（精确，含被丢弃部分）。
    pub output_lines: usize,
    /// stdout 触达 `MAX_SPILL_BYTES`。
    pub truncated: bool,
    /// stderr 超过 `STDERR_RING_CHARS`。
    pub stderr_truncated: bool,
    /// 本次落盘实际采用的编码口径（探测结论，只读回传，见 I56 §8.3）。
    pub output_encoding: OutputEncoding,
}

/// 文件 sink：stdout 边读边落盘，stderr 进有界环形窗口，`finish` 时统一收尾。
pub(super) struct FileSpillSink {
    path: PathBuf,
    file: Option<std::fs::File>,
    stdout_bytes: usize,
    stdout_total_bytes: usize,
    stdout_lines: usize,
    stdout_last_byte: Option<u8>,
    stdout_tail: VecDeque<char>,
    stderr_total_bytes: usize,
    stderr_lines: usize,
    stderr_total_chars: usize,
    stderr_last_byte: Option<u8>,
    stderr_ring: VecDeque<char>,
    /// 码页判定结论：`None` = 尚未判定，两路原始字节暂存于 pending。
    encoding: Option<OutputEncoding>,
    /// 判定前的 stdout 原始字节（首 4 KiB 窗口，判定时一次性冲洗）。
    stdout_pending: Vec<u8>,
    /// 判定前的 stderr 原始字节（同上；任一路满 4 KiB 即触发判定）。
    stderr_pending: Vec<u8>,
    stdout_codec: Option<StreamCodec>,
    stderr_codec: Option<StreamCodec>,
    truncated: bool,
    stderr_truncated: bool,
    file_chars: usize,
    file_ends_with_newline: bool,
    /// `finish` 是否已接手收尾。`false` 且 sink 被 drop = 调用方在收尾前放弃
    /// （turn 取消丢弃 future），此时落盘文件是无人引用的半成品，必须删除。
    finished: bool,
}

/// 单路输出编解码器：`Raw` 直写原始字节、字符由增量 UTF-8 解码器推进；
/// `Transcode` 按 OEM 码页流式解码（跨块保留未配对尾字节）后以 UTF-8 写盘。
enum StreamCodec {
    Raw(Utf8Decoder),
    Transcode(Box<encoding_rs::Decoder>),
}

impl StreamCodec {
    fn new(encoding: OutputEncoding) -> Self {
        match encoding {
            OutputEncoding::Utf8 => StreamCodec::Raw(Utf8Decoder::default()),
            OutputEncoding::Oem(code_page) => StreamCodec::Transcode(Box::new(
                encoding_for_code_page(code_page).new_decoder_without_bom_handling(),
            )),
        }
    }
}

impl FileSpillSink {
    pub(super) fn new(path: PathBuf, file: std::fs::File) -> Self {
        Self {
            path,
            file: Some(file),
            stdout_bytes: 0,
            stdout_total_bytes: 0,
            stdout_lines: 0,
            stdout_last_byte: None,
            stdout_tail: VecDeque::with_capacity(PREVIEW_CHARS + 1),
            stderr_total_bytes: 0,
            stderr_lines: 0,
            stderr_total_chars: 0,
            stderr_last_byte: None,
            stderr_ring: VecDeque::new(),
            encoding: None,
            stdout_pending: Vec::new(),
            stderr_pending: Vec::new(),
            stdout_codec: None,
            stderr_codec: None,
            truncated: false,
            stderr_truncated: false,
            file_chars: 0,
            file_ends_with_newline: false,
            finished: false,
        }
    }

    fn push_stdout_tail_char(&mut self, ch: char) {
        if self.stdout_tail.len() >= PREVIEW_CHARS {
            self.stdout_tail.pop_front();
        }
        self.stdout_tail.push_back(ch);
    }

    fn push_stderr_char(&mut self, ch: char) {
        self.stderr_total_chars += 1;
        if self.stderr_ring.len() >= STDERR_RING_CHARS {
            self.stderr_ring.pop_front();
            self.stderr_truncated = true;
        }
        self.stderr_ring.push_back(ch);
    }

    /// 首 4 KiB 判定一次编码：取两路 pending 中较长的一路作为样本，
    /// 判定后立即把两路 pending 冲洗进各自的 codec（I56 §8.3「判定只做一次」）。
    fn maybe_decide_encoding(&mut self) -> Result<(), String> {
        if self.encoding.is_some() {
            return Ok(());
        }
        let probe = if self.stderr_pending.len() > self.stdout_pending.len() {
            &self.stderr_pending
        } else if self.stdout_pending.is_empty() {
            return Ok(());
        } else {
            &self.stdout_pending
        };
        let encoding = probe_encoding(probe);
        self.encoding = Some(encoding);
        self.stdout_codec = Some(StreamCodec::new(encoding));
        self.stderr_codec = Some(StreamCodec::new(encoding));
        let stdout_pending = std::mem::take(&mut self.stdout_pending);
        if !stdout_pending.is_empty() {
            self.write_stdout_decoded(&stdout_pending)?;
        }
        let stderr_pending = std::mem::take(&mut self.stderr_pending);
        if !stderr_pending.is_empty() {
            self.write_stderr_decoded(&stderr_pending)?;
        }
        Ok(())
    }

    fn write_stdout(&mut self, chunk: &[u8]) -> Result<(), String> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.stdout_total_bytes += chunk.len();
        self.stdout_lines += count_newlines(chunk);
        self.stdout_last_byte = chunk.last().copied();
        if self.truncated {
            return Ok(());
        }
        if self.encoding.is_none() {
            self.stdout_pending.extend_from_slice(chunk);
            if self.stdout_pending.len() >= ENCODING_PROBE_BYTES {
                self.maybe_decide_encoding()?;
            }
            return Ok(());
        }
        self.write_stdout_decoded(chunk)
    }

    fn write_stdout_decoded(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut codec = self
            .stdout_codec
            .take()
            .ok_or_else(|| "result sink encoding not decided".to_string())?;
        let result = match &mut codec {
            StreamCodec::Raw(decoder) => self.write_stdout_raw(bytes, decoder),
            StreamCodec::Transcode(decoder) => self.write_stdout_transcoded(bytes, decoder),
        };
        self.stdout_codec = Some(codec);
        result
    }

    fn write_stdout_raw(&mut self, chunk: &[u8], decoder: &mut Utf8Decoder) -> Result<(), String> {
        let remaining = MAX_SPILL_BYTES.saturating_sub(self.stdout_bytes);
        let take = remaining.min(chunk.len());
        let written = &chunk[..take];
        {
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| "result sink already finished".to_string())?;
            file.write_all(written)
                .map_err(|error| format!("write result failed: {}", error))?;
        }
        self.stdout_bytes += written.len();
        decoder.feed(written, |ch| {
            self.file_chars += 1;
            self.push_stdout_tail_char(ch);
        });
        self.file_ends_with_newline = written.last() == Some(&b'\n');
        if take < chunk.len() {
            self.truncated = true;
        }
        Ok(())
    }

    fn write_stdout_transcoded(
        &mut self,
        chunk: &[u8],
        decoder: &mut encoding_rs::Decoder,
    ) -> Result<(), String> {
        let mut text = String::new();
        let _ = decoder.decode_to_string(chunk, &mut text, false);
        self.write_stdout_text(&text)
    }

    /// 转码路径的写盘：文件字节上限按 UTF-8 文本计，截断点回退到字符边界。
    fn write_stdout_text(&mut self, text: &str) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }
        let remaining = MAX_SPILL_BYTES.saturating_sub(self.stdout_bytes);
        let mut cut = remaining.min(text.len());
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let written = &text[..cut];
        if !written.is_empty() {
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| "result sink already finished".to_string())?;
            file.write_all(written.as_bytes())
                .map_err(|error| format!("write result failed: {}", error))?;
            self.stdout_bytes += written.len();
            for ch in written.chars() {
                self.file_chars += 1;
                self.push_stdout_tail_char(ch);
            }
            self.file_ends_with_newline = written.ends_with('\n');
        }
        if cut < text.len() {
            self.truncated = true;
        }
        Ok(())
    }

    fn write_stderr(&mut self, chunk: &[u8]) -> Result<(), String> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.stderr_total_bytes += chunk.len();
        self.stderr_lines += count_newlines(chunk);
        self.stderr_last_byte = chunk.last().copied();
        if self.encoding.is_none() {
            self.stderr_pending.extend_from_slice(chunk);
            if self.stderr_pending.len() >= ENCODING_PROBE_BYTES {
                self.maybe_decide_encoding()?;
            }
            return Ok(());
        }
        self.write_stderr_decoded(chunk)
    }

    fn write_stderr_decoded(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut codec = self
            .stderr_codec
            .take()
            .ok_or_else(|| "result sink encoding not decided".to_string())?;
        match &mut codec {
            StreamCodec::Raw(decoder) => {
                decoder.feed(bytes, |ch| self.push_stderr_char(ch));
            }
            StreamCodec::Transcode(decoder) => {
                let mut text = String::new();
                let _ = decoder.decode_to_string(bytes, &mut text, false);
                for ch in text.chars() {
                    self.push_stderr_char(ch);
                }
            }
        }
        self.stderr_codec = Some(codec);
        Ok(())
    }

    /// 收口：转码路径把未配对尾字节按 1 字符冲掉，直写路径把不完整 UTF-8 尾字节计 1 字符。
    fn flush_codec_leftovers(&mut self) -> Result<(), String> {
        if let Some(mut codec) = self.stdout_codec.take() {
            match &mut codec {
                StreamCodec::Raw(decoder) => {
                    if !std::mem::take(&mut decoder.pending).is_empty() {
                        self.file_chars += 1;
                        self.push_stdout_tail_char('\u{FFFD}');
                    }
                }
                StreamCodec::Transcode(decoder) => {
                    let mut text = String::new();
                    let _ = decoder.decode_to_string(&[], &mut text, true);
                    self.write_stdout_text(&text)?;
                }
            }
            self.stdout_codec = Some(codec);
        }
        if let Some(mut codec) = self.stderr_codec.take() {
            match &mut codec {
                StreamCodec::Raw(decoder) => {
                    if !std::mem::take(&mut decoder.pending).is_empty() {
                        self.push_stderr_char('\u{FFFD}');
                    }
                }
                StreamCodec::Transcode(decoder) => {
                    let mut text = String::new();
                    let _ = decoder.decode_to_string(&[], &mut text, true);
                    for ch in text.chars() {
                        self.push_stderr_char(ch);
                    }
                }
            }
            self.stderr_codec = Some(codec);
        }
        Ok(())
    }
}

/// 未收尾即被 drop 的兜底：删除落盘文件。
///
/// 触发路径是「调用方放弃」——turn 取消时 agent 侧丢弃 future，`finish` 永不执行，
/// 而文件里只有半截输出且无任何引用指向它，留着就是无主残留。
/// 正常路径 `finish` 已置 `finished`，此处直接返回，行为逐字节不变。
impl Drop for FileSpillSink {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        // 先关句柄再删：Windows 下打开的文件不能被删除。
        self.file = None;
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::debug!(
                "abandoned result spill cleanup skipped ({}): {}",
                self.path.display(),
                error
            ),
        }
    }
}

impl ToolResultSink for FileSpillSink {
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String> {
        match stream {
            OutputStream::Stdout => self.write_stdout(chunk),
            OutputStream::Stderr => self.write_stderr(chunk),
        }
    }

    fn finish(mut self: Box<Self>) -> Result<SpilledOutput, String> {
        // 收尾职责已交给本函数：之后的 `Box` drop 不得再删文件（含早退分支，
        // 与收尾前放弃的路径分开——那条由 `Drop` 负责）。
        self.finished = true;
        // 流提前 EOF 时在这里做唯一一次判定（不足 4 KiB 的样本也用），
        // 保证「判定只做一次、结论只由 sink 产出」（I56 §8.3）。
        self.maybe_decide_encoding()?;
        self.flush_codec_leftovers()?;
        let output_encoding = self.encoding.unwrap_or(OutputEncoding::Utf8);
        let mut file = self
            .file
            .take()
            .ok_or_else(|| "result sink already finished".to_string())?;

        let mut appendix = String::new();
        let mut ends_with_newline = self.file_ends_with_newline;
        let mut has_content = self.file_chars > 0;
        if self.truncated {
            if has_content && !ends_with_newline {
                appendix.push('\n');
            }
            appendix.push_str(SPILL_TRUNCATED_MARKER);
            ends_with_newline = false;
            has_content = true;
        }
        let stderr_body: Option<String> = if self.stderr_ring.is_empty() {
            None
        } else {
            Some(self.stderr_ring.iter().collect())
        };
        if let Some(body) = stderr_body.as_deref() {
            if has_content && !ends_with_newline {
                appendix.push('\n');
            }
            let header = if self.stderr_truncated {
                format!(
                    "--- stderr (last {} of {} chars) ---",
                    self.stderr_ring.len(),
                    self.stderr_total_chars
                )
            } else {
                "--- stderr ---".to_string()
            };
            appendix.push_str(&header);
            appendix.push('\n');
            appendix.push_str(body);
        }

        let appendix_chars = appendix.chars().count();
        self.file_chars += appendix_chars;
        if !appendix.is_empty() {
            file.write_all(appendix.as_bytes())
                .map_err(|error| format!("write result failed: {}", error))?;
        }
        file.flush()
            .map_err(|error| format!("flush result failed: {}", error))?;
        drop(file);

        if self.file_chars == 0 {
            let _ = std::fs::remove_file(&self.path);
            return Ok(SpilledOutput {
                reference: None,
                preview: String::new(),
                total_chars: 0,
                output_bytes: self.stdout_total_bytes + self.stderr_total_bytes,
                output_lines: self.total_lines(),
                truncated: self.truncated,
                stderr_truncated: self.stderr_truncated,
                output_encoding,
            });
        }

        let stdout_tail: String = self.stdout_tail.iter().collect();
        let mut preview = last_n_chars(&format!("{}{}", stdout_tail, appendix), PREVIEW_CHARS);
        // stdout 触达上限的标记行已被 stderr 段挤出尾窗口时，在 `preview` 末尾补一行：
        // 文件里仍只有一处，`parse_truncation` 靠尾窗口 `rfind` 判定。
        if self.truncated && !preview.ends_with(SPILL_TRUNCATED_MARKER) {
            preview.push('\n');
            preview.push_str(SPILL_TRUNCATED_MARKER);
        }

        let bytes = std::fs::metadata(&self.path)
            .map(|meta| meta.len() as usize)
            .map_err(|error| format!("stat result file failed: {}", error))?;

        Ok(SpilledOutput {
            reference: Some(PersistedRef {
                path: self.path.display().to_string(),
                bytes,
            }),
            preview,
            total_chars: self.file_chars,
            output_bytes: self.stdout_total_bytes + self.stderr_total_bytes,
            output_lines: self.total_lines(),
            truncated: self.truncated,
            stderr_truncated: self.stderr_truncated,
            output_encoding,
        })
    }
}

impl FileSpillSink {
    fn total_lines(&self) -> usize {
        let mut lines = self.stdout_lines + self.stderr_lines;
        if self.stdout_total_bytes > 0 && self.stdout_last_byte != Some(b'\n') {
            lines += 1;
        }
        if self.stderr_total_bytes > 0 && self.stderr_last_byte != Some(b'\n') {
            lines += 1;
        }
        lines
    }
}

/// 增量 UTF-8 解码器：分块边界可能切在多字节序列中间，禁止逐块 `from_utf8_lossy` 后拼窗口。
///
/// 非法序列计 1 字符（不 panic），不完整尾字节留到下一块；流结束时由调用方按 1 字符收口。
#[derive(Default)]
struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    fn feed<F: FnMut(char)>(&mut self, chunk: &[u8], mut on_char: F) {
        if chunk.is_empty() && self.pending.is_empty() {
            return;
        }
        let mut buffer = Vec::with_capacity(self.pending.len() + chunk.len());
        buffer.append(&mut self.pending);
        buffer.extend_from_slice(chunk);
        let mut offset = 0;
        while offset < buffer.len() {
            match std::str::from_utf8(&buffer[offset..]) {
                Ok(rest) => {
                    for ch in rest.chars() {
                        on_char(ch);
                    }
                    offset = buffer.len();
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        if let Ok(text) = std::str::from_utf8(&buffer[offset..offset + valid]) {
                            for ch in text.chars() {
                                on_char(ch);
                            }
                        }
                        offset += valid;
                    }
                    match error.error_len() {
                        Some(len) => {
                            on_char('\u{FFFD}');
                            offset += len;
                        }
                        None => {
                            self.pending.extend_from_slice(&buffer[offset..]);
                            offset = buffer.len();
                        }
                    }
                }
            }
        }
    }
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

/// 首 4 KiB 的编码判定（I56 §8.3 表格）。
///
/// 合法 UTF-8（允许末尾不完整序列）一律直写；只有**确定非法**的流才在 Windows 上
/// 按 OEM 码页转码，Unix 侧保持原始字节（非 UTF-8 流可能是任意编码，猜错即乱码）。
fn probe_encoding(bytes: &[u8]) -> OutputEncoding {
    match std::str::from_utf8(bytes) {
        Ok(_) => OutputEncoding::Utf8,
        Err(error) if error.error_len().is_none() => OutputEncoding::Utf8,
        Err(_) => oem_encoding(),
    }
}

#[cfg(windows)]
fn oem_encoding() -> OutputEncoding {
    // SAFETY: `GetOEMCP` 无参数、无副作用，只读取当前控制台 OEM 码页。
    OutputEncoding::Oem(unsafe { windows::Win32::Globalization::GetOEMCP() })
}

#[cfg(not(windows))]
fn oem_encoding() -> OutputEncoding {
    OutputEncoding::Utf8
}

/// 码页 → `encoding_rs` 编码。未列入的码页回退 `windows-1252`
/// （字节可解、不 panic；探测已排除合法 UTF-8，回退只影响扩展字符的精确性）。
fn encoding_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding {
    use encoding_rs::{
        BIG5, EUC_KR, GBK, IBM866, SHIFT_JIS, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
        WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
        WINDOWS_874,
    };
    match code_page {
        866 => IBM866,
        874 => WINDOWS_874,
        932 => SHIFT_JIS,
        936 => GBK,
        949 => EUC_KR,
        950 => BIG5,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        _ => WINDOWS_1252,
    }
}

fn last_n_chars(text: &str, n: usize) -> String {
    let total = text.chars().count();
    if total <= n {
        return text.to_string();
    }
    text.chars().skip(total - n).collect()
}
