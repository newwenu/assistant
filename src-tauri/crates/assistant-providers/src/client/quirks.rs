use crate::base::ProviderError;
use serde_json::Value;

pub fn is_deepseek_url(url: &str) -> bool {
    url.contains("api.deepseek.com")
}

pub fn is_zhipuai_url(url: &str) -> bool {
    reqwest::Url::parse(url.trim()).ok().is_some_and(|parsed| {
        parsed.scheme() == "https"
            && parsed.port_or_known_default() == Some(443)
            && parsed.host_str().is_some_and(|host| {
                matches!(host.trim_end_matches('.'), "open.bigmodel.cn" | "api.z.ai")
            })
    })
}

pub fn is_dashscope_url(url: &str) -> bool {
    url.contains("dashscope.aliyuncs.com") || url.contains("dashscope-intl.aliyuncs.com")
}

pub fn is_siliconflow_url(url: &str) -> bool {
    url.contains("api.siliconflow.cn") || url.contains("api.siliconflow.com")
}

pub fn is_minimax_url(url: &str) -> bool {
    url.contains("api.minimax.chat") || url.contains("minimax")
}

pub fn is_glm_url(url: &str, model: &str) -> bool {
    if !url.contains("bigmodel.cn") {
        return false;
    }
    let lower = model.to_ascii_lowercase();
    lower.starts_with("glm-") || lower.starts_with("glm_")
}

pub fn is_deepseek_reasoning_effort_model(model_name: &str) -> bool {
    matches!(
        model_name.trim().to_ascii_lowercase().as_str(),
        "deepseek-v4-flash" | "deepseek-v4-pro"
    )
}

pub fn is_glm_reasoning_effort_model(model_name: &str) -> bool {
    matches!(
        model_name.trim().to_ascii_lowercase().as_str(),
        "glm-5.2" | "glm-5-2" | "glm-5p2"
    )
}

pub fn normalize_deepseek_reasoning_effort(model_name: &str, effort: &str) -> Option<&'static str> {
    let is_flash = model_name.trim().eq_ignore_ascii_case("deepseek-v4-flash");
    match effort.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "low" if is_flash => Some("low"),
        "high" => Some("high"),
        "max" => Some("max"),
        "low" | "medium" => Some("high"),
        "xhigh" => Some("max"),
        "none" | "minimal" => None,
        _ => None,
    }
}

pub fn normalize_glm_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "high" | "low" | "medium" => Some("high"),
        "max" | "xhigh" => Some("max"),
        "" | "none" | "minimal" => None,
        _ => None,
    }
}

pub fn normalize_generic_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        _ => None,
    }
}

pub fn parse_glm_major_minor(model_name: &str) -> Option<(u32, u32)> {
    let lower = model_name.to_ascii_lowercase();
    let tail = lower.strip_prefix("glm-")?;
    let mut parts = tail.split('-');
    let version = parts.next()?;

    let mut version_parts = version.split('.');
    let major = version_parts.next()?.parse().ok()?;
    let minor = version_parts
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    Some((major, minor))
}

pub fn should_append_tool_stream(url: &str, model_name: &str) -> bool {
    if url.contains("bigmodel.cn") || url.contains("api.z.ai") {
        return true;
    }

    if !url.contains("aliyuncs.com") {
        return false;
    }

    parse_glm_major_minor(model_name)
        .is_some_and(|(major, minor)| major > 4 || (major == 4 && minor >= 5))
}

pub fn apply_reasoning_toggle(request_body: &mut Value, enabled: bool, url: &str) -> bool {
    if is_dashscope_url(url) || is_siliconflow_url(url) {
        request_body["enable_thinking"] = Value::Bool(enabled);
        return true;
    }
    if is_deepseek_url(url) {
        request_body["thinking"] = serde_json::json!({
            "type": if enabled { "enabled" } else { "disabled" }
        });
        return true;
    }
    if is_zhipuai_url(url) {
        request_body["thinking"] = serde_json::json!({
            "type": if enabled { "enabled" } else { "disabled" }
        });
        return true;
    }
    false
}

