use std::fmt;

use serde::{Deserialize, Serialize};

use crate::r#trait::ToolTier;

/// 资源匹配在 Windows 上大小写不敏感：默认规则以 `C:/Windows/**` 形式表达，
/// 而 `os::path` 产出的规范路径可能是任意大小写。放宽匹配只会让 `Deny` 覆盖更广，
/// 不会让 `Allow` 越过用户显式写下的拒绝。
const CASE_INSENSITIVE: bool = cfg!(windows);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionEffect {
    Allow,
    Ask,
    Deny,
}

impl PermissionEffect {
    /// 多层合并的唯一算子：任一 `Deny` 取 `Deny`，否则任一 `Ask` 取 `Ask`。
    /// 委托与项目配置只能收紧，不能放宽。
    pub const fn most_restrictive(self, other: Self) -> Self {
        match (self, other) {
            (Self::Deny, _) | (_, Self::Deny) => Self::Deny,
            (Self::Ask, _) | (_, Self::Ask) => Self::Ask,
            (Self::Allow, Self::Allow) => Self::Allow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    /// `read` / `edit` / `delete` / `execute` / `network` / `external` / `*`
    pub action: String,
    /// 路径 glob / 命令名 / URL pattern / `tier:<tier>` / `*`
    pub resource: String,
    pub effect: PermissionEffect,
    /// 规则来源：只用于审批卡片的「为什么问我」，不参与 wire。
    /// 项目级 `permissions.json` 反序列化时恒为 `Config`。
    #[serde(skip)]
    pub origin: RuleOrigin,
}

impl PermissionRule {
    pub fn new(
        action: impl Into<String>,
        resource: impl Into<String>,
        effect: PermissionEffect,
    ) -> Self {
        Self {
            action: action.into(),
            resource: resource.into(),
            effect,
            origin: RuleOrigin::Config,
        }
    }

    /// 标注规则来源（`ApprovalLevel` 追加的规则需要区别于用户显式规则）。
    pub fn with_origin(mut self, origin: RuleOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn matches(&self, action: &str, resource: &str) -> bool {
        pattern_matches(&self.action, action) && pattern_matches(&self.resource, resource)
    }
}

/// 规则来源。默认 `Config`：应用默认规则、项目配置、tier 约束层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleOrigin {
    #[default]
    Config,
    /// 由审批级别追加的规则（`none` 的 `Allow` / `all_os_tools` 的 `Ask`）。
    ApprovalLevel { level: ApprovalLevel },
}

/// 审批级别（`tool_config.approvalLevel` 的内部形态）。
///
/// 与 storage / DTO 的枚举同名同值：三处都是同一封闭集合的镜像，
/// 各 crate 只依赖自己需要的那一份，避免跨层依赖。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevel {
    /// 全部自动执行（仍受 tier 与工具白名单约束）。
    #[default]
    None,
    /// 仅 `destructive` / `open_world` 需审批。
    DangerousOnly,
    /// 所有非 Sandbox 工具需审批。
    AllOsTools,
}

impl ApprovalLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DangerousOnly => "dangerous_only",
            Self::AllOsTools => "all_os_tools",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionConstraintLayer {
    pub rules: Vec<PermissionRule>,
}

impl PermissionConstraintLayer {
    pub fn new(rules: Vec<PermissionRule>) -> Self {
        Self { rules }
    }
}

/// 主规则集 + 若干约束层。主规则集决定基础结论，约束层逐层取最严。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedPermissionPolicy {
    rules: Vec<PermissionRule>,
    constraint_layers: Vec<PermissionConstraintLayer>,
}

impl ResolvedPermissionPolicy {
    pub fn new(
        rules: Vec<PermissionRule>,
        constraint_layers: Vec<PermissionConstraintLayer>,
    ) -> Self {
        Self {
            rules,
            constraint_layers,
        }
    }

    /// 应用默认：首条 `("*", "*", Ask)`，因此整体「默认需审批」。
    pub fn default_global() -> Self {
        Self::new(default_global_rules(), Vec::new())
    }

    pub fn with_constraint_layer(mut self, layer: PermissionConstraintLayer) -> Self {
        self.constraint_layers.push(layer);
        self
    }

    pub fn rules(&self) -> &[PermissionRule] {
        &self.rules
    }

