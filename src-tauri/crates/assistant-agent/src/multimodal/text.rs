use assistant_providers::types::{ContentBlock, FileAttachmentRef};

use crate::token_estimate::{estimate_text_tokens, truncate_to_tokens};

use super::validate::ext_of;
use super::{MIN_FILE_QUOTA_TOKENS, TRUNCATE_HEAD_PERCENT};

pub(super) fn project_text_file(
    att: &FileAttachmentRef,
    data: &[u8],
    quota: usize,
) -> Result<(ContentBlock, usize), String> {
    let text = decode_text(data).ok_or_else(|| "binary".to_string())?;
    let lang = lang_for(&att.name);
    let total = estimate_text_tokens(&text);

    if total <= quota {
        let block = ContentBlock::Text {
            text: render_text_file(&att.name, lang, &text, None),
        };
        let used = estimate_text_tokens(&block_text(&block));
        return Ok((block, used));
    }

    if quota < MIN_FILE_QUOTA_TOKENS {
        return Ok((placeholder_block(att), placeholder_tokens(att)));
    }

    let truncated = truncate_to_tokens(&text, quota, TRUNCATE_HEAD_PERCENT);
    let body = format!("{}\n...\n{}", truncated.head, truncated.tail);
    let header = format!(
        "[file: {} — truncated, kept head {} + tail {} tokens of {}]",
        att.name, truncated.head_tokens, truncated.tail_tokens, truncated.original_tokens
    );
    let block = ContentBlock::Text {
        text: render_text_file(&att.name, lang, &body, Some(header)),
    };
    let used = estimate_text_tokens(&block_text(&block));
    Ok((block, used))
}

pub(super) fn project_text_body(
    att: &FileAttachmentRef,
    text: &str,
    quota: usize,
) -> (ContentBlock, usize) {
    let lang = lang_for(&att.name);
    let total = estimate_text_tokens(text);
    if total <= quota {
        let block = ContentBlock::Text {
            text: render_text_file(&att.name, lang, text, None),
        };
        let used = estimate_text_tokens(&block_text(&block));
        return (block, used);
    }
    if quota < MIN_FILE_QUOTA_TOKENS {
        return (placeholder_block(att), placeholder_tokens(att));
    }
    let truncated = truncate_to_tokens(text, quota, TRUNCATE_HEAD_PERCENT);
    let body = format!("{}\n...\n{}", truncated.head, truncated.tail);
    let header = format!(
        "[file: {} — truncated, kept head {} + tail {} tokens of {}]",
        att.name, truncated.head_tokens, truncated.tail_tokens, truncated.original_tokens
    );
    let block = ContentBlock::Text {
        text: render_text_file(&att.name, lang, &body, Some(header)),
    };
    let used = estimate_text_tokens(&block_text(&block));
    (block, used)
}

fn block_text(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => text.clone(),
        _ => String::new(),
    }
}

fn decode_text(data: &[u8]) -> Option<String> {
    let text = if let Ok(text) = std::str::from_utf8(data) {
        text.to_string()
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(data, true);
        let encoding = detector.guess(None, true);
        let (text, _, _) = encoding.decode(data);
        text.into_owned()
    };

    // 扩展名白名单是唯一的「看起来像文本」信号；再用控制字符比例兜底，
    // 避免二进制文件改名 `.txt` 后被整段当成正文注入。
    let total = text.chars().count().max(1);
    let control = text
        .chars()
        .filter(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        .count();
    if control as f64 / total as f64 > 0.05 {
        return None;
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn has_fence(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("```"))
}

/// 文件正文渲染：原文已含围栏时不再外包一层，避免嵌套围栏；
/// 否则用代码围栏包住并标注语言。
fn render_text_file(name: &str, lang: &str, body: &str, header: Option<String>) -> String {
    let header = header.unwrap_or_else(|| format!("[file: {}]", name));
    if has_fence(body) {
        format!("{}\n{}", header, body)
    } else {
        format!("{}\n```{}\n{}\n```", header, lang, body)
    }
}

pub(super) fn is_text_file(name: &str) -> bool {
    let ext = ext_of(name);
    matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "csv"
            | "html"
            | "json"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "css"
            | "yaml"
            | "yml"
            | "xml"
            | "rs"
            | "toml"
            | "java"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "sh"
            | "ps1"
            | "sql"
            | "log"
    )
}

fn lang_for(name: &str) -> &'static str {
    match ext_of(name).as_str() {
        "md" => "markdown",
        "csv" => "csv",
        "html" => "html",
        "json" => "json",
        "py" => "python",
        "js" => "javascript",
        "ts" | "tsx" => "typescript",
        "css" => "css",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "rs" => "rust",
        "toml" => "toml",
        "java" => "java",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" => "cpp",
        "sh" => "bash",
        "ps1" => "powershell",
        "sql" => "sql",
        _ => "",
    }
}

pub(super) fn fallback_block(att: &FileAttachmentRef, reason: &str) -> ContentBlock {
    ContentBlock::Text {
        text: format!("[file: {} — {}]", att.handle_text(), reason),
    }
}

pub(super) fn file_not_found_block() -> ContentBlock {
    ContentBlock::Text {
        text: "[File not found]".to_string(),
    }
}

pub(super) fn placeholder_block(att: &FileAttachmentRef) -> ContentBlock {
    ContentBlock::Text {
        text: att.handle_text(),
    }
}

pub(super) fn placeholder_tokens(att: &FileAttachmentRef) -> usize {
    estimate_text_tokens(&att.handle_text())
}

pub fn file_handle_text(att: &FileAttachmentRef) -> String {
    att.handle_text()
}
