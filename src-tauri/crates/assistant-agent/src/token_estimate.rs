//! 全仓库唯一的 token 估算口径（语义规范 §3.4 E1）。
//!
//! 本模块只提供纯函数，不依赖运行时、不读写 IO；任何需要「无 usage 时的
//! token 估算」的地方都必须复用这里的实现，禁止各自再写 `len / 4`。

use assistant_providers::types::{parse_content, ChatMessage, ContentBlock};

/// 原生 PDF 附件的固定记账常量。
///
/// PDF 的 base64 长度与真实 token 成本无关：1MB 文件的 base64 约 133 万字符，
/// 按文本口径会估成 30 万 token，导致「挂任何 PDF 都必然超预算」。
/// 这里按「页数 × 每页估算」的粗粒度建模，P2 落地后可按页数校准。
pub const PDF_NATIVE_TOKENS: usize = 30_000;

/// 原生 PDF 的扩展名判定（大小写不敏感）。
pub fn is_pdf_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".pdf")
}

/// CJK 与全角符号：这些字符在主流分词器里约 1 token / 字，不能按 ASCII 的 4 字节/token 计。
fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x303F
            | 0x3040..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2FA1F
            | 0x30000..=0x3134F
    )
}

/// 单字符权重，单位是「四分之一 token」：ASCII 记 1，CJK 记 4，其余记 2。
///
/// 暴露给截断逻辑按字符累加；外部仍应优先使用 [`estimate_text_tokens`]。
pub fn char_weight_quarters(ch: char) -> usize {
    if ch.is_ascii() {
        1
    } else if is_cjk(ch) {
        4
    } else {
        2
    }
}

/// 文本 token 估算（不含消息结构开销）：ASCII 4 字节/token，CJK 1 token/字，其余非 ASCII 2 字/token。
///
/// 返回值是**估算**，任何进入 `O` 或账单的地方都必须带 `estimated` 标记，不得冒充 provider 上报值。
pub fn estimate_text_tokens(text: &str) -> usize {
    let quarters: usize = text.chars().map(char_weight_quarters).sum();
    quarters.div_ceil(4)
}

/// 图片 token 估算：`85 + 170 × ceil(w/512) × ceil(h/512)`。
///
/// 768×768 恰为 `85 + 170 × 4 = 765`，与 I29 E2 的固定值自洽；
/// 宽高缺失时回退 `765`，避免历史附件因缺少尺寸而被系统性低估或高估。
pub fn estimate_image_tokens(width: Option<u32>, height: Option<u32>) -> usize {
    match (width, height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => {
            let tiles = (w.div_ceil(512) as usize) * (h.div_ceil(512) as usize);
            85 + 170 * tiles
        }
        _ => 765,
    }
}

/// 单个内容块的 token 估算。
///
/// `File` 已缓存且为 PDF 时按 [`PDF_NATIVE_TOKENS`] 固定记账，
/// **不得**按 base64 长度线性增长。
pub fn estimate_block_tokens(block: &ContentBlock) -> usize {
    match block {
        ContentBlock::Text { text } => estimate_text_tokens(text),
        ContentBlock::Image { attachment, .. } => {
            estimate_image_tokens(attachment.width, attachment.height)
        }
        ContentBlock::File {
            attachment,
            cached_data,
            ..
        } => match cached_data.as_deref().filter(|d| !d.is_empty()) {
            Some(_) if is_pdf_name(&attachment.name) => PDF_NATIVE_TOKENS,
            Some(data) => estimate_text_tokens(data),
            None => estimate_text_tokens(&attachment.handle_text()),
        },
        ContentBlock::Thinking { thinking, .. } => estimate_text_tokens(thinking),
        ContentBlock::ToolUse { input, .. } => estimate_text_tokens(input),
        ContentBlock::ToolResult { output, .. } => estimate_text_tokens(output),
    }
}

fn blocks_of(msg: &ChatMessage) -> Option<Vec<ContentBlock>> {
    msg.content_blocks
        .clone()
        .or_else(|| parse_content(&msg.content))
}

/// 出站消息的 token 估算总和，与 offload 预算共用同一口径。
pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|msg| match blocks_of(msg) {
            Some(blocks) => blocks.iter().map(estimate_block_tokens).sum(),
            None => estimate_text_tokens(&msg.content),
        })
        .sum()
}

