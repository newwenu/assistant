use crate::model_catalog;

pub fn resolve_support_vision(model_id: &str, api_hint: Option<bool>) -> bool {
    let catalog_result = model_catalog::catalog_supports_vision(model_id);
    api_hint.or(catalog_result).unwrap_or(false)
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_api_hint_over_catalog() {
        assert_eq!(resolve_support_vision("gpt-4o", Some(false)), false);
        assert_eq!(resolve_support_vision("gpt-3.5-turbo", Some(true)), true);
    }

    #[test]
    fn resolve_api_hint_fills_gap() {
        assert_eq!(resolve_support_vision("my-custom-vlm", Some(true)), true);
        assert_eq!(resolve_support_vision("my-custom-llm", Some(false)), false);
    }

    #[test]
    fn resolve_falls_back_to_catalog() {
        let gpt4o = model_catalog::catalog_supports_vision("gpt-4o");
        assert_eq!(
            resolve_support_vision("gpt-4o", None),
            gpt4o.unwrap_or(false)
        );
        assert_eq!(resolve_support_vision("gpt-3.5-turbo", None), false);
        assert_eq!(resolve_support_vision("claude-sonnet-4-5", None), true);
        assert_eq!(resolve_support_vision("unknown-model", None), false);
    }

    #[test]
    fn resolve_unknown_model_defaults_false() {
        assert_eq!(resolve_support_vision("unknown-model", None), false);
    }
}