pub fn wants_bearer_auth(url: &str) -> bool {
    let nous_portal = reqwest::Url::parse(url).ok().is_some_and(|parsed| {
        parsed.scheme() == "https"
            && parsed.host_str() == Some("inference-api.nousresearch.com")
            && parsed.port_or_known_default() == Some(443)
    });
    let xiaomi_mimo = reqwest::Url::parse(url).ok().is_some_and(|parsed| {
        parsed.scheme() == "https"
            && matches!(
                parsed.host_str(),
                Some("api.xiaomimimo.com" | "token-plan-cn.xiaomimimo.com")
            )
            && parsed.port_or_known_default() == Some(443)
            && (parsed.path() == "/anthropic" || parsed.path().starts_with("/anthropic/"))
    });
    nous_portal
        || xiaomi_mimo
        || url.contains("bigmodel.cn")
        || url.contains("api.z.ai")
        || url.contains("api.kimi.com/coding")
        || ((url.contains("api.moonshot.cn") || url.contains("api.moonshot.ai"))
            && url.contains("/anthropic"))
}

pub fn is_valid_chat_completion_chunk(event_json: &Value) -> bool {
    matches!(
        event_json.get("object").and_then(|v| v.as_str()),
        Some("chat.completion.chunk") | Some("chat.completion")
    )
}

