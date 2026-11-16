use grep_searcher::{Searcher, Sink, SinkContext, SinkContextKind, SinkMatch};
use std::time::Instant;

use super::outcome::SearchOutcome;
use super::{MAX_MATCHES, MAX_OUTPUT_LINES};

/// 单文件缓冲：命中行与上下文行先落到这里，文件结束时整体提交。
///
/// 按文件缓冲是「二进制整文件丢弃」与「文件间排序」的前提——逐行即时输出既无法
/// 在发现 NUL 后撤回已产出的行，也无法把同一文件的行聚在一起排序。
pub(super) struct FileSink<'a> {
    path: String,
    outcome: &'a mut SearchOutcome,
    deadline: Instant,
    buffer: Vec<String>,
    matches: usize,
    lossy: usize,
    binary: bool,
}

impl<'a> FileSink<'a> {
    pub(super) fn new(path: String, deadline: Instant, outcome: &'a mut SearchOutcome) -> Self {
        Self {
            path,
            outcome,
            deadline,
            buffer: Vec::new(),
            matches: 0,
            lossy: 0,
            binary: false,
        }
    }

    /// 当前文件还能再放多少行（命中行 + 上下文行 + 分隔符共用同一预算）。
    fn lines_left(&self) -> usize {
        MAX_OUTPUT_LINES.saturating_sub(self.outcome.line_count + self.buffer.len())
    }

    /// 提交或丢弃本文件的缓冲。
    pub(super) fn finish(&mut self, failed: bool) {
        if failed {
            return;
        }
        if self.binary {
            // 二进制文件整文件丢弃：NUL 之前也可能已有命中，逐行输出会把乱码混进结果。
            self.outcome.binary_skipped += 1;
            return;
        }
        self.outcome.match_count += self.matches;
        self.outcome.lossy_lines += self.lossy;
        self.outcome.line_count += self.buffer.len();
        if !self.buffer.is_empty() {
            let lines = std::mem::take(&mut self.buffer);
            self.outcome.files.push((self.path.clone(), lines));
        }
    }
}

impl Sink for FileSink<'_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        if self.outcome.match_count + self.matches >= MAX_MATCHES {
            self.outcome.match_limit = true;
            return Ok(false);
        }
        if Instant::now() >= self.deadline {
            self.outcome.timed_out = true;
            return Ok(false);
        }

        let lines: Vec<&[u8]> = mat.lines().collect();
        if lines.len() > self.lines_left() {
            self.outcome.line_limit = true;
            return Ok(false);
        }

        let first_line = mat.line_number().unwrap_or(0);
        for (index, line) in lines.into_iter().enumerate() {
            let number = first_line + index as u64;
            let (text, lossy) = trim_line_end(line);
            if lossy {
                self.lossy += 1;
            }
            self.buffer
                .push(format!("{}:{}:{}", self.path, number, text));
        }
        self.matches += 1;
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> Result<bool, std::io::Error> {
        if matches!(context.kind(), SinkContextKind::Other) {
            return Ok(true);
        }
        // 上下文行不经过 `matched`，大 `context` 的海量行只在这里出现，必须自查 deadline。
        if Instant::now() >= self.deadline {
            self.outcome.timed_out = true;
            return Ok(false);
        }
        if self.lines_left() == 0 {
            self.outcome.line_limit = true;
            return Ok(false);
        }

        let number = context.line_number().unwrap_or(0);
        // 上下文行同样以替换字符渲染给模型，因此与命中行一起计入 `lossyLines`。
        let (text, lossy) = trim_line_end(context.bytes());
        if lossy {
            self.lossy += 1;
        }
        self.buffer
            .push(format!("{}-{}-{}", self.path, number, text));
        Ok(true)
    }

    fn binary_data(
        &mut self,
        _searcher: &Searcher,
        _binary_byte_offset: u64,
    ) -> Result<bool, std::io::Error> {
        self.binary = true;
        // 返回 false 即放弃本文件：`quit` 语义下继续读没有意义，只会产出乱码行。
        Ok(false)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> Result<bool, std::io::Error> {
        if self.buffer.is_empty() {
            return Ok(true);
        }
        if self.lines_left() == 0 {
            self.outcome.line_limit = true;
            return Ok(false);
        }
        self.buffer.push("--".to_string());
        Ok(true)
    }
}

/// 去掉行尾换行；返回文本是否含替换字符（原始字节非法 UTF-8）。
///
/// 先走 `from_utf8` 快速路径（合法时零分配），失败才 `from_utf8_lossy`
/// 并置 `lossy`，让调用方能在结果里如实报告「这一行不是 UTF-8」。
fn trim_line_end(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.trim_end_matches(['\n', '\r']).to_string(), false),
        Err(_) => (
            String::from_utf8_lossy(bytes)
                .trim_end_matches(['\n', '\r'])
                .to_string(),
            true,
        ),
    }
}
