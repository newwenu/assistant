use crate::base::{ProviderError, ProviderKind};
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

fn is_deepseek_flash(model_name: &str) -> bool {
    model_name.trim().eq_ignore_ascii_case("deepseek-v4-flash")
}

/// 通用端点（OpenAI Chat Completions / Anthropic / Gemini）可原样下发的档位。
pub const GENERIC_EFFORTS: [&str; 3] = ["low", "medium", "high"];
/// GLM 端点可原样下发的档位。
pub const GLM_EFFORTS: [&str; 2] = ["high", "max"];
/// DeepSeek 非 flash 模型可原样下发的档位。
pub const DEEPSEEK_EFFORTS: [&str; 2] = ["high", "max"];
/// DeepSeek flash 额外支持 `low`，故单独成常量。
pub const DEEPSEEK_FLASH_EFFORTS: [&str; 3] = ["low", "high", "max"];

const GENERIC_ALIASES: [(&str, &str); 0] = [];
const DEEPSEEK_ALIASES: [(&str, &str); 2] = [("medium", "high"), ("xhigh", "max")];
const DEEPSEEK_NON_FLASH_ALIASES: [(&str, &str); 3] =
    [("medium", "high"), ("xhigh", "max"), ("low", "high")];
const GLM_ALIASES: [(&str, &str); 3] = [("low", "high"), ("medium", "high"), ("xhigh", "max")];

/// 归一化的单一实现：先查可表达集合（命中即原样返回），再查别名表，都不命中返回 `None`。
fn normalize_with(
    effort: &str,
    expressible: &[&'static str],
    aliases: &[(&'static str, &'static str)],
) -> Option<&'static str> {
    let lower = effort.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if let Some(value) = expressible.iter().find(|value| **value == lower.as_str()) {
        return Some(*value);
    }
    aliases
        .iter()
        .find(|(from, _)| *from == lower.as_str())
        .map(|(_, to)| *to)
}

pub fn normalize_deepseek_reasoning_effort(model_name: &str, effort: &str) -> Option<&'static str> {
    if is_deepseek_flash(model_name) {
        normalize_with(effort, &DEEPSEEK_FLASH_EFFORTS, &DEEPSEEK_ALIASES)
    } else {
        normalize_with(effort, &DEEPSEEK_EFFORTS, &DEEPSEEK_NON_FLASH_ALIASES)
    }
}

pub fn normalize_glm_reasoning_effort(effort: &str) -> Option<&'static str> {
    normalize_with(effort, &GLM_EFFORTS, &GLM_ALIASES)
}

pub fn normalize_generic_reasoning_effort(effort: &str) -> Option<&'static str> {
    normalize_with(effort, &GENERIC_EFFORTS, &GENERIC_ALIASES)
}

/// 该端点可原样下发的档位；`None` 表示该端点不归一、任意非空档位原样下发。
pub fn expressible_efforts(kind: &ProviderKind, model_id: &str) -> Option<Vec<&'static str>> {
    match kind {
        ProviderKind::Anthropic | ProviderKind::Gemini | ProviderKind::GeminiCodeAssist => {
            Some(GENERIC_EFFORTS.to_vec())
        }
        // responses 流直接把 effort 写入 reasoning.effort，不做归一
        ProviderKind::OpenAIResponses => None,
        ProviderKind::OpenAI | ProviderKind::Custom { .. } => {
            if is_deepseek_reasoning_effort_model(model_id) {
                Some(if is_deepseek_flash(model_id) {
                    DEEPSEEK_FLASH_EFFORTS.to_vec()
                } else {
                    DEEPSEEK_EFFORTS.to_vec()
                })
            } else if is_glm_reasoning_effort_model(model_id) {
                Some(GLM_EFFORTS.to_vec())
            } else {
                Some(GENERIC_EFFORTS.to_vec())
            }
        }
    }
}

/// 目录声明档位里会被该端点丢弃的项；**逐项跑归一化**，别名项（被改写成别的档位）不计入。
pub fn unsupported_efforts(
    kind: &ProviderKind,
    model_id: &str,
    declared: &[String],
) -> Vec<String> {
    if expressible_efforts(kind, model_id).is_none() {
        return Vec::new();
    }
    declared
        .iter()
        .filter(|effort| normalize_for(kind, model_id, effort).is_none())
        .cloned()
        .collect()
}

