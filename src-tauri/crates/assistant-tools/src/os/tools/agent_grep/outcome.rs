pub(super) struct SearchOutcome {
    /// 已提交的文件（相对路径 + 该文件的渲染行）。文件内行号升序，文件间最后按路径排序。
    pub(super) files: Vec<(String, Vec<String>)>,
    /// 已提交的命中块数。
    pub(super) match_count: usize,
    /// 已提交的渲染行数（含上下文行与 `--` 分隔符）。
    pub(super) line_count: usize,
    pub(super) match_limit: bool,
    pub(super) line_limit: bool,
    pub(super) timed_out: bool,
    pub(super) binary_skipped: usize,
    pub(super) large_skipped: usize,
    pub(super) unreadable: usize,
    pub(super) lossy_lines: usize,
    pub(super) probe: Option<NonUtf8Probe>,
}

impl SearchOutcome {
    pub(super) fn new() -> Self {
        Self {
            files: Vec::new(),
            match_count: 0,
            line_count: 0,
            match_limit: false,
            line_limit: false,
            timed_out: false,
            binary_skipped: 0,
            large_skipped: 0,
            unreadable: 0,
            lossy_lines: 0,
            probe: None,
        }
    }

    pub(super) fn stopped(&self) -> bool {
        self.match_limit || self.line_limit || self.timed_out
    }

    /// 任一输出上限触发（`data.truncated` 的语义）。
    pub(super) fn truncated(&self) -> bool {
        self.match_limit || self.line_limit
    }

    pub(super) fn skipped_files(&self) -> usize {
        self.binary_skipped + self.large_skipped + self.unreadable
    }

    /// 文件间按路径字典序提交，保证同一子集的输出顺序可复现（[grep §3.3]）。
    pub(super) fn take_lines(&mut self) -> Vec<String> {
        self.files.sort_by(|left, right| left.0.cmp(&right.0));
        let mut lines = Vec::with_capacity(self.line_count);
        for (_, file_lines) in self.files.drain(..) {
            lines.extend(file_lines);
        }
        lines
    }
}

/// 零命中探测结论（上级 §14.7）：只回答「scope 里有没有非 UTF-8 文件」。
pub(super) struct NonUtf8Probe {
    pub(super) files: usize,
    pub(super) truncated: bool,
}
