use std::time::Duration;

use super::outcome::SearchOutcome;
use super::{MAX_MATCHES, MAX_OUTPUT_LINES};

pub(super) fn assistant_text(
    outcome: &SearchOutcome,
    lines: &[String],
    budget: Duration,
) -> String {
    // 超时且零行时不写 `(no matches)`，否则会多出一个前导空行（footer 前只留提示行）。
    let mut text = if lines.is_empty() {
        if outcome.timed_out {
            String::new()
        } else {
            "(no matches)".to_string()
        }
    } else {
        lines.join("\n")
    };

    if outcome.timed_out {
        push_line(
            &mut text,
            &format!(
                "[search stopped at the {}ms time budget; results may be incomplete; narrow \
                 'path' or 'include']",
                budget.as_millis()
            ),
        );
    } else if outcome.match_limit {
        push_line(
            &mut text,
            &format!(
                "[truncated: showing the first {} matching lines; total unknown; narrow 'path' \
                 or 'include']",
                MAX_MATCHES
            ),
        );
    } else if outcome.line_limit {
        push_line(
            &mut text,
            &format!(
                "[truncated: output line limit ({}) reached; total unknown; narrow 'path', \
                 'include', or reduce 'context']",
                MAX_OUTPUT_LINES
            ),
        );
    }

    // footer 标记集：各自独占一行、只给事实（[I38c §14.4] / [grep §3.1]）。
    if let Some(probe) = &outcome.probe {
        if probe.files > 0 {
            push_line(&mut text, &format!("[non-utf8: {} file(s)]", probe.files));
        }
    }
    let skipped = outcome.skipped_files();
    if skipped > 0 {
        push_line(&mut text, &format!("[skipped: {} file(s)]", skipped));
    }
    if outcome.lossy_lines > 0 {
        push_line(
            &mut text,
            &format!("[lossy: {} line(s)]", outcome.lossy_lines),
        );
    }
    text
}

fn push_line(text: &mut String, line: &str) {
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
}
