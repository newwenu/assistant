use serde::{Deserialize, Serialize};

use crate::base::ProviderKind;

/// 能力事实的三态。
///
/// `Unknown` 与 `Unsupported` 必须在类型层可分：前者是「不知道」，后者是「确定不支持」，
/// 两者在各消费点的策略不同（见规格 §4.3）。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

impl Support {
    pub fn from_bool(value: bool) -> Self {
        if value {
            Support::Supported
        } else {
            Support::Unsupported
        }
    }
}

/// 能力事实的来源。变体声明顺序即合并优先级（后者高于前者）。
///
/// 没有 `AdapterKnown` 变体：本批唯一的适配器已知事实是「端点能否原生发 PDF」，
/// 属端点维度；放进同一合并链会把「目录说不支持 PDF + Anthropic 端点」误升为
/// `Supported`。端点可表达性留在消费点的合取里。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Fallback,
    Snapshot,
    Catalog,
    /// wire 上写作 `api`，与 `Provenance::as_str` 及 `CapabilitySourceDto` 一致
    #[serde(rename = "api")]
    ApiDeclared,
}

impl Provenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provenance::Fallback => "fallback",
            Provenance::Snapshot => "snapshot",
            Provenance::Catalog => "catalog",
            Provenance::ApiDeclared => "api",
        }
    }
}

/// 一条能力事实：三态 + 来源。
///
/// `provenance` 在 wire 上的键名是 `source`，与 `models_json` 及 `ModelCapabilityDto` 一致。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityFact {
    pub support: Support,
    #[serde(rename = "source")]
    pub provenance: Provenance,
}

impl CapabilityFact {
    pub fn new(support: Support, provenance: Provenance) -> Self {
        Self {
            support,
            provenance,
        }
    }

    pub fn unknown(provenance: Provenance) -> Self {
        Self::new(Support::Unknown, provenance)
    }

    pub fn is_supported(&self) -> bool {
        matches!(self.support, Support::Supported)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self.support, Support::Unknown)
    }
}

/// 被判定的是哪一项能力。仅用于选择快照里的字段与日志标注。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityFeature {
    Vision,
    Pdf,
    Attachment,
}

impl CapabilityFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            CapabilityFeature::Vision => "vision",
            CapabilityFeature::Pdf => "pdf",
            CapabilityFeature::Attachment => "attachment",
        }
    }
}

/// 端点的请求形态；能力事实的 key 之一（`(provider_id, endpoint_shape, model_id)`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointShape {
    ChatCompletions,
    Responses,
    AnthropicMessages,
    GeminiGenerateContent,
    GeminiCodeAssist,
    Custom,
}

impl EndpointShape {
    /// 唯一映射点：端点形态由 `ProviderKind` 派生，不做 base_url 归一
    /// （base_url 归一会把自建代理误判为官方端点）。
    pub fn from_kind(kind: &ProviderKind) -> Self {
        match kind {
            ProviderKind::OpenAI => EndpointShape::ChatCompletions,
            ProviderKind::OpenAIResponses => EndpointShape::Responses,
            ProviderKind::Anthropic => EndpointShape::AnthropicMessages,
            ProviderKind::Gemini => EndpointShape::GeminiGenerateContent,
            ProviderKind::GeminiCodeAssist => EndpointShape::GeminiCodeAssist,
            ProviderKind::Custom { .. } => EndpointShape::Custom,
        }
    }
}

/// 目录断言：目录未收录（或未声明该键）→ `Unknown / Fallback`。
pub fn catalog_fact(model_id: &str, feature: CapabilityFeature) -> CapabilityFact {
    let declared = match feature {
        CapabilityFeature::Vision => crate::model_catalog::catalog_supports_vision(model_id),
        CapabilityFeature::Pdf => crate::model_catalog::catalog_supports_pdf(model_id),
        CapabilityFeature::Attachment => {
            crate::model_catalog::catalog_supports_attachment(model_id)
        }
    };
    match declared {
        Some(value) => CapabilityFact::new(Support::from_bool(value), Provenance::Catalog),
        None => CapabilityFact::unknown(Provenance::Fallback),
    }
}

