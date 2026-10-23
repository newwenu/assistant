use serde_json::Value;

fn parse_content_blocks(content: &str) -> Option<Vec<Value>> {
    let parsed: Value = serde_json::from_str(content).ok()?;
    match parsed {
        Value::Array(arr) if !arr.is_empty() => Some(arr),
        _ => None,
    }
}

fn is_text_block(v: &Value) -> bool {
    v.get("type").and_then(|t| t.as_str()) == Some("text")
}

fn extract_text_from_block(v: &Value) -> Option<String> {
    if is_text_block(v) {
        v.get("text").and_then(|t| t.as_str()).map(String::from)
    } else {
        None
    }
}

pub fn is_content_empty(content: &str) -> bool {
    if content.trim().is_empty() {
        return true;
    }
    if let Some(blocks) = parse_content_blocks(content) {
        if blocks.is_empty() {
            return true;
        }
        return blocks
            .iter()
            .filter_map(extract_text_from_block)
            .all(|t| t.trim().is_empty());
    }
    false
}

pub fn content_text_preview(content: &str, max_len: usize) -> String {
    if let Some(blocks) = parse_content_blocks(content) {
        let plain = blocks
            .iter()
            .filter_map(extract_text_from_block)
            .collect::<Vec<_>>()
            .join(" ");
        if plain.len() <= max_len {
            plain
        } else {
            let mut end = max_len;
            while end > 0 && !plain.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &plain[..end])
        }
    } else {
        if content.len() <= max_len {
            content.to_string()
        } else {
            let mut end = max_len;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &content[..end])
        }
    }
}

pub fn content_plain_text(content: &str) -> String {
    if let Some(blocks) = parse_content_blocks(content) {
        blocks
            .iter()
            .filter_map(extract_text_from_block)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content.to_string()
    }
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
        let json = r#"[{"type":"image","attachment_id":"abc.png"}]"#;
        assert!(is_content_empty(json));
        assert_eq!(content_plain_text(json), "");
    }

    #[test]
    fn test_content_blocks_mixed() {
        let json = r#"[{"type":"text","text":"look at this"},{"type":"image","attachment_id":"abc.png"}]"#;
        assert!(!is_content_empty(json));
        assert_eq!(content_plain_text(json), "look at this");
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