use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use dashmap::DashMap;

/// 单结果落盘阈值：超过即落盘，与单轮预算无关。
pub const PER_TOOL_LIMIT_CHARS: usize = 50_000;

/// 单轮内联预算：所有内联结果回填文本的合计上限。
pub const PER_ROUND_LIMIT_CHARS: usize = 200_000;

/// 落盘回填预览的头部窗口（字符）。
pub const PREVIEW_HEAD_CHARS: usize = 600;

/// 落盘回填预览的尾部窗口（字符）。
pub const PREVIEW_TAIL_CHARS: usize = 600;

/// 落盘后回填的预览字符数（头尾合计）。
pub const PREVIEW_CHARS: usize = PREVIEW_HEAD_CHARS + PREVIEW_TAIL_CHARS;

/// stdout 落盘上限：触达后继续读取并丢弃（只计数），不杀进程。
pub const MAX_SPILL_BYTES: usize = 60 * 1024 * 1024;

/// stderr 内存环形窗口（字符）：stderr 独占保留区，避免被 stdout 挤掉错误末行。
pub const STDERR_RING_CHARS: usize = 2 * 1024 * 1024;

/// stdout 触达上限后写入文件的标记行（内容事实，唯一允许写进文件的标记）。
pub const SPILL_TRUNCATED_MARKER: &str = "[truncated: output exceeded 60 MiB; total unknown]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    /// 会话 id；用于目录命名空间（I46 compact §2.9 D9）。`None` 归入共享桶。
    pub session_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRef {
    pub path: String,
    pub bytes: usize,
}

#[async_trait]
pub trait ToolResultStorage: Send + Sync {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String>;

    /// 流式入口：先建文件，边写边算头尾窗口，结束时给出引用与窗口。
    async fn open_sink(&self, req: PersistToolResult) -> Result<Box<dyn ToolResultSink>, String>;

    /// 本存储为某个会话分配的落盘目录（回读域）。默认 `None` = 不提供回读。
    ///
    /// `persist` / `open_sink` / 回读边界三处必须取同一个来源，
    /// 禁止各自 `root.join(session_namespace(..))`——那是三个事实源的起点。
    fn namespace_root(&self, _session_id: Option<&str>) -> Option<PathBuf> {
        None
    }
}

pub struct FileToolResultStorage {
    root: PathBuf,
}

impl FileToolResultStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn result_path(&self, req: &PersistToolResult) -> Result<PathBuf, String> {
        let dir = self
            .namespace_root(req.session_id.as_deref())
            .ok_or_else(|| "tool result storage has no namespace root".to_string())?;
        let file = format!(
            "{}-{}-{}",
            unique_prefix(),
            safe_component(&req.tool_name),
            result_file_name(&req.tool_call_id)
        );
        Ok(dir.join(file))
    }
}

#[async_trait]
impl ToolResultStorage for FileToolResultStorage {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String> {
        let path = self.result_path(&req)?;
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "tool result path has no parent".to_string())?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let bytes = req.content.len();
        tokio::fs::write(&path, req.content.as_bytes())
            .await
            .map_err(|error| format!("write result failed: {}", error))?;
        Ok(PersistedRef {
            path: path.display().to_string(),
            bytes,
        })
    }

    async fn open_sink(&self, req: PersistToolResult) -> Result<Box<dyn ToolResultSink>, String> {
        let path = self.result_path(&req)?;
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "tool result path has no parent".to_string())?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let handle = tokio::fs::File::create(&path)
            .await
            .map_err(|error| format!("create result file failed: {}", error))?;
        let handle = handle.into_std().await;
        Ok(Box::new(FileSpillSink::new(path, handle)))
    }

    fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
        Some(self.root.join(session_namespace(session_id)))
    }
}

/// 会话命名空间：会话 id 的 FNV-1a 64 位摘要（不引第三方哈希库，跨进程稳定）。
///
/// 不用 `DefaultHasher`：它的算法与密钥未承诺跨版本稳定，而定位符要跨重启有效。
/// 空 id / `None` 归入 `shared`，与 Agent 会话互不干扰。
pub fn session_namespace(session_id: Option<&str>) -> String {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return "shared".to_string();
    };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// 文件名的唯一前缀：纳秒时间戳 + 进程内自增序号。
///
/// 只保证「同一目录内不撞名」，不承担不可猜测性：定位符本身不是凭据，
/// 越权访问由工作区边界与审批约束。
fn unique_prefix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    format!("{:012x}{:04x}", nanos & 0xffff_ffff_ffff, seq & 0xffff)
}

