use std::collections::VecDeque;
use std::fmt::Write as _;

use crate::os::footer;

/// 裁切方向（I46 compact §2.7 D7）。
///
/// 策略是**工具的声明**，不是框架的全局开关：条目型工具保留前缀，命令类输出保留
/// 后缀，大段日志保留首尾。默认 `Head` 保持既有行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TruncationStrategy {
    /// 保留前缀：条目型（`glob` / `grep` / `list`）与 `read` 行窗口。
    #[default]
    Head,
    /// 保留后缀：命令类输出（错误栈、退出前末行）。当前无此类工具，为预留能力。
    Tail,
    /// 稳定首尾、省略中段：大段日志 / 抓取正文。
    HeadTail,
}

/// 单条工具输出的上限。
///
/// `max_bytes` / `max_lines` 为 0 表示该维度不设上限；`preview_bytes` 仅在发生截断时
/// 生效（0 表示不额外收缩），用于在截断后只保留一段预览。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLimits {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_bytes: usize,
    pub strategy: TruncationStrategy,
}

impl OutputLimits {
    /// 保留前缀（默认策略）。
    pub const fn head(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> Self {
        Self {
            max_bytes,
            max_lines,
            preview_bytes,
            strategy: TruncationStrategy::Head,
        }
    }

    /// 保留后缀。
    pub const fn tail(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> Self {
        Self {
            max_bytes,
            max_lines,
            preview_bytes,
            strategy: TruncationStrategy::Tail,
        }
    }

    /// 稳定首尾、省略中段。
    pub const fn head_tail(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> Self {
        Self {
            max_bytes,
            max_lines,
            preview_bytes,
            strategy: TruncationStrategy::HeadTail,
        }
    }
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self::head(0, 0, 0)
    }
}

/// 截断后的输出。`text` 已包含显式截断提示，调用方可直接回填模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncatedOutput {
    pub text: String,
    pub truncated: bool,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub returned_lines: usize,
}

/// 按行数 / 字节数截断输出，并在截断时显式告知模型实际规模。
///
/// 保留区间按 `limits.strategy` 选：`Head` 从尾部向内收缩，`Tail` 从头部向内收缩，
/// `HeadTail` 两侧各留一半并在中段插入省略标记。所有切点都落在 UTF-8 字符边界上，
/// 行优先策略尽量落在行边界，只有单行本身超过字节上限时才按字符边界硬切。
///
/// 截断提示只有三态（I46 compact §2.2 D2）：无省略时不出现 `[truncated:`；精确时写
/// `showing N of M`；无法计数时显式写 `total unknown`，不留空。
pub fn truncate_output(text: &str, limits: &OutputLimits) -> TruncatedOutput {
    let total_bytes = text.len();
    let total_lines = count_lines(text);

    let (head_end, tail_start) = retained_ranges(text, limits);
    let truncated = head_end < tail_start;
    let returned_lines = count_lines(&text[..head_end]) + count_lines(&text[tail_start..]);
    let retained_bytes = head_end + total_bytes - tail_start;

    let mut output = String::with_capacity(retained_bytes + TRUNCATION_NOTICE_RESERVE);
    output.push_str(&text[..head_end]);
    if truncated {
        // `HeadTail` 在省略区间不含整行时会退化为纯前缀，此时没有后段，写中段标记
        // 会把「半行」谎报成「省略 N 行」。判据只看是否真有后段，不看策略名。
        let has_tail = tail_start < total_bytes;
        if limits.strategy == TruncationStrategy::HeadTail && has_tail {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            let elided_lines = total_lines.saturating_sub(returned_lines);
            let _ = write!(output, "[... elided {} lines ...]", elided_lines);
            output.push('\n');
        } else if !output.is_empty()
            && !output.ends_with('\n')
            && !text[tail_start..].starts_with('\n')
        {
            // 后段起点被硬切在半行中：补一个换行，避免两段粘成一行。
            output.push('\n');
        }
        output.push_str(&text[tail_start..]);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        let _ = write!(
            output,
            "{}",
            footer::render(
                footer::TRUNCATED,
                format!(
                    "showing {} of {} lines, {} of {} bytes",
                    returned_lines, total_lines, retained_bytes, total_bytes
                )
            )
        );
    } else {
        output.push_str(&text[tail_start..]);
    }

    TruncatedOutput {
        text: output,
        truncated,
        total_bytes,
        total_lines,
        returned_lines,
    }
}

/// 按策略求保留区间 `[head_end, tail_start)`；未发生省略时两值重合或为 `(len, len)`。
fn retained_ranges(text: &str, limits: &OutputLimits) -> (usize, usize) {
    match limits.strategy {
        TruncationStrategy::Head => (
            head_end(
                text,
                limits.max_bytes,
                limits.max_lines,
                limits.preview_bytes,
            ),
            text.len(),
        ),
        TruncationStrategy::Tail => (
            0,
            tail_start(
                text,
                limits.max_bytes,
                limits.max_lines,
                limits.preview_bytes,
            ),
        ),
        TruncationStrategy::HeadTail => {
            let head_lines = half_budget(limits.max_lines, true);
            let tail_lines = half_budget(limits.max_lines, false);
            let head_bytes = half_budget(limits.max_bytes, true);
            let tail_bytes = half_budget(limits.max_bytes, false);
            let head_preview = half_budget(limits.preview_bytes, true);
            let tail_preview = half_budget(limits.preview_bytes, false);
            let head_stop = head_end(text, head_bytes, head_lines, head_preview);
            let tail_begin = tail_start(text, tail_bytes, tail_lines, tail_preview);
            if head_stop >= tail_begin {
                // 两侧已经覆盖全文：无需省略。
                return (text.len(), text.len());
            }
            if !text[head_stop..tail_begin].contains('\n') {
                // 省略区间不含整行（单行超预算被硬切）：中段标记无意义，退化为纯 head。
                return (head_stop, text.len());
            }
            (head_stop, tail_begin)
        }
    }
}

/// 把总预算对半分：`prefer_head` 时多分 1，保证 `head + tail == total`。
fn half_budget(total: usize, prefer_head: bool) -> usize {
    if total == 0 {
        0
    } else if prefer_head {
        total / 2 + total % 2
    } else {
        total / 2
    }
}

fn head_end(text: &str, max_bytes: usize, max_lines: usize, preview_bytes: usize) -> usize {
    let mut end = text.len();

    if max_lines > 0 {
        end = end.min(line_boundary(text, max_lines));
    }
    if max_bytes > 0 {
        end = end.min(line_aligned_byte_boundary(text, max_bytes));
    }
    if end < text.len() && preview_bytes > 0 {
        end = end.min(line_aligned_byte_boundary(text, preview_bytes));
    }

    end
}

fn tail_start(text: &str, max_bytes: usize, max_lines: usize, preview_bytes: usize) -> usize {
    let mut start = 0usize;

    if max_lines > 0 {
        start = start.max(tail_line_start(text, max_lines));
    }
    if max_bytes > 0 {
        start = start.max(tail_byte_start(text, start, max_bytes));
    }
    if start > 0 && preview_bytes > 0 {
        start = start.max(tail_byte_start(text, start, preview_bytes));
    }

    start
}

/// 最后 `max_lines` 行的起始偏移；用环形缓冲，内存只与行数上限相关。
fn tail_line_start(text: &str, max_lines: usize) -> usize {
    let mut starts: VecDeque<usize> = VecDeque::with_capacity(max_lines.saturating_add(1));
    starts.push_back(0);
    for (index, ch) in text.char_indices() {
        // 末尾换行不产生新行（与 `str::lines` 口径一致）。
        if ch == '\n' && index + 1 < text.len() {
            if starts.len() >= max_lines {
                starts.pop_front();
            }
            starts.push_back(index + 1);
        }
    }
    starts.front().copied().unwrap_or(0)
}

/// 从 `start` 起再向后收缩，使后段字节数不超过 `max_bytes`；切点优先落在行首。
fn tail_byte_start(text: &str, start: usize, max_bytes: usize) -> usize {
    if max_bytes == 0 || text.len().saturating_sub(start) <= max_bytes {
        return start;
    }
    let mut candidate = text.len() - max_bytes;
    if candidate < start {
        candidate = start;
    }
    while candidate < text.len() && !text.is_char_boundary(candidate) {
        candidate += 1;
    }
    if candidate == 0 || text[..candidate].ends_with('\n') {
        return candidate;
    }
    match text[candidate..].find('\n') {
        Some(offset) => candidate + offset + 1,
        None => candidate,
    }
}

fn count_lines(text: &str) -> usize {
    text.lines().count()
}

fn line_boundary(text: &str, max_lines: usize) -> usize {
    let mut seen = 0usize;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            seen += 1;
            if seen == max_lines {
                return index + 1;
            }
        }
    }
    text.len()
}

