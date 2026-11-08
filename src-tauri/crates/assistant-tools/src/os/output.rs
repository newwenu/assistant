use std::fmt::Write as _;

/// 单条工具输出的上限。
///
/// `max_bytes` / `max_lines` 为 0 表示该维度不设上限；`preview_bytes` 仅在发生截断时
/// 生效（0 表示不额外收缩），用于在截断后只保留一段预览。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLimits {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_bytes: usize,
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
/// 保留内容尽量在行边界结束，避免把半行当成完整行返回；单行本身超过字节上限时
/// 才按字符边界硬切。截断提示包含「已截断」与「实际行数 / 字节数」，不静默丢弃。
pub fn truncate_output(text: &str, limits: &OutputLimits) -> TruncatedOutput {
    let total_bytes = text.len();
    let total_lines = count_lines(text);

    let end = retained_end(text, limits);
    let truncated = end < total_bytes;
    let retained = &text[..end];
    let returned_lines = count_lines(retained);

    let mut output = String::with_capacity(retained.len() + TRUNCATION_NOTICE_RESERVE);
    output.push_str(retained);
    if truncated {
        if !retained.ends_with('\n') {
            output.push('\n');
        }
        let _ = write!(
            output,
            "[truncated: showing {} of {} lines, {} of {} bytes]",
            returned_lines,
            total_lines,
            retained.len(),
            total_bytes
        );
    }

    TruncatedOutput {
        text: output,
        truncated,
        total_bytes,
        total_lines,
        returned_lines,
    }
}

fn retained_end(text: &str, limits: &OutputLimits) -> usize {
    let mut end = text.len();

    if limits.max_lines > 0 {
        end = end.min(line_boundary(text, limits.max_lines));
    }
    if limits.max_bytes > 0 {
        end = end.min(line_aligned_byte_boundary(text, limits.max_bytes));
    }

    if end < text.len() && limits.preview_bytes > 0 {
        end = end.min(line_aligned_byte_boundary(text, limits.preview_bytes));
    }

    end
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
        OutputLimits {
            max_bytes,
            max_lines,
            preview_bytes,
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