/// 工具名只作可读提示，非法字符一律替换，避免路径分隔符与保留字符。
fn safe_component(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.chars().take(32).collect()
    }
}

/// 启动清理结果：只用于日志与测试断言，不参与任何业务判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: usize,
    pub failed: usize,
}

/// 按 `mtime` 年龄清理落盘目录（I46 compact §2.9 D9，门禁 10）。
///
/// 约束：只删常规文件、不跟随符号链接、失败只计数不抛出、`retention_days == 0`
/// 完全禁用。目录本身不修剪（OQ6 定稿：只删文件、保留空目录）。
/// 旧布局（根目录下的扁平 `<call_id>.txt`）同样按年龄清理，不做迁移。
pub fn cleanup_expired_results(root: &Path, retention_days: u32) -> CleanupReport {
    let mut report = CleanupReport::default();
    if retention_days == 0 {
        return report;
    }
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return report;
    };
    sweep_directory(root, cutoff, &mut report);
    report
}

fn retention_cutoff(retention_days: u32) -> Option<SystemTime> {
    let span = Duration::from_secs(u64::from(retention_days).saturating_mul(86_400));
    SystemTime::now().checked_sub(span)
}

fn sweep_directory(dir: &Path, cutoff: SystemTime, report: &mut CleanupReport) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // 目录不存在是正常空态（落盘目录按需创建，从未落盘过结果时压根没有），
        // 不是失败：不告警、不计数，避免每次启动都刷一条无意义的 WARN。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            log::warn!(
                "cleanup tool results: read dir '{}' failed: {}",
                dir.display(),
                error
            );
            report.failed += 1;
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: bad entry in '{}': {}",
                    dir.display(),
                    error
                );
                report.failed += 1;
                continue;
            }
        };
        let path = entry.path();
        // `symlink_metadata` 不跟随链接：符号链接按「非常规文件」跳过，绝不删目标。
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: stat '{}' failed: {}",
                    path.display(),
                    error
                );
                report.failed += 1;
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            sweep_directory(&path, cutoff, report);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let expired = meta.modified().map(|mtime| mtime < cutoff).unwrap_or(false);
        if !expired {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: remove '{}' failed: {}",
                    path.display(),
                    error
                );
                report.failed += 1;
            }
        }
    }
}

/// 选择本轮需要落盘的结果。
///
/// 规则：单结果超 `PER_TOOL_LIMIT_CHARS` 直接落盘；其余按字符数**降序**累加，
/// 累计超出 `PER_ROUND_LIMIT_CHARS` 的才落盘。降序保证了落盘数量最小：
/// 先把最大的挪出内联预算，剩下的内联总量才可能装得下。
///
/// 入参与返回都按原始顺序，调用方无需重排。
pub fn select_persist_targets(sizes: &[usize]) -> Vec<bool> {
    let mut persist = vec![false; sizes.len()];
    let mut candidates: Vec<usize> = Vec::new();

    for (index, &size) in sizes.iter().enumerate() {
        if size > PER_TOOL_LIMIT_CHARS {
            persist[index] = true;
        } else {
            candidates.push(index);
        }
    }

    candidates.sort_by(|&left, &right| sizes[right].cmp(&sizes[left]));

    let mut inline_budget = 0usize;
    for index in candidates {
        let size = sizes[index];
        if inline_budget + size > PER_ROUND_LIMIT_CHARS {
            persist[index] = true;
        } else {
            inline_budget += size;
        }
    }

    persist
}

/// 落盘后的回填文本：引用 + 头 `PREVIEW_HEAD_CHARS` + 省略标记 + 尾 `PREVIEW_TAIL_CHARS`。
///
/// 尾部窗口不可省：工具自己的省略 footer（`[truncated: ...]` / `[encoding: ...]`）
/// 与命令行类输出的错误末行都在原文末尾，只留头等于把它们全部丢掉。
///
/// 标记行**不得复用** `[truncated:` 语法：那是「工具结果被采集侧截断」的专用口径，
/// 复用会让 `compact.truncated` 把「存储预览被裁」误读成「结果不完整」。
pub fn persisted_reference(path: &str, content: &str) -> String {
    let total = content.chars().count();
    if total <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, content);
    }

    let head: String = content.chars().take(PREVIEW_HEAD_CHARS).collect();
    let tail: String = content.chars().skip(total - PREVIEW_TAIL_CHARS).collect();
    let omitted = total - PREVIEW_HEAD_CHARS - PREVIEW_TAIL_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n{}\n[… {} chars omitted; full output in the file above …]\n{}",
        path, head, omitted, tail
    )
}