pub fn extract_sse_api_error(event_json: &Value) -> Option<ProviderError> {
    let error = event_json.get("error")?;

    let code = error
        .get("code")
        .or_else(|| error.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let message = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .unwrap_or("An error occurred during streaming");

    let lower_code = code.to_ascii_lowercase();
    let lower_message = message.to_ascii_lowercase();

    if lower_code.contains("context_length_exceeded")
        || lower_message.contains("context_length_exceeded")
        || lower_message.contains("maximum context length")
    {
        return Some(ProviderError::ContextOverflow(message.to_string()));
    }

    if lower_code.contains("rate_limit") || lower_message.contains("rate_limit") {
        return Some(ProviderError::RateLimited {
            retry_after_ms: None,
        });
    }

    if lower_code.contains("authentication")
        || lower_code.contains("auth")
        || lower_message.contains("invalid api key")
        || lower_message.contains("unauthorized")
    {
        return Some(ProviderError::AuthFailed(message.to_string()));
    }

    Some(ProviderError::StreamError(message.to_string()))
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn url_detection_table() {
        let openai = "https://api.openai.com/v1/chat/completions";
        assert!(is_deepseek_url(
            "https://api.deepseek.com/v1/chat/completions"
        ));
        assert!(!is_deepseek_url(openai));

        assert!(is_zhipuai_url(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions"
        ));
        assert!(is_zhipuai_url("https://api.z.ai/v1/chat/completions"));
        assert!(!is_zhipuai_url(openai));

        assert!(is_dashscope_url(
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        ));
        assert!(is_dashscope_url(
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
        ));
        assert!(!is_dashscope_url(openai));

        assert!(is_siliconflow_url(
            "https://api.siliconflow.cn/v1/chat/completions"
        ));
        assert!(is_siliconflow_url(
            "https://api.siliconflow.com/v1/chat/completions"
        ));
        assert!(!is_siliconflow_url(openai));

        assert!(is_minimax_url(
            "https://api.minimax.chat/v1/chat/completions"
        ));
        assert!(!is_minimax_url(openai));

        assert!(is_glm_url(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "glm-4"
        ));
        assert!(!is_glm_url(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "chatglm"
        ));
        assert!(!is_glm_url(openai, "glm-4"));
    }

    #[test]
    fn reasoning_effort_models() {
        assert!(is_deepseek_reasoning_effort_model("deepseek-v4-flash"));
        assert!(is_deepseek_reasoning_effort_model("deepseek-v4-pro"));
        assert!(!is_deepseek_reasoning_effort_model("deepseek-r1"));
        assert!(is_glm_reasoning_effort_model("glm-5.2"));
        assert!(is_glm_reasoning_effort_model("glm-5-2"));
        assert!(!is_glm_reasoning_effort_model("glm-4"));
    }

    #[test]
    fn normalize_effort_table() {
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-flash", "low"),
            Some("low")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "low"),
            Some("high")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "medium"),
            Some("high")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "high"),
            Some("high")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "max"),
            Some("max")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "xhigh"),
            Some("max")
        );
        assert_eq!(
            normalize_deepseek_reasoning_effort("deepseek-v4-pro", "none"),
            None
        );
        assert_eq!(normalize_glm_reasoning_effort("high"), Some("high"));
        assert_eq!(normalize_glm_reasoning_effort("low"), Some("high"));
        assert_eq!(normalize_glm_reasoning_effort("max"), Some("max"));
        assert_eq!(normalize_glm_reasoning_effort("none"), None);
    }

    #[test]
    fn parse_glm_version() {
        assert_eq!(parse_glm_major_minor("glm-4"), Some((4, 0)));
        assert_eq!(parse_glm_major_minor("glm-4.5"), Some((4, 5)));
        assert_eq!(parse_glm_major_minor("glm-5.2"), Some((5, 2)));
        assert_eq!(parse_glm_major_minor("chatglm"), None);
    }

    #[test]
    fn tool_stream_rules() {
        assert!(should_append_tool_stream(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "glm-4"
        ));
        assert!(should_append_tool_stream(
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "glm-4.5"
        ));
        assert!(!should_append_tool_stream(
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "glm-4"
        ));
        assert!(!should_append_tool_stream(
            "https://api.openai.com/v1/chat/completions",
            "gpt-4o"
        ));
    }

    #[test]
    fn reasoning_toggle() {
        let mut body = serde_json::json!({});
        assert!(apply_reasoning_toggle(
            &mut body,
            true,
            "https://api.deepseek.com/v1/chat/completions"
        ));
        assert_eq!(body["thinking"]["type"], "enabled");

        let mut body = serde_json::json!({});
        assert!(apply_reasoning_toggle(
            &mut body,
            true,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        ));
        assert_eq!(body["enable_thinking"], true);

        let mut body = serde_json::json!({});
        assert!(apply_reasoning_toggle(
            &mut body,
            false,
            "https://api.siliconflow.cn/v1/chat/completions"
        ));
        assert_eq!(body["enable_thinking"], false);

        let mut body = serde_json::json!({});
        assert!(!apply_reasoning_toggle(
            &mut body,
            true,
            "https://api.openai.com/v1/chat/completions"
        ));
    }

    #[test]
    fn bearer_auth_rules() {
        assert!(!wants_bearer_auth("https://api.anthropic.com/v1/messages"));
        assert!(wants_bearer_auth(
            "https://open.bigmodel.cn/api/paas/v4/chat/completions"
        ));
        assert!(wants_bearer_auth("https://api.kimi.com/coding/v1/models"));
        assert!(wants_bearer_auth(
            "https://api.moonshot.cn/anthropic/v1/messages"
        ));
        assert!(!wants_bearer_auth("https://api.moonshot.cn/v1/models"));
    }

    #[test]
    fn valid_chunk_detection() {
        assert!(is_valid_chat_completion_chunk(
            &serde_json::json!({"object": "chat.completion.chunk"})
        ));
        assert!(is_valid_chat_completion_chunk(
            &serde_json::json!({"object": "chat.completion"})
        ));
        assert!(!is_valid_chat_completion_chunk(
            &serde_json::json!({"object": "ping"})
        ));
        assert!(!is_valid_chat_completion_chunk(
            &serde_json::json!({"id": "chatcmpl-123"})
        ));
    }

    #[test]
    fn extract_sse_api_errors() {
        let ctx = serde_json::json!({"error": {"code": "context_length_exceeded", "message": "max 8192 tokens"}});
        assert!(matches!(
            extract_sse_api_error(&ctx).unwrap(),
            ProviderError::ContextOverflow(_)
        ));

        let rate =
            serde_json::json!({"error": {"type": "rate_limit_error", "message": "Rate limit"}});
        assert!(matches!(
            extract_sse_api_error(&rate).unwrap(),
            ProviderError::RateLimited { .. }
        ));

        let auth = serde_json::json!({"error": {"code": "authentication_error", "message": "Invalid API key"}});
        assert!(matches!(
            extract_sse_api_error(&auth).unwrap(),
            ProviderError::AuthFailed(_)
        ));

        let generic = serde_json::json!({"error": {"message": "Internal server error"}});
        assert!(matches!(
            extract_sse_api_error(&generic).unwrap(),
            ProviderError::StreamError(_)
        ));

        let string_err = serde_json::json!({"error": "something went wrong"});
        assert!(matches!(
            extract_sse_api_error(&string_err).unwrap(),
            ProviderError::StreamError(_)
        ));

        assert!(extract_sse_api_error(&serde_json::json!({"choices": []})).is_none());
    }
}