    pub fn constraint_layers(&self) -> &[PermissionConstraintLayer] {
        &self.constraint_layers
    }

    pub fn evaluate(&self, action: &str, resource: &str) -> PermissionEffect {
        let mut result = evaluate_rules(&self.rules, action, resource);
        for layer in &self.constraint_layers {
            result = result.most_restrictive(evaluate_rules(&layer.rules, action, resource));
        }
        result
    }

    /// 命中规则解释：返回主规则集与各约束层中**实际生效**的那条规则。
    ///
    /// 与 [`Self::evaluate`] 共用同一套 `matches` 语义（单层 last-match-wins），
    /// 因此解释与结论永不背离。纯函数、无 IO，供审批卡片回答「为什么问我」。
    pub fn explain(&self, action: &str, resource: &str) -> Vec<&PermissionRule> {
        let mut matched = Vec::new();
        if let Some(rule) = last_match(&self.rules, action, resource) {
            matched.push(rule);
        }
        for layer in &self.constraint_layers {
            if let Some(rule) = last_match(&layer.rules, action, resource) {
                matched.push(rule);
            }
        }
        matched
    }

    /// 层级硬上限判定：把工具的 `tier` 投影成资源 `tier:<tier>` 后按 `("*", tier)` 评估。
    ///
    /// tier 约束层的规则（`("*", "tier:os", Deny)` 等）不针对具体 action，
    /// 因此不能混进 intent 的 `(action, resource)` 评估，必须单独判一次。
    /// **只看约束层**：主规则集里的 `("*","*")` 会匹配任意资源串，
    /// 若参与本次判定会把「默认需审批」误判成层级拒绝。
    pub fn evaluate_tier_ceiling(&self, tier: ToolTier) -> PermissionEffect {
        let resource = format!("tier:{}", tier.as_str());
        let mut result = PermissionEffect::Allow;
        for layer in &self.constraint_layers {
            result = result.most_restrictive(evaluate_rules(&layer.rules, "*", &resource));
        }
        result
    }
}

/// Agent 模式（或会话有效工具配置）→ 权限策略（I38d §4.6）。
///
/// 主规则集 = 基础规则（审批级别可改写）+ 硬保护规则；约束层承载 tier 上限。
/// 项目级 `permissions.json` 作为额外约束层由调用方追加（只能收紧，不能放宽）。
pub fn policy_from_agent_mode(
    max_tier: ToolTier,
    approval_level: ApprovalLevel,
) -> ResolvedPermissionPolicy {
    let mut base = base_rules();

    match approval_level {
        // `none`：把「未命中具体规则」时的基座结论由 `Ask` 改写为 `Allow`。
        // 单层内 last-match-wins，基座是宽泛规则、必须排在最前，因此改写而非追加；
        // 追加到末尾会连硬保护 Deny 一起覆盖。
        ApprovalLevel::None => {
            base[0] = PermissionRule::new("*", "*", PermissionEffect::Allow).with_origin(
                RuleOrigin::ApprovalLevel {
                    level: approval_level,
                },
            );
        }
        // `dangerous_only`：保持默认（`read` 允许，`edit` / `execute` 需审批）。
        ApprovalLevel::DangerousOnly => {}
        // `all_os_tools`：读取也要审批。追加在基础规则内（仍在硬保护之前），
        // 这样「读敏感路径」的 Deny 不会被这条宽泛 `read` 规则覆盖成 `Ask`。
        ApprovalLevel::AllOsTools => base.push(
            PermissionRule::new("read", "*", PermissionEffect::Ask).with_origin(
                RuleOrigin::ApprovalLevel {
                    level: approval_level,
                },
            ),
        ),
    }

    let mut global_rules = base;
    global_rules.extend(hard_deny_rules());

    let tier_rules = match max_tier {
        ToolTier::Sandbox => vec![
            PermissionRule::new("*", "tier:os", PermissionEffect::Deny),
            PermissionRule::new("*", "tier:external", PermissionEffect::Deny),
        ],
        ToolTier::Os => vec![PermissionRule::new(
            "*",
            "tier:external",
            PermissionEffect::Deny,
        )],
        ToolTier::External => Vec::new(),
    };

    ResolvedPermissionPolicy::new(
        global_rules,
        vec![PermissionConstraintLayer::new(tier_rules)],
    )
}