fn normalize_for(kind: &ProviderKind, model_id: &str, effort: &str) -> Option<&'static str> {
    match kind {
        ProviderKind::Anthropic | ProviderKind::Gemini | ProviderKind::GeminiCodeAssist => {
            normalize_generic_reasoning_effort(effort)
        }
        ProviderKind::OpenAIResponses => None,
        ProviderKind::OpenAI | ProviderKind::Custom { .. } => {
            if is_deepseek_reasoning_effort_model(model_id) {
                normalize_deepseek_reasoning_effort(model_id, effort)
            } else if is_glm_reasoning_effort_model(model_id) {
                normalize_glm_reasoning_effort(effort)
            } else {
                normalize_generic_reasoning_effort(effort)
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const PROBES: [&str; 9] = [
        "", "low", "medium", "high", "xhigh", "max", "minimal", "none", "weird",
    ];

    fn declared(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// 等价性护栏：归一化结果必须与改动前逐项一致，且恒落在可表达集合内。
    #[test]
    fn normalization_stays_within_expressible_set() {
        type Normalizer = fn(&str, &str) -> Option<&'static str>;
        let cases: [(&ProviderKind, &str, Normalizer); 4] = [
            (
                &ProviderKind::OpenAI,
                "deepseek-v4-flash",
                normalize_deepseek_reasoning_effort,
            ),
            (
                &ProviderKind::OpenAI,
                "deepseek-v4-pro",
                normalize_deepseek_reasoning_effort,
            ),
            (&ProviderKind::OpenAI, "glm-5.2", |_, effort| {
                normalize_glm_reasoning_effort(effort)
            }),
            (&ProviderKind::OpenAI, "gpt-4o", |_, effort| {
                normalize_generic_reasoning_effort(effort)
            }),
        ];
        for (kind, model_id, normalize) in cases {
            let expressible = expressible_efforts(kind, model_id).unwrap();
            for probe in PROBES {
                let result = normalize(model_id, probe);
                if let Some(value) = result {
                    assert!(
                        expressible.contains(&value),
                        "{model_id} / {probe} → {value} 不在可表达集合内"
                    );
                }
            }
        }
    }

    #[test]
    fn normalization_matches_legacy_table() {
        let legacy_deepseek = |is_flash: bool, effort: &str| match effort {
            "" => None,
            "low" if is_flash => Some("low"),
            "high" => Some("high"),
            "max" => Some("max"),
            "low" | "medium" => Some("high"),
            "xhigh" => Some("max"),
            _ => None,
        };
        let legacy_glm = |effort: &str| match effort {
            "high" | "low" | "medium" => Some("high"),
            "max" | "xhigh" => Some("max"),
            _ => None,
        };
        let legacy_generic = |effort: &str| match effort {
            "low" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            _ => None,
        };

        for probe in PROBES {
            assert_eq!(
                normalize_deepseek_reasoning_effort("deepseek-v4-flash", probe),
                legacy_deepseek(true, probe),
                "flash / {probe}"
            );
            assert_eq!(
                normalize_deepseek_reasoning_effort("deepseek-v4-pro", probe),
                legacy_deepseek(false, probe),
                "pro / {probe}"
            );
            assert_eq!(
                normalize_glm_reasoning_effort(probe),
                legacy_glm(probe),
                "glm / {probe}"
            );
            assert_eq!(
                normalize_generic_reasoning_effort(probe),
                legacy_generic(probe),
                "generic / {probe}"
            );
        }
    }

    #[test]
    fn generic_endpoint_drops_only_unknown_words() {
        let kind = ProviderKind::OpenAI;
        assert_eq!(
            expressible_efforts(&kind, "gpt-4o").unwrap(),
            GENERIC_EFFORTS.to_vec()
        );
        assert_eq!(
            unsupported_efforts(
                &kind,
                "gpt-4o",
                &declared(&["minimal", "low", "medium", "high", "xhigh", "max"])
            ),
            declared(&["minimal", "xhigh", "max"])
        );
    }

    #[test]
    fn alias_words_are_not_reported_as_unsupported() {
        // glm：low / medium 被改写成 high，仍会下发，故不算不支持
        assert_eq!(
            unsupported_efforts(
                &ProviderKind::OpenAI,
                "glm-5.2",
                &declared(&["low", "medium", "high", "xhigh", "max"])
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            expressible_efforts(&ProviderKind::OpenAI, "glm-5.2").unwrap(),
            GLM_EFFORTS.to_vec()
        );

        // deepseek pro：low 归一到 high，不在可表达集合，也不记不支持
        assert_eq!(
            unsupported_efforts(
                &ProviderKind::OpenAI,
                "deepseek-v4-pro",
                &declared(&["low", "medium", "high", "xhigh", "max"])
            ),
            Vec::<String>::new()
        );
        assert!(
            !expressible_efforts(&ProviderKind::OpenAI, "deepseek-v4-pro")
                .unwrap()
                .contains(&"low")
        );

        // deepseek flash：low 可表达
        assert!(
            expressible_efforts(&ProviderKind::OpenAI, "deepseek-v4-flash")
                .unwrap()
                .contains(&"low")
        );
    }

    #[test]
    fn responses_endpoint_does_not_normalize() {
        assert_eq!(
            expressible_efforts(&ProviderKind::OpenAIResponses, "gpt-5"),
            None
        );
        assert!(unsupported_efforts(
            &ProviderKind::OpenAIResponses,
            "gpt-5",
            &declared(&["minimal", "xhigh"])
        )
        .is_empty());
    }

    #[test]
    fn native_endpoints_share_the_generic_set() {
        for kind in [
            ProviderKind::Anthropic,
            ProviderKind::Gemini,
            ProviderKind::GeminiCodeAssist,
        ] {
            assert_eq!(
                expressible_efforts(&kind, "any-model").unwrap(),
                GENERIC_EFFORTS.to_vec()
            );
        }
    }
}
