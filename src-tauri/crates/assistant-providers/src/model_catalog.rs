use std::collections::HashMap;
use std::sync::LazyLock;

static BUNDLED_CATALOG_JSON: &str = include_str!("../assets/models-dev.json");

/// 目录声明的推理可选项。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReasoningLevels {
    /// 模型提供开启/关闭开关（目录里的 `toggle`，或 effort 值含 `none`）
    pub toggle: bool,
    /// 可选的 effort 档位，保持目录顺序
    pub efforts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModelFacts {
    pub supports_vision: bool,
    pub context_limit: Option<u32>,
    pub output_limit: Option<u32>,
    pub reasoning: bool,
    pub tool_call: bool,
    /// 输入模态列表（"text"/"image"/"pdf"/"audio"/"video"）
    pub input_modalities: Vec<String>,
    /// 目录未声明可表达的 `reasoning_options` 时为 None
    pub reasoning_levels: Option<ReasoningLevels>,
}

const KNOWN_EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

fn parse_reasoning_levels(model_val: &serde_json::Value) -> Option<ReasoningLevels> {
    let raw = model_val.get("reasoning_options")?;
    let items: Vec<&serde_json::Value> = match raw {
        serde_json::Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    let mut levels = ReasoningLevels::default();
    for item in items {
        match item.get("type").and_then(|v| v.as_str()) {
            Some("toggle") => levels.toggle = true,
            Some("effort") => {
                let Some(values) = item.get("values").and_then(|v| v.as_array()) else {
                    continue;
                };
                for value in values {
                    let Some(value) = value.as_str() else {
                        continue;
                    };
                    let value = value.trim().to_ascii_lowercase();
                    if value == "none" || value == "off" {
                        levels.toggle = true;
                        continue;
                    }
                    if KNOWN_EFFORTS.contains(&value.as_str()) && !levels.efforts.contains(&value) {
                        levels.efforts.push(value);
                    }
                }
            }
            _ => {}
        }
    }
    if levels.toggle || !levels.efforts.is_empty() {
        Some(levels)
    } else {
        None
    }
}

static MODEL_CATALOG: LazyLock<HashMap<String, ModelFacts>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    let Ok(root) = serde_json::from_str::<serde_json::Value>(BUNDLED_CATALOG_JSON) else {
        return map;
    };
    let Some(providers) = root.as_object() else {
        return map;
    };
    for (_provider_id, provider_val) in providers {
        let Some(models) = provider_val.get("models").and_then(|v| v.as_object()) else {
            continue;
        };
        for (model_id, model_val) in models {
            let input_modalities = model_val
                .get("modalities")
                .and_then(|m| m.get("input"))
                .and_then(|i| i.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            let supports_vision = input_modalities.iter().any(|m| m == "image");

            let context_limit = model_val
                .get("limit")
                .and_then(|l| l.get("context"))
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok());

            let output_limit = model_val
                .get("limit")
                .and_then(|l| l.get("output"))
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok());

            let reasoning = model_val
                .get("reasoning")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let tool_call = model_val
                .get("tool_call")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            map.insert(
                model_id.clone(),
                ModelFacts {
                    supports_vision,
                    context_limit,
                    output_limit,
                    reasoning,
                    tool_call,
                    input_modalities,
                    reasoning_levels: parse_reasoning_levels(model_val),
                },
            );
        }
    }
    map
});

pub fn catalog_model_facts(model_id: &str) -> Option<ModelFacts> {
    MODEL_CATALOG.get(model_id).cloned()
}

pub fn catalog_input_modalities(model_id: &str) -> Option<Vec<String>> {
    MODEL_CATALOG
        .get(model_id)
        .map(|f| f.input_modalities.clone())
}

pub fn catalog_supports_vision(model_id: &str) -> Option<bool> {
    MODEL_CATALOG.get(model_id).map(|f| f.supports_vision)
}

pub fn catalog_context_limit(model_id: &str) -> Option<u32> {
    MODEL_CATALOG.get(model_id).and_then(|f| f.context_limit)
}

pub fn catalog_output_limit(model_id: &str) -> Option<u32> {
    MODEL_CATALOG.get(model_id).and_then(|f| f.output_limit)
}

pub fn catalog_reasoning(model_id: &str) -> Option<bool> {
    MODEL_CATALOG.get(model_id).map(|f| f.reasoning)
}

pub fn catalog_tool_call(model_id: &str) -> Option<bool> {
    MODEL_CATALOG.get(model_id).map(|f| f.tool_call)
}

/// 目录声明的推理可选项。
///
/// - `None`：模型不在目录内，或目录只声明了 `budget_tokens` 这类我们协议无法表达的 action
/// - `Some`：`toggle` 表示有开关，`efforts` 为可选档位（可能为空）
pub fn catalog_reasoning_levels(model_id: &str) -> Option<ReasoningLevels> {
    MODEL_CATALOG
        .get(model_id)
        .and_then(|f| f.reasoning_levels.clone())
}

