/// 行视图：`(内容起点, 内容终点（不含 \n）, 下一行起点)`。
pub(super) fn line_spans(text: &str) -> Vec<(usize, usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            spans.push((start, index, index + 1));
            start = index + 1;
        }
    }
    if start < text.len() {
        spans.push((start, text.len(), text.len()));
    }
    spans
}

pub(super) fn line_index_at(text: &str, byte: usize) -> usize {
    text[..byte].bytes().filter(|byte| *byte == b'\n').count()
}

pub(super) fn line_number_at(text: &str, byte: usize) -> usize {
    line_index_at(text, byte) + 1
}

pub(super) fn pattern_lines(pattern: &str) -> Vec<&str> {
    let trimmed = pattern.strip_suffix('\n').unwrap_or(pattern);
    trimmed.split('\n').collect()
}

pub(super) fn substring_hits(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    haystack
        .match_indices(needle)
        .map(|(start, matched)| (start, start + matched.len()))
        .collect()
}