/// 只统计「非文件正文」的 token：`File` 块按 0 计，其余块按正常口径。
///
/// 文件预算 = 输入预算 - 本函数结果，避免文件与文本 / 图片重复占用同一份预算。
pub fn estimate_non_file_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|msg| match blocks_of(msg) {
            Some(blocks) => blocks
                .iter()
                .filter(|b| !matches!(b, ContentBlock::File { .. }))
                .map(estimate_block_tokens)
                .sum(),
            None => estimate_text_tokens(&msg.content),
        })
        .sum()
}

/// 头尾截断结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncatedText {
    /// 头部片段（已按需补齐 Markdown 围栏闭合）。
    pub head: String,
    /// 尾部片段（已按需补齐 Markdown 围栏开启）。
    pub tail: String,
    pub original_tokens: usize,
    pub head_tokens: usize,
    pub tail_tokens: usize,
    pub truncated: bool,
}

/// 取头部不超过 `max_tokens` 的字符前缀。
pub fn take_head_tokens(text: &str, max_tokens: usize) -> (String, usize) {
    let limit_quarters = max_tokens.saturating_mul(4);
    let mut quarters = 0usize;
    let mut end = 0usize;
    for (idx, ch) in text.char_indices() {
        let w = char_weight_quarters(ch);
        if quarters + w > limit_quarters {
            break;
        }
        quarters += w;
        end = idx + ch.len_utf8();
    }
    (text[..end].to_string(), quarters.div_ceil(4))
}

/// 取尾部不超过 `max_tokens` 的字符后缀。
pub fn take_tail_tokens(text: &str, max_tokens: usize) -> (String, usize) {
    let limit_quarters = max_tokens.saturating_mul(4);
    let mut quarters = 0usize;
    let mut start = text.len();
    for (idx, ch) in text.char_indices().rev() {
        let w = char_weight_quarters(ch);
        if quarters + w > limit_quarters {
            break;
        }
        quarters += w;
        start = idx;
    }
    (text[start..].to_string(), quarters.div_ceil(4))
}

/// 返回 `text[..byte_end]` 处仍处于打开状态的 Markdown 围栏语言（无语言为空串）。
///
/// 只统计以 ` ``` ` 开头的行，奇偶切换；`byte_end` 必须落在字符边界。
fn open_fence_at(text: &str, byte_end: usize) -> Option<String> {
    let mut open: Option<String> = None;
    let prefix = &text[..byte_end.min(text.len())];
    for line in prefix.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            match open {
                Some(_) => open = None,
                None => open = Some(rest.trim().to_string()),
            }
        }
    }
    open
}

