use crate::capability::{pick_highest, CapabilityFact, Provenance, Support};
use crate::model_catalog;

/// 解析模型是否支持视觉输入（多模态图片），返回三态事实。
///
/// 候选顺序：Provider API 声明（权威来源）→ 内置目录断言 → 兜底。
/// 由 `pick_highest` 按 provenance 取最高，目录未收录且无声明时为 `Unknown`，
/// 不再把「不知道」写成 `false`。
pub fn resolve_support_vision(model_id: &str, api_hint: Option<bool>) -> CapabilityFact {
    let mut candidates = Vec::new();
    if let Some(hint) = api_hint {
        candidates.push(CapabilityFact::new(
            Support::from_bool(hint),
            Provenance::ApiDeclared,
        ));
    }
    if let Some(supported) = model_catalog::catalog_supports_vision(model_id) {
        candidates.push(CapabilityFact::new(
            Support::from_bool(supported),
            Provenance::Catalog,
        ));
    }
    pick_highest(&candidates)
}

/// 模型的输入模态列表；目录未收录返回 `None`（不再兜底 `["text"]`，
/// 把「不知道」写成「只有文本」是三态退化的一种）。
pub fn input_modalities_for(model_id: &str) -> Option<Vec<String>> {
    model_catalog::catalog_input_modalities(model_id)
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn api_hint_wins_over_catalog() {
        let unsupported = resolve_support_vision("gpt-4o", Some(false));
        assert_eq!(unsupported.support, Support::Unsupported);
        assert_eq!(unsupported.provenance, Provenance::ApiDeclared);

        let supported = resolve_support_vision("gpt-3.5-turbo", Some(true));
        assert_eq!(supported.support, Support::Supported);
        assert_eq!(supported.provenance, Provenance::ApiDeclared);
    }

    #[test]
    fn custom_model_falls_back_to_hint() {
        assert!(resolve_support_vision("my-custom-vlm", Some(true)).is_supported());
        assert_eq!(
            resolve_support_vision("my-custom-llm", Some(false)).support,
            Support::Unsupported
        );
    }

    #[test]
    fn catalog_assertion_when_no_hint() {
        let supported = resolve_support_vision("gpt-4o", None);
        assert_eq!(supported.support, Support::Supported);
        assert_eq!(supported.provenance, Provenance::Catalog);

        let unsupported = resolve_support_vision("gpt-3.5-turbo", None);
        assert_eq!(unsupported.support, Support::Unsupported);
        assert_eq!(unsupported.provenance, Provenance::Catalog);

        assert!(resolve_support_vision("claude-sonnet-4-5", None).is_supported());
    }

    /// 目录未收录且无声明 → `Unknown`（不是 `Unsupported`）
    #[test]
    fn unknown_model_yields_unknown_not_unsupported() {
        let fact = resolve_support_vision("unknown-model", None);
        assert!(fact.is_unknown());
        assert_eq!(fact.provenance, Provenance::Fallback);
    }

    #[test]
    fn input_modalities_for_returns_none_when_not_in_catalog() {
        let known = input_modalities_for("gpt-4o").expect("gpt-4o is in catalog");
        assert!(known.contains(&"image".to_string()));
        assert_eq!(input_modalities_for("unknown-model"), None);
    }

    /// 无声明时 modalities 与 vision 必须一致：目录有模态 ⇔ 有确定结论
    #[test]
    fn modalities_and_vision_agree_when_no_hint() {
        for id in [
            "gpt-4o",
            "gpt-3.5-turbo",
            "claude-sonnet-4-5",
            "unknown-model",
        ] {
            let has_image = input_modalities_for(id).map(|m| m.contains(&"image".to_string()));
            let fact = resolve_support_vision(id, None);
            let expected = match fact.support {
                Support::Supported => Some(true),
                Support::Unsupported => Some(false),
                Support::Unknown => None,
            };
            assert_eq!(has_image, expected, "model: {id}");
        }
    }
}
