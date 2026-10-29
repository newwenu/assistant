use crate::model_catalog;

/// 解析模型是否支持视觉输入（多模态图片）。
///
/// 判定优先级：
/// 1. `api_hint`（Provider API 自身的声明，权威来源）
/// 2. 内置 catalog（models-dev.json 的 modalities）
///
/// 注：`input_modalities_for` 的返回值也源自 catalog，仅供 DTO 输出使用，
/// 不参与判定，避免把 catalog 结论当作"显式声明"反向覆盖 API 声明。
pub fn resolve_support_vision(model_id: &str, api_hint: Option<bool>) -> bool {
    api_hint
        .or_else(|| model_catalog::catalog_supports_vision(model_id))
        .unwrap_or(false)
}

/// 模型的输入模态列表；catalog 未收录的模型默认仅支持文本。
pub fn input_modalities_for(model_id: &str) -> Vec<String> {
    model_catalog::catalog_input_modalities(model_id).unwrap_or_else(|| vec!["text".to_string()])
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn api_hint_wins_over_catalog() {
        assert!(!resolve_support_vision("gpt-4o", Some(false)));
        assert!(resolve_support_vision("gpt-3.5-turbo", Some(true)));
    }

    #[test]
    fn custom_model_falls_back_to_hint() {
        assert!(resolve_support_vision("my-custom-vlm", Some(true)));
        assert!(!resolve_support_vision("my-custom-llm", Some(false)));
    }

    #[test]
    fn catalog_fallback_when_no_hint() {
        assert!(resolve_support_vision("gpt-4o", None));
        assert!(!resolve_support_vision("gpt-3.5-turbo", None));
        assert!(resolve_support_vision("claude-sonnet-4-5", None));
        assert!(!resolve_support_vision("unknown-model", None));
    }

    #[test]
    fn input_modalities_for_defaults_to_text() {
        let known = input_modalities_for("gpt-4o");
        assert!(known.contains(&"image".to_string()));
        assert_eq!(
            input_modalities_for("unknown-model"),
            vec!["text".to_string()]
        );
    }

    /// modalities 输出与 vision 判定必须一致：含 image ⇔ 支持视觉
    #[test]
    fn modalities_and_vision_agree_when_no_hint() {
        for id in [
            "gpt-4o",
            "gpt-3.5-turbo",
            "claude-sonnet-4-5",
            "unknown-model",
        ] {
            let has_image = input_modalities_for(id).iter().any(|m| m == "image");
            assert_eq!(has_image, resolve_support_vision(id, None), "model: {id}");
        }
    }
}