/// 流式落盘后的回填文本：标签 + 省略标记 + 尾窗口（**不取头窗口**）。
///
/// 命令输出的信息密度集中在末尾（失败栈、测试摘要、退出前的现场），头部多是启动横幅
/// 与进度条重绘——因此预览整块取自文件末，不与头部分享预算。整段生产者
/// [`persisted_reference`] 保留头尾窗口：那里预览的是文档，开头就是内容本身。
///
/// 标签与省略标记必须与 [`persisted_reference`] **逐字节一致**：
/// 两个生产者共用同一个 `parse_persisted_tag` 回退路径。
pub fn persisted_reference_from_tail(path: &str, tail: &str, total_chars: usize) -> String {
    if total_chars <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, tail);
    }

    let omitted = total_chars - PREVIEW_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n[… {} chars omitted; full output in the file above …]\n{}",
        path, omitted, tail
    )
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
}

/// 文件 sink：stdout 边读边落盘，stderr 进有界环形窗口，`finish` 时统一收尾。
struct FileSpillSink {
    path: PathBuf,
    file: Option<std::fs::File>,
    stdout_bytes: usize,
    stdout_total_bytes: usize,
    stdout_lines: usize,
    stdout_last_byte: Option<u8>,
    stdout_tail: VecDeque<char>,
    stdout_decoder: Utf8Decoder,
    stderr_total_bytes: usize,
    stderr_lines: usize,
    stderr_total_chars: usize,
    stderr_last_byte: Option<u8>,
    stderr_ring: VecDeque<char>,
    stderr_decoder: Utf8Decoder,
    truncated: bool,
    stderr_truncated: bool,
    file_chars: usize,
    file_ends_with_newline: bool,
}

impl FileSpillSink {
    fn new(path: PathBuf, file: std::fs::File) -> Self {
        Self {
            path,
            file: Some(file),
            stdout_bytes: 0,
            stdout_total_bytes: 0,
            stdout_lines: 0,
            stdout_last_byte: None,
            stdout_tail: VecDeque::with_capacity(PREVIEW_CHARS + 1),
            stdout_decoder: Utf8Decoder::default(),
            stderr_total_bytes: 0,
            stderr_lines: 0,
            stderr_total_chars: 0,
            stderr_last_byte: None,
            stderr_ring: VecDeque::new(),
            stderr_decoder: Utf8Decoder::default(),
            truncated: false,
            stderr_truncated: false,
            file_chars: 0,
            file_ends_with_newline: false,
        }
    }

