use assistant_agent::token_estimate::estimate_text_tokens;

#[test]
fn estimate_ascii_four_bytes_per_token() {
    let text = "a".repeat(1000);
    assert_eq!(estimate_text_tokens(&text), 250);
}

#[test]
fn estimate_cjk_one_token_per_char() {
    let text = "上下文占用".repeat(20);
    assert_eq!(text.chars().count(), 100);
    assert_eq!(estimate_text_tokens(&text), 100);
    assert!(estimate_text_tokens(&text) > text.len() / 4);
}

#[test]
fn estimate_mixed_text() {
    assert_eq!(estimate_text_tokens("abcdefgh中文测试"), 6);
}