/// 按 token 预算做头尾截断：头部 `head_percent`，尾部其余部分。
///
/// 未超预算时原样返回且 `truncated = false`；超预算时按字符遍历切分，
/// 不再混用字节 / 字符口径。截断点落在 Markdown 围栏内部时自动补齐围栏。
pub fn truncate_to_tokens(text: &str, max_tokens: usize, head_percent: u8) -> TruncatedText {
    let original_tokens = estimate_text_tokens(text);
    if original_tokens <= max_tokens {
        return TruncatedText {
            head: text.to_string(),
            tail: String::new(),
            original_tokens,
            head_tokens: original_tokens,
            tail_tokens: 0,
            truncated: false,
        };
    }

    let head_percent = head_percent.min(100);
    let head_budget = max_tokens * head_percent as usize / 100;
    let tail_budget = max_tokens.saturating_sub(head_budget);

    let (mut head_raw, head_tokens) = take_head_tokens(text, head_budget);
    let head_end = head_raw.len();
    let (mut tail_raw, tail_tokens) = take_tail_tokens(text, tail_budget);
    let tail_start = text.len().saturating_sub(tail_raw.len());

    if head_end >= tail_start {
        // 预算过小导致片段重叠：只保留头部，避免重复内容。
        return TruncatedText {
            head: head_raw,
            tail: String::new(),
            original_tokens,
            head_tokens,
            tail_tokens: 0,
            truncated: true,
        };
    }

    if open_fence_at(text, head_end).is_some() {
        head_raw.push_str("\n```");
    }
    if let Some(lang) = open_fence_at(text, tail_start) {
        tail_raw = format!("```{}\n{}", lang, tail_raw);
    }

    TruncatedText {
        head: head_raw,
        tail: tail_raw,
        original_tokens,
        head_tokens,
        tail_tokens,
        truncated: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_providers::types::{FileAttachmentRef, ImageAttachmentRef};

    #[test]
    fn text_weights_are_stable() {
        assert_eq!(estimate_text_tokens("hello world"), 3);
        assert_eq!(estimate_text_tokens("你好世界"), 4);
        // 中英混排：4 个汉字（4 token）+ 4 个 ASCII（1 token）
        assert_eq!(estimate_text_tokens("你好世界abcd"), 5);
    }

    #[test]
    fn image_768_is_765() {
        assert_eq!(estimate_image_tokens(Some(768), Some(768)), 765);
        assert_eq!(estimate_image_tokens(None, None), 765);
    }

    #[test]
    fn image_grows_with_tiles() {
        assert_eq!(estimate_image_tokens(Some(512), Some(512)), 85 + 170);
        assert_eq!(estimate_image_tokens(Some(1024), Some(512)), 85 + 170 * 2);
        assert!(
            estimate_image_tokens(Some(2048), Some(2048))
                > estimate_image_tokens(Some(1024), Some(1024))
        );
    }

    #[test]
    fn pdf_is_not_linear_in_base64() {
        let small = ContentBlock::File {
            attachment: FileAttachmentRef {
                attachment_id: "a.pdf".to_string(),
                name: "a.pdf".to_string(),
                bytes: 1024,
            },
            cached_data: Some("A".repeat(4_000)),
            file_path: None,
        };
        let large = ContentBlock::File {
            attachment: FileAttachmentRef {
                attachment_id: "b.pdf".to_string(),
                name: "b.pdf".to_string(),
                bytes: 1_048_576,
            },
            cached_data: Some("A".repeat(1_300_000)),
            file_path: None,
        };
        assert_eq!(estimate_block_tokens(&small), PDF_NATIVE_TOKENS);
        assert_eq!(estimate_block_tokens(&large), PDF_NATIVE_TOKENS);
    }

    #[test]
    fn file_without_cache_uses_handle_text() {
        let block = ContentBlock::File {
            attachment: FileAttachmentRef {
                attachment_id: "a.txt".to_string(),
                name: "a.txt".to_string(),
                bytes: 1024,
            },
            cached_data: None,
            file_path: None,
        };
        assert!(estimate_block_tokens(&block) > 0);
        assert!(estimate_block_tokens(&block) < PDF_NATIVE_TOKENS);
    }

    #[test]
    fn image_block_uses_attachment_dimensions() {
        let block = ContentBlock::Image {
            attachment: ImageAttachmentRef {
                attachment_id: "img.png".to_string(),
                media_type: Some("image/png".to_string()),
                bytes: Some(1),
                width: Some(768),
                height: Some(768),
                name: None,
            },
            cached_data: None,
            file_path: None,
        };
        assert_eq!(estimate_block_tokens(&block), 765);
    }

    #[test]
    fn truncate_keeps_head_and_tail() {
        let text: String = (0..1000).map(|i| format!("line {}\n", i)).collect();
        let result = truncate_to_tokens(&text, 100, 60);
        assert!(result.truncated);
        assert!(result.head_tokens <= 60);
        assert!(result.tail_tokens <= 40);
        assert!(result.head.starts_with("line 0"));
        assert!(result.tail.ends_with("line 999\n"));
        assert!(result.head_tokens + result.tail_tokens <= 100);
    }

    #[test]
    fn truncate_not_triggered_when_within_budget() {
        let result = truncate_to_tokens("hello", 100, 60);
        assert!(!result.truncated);
        assert_eq!(result.head, "hello");
        assert!(result.tail.is_empty());
    }

    #[test]
    fn truncate_closes_and_reopens_fence() {
        let text = "```rust\nfn main() {\n    let x = 1;\n    let y = 2;\n}\n```\n";
        let result = truncate_to_tokens(text, 4, 60);
        assert!(result.truncated);
        assert!(result.head.starts_with("```rust"));
        assert!(result.head.ends_with("\n```"));
        assert!(result.tail.starts_with("```rust\n"));
    }

    #[test]
    fn truncate_cjk_stays_within_budget() {
        let text = "中文内容".repeat(10_000);
        let result = truncate_to_tokens(&text, 1_000, 60);
        assert!(result.truncated);
        assert!(result.head_tokens <= 600);
        assert!(result.tail_tokens <= 400);
    }
}
