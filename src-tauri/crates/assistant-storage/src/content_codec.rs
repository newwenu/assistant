use serde_json::Value;

fn parse_content_blocks(content: &str) -> Option<Vec<Value>> {
    let parsed: Value = serde_json::from_str(content).ok()?;
    match parsed {
        // 空数组视同「不是块数组」，回退按原字符串处理（与前端 buildContentBlocks 不产出 [] 对齐）
        Value::Array(arr) if !arr.is_empty() => Some(arr),
        _ => None,
    }
}

fn is_text_block(v: &Value) -> bool {
    v.get("type").and_then(|t| t.as_str()) == Some("text")
}

fn text_of_block(v: &Value) -> Option<String> {
    v.get("text").and_then(|t| t.as_str()).map(String::from)
}

/// 非文本块的派生占位符：`image → [图片]`、`file → [文件 <name>]`（名字缺失则 `[文件]`）、
/// 其它类型 → `[<type>]`。占位符硬编码单一语言（受众是模型与索引，不是用户）。
fn block_placeholder(v: &Value) -> Option<String> {
    let kind = v.get("type").and_then(|t| t.as_str())?;
    match kind {
        "image" => Some("[图片]".to_string()),
        "file" => {
            let name = v
                .get("attachment")
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .filter(|n| !n.is_empty());
            Some(match name {
                Some(n) => format!("[文件 {}]", n),
                None => "[文件]".to_string(),
            })
        }
        other => Some(format!("[{}]", other)),
    }
}

/// 派生文本：text 块取文本，非 text 块取占位符（不静默丢弃）。
fn derived_text_of_block(v: &Value) -> Option<String> {
    if is_text_block(v) {
        text_of_block(v)
    } else {
        block_placeholder(v)
    }
}

fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// 空内容判定：解析不出块时按原字符串判定；有块时「存在任一非 text 块 → 非空」，
/// 否则「所有 text 块文本 trim 后为空 → 空」。
pub fn is_content_empty(content: &str) -> bool {
    if content.trim().is_empty() {
        return true;
    }
    if let Some(blocks) = parse_content_blocks(content) {
        return blocks.iter().all(|b| {
            is_text_block(b)
                && text_of_block(b)
                    .map(|t| t.trim().is_empty())
                    .unwrap_or(true)
        });
    }
    false
}

/// 固定宽度预览：按块累加（空格连接），文本块按剩余预算做字符边界截断，
/// 占位符整体放入或整体不放；结果长度不超过 `max_len + 1`（补一个 `…`）。
pub fn content_text_preview(content: &str, max_len: usize) -> String {
    let Some(blocks) = parse_content_blocks(content) else {
        let prefix = truncate_to_char_boundary(content, max_len);
        return if prefix.len() == content.len() {
            content.to_string()
        } else {
            format!("{}…", prefix)
        };
    };

    let mut out = String::new();
    let mut truncated = false;
    for block in blocks.iter() {
        let Some(piece) = derived_text_of_block(block) else {
            continue;
        };
        let sep_len = usize::from(!out.is_empty());
        if out.len() + sep_len + piece.len() <= max_len {
            if sep_len == 1 {
                out.push(' ');
            }
            out.push_str(&piece);
            continue;
        }
        if is_text_block(block) {
            let remaining = max_len.saturating_sub(out.len() + sep_len);
            let head = truncate_to_char_boundary(&piece, remaining);
            if !head.is_empty() {
                if sep_len == 1 {
                    out.push(' ');
                }
                out.push_str(head);
            }
        }
        truncated = true;
        break;
    }
    if truncated {
        out.push('…');
    }
    out
}

pub fn content_plain_text(content: &str) -> String {
    if let Some(blocks) = parse_content_blocks(content) {
        blocks
            .iter()
            .filter_map(derived_text_of_block)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content.to_string()
    }
}

