//! 全仓库唯一的文本 token 估算口径（语义规范 §3.4 E1）。
//!
//! 本模块只提供纯函数，不依赖运行时、不读写 IO；任何需要「无 usage 时的
//! token 估算」的地方都必须复用这里的实现，禁止各自再写 `len / 4`。

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

/// 文本 token 估算（不含消息结构开销）：ASCII 4 字节/token，CJK 1 token/字，其余非 ASCII 2 字/token。
///
/// 返回值是**估算**，任何进入 `O` 或账单的地方都必须带 `estimated` 标记，不得冒充 provider 上报值。
pub fn estimate_text_tokens(text: &str) -> usize {
    let quarters: usize = text
        .chars()
        .map(|ch| {
            if ch.is_ascii() {
                1
            } else if is_cjk(ch) {
                4
            } else {
                2
            }
        })
        .sum();
    quarters.div_ceil(4)
}