/// 按 `provenance` 取最高优先级的事实，不是「先命中先返回」。
///
/// 快照里可能存着上一次从目录派生的 `catalog` 值，而本次 Provider 已明确声明 `api`；
/// 先命中会把声明压掉。同层（同一 `provenance`）取先出现的候选，因此调用方必须按
/// 「快照 → API 声明 → 目录 → 兜底」的顺序构造候选，以满足「同层以快照命中者优先」。
///
/// 候选为空时返回 `Unknown / Fallback`。
pub fn pick_highest(facts: &[CapabilityFact]) -> CapabilityFact {
    let mut best = CapabilityFact::unknown(Provenance::Fallback);
    for fact in facts {
        if fact.provenance > best.provenance {
            best = *fact;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_declaration_beats_snapshot_catalog_value() {
        let facts = [
            CapabilityFact::new(Support::Unsupported, Provenance::Catalog),
            CapabilityFact::new(Support::Supported, Provenance::ApiDeclared),
        ];
        let picked = pick_highest(&facts);
        assert_eq!(picked.provenance, Provenance::ApiDeclared);
        assert!(picked.is_supported());
    }

    #[test]
    fn provenance_order_is_respected_regardless_of_position() {
        let facts = [
            CapabilityFact::unknown(Provenance::Fallback),
            CapabilityFact::new(Support::Unsupported, Provenance::Snapshot),
            CapabilityFact::new(Support::Supported, Provenance::Catalog),
        ];
        let picked = pick_highest(&facts);
        assert_eq!(picked.provenance, Provenance::Catalog);
        assert!(picked.is_supported());
    }

    #[test]
    fn same_provenance_keeps_the_first_candidate() {
        let facts = [
            CapabilityFact::new(Support::Supported, Provenance::Catalog),
            CapabilityFact::new(Support::Unsupported, Provenance::Catalog),
        ];
        assert!(pick_highest(&facts).is_supported());
    }

    #[test]
    fn empty_candidates_yield_unknown_fallback() {
        let picked = pick_highest(&[]);
        assert_eq!(picked.support, Support::Unknown);
        assert_eq!(picked.provenance, Provenance::Fallback);
    }

    #[test]
    fn unknown_and_unsupported_differ_only_on_is_unknown() {
        let unsupported = CapabilityFact::new(Support::Unsupported, Provenance::Catalog);
        let unknown = CapabilityFact::unknown(Provenance::Fallback);
        assert!(!unsupported.is_supported());
        assert!(!unknown.is_supported());
        assert!(!unsupported.is_unknown());
        assert!(unknown.is_unknown());
    }

    #[test]
    fn fact_serializes_provenance_as_source() {
        let fact = CapabilityFact::new(Support::Supported, Provenance::ApiDeclared);
        let json = serde_json::to_value(fact).expect("serialize");
        assert_eq!(json["support"], "supported");
        assert_eq!(json["source"], "api");
        assert!(json.get("provenance").is_none());

        let parsed: CapabilityFact = serde_json::from_value(json).expect("deserialize");
        assert_eq!(parsed, fact);
    }

    #[test]
    fn endpoint_shape_maps_every_kind() {
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::OpenAI),
            EndpointShape::ChatCompletions
        );
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::OpenAIResponses),
            EndpointShape::Responses
        );
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::Anthropic),
            EndpointShape::AnthropicMessages
        );
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::Gemini),
            EndpointShape::GeminiGenerateContent
        );
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::GeminiCodeAssist),
            EndpointShape::GeminiCodeAssist
        );
        assert_eq!(
            EndpointShape::from_kind(&ProviderKind::Custom {
                base_url: "https://proxy.example.com".to_string()
            }),
            EndpointShape::Custom
        );
    }
}
