use log::{debug, log_enabled, trace, Level};
use serde_json::Value;

pub(crate) fn redact_api_key(url: &str) -> String {
    let mut result = url.to_string();
    if let Some(pos) = result.find("key=") {
        let start = pos + 4;
        if let Some(end) = result[start..]
            .find('&')
            .map(|i| start + i)
            .or(Some(result.len()))
        {
            let key_len = end - start;
            if key_len > 4 {
                result.replace_range(start..end, &format!("{}***", &result[start..start + 4]));
            } else {
                result.replace_range(start..end, "***");
            }
        }
    }
    result
}

pub(crate) fn redact_json_api_key(body: &Value) -> Value {
    let mut body = body.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.remove("apiKey");
        obj.remove("api_key");
    }
    body
}

pub(crate) fn log_request(url: &str, body: &Value, attempt: u32) {
    let safe_url = redact_api_key(url);
    let safe_body = redact_json_api_key(body);
    debug!(
        "[trace] request attempt={} url={} body_len={}",
        attempt,
        safe_url,
        serde_json::to_string(&safe_body)
            .map(|s| s.len())
            .unwrap_or(0),
    );
}

/// 按字节上限截断字符串，边界回退到最近的字符边界，避免多字节字符被切断。
pub(crate) fn truncate_utf8(data: &str, max_bytes: usize) -> String {
    if data.len() <= max_bytes {
        return data.to_string();
    }
    let end = data
        .char_indices()
        .take_while(|(i, _)| *i < max_bytes)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...(truncated {} bytes)", &data[..end], data.len() - end)
}

pub(crate) fn log_response_chunk(attempt: u32, chunk_index: u32, data: &str) {
    if log_enabled!(Level::Trace) {
        let preview = truncate_utf8(data, 200);
        trace!(
            "[trace] response attempt={} chunk={} data={}",
            attempt,
            chunk_index,
            preview,
        );
    }
}

pub(crate) fn log_error(attempt: u32, error: &str) {
    debug!("[trace] error attempt={} error={}", attempt, error);
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn redact_api_key_with_key_param() {
        let url = "https://example.com/v1/models?key=AIzaSyABCDEF1234567890&page=1";
        let result = redact_api_key(url);
        assert!(result.contains("AIza***"));
        assert!(!result.contains("ABCDEF1234567890"));
        assert!(result.contains("page=1"));
    }

    #[test]
    fn redact_api_key_no_key_param() {
        let url = "https://example.com/v1/models";
        let result = redact_api_key(url);
        assert_eq!(result, url);
    }

    #[test]
    fn redact_api_key_short_key() {
        let url = "https://example.com?key=abc";
        let result = redact_api_key(url);
        assert!(result.contains("***"));
    }

    #[test]
    fn redact_json_removes_api_key_variants() {
        let body = serde_json::json!({"apiKey": "secret", "contents": []});
        let result = redact_json_api_key(&body);
        assert!(result.get("apiKey").is_none());
        assert!(result.get("contents").is_some());

        let body = serde_json::json!({"api_key": "secret", "model": "gemini-pro"});
        let result = redact_json_api_key(&body);
        assert!(result.get("api_key").is_none());
        assert!(result.get("model").is_some());
    }

    #[test]
    fn truncate_utf8_keeps_short_input_intact() {
        assert_eq!(truncate_utf8("abc", 200), "abc");
        assert_eq!(truncate_utf8("中文", 200), "中文");
    }

    #[test]
    fn truncate_utf8_never_splits_multibyte_char() {
        let data = "中文中文中文";
        let result = truncate_utf8(data, 7);
        assert!(result.starts_with("中文中"), "result: {result}");
        assert!(result.ends_with("(truncated 9 bytes)"), "result: {result}");
    }

    #[test]
    fn truncate_utf8_handles_boundary_at_exact_char_end() {
        let data = "中文中文";
        let result = truncate_utf8(data, 6);
        assert_eq!(result, "中文...(truncated 6 bytes)");
    }
}