/// 导出专用派生文本（I51 §5.2）：`text` 块取原文，`image` 块输出
/// `![image omitted]` 占位（图片正文是 base64 / 附件引用，禁止写进导出文件），
/// 其余非文本块沿用既有占位符。返回 `(正文, 被省略的图片块数)`。
///
/// 与 [`content_plain_text`] 分开实现的原因：后者被全文检索复用，占位口径不能改。
pub fn content_markdown_text(content: &str) -> (String, usize) {
    let Some(blocks) = parse_content_blocks(content) else {
        return (content.to_string(), 0);
    };
    let mut images = 0usize;
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|block| {
            if is_text_block(block) {
                return text_of_block(block);
            }
            if block.get("type").and_then(|t| t.as_str()) == Some("image") {
                images += 1;
                return Some("![image omitted]".to_string());
            }
            block_placeholder(block)
        })
        .collect();
    (parts.join("\n"), images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_passthrough() {
        assert!(!is_content_empty("hello"));
        assert_eq!(content_plain_text("hello"), "hello");
        assert_eq!(content_text_preview("hello world", 5), "hello…");
    }

    #[test]
    fn test_empty_content() {
        assert!(is_content_empty(""));
        assert!(is_content_empty("   "));
    }

    #[test]
    fn test_content_blocks_text_only() {
        let json = r#"[{"type":"text","text":"hello"}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "hello");
        assert_eq!(content_text_preview(json, 100), "hello");
    }

    #[test]
    fn test_content_blocks_image_only() {
        let json = r#"[{"type":"image","attachment":{"attachmentId":"abc.png"}}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "[图片]");
    }

    #[test]
    fn test_content_blocks_file_only() {
        let json = r#"[{"type":"file","attachment":{"attachmentId":"a","name":"a.pdf"}}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "[文件 a.pdf]");
        assert_eq!(content_text_preview(json, 100), "[文件 a.pdf]");
    }

    #[test]
    fn test_content_blocks_file_without_name() {
        let json = r#"[{"type":"file","attachment":{"attachmentId":"a"}}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "[文件]");
    }

    #[test]
    fn test_content_blocks_empty_text_with_image() {
        let json = r#"[{"type":"text","text":""},{"type":"image"}]"#;
        assert!(!is_content_empty(json));
    }

    #[test]
    fn test_content_blocks_blank_text_only() {
        let json = r#"[{"type":"text","text":"   "}]"#;
        assert!(is_content_empty(json));
    }

    #[test]
    fn test_content_blocks_unknown_type_placeholder() {
        let json = r#"[{"type":"artifact"}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "[artifact]");
    }

    #[test]
    fn test_empty_block_array_falls_back_to_raw() {
        // 空数组不视为块数组，按原字符串判定（前端 buildContentBlocks 不会产出 "[]"）
        assert!(!is_content_empty("[]"));
        assert_eq!(content_plain_text("[]"), "[]");
    }

    #[test]
    fn test_content_blocks_mixed() {
        let json = r#"[{"type":"text","text":"look at this"},{"type":"image","attachment":{"attachmentId":"abc.png"}}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "look at this\n[图片]");
        assert_eq!(content_text_preview(json, 100), "look at this [图片]");
    }

    #[test]
    fn test_markdown_text_omits_image_payload() {
        let json = r#"[{"type":"text","text":"look at this"},{"type":"image","source":{"type":"base64","data":"QUJD"}}]"#;
        let (body, images) = content_markdown_text(json);
        assert!(body.contains("look at this"));
        assert!(body.contains("![image omitted]"));
        assert_eq!(images, 1);
        assert!(
            !body.contains("QUJD"),
            "base64 payload must never reach the exported markdown"
        );
    }

    #[test]
    fn test_preview_never_cuts_placeholder() {
        // 占位符放不下时整体不放，补省略号
        let json = r#"[{"type":"text","text":"abc"},{"type":"image"}]"#;
        assert_eq!(content_text_preview(json, 5), "abc…");
        // 占位符放得下时整体放入（"[图片]" 占 8 字节）
        assert_eq!(content_text_preview(json, 12), "abc [图片]");
        // 首个块即占位符且放不下 → 只剩省略号，不出现半个占位符
        assert_eq!(content_text_preview(r#"[{"type":"image"}]"#, 3), "…");
    }

    #[test]
    fn test_preview_truncates_text_block_by_budget() {
        let json = r#"[{"type":"text","text":"abcdefghij"}]"#;
        assert_eq!(content_text_preview(json, 4), "abcd…");
    }

    #[test]
    fn test_content_blocks_multiple_text() {
        let json = r#"[{"type":"text","text":"line1"},{"type":"text","text":"line2"}]"#;
        assert_eq!(content_plain_text(json), "line1\nline2");
        assert_eq!(content_text_preview(json, 100), "line1 line2");
    }

    #[test]
    fn test_non_content_block_json() {
        let json = r#"{"not":"blocks"}"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), json);
    }
}
