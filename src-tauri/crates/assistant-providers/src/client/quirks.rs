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