    fn push_stdout_tail_char(&mut self, ch: char) {
        if self.stdout_tail.len() >= PREVIEW_CHARS {
            self.stdout_tail.pop_front();
        }
        self.stdout_tail.push_back(ch);
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
        let mut decoder = std::mem::take(&mut self.stdout_decoder);
        decoder.feed(written, |ch| {
            self.file_chars += 1;
            self.push_stdout_tail_char(ch);
        });
        self.stdout_decoder = decoder;
        self.file_ends_with_newline = written.last() == Some(&b'\n');
        if take < chunk.len() {
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
        let mut decoder = std::mem::take(&mut self.stderr_decoder);
        decoder.feed(chunk, |ch| {
            self.stderr_total_chars += 1;
            if self.stderr_ring.len() >= STDERR_RING_CHARS {
                self.stderr_ring.pop_front();
                self.stderr_truncated = true;
            }
            self.stderr_ring.push_back(ch);
        });
        self.stderr_decoder = decoder;
        Ok(())
    }

    fn flush_decoder_leftovers(&mut self) {
        let stdout_leftover = std::mem::take(&mut self.stdout_decoder.pending);
        if !stdout_leftover.is_empty() {
            self.file_chars += 1;
            self.push_stdout_tail_char('\u{FFFD}');
        }
        let stderr_leftover = std::mem::take(&mut self.stderr_decoder.pending);
        if !stderr_leftover.is_empty() {
            self.stderr_total_chars += 1;
            if self.stderr_ring.len() >= STDERR_RING_CHARS {
                self.stderr_ring.pop_front();
                self.stderr_truncated = true;
            }
            self.stderr_ring.push_back('\u{FFFD}');
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
        let mut file = self
            .file
            .take()
            .ok_or_else(|| "result sink already finished".to_string())?;
        self.flush_decoder_leftovers();

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

fn last_n_chars(text: &str, n: usize) -> String {
    let total = text.chars().count();
    if total <= n {
        return text.to_string();
    }
    text.chars().skip(total - n).collect()
}

/// 读取回执的键：同一会话、同一路径。
///
/// 行范围**不参与键**：回执不承担去重，重复 `read` 每次都完整回传并覆盖刷新同一条回执。
///
/// `session_id` 来自 `ToolUseContext.session_id`；ToolBox 链路为 `None`，
/// 所有 ToolBox 调用归入同一桶，不会与 Agent 会话互相抑制。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReadReceiptKey {
    pub session_id: Option<String>,
    pub path: String,
}

/// 行窗口「每行字符上限」的默认值（I57 §3.5）。
///
/// `read` 的 `MAX_LINE_CHARS` 与本模块 `ReadReceipt::utf8` 的默认宽度共用此常量，
/// 避免「闸门文案里的宽度」与「实际切行宽度」两处各写一个 `2000` 而漂移。
pub const DEFAULT_LINE_WINDOW_MAX_CHARS: usize = 2000;

/// 已读取过的文件记录。`revision` 变化即失效，`edit` / `write` 据此拒绝「未读过或已过期」。
///
/// `encoding` / `has_bom` 是写侧（`write` / `edit`）唯一的编码来源：
/// 它们只消费、不重判（I38c §14.2），因此必须由 `read` 成功时写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadReceipt {
    pub revision: String,
    pub tool_call_id: String,
    /// 实际生效编码的规范名（WHATWG label，如 `UTF-8` / `GBK`）。
    pub encoding: String,
    /// 文件是否带 BOM；写回时据此补回。
    pub has_bom: bool,
    /// 本次读取中**有行被切**的行数（I57 §3.5）。`0` 表示完整读到可见段；
    /// `> 0` 时 `edit` 拒绝写入——被切掉的尾部无法与 `old_string` 逐字校验。
    pub truncated_lines: usize,
    /// 本次读取使用的每行宽度，只供 `edit` 闸门文案归因（§6.3 的 `{width}`）。
    ///
    /// 在 A 内恒为 [`DEFAULT_LINE_WINDOW_MAX_CHARS`]、不产生任何分支；
    /// 它是为 I58 的 `super_read` 预留的——B 写自己的宽度，`edit` 文案自动正确。
    pub line_window_max_chars: usize,
}

impl ReadReceipt {
    /// 默认回执：UTF-8 无 BOM。适用于新建文件与尚未探测编码的路径。
    pub fn utf8(revision: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            tool_call_id: tool_call_id.into(),
            encoding: "UTF-8".to_string(),
            has_bom: false,
            truncated_lines: 0,
            line_window_max_chars: DEFAULT_LINE_WINDOW_MAX_CHARS,
        }
    }
}

/// 读取回执端口（P2 定义，P5 由 `read` / `edit` / `write` 使用）。
///
/// 只承担两件事：`edit` / `write` 的「本会话读过该文件」闸门，以及编码结论继承。
/// **不承担去重**：`read` 不做 `lookup` 判断是否跳过回传。
#[async_trait]
pub trait ReadReceiptStore: Send + Sync {
    async fn lookup(&self, key: &ReadReceiptKey) -> Option<ReadReceipt>;
    async fn record(&self, key: ReadReceiptKey, receipt: ReadReceipt);

    /// 按组件级前缀失效：移除 `path` 自身及其子树下的所有回执。
    ///
    /// 供 `delete` / `move` 在成功后调用，覆盖「删除后重建且长度 + mtime 恰好相同」
    /// 时修订号无法识别的情形（见 `os/path::is_component_prefix`）。
    async fn invalidate_path(&self, path: &Path);
}

/// 进程内回执存储（`dashmap`）。生命周期由构造方决定，跨会话共享同一实例。
pub struct InMemoryReadReceiptStore {
    receipts: DashMap<ReadReceiptKey, ReadReceipt>,
}

impl InMemoryReadReceiptStore {
    pub fn new() -> Self {
        Self {
            receipts: DashMap::new(),
        }
    }
}

impl Default for InMemoryReadReceiptStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReadReceiptStore for InMemoryReadReceiptStore {
    async fn lookup(&self, key: &ReadReceiptKey) -> Option<ReadReceipt> {
        self.receipts.get(key).map(|entry| entry.value().clone())
    }

    async fn record(&self, key: ReadReceiptKey, receipt: ReadReceipt) {
        self.receipts.insert(key, receipt);
    }

    async fn invalidate_path(&self, path: &Path) {
        self.receipts
            .retain(|key, _| !crate::os::path::is_component_prefix(Path::new(&key.path), path));
    }
}

fn result_file_name(tool_call_id: &str) -> String {
    let sanitized: String = tool_call_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() || sanitized.chars().all(|ch| ch == '_') {
        "result.txt".to_string()
    } else {
        format!("{}.txt", sanitized)
    }
}