fn line_aligned_byte_boundary(text: &str, max_bytes: usize) -> usize {
    let end = byte_boundary(text, max_bytes);
    if end >= text.len() {
        return text.len();
    }
    match text[..end].rfind('\n') {
        Some(position) => position + 1,
        None => end,
    }
}

fn byte_boundary(text: &str, max_bytes: usize) -> usize {
    if max_bytes >= text.len() {
        return text.len();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

const TRUNCATION_NOTICE_RESERVE: usize = 128;

/// 把字节量格式化为审批文案里的一句话（`340 B` / `12.3 KB` / `3.4 MB`）。
///
/// 只用于展示，不参与任何判定，因此不追求精确，统一保留一位小数。
/// `write` 的覆盖预演与 `delete` 的扫描预演共用，避免两处各写一套换算。
pub fn human_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KB {
        format!("{} B", bytes)
    } else if value < KB * KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{:.1} MB", value / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> OutputLimits {
        OutputLimits::head(max_bytes, max_lines, preview_bytes)
    }

    #[test]
    fn tail_keeps_the_last_lines_and_reports_totals() {
        let text = "one\ntwo\nthree\nfour\n";
        let output = truncate_output(text, &OutputLimits::tail(0, 2, 0));

        assert!(output.truncated);
        assert_eq!(output.total_lines, 4);
        assert_eq!(output.returned_lines, 2);
        assert!(output.text.starts_with("three\nfour\n"));
        assert!(output.text.contains("showing 2 of 4 lines"));
    }

    #[test]
    fn tail_byte_limit_keeps_the_suffix_on_a_line_boundary() {
        let text = "one\ntwo\nthree\n";
        // 8 字节预算只够末行 `three\n`（6 字节）；再带上 `two\n` 需 10 字节，超预算。
        let output = truncate_output(text, &OutputLimits::tail(8, 0, 0));

        assert!(output.truncated);
        assert_eq!(output.returned_lines, 1);
        assert!(output.text.starts_with("three\n"));
        assert!(output.text.contains("showing 1 of 3 lines"));
        assert!(output.text.contains("6 of 14 bytes"));
    }

    #[test]
    fn tail_without_truncation_returns_the_original_text() {
        let text = "one\ntwo\n";
        let output = truncate_output(text, &OutputLimits::tail(1_000, 100, 0));

        assert!(!output.truncated);
        assert_eq!(output.text, text);
        assert_eq!(output.returned_lines, 2);
    }

    #[test]
    fn head_tail_keeps_both_ends_and_marks_the_elision() {
        let text = "one\ntwo\nthree\nfour\nfive\nsix\n";
        let output = truncate_output(text, &OutputLimits::head_tail(0, 4, 0));

        assert!(output.truncated);
        assert_eq!(output.total_lines, 6);
        assert_eq!(output.returned_lines, 4);
        assert!(output.text.starts_with("one\ntwo\n"));
        assert!(output.text.contains("[... elided 2 lines ...]"));
        assert!(output.text.contains("five\nsix\n"));
        assert!(output.text.contains("showing 4 of 6 lines"));
    }

    #[test]
    fn head_tail_byte_limit_splits_the_budget_between_ends() {
        let text = "one\ntwo\nthree\nfour\nfive\n";
        let output = truncate_output(text, &OutputLimits::head_tail(12, 0, 0));

        assert!(output.truncated);
        assert_eq!(output.returned_lines, 2);
        assert!(output.text.starts_with("one\n"));
        assert!(output.text.contains("[... elided 3 lines ...]"));
        assert!(output.text.contains("five\n"));
    }

    #[test]
    fn head_tail_within_limits_returns_the_original_text() {
        let text = "one\ntwo\n";
        let output = truncate_output(text, &OutputLimits::head_tail(1_000, 100, 0));

        assert!(!output.truncated);
        assert_eq!(output.text, text);
        assert!(!output.text.contains("elided"));
    }

    #[test]
    fn single_oversized_line_falls_back_to_head_in_head_tail_mode() {
        let text = "abcdefghij";
        let output = truncate_output(text, &OutputLimits::head_tail(4, 0, 0));

        assert!(output.truncated);
        assert!(output.text.starts_with("ab\n"));
        assert!(!output.text.contains("elided"));
    }

    #[test]
    fn every_strategy_reports_no_omission_when_nothing_is_dropped() {
        let text = "alpha\nbeta\n";
        for strategy in [
            TruncationStrategy::Head,
            TruncationStrategy::Tail,
            TruncationStrategy::HeadTail,
        ] {
            let limits = OutputLimits {
                max_bytes: 1_000,
                max_lines: 100,
                preview_bytes: 0,
                strategy,
            };
            let output = truncate_output(text, &limits);
            assert!(
                !output.truncated,
                "strategy {strategy:?} should not truncate"
            );
            assert_eq!(output.text, text, "strategy {strategy:?} should keep text");
            assert!(
                !output.text.contains("[truncated:"),
                "strategy {strategy:?}"
            );
        }
    }

    #[test]
    fn every_strategy_reports_exact_totals_when_truncated() {
        let text = "one\ntwo\nthree\nfour\n";
        for strategy in [
            TruncationStrategy::Head,
            TruncationStrategy::Tail,
            TruncationStrategy::HeadTail,
        ] {
            let limits = OutputLimits {
                max_bytes: 0,
                max_lines: 2,
                preview_bytes: 0,
                strategy,
            };
            let output = truncate_output(text, &limits);
            assert!(output.truncated, "strategy {strategy:?} should truncate");
            assert_eq!(output.total_lines, 4, "strategy {strategy:?}");
            assert_eq!(output.returned_lines, 2, "strategy {strategy:?}");
            assert!(
                output.text.contains("showing 2 of 4 lines"),
                "strategy {strategy:?} notice: {}",
                output.text
            );
            assert!(
                output.text.contains(&format!("of {} bytes]", text.len())),
                "strategy {strategy:?} notice: {}",
                output.text
            );
        }
    }

    #[test]
    fn output_within_limits_is_returned_unchanged() {
        let text = "alpha\nbeta\n";
        let output = truncate_output(text, &limits(1_000, 100, 0));

        assert_eq!(output.text, text);
        assert!(!output.truncated);
        assert_eq!(output.total_lines, 2);
        assert_eq!(output.total_bytes, text.len());
        assert_eq!(output.returned_lines, 2);
    }

    #[test]
    fn line_limit_truncates_and_reports_totals() {
        let text = "one\ntwo\nthree\nfour\n";
        let output = truncate_output(text, &limits(0, 2, 0));

        assert!(output.truncated);
        assert_eq!(output.total_lines, 4);
        assert_eq!(output.returned_lines, 2);
        assert!(output.text.starts_with("one\ntwo\n"));
        assert!(output.text.contains("showing 2 of 4 lines"));
        assert!(output.text.contains(&format!("of {} bytes", text.len())));
    }

    #[test]
    fn byte_limit_truncates_on_a_line_boundary() {
        let text = "one\ntwo\nthree\n";
        let output = truncate_output(text, &limits(8, 0, 0));

        assert!(output.truncated);
        assert_eq!(output.total_bytes, text.len());
        assert_eq!(output.returned_lines, 2);
        assert!(output.text.starts_with("one\ntwo\n"));
        assert!(output.text.contains("showing 2 of 3 lines"));
        let expected_total = format!("{} of {} bytes", 8, text.len());
        assert!(output.text.contains(&expected_total));
    }

    #[test]
    fn single_oversized_line_is_cut_at_a_char_boundary() {
        let text = "abcdefghij";
        let output = truncate_output(text, &limits(4, 0, 0));

        assert!(output.truncated);
        assert_eq!(output.returned_lines, 1);
        assert!(output.text.starts_with("abcd\n"));
        assert!(output.text.contains("showing 1 of 1 lines"));
        assert!(output.text.contains("4 of 10 bytes"));
    }

    #[test]
    fn preview_bytes_shrinks_truncated_output() {
        let text = "one\ntwo\nthree\nfour\n";
        let output = truncate_output(text, &limits(0, 2, 5));

        assert!(output.truncated);
        assert_eq!(output.returned_lines, 1);
        assert!(output.text.starts_with("one\n"));
        assert!(output.text.contains("showing 1 of 4 lines"));
    }

    #[test]
    fn empty_input_reports_zero_lines() {
        let output = truncate_output("", &limits(1_000, 100, 0));

        assert!(!output.truncated);
        assert_eq!(output.total_lines, 0);
        assert_eq!(output.returned_lines, 0);
        assert_eq!(output.text, "");
    }
}