/// 单层内的生效规则（last-match-wins）：解释必须与 [`evaluate_rules`] 同源。
fn last_match<'a>(
    rules: &'a [PermissionRule],
    action: &str,
    resource: &str,
) -> Option<&'a PermissionRule> {
    rules
        .iter()
        .rev()
        .find(|rule| rule.matches(action, resource))
}

/// 单层内 **last-match-wins**：宽泛规则在前，具体规则在后覆盖；未命中默认 `Allow`。
fn evaluate_rules(rules: &[PermissionRule], action: &str, resource: &str) -> PermissionEffect {
    let mut result = PermissionEffect::Allow;
    for rule in rules {
        if rule.matches(action, resource) {
            result = rule.effect;
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeilingValidationError {
    AllowRule { action: String, resource: String },
}

impl fmt::Display for CeilingValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowRule { action, resource } => write!(
                formatter,
                "runtime ceiling must not relax permissions: rule '{}' + '{}' is Allow",
                action, resource
            ),
        }
    }
}

impl std::error::Error for CeilingValidationError {}

/// 运行时上限：委托/子 Agent 只能收紧。构造期即拒绝任何 `Allow` 规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRuntimeCeiling {
    rules: Vec<PermissionRule>,
}

impl PermissionRuntimeCeiling {
    pub fn try_new(rules: Vec<PermissionRule>) -> Result<Self, CeilingValidationError> {
        if let Some(rule) = rules
            .iter()
            .find(|rule| rule.effect == PermissionEffect::Allow)
        {
            return Err(CeilingValidationError::AllowRule {
                action: rule.action.clone(),
                resource: rule.resource.clone(),
            });
        }
        Ok(Self { rules })
    }

    pub fn rules(&self) -> &[PermissionRule] {
        &self.rules
    }

    pub fn constraint_layer(&self) -> PermissionConstraintLayer {
        PermissionConstraintLayer::new(self.rules.clone())
    }
}

/// 应用默认规则集：基础规则 + 硬保护规则，顺序固定（宽泛在前、具体在后）。
pub fn default_global_rules() -> Vec<PermissionRule> {
    let mut rules = base_rules();
    rules.extend(hard_deny_rules());
    rules
}

/// 基础规则：决定「未命中具体规则」时的结论。审批级别会改写首条或追加 `read` 规则。
fn base_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule::new("*", "*", PermissionEffect::Ask),
        PermissionRule::new("read", "*", PermissionEffect::Allow),
    ]
}

