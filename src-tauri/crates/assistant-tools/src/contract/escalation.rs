use async_trait::async_trait;

/// 可升级段的一项（I58 §3.6 `brief` 档）：只有名字与一句用途，不含 schema。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationCandidate {
    pub id: String,
    pub name: String,
    pub brief: String,
}

/// 单条申请被拒的原因（I58 §5 F13 / F14）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationRejectReason {
    /// 基工具不在本轮可用集（`companion_of` 的基工具 id ∉ `available`）。
    BaseNotAvailable,
    /// 目标没有 `companion_of`：不是伴随工具，申请无意义。
    NotCompanion,
    /// `tier > 本轮 maxToolTier`。
    TierExceeded,
    /// 名字解析不到任何工具。
    NotInCatalog,
}

impl EscalationRejectReason {
    /// 稳定的英文原因码：进 `data.rejected`，也进文案，两处不得各写一套。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaseNotAvailable => "base_not_available",
            Self::NotCompanion => "not_companion",
            Self::TierExceeded => "tier_exceeded",
            Self::NotInCatalog => "not_in_catalog",
        }
    }

    /// 给模型看的一句话原因。
    pub const fn message(self) -> &'static str {
        match self {
            Self::BaseNotAvailable => "its base tool is not available in this turn",
            Self::NotCompanion => "it is not a companion tool, so it cannot be enabled on demand",
            Self::TierExceeded => "it exceeds this turn's tool tier limit",
            Self::NotInCatalog => "no tool with this id or name exists",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationRejection {
    pub id: String,
    pub reason: EscalationRejectReason,
}

/// 一次申请的结果：通过项已登记到本轮收集器，未通过项逐条给出原因。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscalationOutcome {
    pub enabled: Vec<String>,
    pub rejected: Vec<EscalationRejection>,
}

/// 升级端口：`request_tool(detail = "enable")` 的唯一出口（I58 §3.8）。
///
/// 由适配层（`adapters/tool_escalation.rs`）每次 `execute` 现构造并注入
/// `ToolUseContext::escalation`：注册表引用 + 本轮层级 + 本轮收集器三样，不读其它状态。
///
/// 职责边界：端口**只校验与登记**，不改注册表、不改 provider 请求；
/// `id → ToolSpec` 的解析在这里完成（只有适配层持有注册表），
/// 但成品类型不出现在本契约里——`assistant-tools` 不依赖 `assistant-providers`。
#[async_trait]
pub trait ToolEscalationPort: Send + Sync {
    /// 可升级段：`audience.agent` ∧ `tier <= 本轮上限` ∧ 有 `companion_of`
    /// ∧ 基工具在本轮可用 ∧ 自身不在本轮可用名。按 `id` 字典序。
    async fn candidates(&self, available: &[String]) -> Vec<EscalationCandidate>;

    /// 逐条校验并登记通过项；`requested` 按 `id` 或 `name` 解析。
    ///
    /// 不做整体拒绝：单条失败只进 `rejected`，合法项照常登记（I58 §5 F14）。
    async fn request(&self, requested: &[String], available: &[String]) -> EscalationOutcome;
}

impl std::fmt::Debug for dyn ToolEscalationPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolEscalationPort")
    }
}