const DEFAULT_CONTEXT_WINDOW: u32 = 128_000;

fn fallback_context_window(model_id: &str) -> u32 {
    let id = model_id.to_lowercase();
    if id.contains("gpt-4-32k") {
        32_768
    } else if id.contains("gpt-3.5-turbo-16k") {
        16_384
    } else {
        DEFAULT_CONTEXT_WINDOW
    }
}

pub fn get_context_window_tokens(model_id: &str) -> u32 {
    catalog_context_limit(model_id).unwrap_or_else(|| fallback_context_window(model_id))
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn catalog_openai_vision_models() {
        assert_eq!(catalog_supports_vision("gpt-4o"), Some(true));
        assert_eq!(catalog_supports_vision("gpt-4o-mini"), Some(true));
        assert_eq!(catalog_supports_vision("gpt-4-turbo"), Some(true));
        assert_eq!(catalog_supports_vision("o1"), Some(true));
        assert_eq!(catalog_supports_vision("o3"), Some(true));
        assert_eq!(catalog_supports_vision("o4-mini"), Some(true));
    }

    #[test]
    fn catalog_anthropic_vision_models() {
        assert_eq!(catalog_supports_vision("claude-sonnet-4-5"), Some(true));
        assert_eq!(catalog_supports_vision("claude-haiku-4-5"), Some(true));
        assert_eq!(catalog_supports_vision("claude-opus-4-5"), Some(true));
    }

    #[test]
    fn catalog_text_only_models() {
        assert_eq!(catalog_supports_vision("gpt-3.5-turbo"), Some(false));
        assert_eq!(catalog_supports_vision("qwen-mt-plus"), Some(false));
    }

    #[test]
    fn catalog_google_vision_models() {
        assert_eq!(catalog_supports_vision("gemini-2.5-pro"), Some(true));
        assert_eq!(catalog_supports_vision("gemini-2.5-flash"), Some(true));
    }

    #[test]
    fn catalog_unknown_model_returns_none() {
        assert_eq!(catalog_supports_vision("my-custom-llama-3b"), None);
        assert_eq!(catalog_supports_vision("totally-unknown-model"), None);
    }

    #[test]
    fn catalog_not_empty() {
        assert!(!MODEL_CATALOG.is_empty());
    }

    #[test]
    fn catalog_context_limits() {
        assert_eq!(catalog_context_limit("gpt-4o"), Some(128_000));
        assert_eq!(catalog_context_limit("gpt-3.5-turbo"), Some(16_385));
        assert_eq!(catalog_context_limit("gemini-2.5-pro"), Some(1_048_576));
        assert_eq!(catalog_context_limit("claude-sonnet-4-5"), Some(198_000));
        assert_eq!(catalog_context_limit("unknown-model"), None);
    }

    #[test]
    fn catalog_output_limits() {
        let gpt4o = catalog_model_facts("gpt-4o").unwrap();
        assert!(gpt4o.output_limit.is_some());
        assert!(gpt4o.output_limit.unwrap() > 0);
    }

    #[test]
    fn catalog_reasoning_flags() {
        assert_eq!(catalog_reasoning("o1"), Some(true));
        assert_eq!(catalog_reasoning("gpt-3.5-turbo"), Some(false));
        assert_eq!(catalog_reasoning("claude-sonnet-4-5"), Some(true));
        assert_eq!(catalog_reasoning("unknown-model"), None);
    }

    #[test]
    fn catalog_reasoning_levels_effort_only() {
        let levels = catalog_reasoning_levels("claude-fable-5").unwrap();
        assert!(!levels.toggle);
        assert_eq!(
            levels.efforts,
            vec!["low", "medium", "high", "xhigh", "max"]
        );
    }

    #[test]
    fn catalog_reasoning_levels_toggle_and_effort() {
        let levels = catalog_reasoning_levels("claude-haiku-5-5").unwrap();
        assert!(levels.toggle);
        assert_eq!(
            levels.efforts,
            vec!["low", "medium", "high", "xhigh", "max"]
        );
    }

    #[test]
    fn catalog_reasoning_levels_toggle_only() {
        let levels = catalog_reasoning_levels("glm-4.5").unwrap();
        assert!(levels.toggle);
        assert!(levels.efforts.is_empty());
    }

    #[test]
    fn catalog_reasoning_levels_unknown_and_unsupported() {
        assert_eq!(catalog_reasoning_levels("unknown-model"), None);
        // 目录只声明 budget_tokens，我们协议没有该通道，按「未知」处理
        assert_eq!(
            catalog_reasoning_levels("qwen3-next-80b-a3b-thinking"),
            None
        );
    }

    #[test]
    fn catalog_tool_call_flags() {
        assert_eq!(catalog_tool_call("gpt-4o"), Some(true));
        assert_eq!(catalog_tool_call("gpt-3.5-turbo"), Some(false));
        assert_eq!(catalog_tool_call("unknown-model"), None);
    }
}