/// 硬保护规则：敏感路径恒为 `Deny`，任何审批级别都不得放宽。
fn hard_deny_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule::new("read", "/etc/shadow", PermissionEffect::Deny),
        PermissionRule::new("read", "/etc/ssh/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "/etc/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Windows/**", PermissionEffect::Deny),
        PermissionRule::new("edit", "C:/Program Files/**", PermissionEffect::Deny),
    ]
}

fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    let pattern: Vec<char> = pattern.replace('\\', "/").chars().collect();
    let value: Vec<char> = value.replace('\\', "/").chars().collect();
    glob_matches(&pattern, &value)
}

/// `*` 不跨路径分隔符，`**` 可跨。分隔符统一按 `/` 比较。
fn glob_matches(pattern: &[char], value: &[char]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == '*' {
        let double = pattern.len() > 1 && pattern[1] == '*';
        let rest = if double { &pattern[2..] } else { &pattern[1..] };
        if rest.is_empty() {
            return double || !value.contains(&'/');
        }
        for index in 0..=value.len() {
            if glob_matches(rest, &value[index..]) {
                return true;
            }
            if index < value.len() && !double && value[index] == '/' {
                return false;
            }
        }
        return false;
    }
    if value.is_empty() {
        return false;
    }
    if !chars_equal(pattern[0], value[0]) {
        return false;
    }
    glob_matches(&pattern[1..], &value[1..])
}

fn chars_equal(left: char, right: char) -> bool {
    if CASE_INSENSITIVE {
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(level: ApprovalLevel, tier: ToolTier) -> ResolvedPermissionPolicy {
        policy_from_agent_mode(tier, level)
    }

    #[test]
    fn none_level_allows_non_protected_actions() {
        let policy = policy(ApprovalLevel::None, ToolTier::External);
        assert_eq!(
            policy.evaluate("read", "/tmp/a.txt"),
            PermissionEffect::Allow
        );
        assert_eq!(
            policy.evaluate("edit", "/tmp/a.txt"),
            PermissionEffect::Allow
        );
        assert_eq!(
            policy.evaluate("execute", "rm -rf /"),
            PermissionEffect::Allow
        );
        // 自动执行不等于取消硬保护：敏感路径恒为 Deny。
        assert_eq!(
            policy.evaluate("read", "/etc/shadow"),
            PermissionEffect::Deny
        );
        assert_eq!(
            policy.evaluate("edit", "C:/Windows/System32/a.dll"),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn dangerous_only_keeps_default_reads_allowed() {
        let policy = policy(ApprovalLevel::DangerousOnly, ToolTier::External);
        assert_eq!(
            policy.evaluate("read", "/tmp/a.txt"),
            PermissionEffect::Allow
        );
        assert_eq!(policy.evaluate("edit", "/tmp/a.txt"), PermissionEffect::Ask);
        assert_eq!(
            policy.evaluate("execute", "git status"),
            PermissionEffect::Ask
        );
    }

    #[test]
    fn all_os_tools_asks_even_for_reads() {
        let policy = policy(ApprovalLevel::AllOsTools, ToolTier::External);
        assert_eq!(policy.evaluate("read", "/tmp/a.txt"), PermissionEffect::Ask);
        assert_eq!(policy.evaluate("edit", "/tmp/a.txt"), PermissionEffect::Ask);
        // 宽泛 `read` 的 Ask 不得覆盖敏感路径的硬保护 Deny。
        assert_eq!(
            policy.evaluate("read", "/etc/shadow"),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn approval_level_rules_are_explained_as_approval_level() {
        let policy = policy(ApprovalLevel::AllOsTools, ToolTier::External);
        let matched = policy.explain("read", "/tmp/a.txt");
        let rule = matched
            .iter()
            .find(|rule| rule.effect == PermissionEffect::Ask)
            .expect("read must hit the approval-level rule");
        assert_eq!(
            rule.origin,
            RuleOrigin::ApprovalLevel {
                level: ApprovalLevel::AllOsTools
            }
        );
    }

    #[test]
    fn sandbox_ceiling_denies_os_and_external_tiers() {
        // 审批级别为 `none` 时 tier 上限仍然生效：自动执行不等于越过层级。
        let policy = policy(ApprovalLevel::None, ToolTier::Sandbox);
        assert_eq!(
            policy.evaluate_tier_ceiling(ToolTier::Os),
            PermissionEffect::Deny
        );
        assert_eq!(
            policy.evaluate_tier_ceiling(ToolTier::External),
            PermissionEffect::Deny
        );
        assert_ne!(
            policy.evaluate_tier_ceiling(ToolTier::Sandbox),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn os_ceiling_denies_only_external_tier() {
        let policy = policy(ApprovalLevel::DangerousOnly, ToolTier::Os);
        assert_eq!(
            policy.evaluate_tier_ceiling(ToolTier::External),
            PermissionEffect::Deny
        );
        assert_ne!(
            policy.evaluate_tier_ceiling(ToolTier::Os),
            PermissionEffect::Deny
        );
        assert_ne!(
            policy.evaluate_tier_ceiling(ToolTier::Sandbox),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn external_ceiling_denies_nothing() {
        let policy = policy(ApprovalLevel::DangerousOnly, ToolTier::External);
        for tier in [ToolTier::Sandbox, ToolTier::Os, ToolTier::External] {
            assert_ne!(
                policy.evaluate_tier_ceiling(tier),
                PermissionEffect::Deny,
                "{tier:?} must not be denied by an external ceiling"
            );
        }
    }

    #[test]
    fn constraint_layer_tightens_but_never_relaxes() {
        // 项目层只有 `Ask` 规则：`none` 级别的 `Allow` 结论必须被收紧为 `Ask`。
        let policy = policy(ApprovalLevel::None, ToolTier::External).with_constraint_layer(
            PermissionConstraintLayer::new(vec![PermissionRule::new(
                "edit",
                "**/*.env",
                PermissionEffect::Ask,
            )]),
        );
        assert_eq!(policy.evaluate("edit", "/tmp/.env"), PermissionEffect::Ask);
        assert_eq!(
            policy.evaluate("edit", "/tmp/a.txt"),
            PermissionEffect::Allow
        );
    }
}
