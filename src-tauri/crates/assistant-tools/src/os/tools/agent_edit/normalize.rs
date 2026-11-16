use super::text::substring_hits;
use super::TAB_WIDTHS;

/// 归一化视图：逐字符 1:1 折叠，`orig_offsets` 把折叠视图的字符下标映射回原文。
struct FoldedView {
    text: String,
    orig_offsets: Vec<usize>,
    fold_offsets: Vec<usize>,
}

fn fold_char(ch: char) -> char {
    match ch {
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201C}' | '\u{201D}' => '"',
        other => other,
    }
}

fn fold_view(text: &str) -> FoldedView {
    let mut folded = String::with_capacity(text.len());
    let mut orig_offsets = Vec::new();
    let mut fold_offsets = Vec::new();
    for (index, ch) in text.char_indices() {
        orig_offsets.push(index);
        fold_offsets.push(folded.len());
        folded.push(fold_char(ch));
    }
    orig_offsets.push(text.len());
    fold_offsets.push(folded.len());
    FoldedView {
        text: folded,
        orig_offsets,
        fold_offsets,
    }
}

impl FoldedView {
    fn char_index(&self, byte: usize) -> usize {
        self.fold_offsets.partition_point(|offset| *offset < byte)
    }
}

pub(super) fn normalized_hits(content: &str, old: &str) -> Vec<(usize, usize)> {
    let view = fold_view(content);

    let mut candidates = Vec::new();
    candidates.push(fold_quotes(old));
    for width in TAB_WIDTHS {
        candidates.push(fold_quotes(&convert_leading_spaces_to_tabs(old, width)));
        candidates.push(fold_quotes(&convert_leading_tabs_to_spaces(old, width)));
    }

    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        let hits: Vec<(usize, usize)> = substring_hits(&view.text, &candidate)
            .into_iter()
            .map(|(start, end)| {
                (
                    view.orig_offsets[view.char_index(start)],
                    view.orig_offsets[view.char_index(end)],
                )
            })
            .collect();
        if !hits.is_empty() {
            return hits;
        }
    }
    Vec::new()
}

fn fold_quotes(text: &str) -> String {
    text.chars().map(fold_char).collect()
}

fn leading_whitespace_len(line: &str) -> usize {
    line.len() - line.trim_start_matches(&[' ', '\t'][..]).len()
}

fn convert_leading_tabs_to_spaces(text: &str, width: usize) -> String {
    let converted: Vec<String> = text
        .split('\n')
        .map(|line| {
            let prefix_len = leading_whitespace_len(line);
            let prefix = &line[..prefix_len];
            if !prefix.contains('\t') {
                return line.to_string();
            }
            format!(
                "{}{}",
                prefix.replace('\t', &" ".repeat(width)),
                &line[prefix_len..]
            )
        })
        .collect();
    converted.join("\n")
}

fn convert_leading_spaces_to_tabs(text: &str, width: usize) -> String {
    let converted: Vec<String> = text
        .split('\n')
        .map(|line| {
            let prefix_len = leading_whitespace_len(line);
            let prefix = &line[..prefix_len];
            if width == 0
                || prefix.is_empty()
                || prefix.contains('\t')
                || !prefix_len.is_multiple_of(width)
            {
                return line.to_string();
            }
            format!("{}{}", "\t".repeat(prefix_len / width), &line[prefix_len..])
        })
        .collect();
    converted.join("\n")
}
