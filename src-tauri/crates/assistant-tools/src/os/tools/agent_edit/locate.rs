use crate::os::agent_read::LINE_NUMBER_SEPARATOR;

use super::normalize::normalized_hits;
use super::text::{line_spans, pattern_lines, substring_hits};

/// 一次命中的定位结果。`hits` 的偏移全部落在 LF 归一后的文本上。
pub(super) struct Located {
    pub(super) hits: Vec<(usize, usize)>,
    replacement: String,
    pub(super) tier: &'static str,
    /// 行级匹配（档 4 / 5）的命中片段包含行尾换行，替换时需补齐。
    line_based: bool,
}

/// 渐进匹配主流程：档 1 → 2 → 3 → 4 → 5 → 6，命中即停。
///
/// 每档都先拿到该档的全部命中，再交给调用方判定「唯一 / 歧义」，
/// 因此不存在「先取首个再回退」的路径。全部档位失败返回 `None`。
pub(super) fn locate(content: &str, old: &str, new: &str) -> Option<Located> {
    let exact = substring_hits(content, old);
    if !exact.is_empty() {
        return Some(Located {
            hits: exact,
            replacement: new.to_string(),
            tier: "exact",
            line_based: false,
        });
    }

    if let Some(stripped) = strip_read_line_prefixes(old) {
        let hits = substring_hits(content, &stripped);
        if !hits.is_empty() {
            return Some(Located {
                hits,
                replacement: new.to_string(),
                tier: "prefix-stripped",
                line_based: false,
            });
        }
    }

    if !old.ends_with('\n') {
        let mut candidate = old.to_string();
        candidate.push('\n');
        let hits = substring_hits(content, &candidate);
        if !hits.is_empty() {
            let mut replacement = new.to_string();
            if !replacement.ends_with('\n') {
                replacement.push('\n');
            }
            return Some(Located {
                hits,
                replacement,
                tier: "trailing-newline",
                line_based: false,
            });
        }
    }

    let trimmed_end = line_hits(content, old, LineMode::TrimEnd);
    if !trimmed_end.is_empty() {
        return Some(Located {
            hits: trimmed_end,
            replacement: new.to_string(),
            tier: "trim-end",
            line_based: true,
        });
    }

    let trimmed = line_hits(content, old, LineMode::Trim);
    if !trimmed.is_empty() {
        return Some(Located {
            hits: trimmed,
            replacement: new.to_string(),
            tier: "trim",
            line_based: true,
        });
    }

    let normalized = normalized_hits(content, old);
    if !normalized.is_empty() {
        return Some(Located {
            hits: normalized,
            replacement: new.to_string(),
            tier: "normalized",
            line_based: false,
        });
    }

    None
}

pub(super) fn effective_replacement(
    content: &str,
    hit: (usize, usize),
    located: &Located,
) -> String {
    let mut replacement = located.replacement.clone();
    if located.line_based && content[hit.0..hit.1].ends_with('\n') && !replacement.ends_with('\n') {
        replacement.push('\n');
    }
    replacement
}

pub(super) fn apply_hits(content: &str, hits: &[(usize, usize)], located: &Located) -> String {
    let mut updated = content.to_string();
    for hit in hits.iter().rev() {
        let replacement = effective_replacement(content, *hit, located);
        updated.replace_range(hit.0..hit.1, &replacement);
    }
    updated
}

/// 档 2 的行号前缀剥离：仅当每个非空行都形如「空格 + 数字 + tab 或 `→`」时才生效。
pub(super) fn strip_read_line_prefixes(old: &str) -> Option<String> {
    let mut stripped = String::with_capacity(old.len());
    let mut saw_content = false;
    for (index, line) in old.split('\n').enumerate() {
        if index > 0 {
            stripped.push('\n');
        }
        if line.trim().is_empty() {
            stripped.push_str(line);
            continue;
        }
        let rest = strip_one_line_prefix(line)?;
        saw_content = true;
        stripped.push_str(rest);
    }
    saw_content.then_some(stripped)
}

fn strip_one_line_prefix(line: &str) -> Option<&str> {
    let without_indent = line.trim_start_matches(' ');
    let digits = without_indent
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .count();
    if digits == 0 {
        return None;
    }
    let rest = &without_indent[digits..];
    if let Some(after_separator) = rest.strip_prefix(LINE_NUMBER_SEPARATOR) {
        return Some(after_separator);
    }
    let after_arrow = rest.trim_start_matches(' ').strip_prefix('→')?;
    Some(after_arrow.strip_prefix(' ').unwrap_or(after_arrow))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineMode {
    TrimEnd,
    Trim,
}

fn line_hits(content: &str, pattern: &str, mode: LineMode) -> Vec<(usize, usize)> {
    let spans = line_spans(content);
    let pattern_lines = pattern_lines(pattern);
    if spans.is_empty() || pattern_lines.is_empty() || pattern_lines.len() > spans.len() {
        return Vec::new();
    }

    let normalize = |line: &str| match mode {
        LineMode::TrimEnd => line.trim_end().to_string(),
        LineMode::Trim => line.trim().to_string(),
    };
    let needle: Vec<String> = pattern_lines.iter().map(|line| normalize(line)).collect();

    let mut hits = Vec::new();
    for start in 0..=(spans.len() - needle.len()) {
        let matched = (0..needle.len()).all(|offset| {
            let (line_start, line_end, _) = spans[start + offset];
            normalize(&content[line_start..line_end]) == needle[offset]
        });
        if matched {
            let first = spans[start];
            let last = spans[start + needle.len() - 1];
            hits.push((first.0, last.2));
        }
    }
    hits
}
