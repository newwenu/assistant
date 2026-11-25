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
    /// 由审批级别追加的规则（`none` 的 `Allow` / `all_os_tools` 的 `Ask` / `workspace_write` 的 W1–W5）。
    ApprovalLevel { level: ApprovalLevel },
    /// 用户规则（`approvalRules`）：解释链必须与 `Config` 区分（I70 §7.1）。
    UserRule,
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
    /// 读取与工作区写入自动执行，其余需审批（I70 §3.1）。
    WorkspaceWrite,
    /// 读取放行、其余确认（写入 / 执行 / 删除 / MCP / 工具输出回读）。
    DangerousOnly,
    /// 所有非 Sandbox 工具需审批。
    AllOsTools,
}

impl ApprovalLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::WorkspaceWrite => "workspace_write",
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

/// 预设档规则集（I70 §3.1 / §3.1.1）：基础规则 + 档位改写 / 追加，**不含**硬保护规则。
///
/// 独立成函数是为了让用户规则层能插在「预设档」与「硬保护」之间（I70 §4.1）：
/// 用户规则可放宽应用默认，但不可放宽底线。
fn preset_rules(approval_level: ApprovalLevel) -> Vec<PermissionRule> {
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
        // `workspace_write`：读取已放行，这里追加「工作区写入放行 + 敏感文件例外」。
        // W1 的宽泛 `edit **` 必须排在例外之前：单层 last-match-wins 让 W2–W5 覆盖它。
        ApprovalLevel::WorkspaceWrite => {
            let origin = RuleOrigin::ApprovalLevel {
                level: approval_level,
            };
            for (action, resource, effect) in [
                ("edit", "**", PermissionEffect::Allow),
                ("edit", "**/.env", PermissionEffect::Ask),
                ("edit", "**/.env.*", PermissionEffect::Ask),
                ("edit", "**/.env.example", PermissionEffect::Allow),
                ("edit", "**/.git/**", PermissionEffect::Ask),
            ] {
                base.push(PermissionRule::new(action, resource, effect).with_origin(origin));
            }
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

    base
}

/// Agent 模式（或会话有效工具配置）→ 权限策略（I38d §4.6）。
///
/// 主规则集 = 基础规则（审批级别可改写）+ 硬保护规则；约束层承载 tier 上限。
/// 项目级 `permissions.json` 作为额外约束层由调用方追加（只能收紧，不能放宽）。
pub fn policy_from_agent_mode(
    max_tier: ToolTier,
    approval_level: ApprovalLevel,
) -> ResolvedPermissionPolicy {
    policy_from_agent_mode_with_rules(max_tier, approval_level, &[])
}

/// 同 [`policy_from_agent_mode`]，但插入用户规则层（I70 §4.1）。
///
/// 顺序固定为：预设档规则 → 用户规则 → 硬保护规则。
/// 用户规则因此可以放宽预设档的 `Ask`，但永远压不过末段的硬保护 `Deny`。
pub fn policy_from_agent_mode_with_rules(
    max_tier: ToolTier,
    approval_level: ApprovalLevel,
    user_rules: &[PermissionRule],
) -> ResolvedPermissionPolicy {
    let mut global_rules = preset_rules(approval_level);
    global_rules.extend(user_rules.iter().cloned());
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

/// 用户规则校验失败原因（I70 §6.1 E1–E5）。E6（未知 `effect`）在反序列化阶段即失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleValidationError {
    /// E1：条数超限。
    TooManyRules { count: usize, limit: usize },
    /// E2：`action` 为空或超长。
    ActionLength { index: usize, len: usize },
    /// E3：`resource` 为空或超长。
    ResourceLength { index: usize, len: usize },
    /// E4：`allow` 触碰底线动作（`readToolOutput` / `readScratch` / `mcp`）。
    AllowTouchesBottomLine { index: usize, action: String },
    /// E5：`resource` 使用了约束层专用语法 `tier:`。
    ReservedTierResource { index: usize, resource: String },
}

impl fmt::Display for RuleValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyRules { count, limit } => {
                write!(formatter, "approval rules exceed limit: {count} > {limit}")
            }
            Self::ActionLength { index, len } => write!(
                formatter,
                "approval rule #{index} action must be 1..=32 chars, got {len}"
            ),
            Self::ResourceLength { index, len } => write!(
                formatter,
                "approval rule #{index} resource must be 1..=256 chars, got {len}"
            ),
            Self::AllowTouchesBottomLine { index, action } => write!(
                formatter,
                "approval rule #{index} must not allow '{action}': it is never relaxable"
            ),
            Self::ReservedTierResource { index, resource } => write!(
                formatter,
                "approval rule #{index} resource '{resource}' uses reserved 'tier:' syntax"
            ),
        }
    }
}

impl std::error::Error for RuleValidationError {}

const APPROVAL_RULES_MAX: usize = 64;
const APPROVAL_RULE_ACTION_MAX: usize = 32;
const APPROVAL_RULE_RESOURCE_MAX: usize = 256;

/// 用户规则层校验（I70 §6.1）：保存与运行时装配共用同一实现，避免两套真值。
///
/// 只校验「结构性不可接受」，不判断规则是否会被命中：
/// 未知 `action` 天然不匹配任何意图，属 fail-safe，不在此拒绝。
pub fn validate_rules(rules: &[PermissionRule]) -> Result<(), RuleValidationError> {
    if rules.len() > APPROVAL_RULES_MAX {
        return Err(RuleValidationError::TooManyRules {
            count: rules.len(),
            limit: APPROVAL_RULES_MAX,
        });
    }
    for (index, rule) in rules.iter().enumerate() {
        let action_len = rule.action.chars().count();
        if action_len == 0 || action_len > APPROVAL_RULE_ACTION_MAX {
            return Err(RuleValidationError::ActionLength {
                index,
                len: action_len,
            });
        }
        let resource_len = rule.resource.chars().count();
        if resource_len == 0 || resource_len > APPROVAL_RULE_RESOURCE_MAX {
            return Err(RuleValidationError::ResourceLength {
                index,
                len: resource_len,
            });
        }
        if rule.resource.starts_with("tier:") {
            return Err(RuleValidationError::ReservedTierResource {
                index,
                resource: rule.resource.clone(),
            });
        }
        // E4：`allow` 不得匹配底线动作。`pattern_matches` 对 `*` / `**` 恒真，
        // 因此 `("*","*",Allow)` 也会在此被拒（I70 §6.1 的刻意后果）。
        if rule.effect == PermissionEffect::Allow
            && (pattern_matches(&rule.action, "readToolOutput")
                || pattern_matches(&rule.action, "readScratch")
                || pattern_matches(&rule.action, "mcp"))
        {
            return Err(RuleValidationError::AllowTouchesBottomLine {
                index,
                action: rule.action.clone(),
            });
        }
    }
    Ok(())
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
        // 读取本会话输出域：跨出工作区边界，因此恒 `Ask`（I52 §5.11.4(3)）。
        // 排在 `("*", "*")` 与 `("read", "*")` 之后 → `none` 档改写首条也压不住它；
        // 动作名与 `read` 不同 → 不会被 `read` 的默认 `Allow` 漏过来。
        PermissionRule::new("readToolOutput", "*", PermissionEffect::Ask),
        // 读取本会话临时区：与输出域同一底线（I73 §3.8 P5 / P4.9「同底线」）。
        // 动作名独立，故 `read` 的默认 `Allow` 同样漏不过来。
        PermissionRule::new("readScratch", "*", PermissionEffect::Ask),
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
            policy.evaluate("edit", "/tmp/a.txt"),
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
    }

    #[test]
    fn all_os_tools_asks_even_for_reads() {
        let policy = policy(ApprovalLevel::AllOsTools, ToolTier::External);
        assert_eq!(policy.evaluate("read", "/tmp/a.txt"), PermissionEffect::Ask);
        // 宽泛 `read` 的 Ask 不得覆盖敏感路径的硬保护 Deny。
        assert_eq!(
            policy.evaluate("read", "/etc/shadow"),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn internal_domain_reads_ask_at_every_approval_level() {
        // 输出域与临时区都在工作区之外：`none` 档的「免审」只覆盖工作区内，
        // 压不住这两条更具体的规则（I73 §3.8 P5 与输出域同底线）。
        for level in [
            ApprovalLevel::None,
            ApprovalLevel::DangerousOnly,
            ApprovalLevel::AllOsTools,
        ] {
            let policy = policy(level, ToolTier::External);
            for action in ["readToolOutput", "readScratch"] {
                assert_eq!(
                    policy.evaluate(action, "/tmp/outputs/abc"),
                    PermissionEffect::Ask,
                    "{action} must stay Ask under {:?}",
                    level
                );
            }
        }
    }

    #[test]
    fn internal_domain_reads_are_not_covered_by_reads_default_allow() {
        let policy = policy(ApprovalLevel::DangerousOnly, ToolTier::External);
        // 动作名精确匹配：`("read", "*", Allow)` 不得漏给内部域动作。
        assert_eq!(
            policy.evaluate("read", "/tmp/a.txt"),
            PermissionEffect::Allow
        );
        for action in ["readToolOutput", "readScratch"] {
            assert_eq!(
                policy.evaluate(action, "/tmp/a.txt"),
                PermissionEffect::Ask,
                "{action} must not inherit read's default Allow"
            );
        }
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

    /// glob 引擎是私有递归实现，`evaluate` 只能间接覆盖它的分支，这里直接测边界。
    fn glob(pattern: &str, value: &str) -> bool {
        let pattern: Vec<char> = pattern.chars().collect();
        let value: Vec<char> = value.chars().collect();
        glob_matches(&pattern, &value)
    }

    #[test]
    fn empty_pattern_matches_only_an_empty_value() {
        assert!(glob("", ""));
        assert!(!glob("", "a"));
        assert!(!glob("a", ""));
    }

    #[test]
    fn single_star_stops_at_the_separator() {
        assert!(glob("*", "abc"));
        assert!(!glob("*", "a/b"));
        assert!(!glob("*a", "a/b"), "单星遇到分隔符必须提前失败");
        assert!(glob("/etc/*", "/etc/hosts"));
        assert!(!glob("/etc/*", "/etc/ssh/sshd_config"));
    }

    #[test]
    fn double_star_crosses_the_separator() {
        assert!(glob("**", ""));
        assert!(glob("**", "a/b/c"));
        assert!(glob("/etc/**", "/etc/ssh/sshd_config"));
        assert!(
            glob("a/**/b", "a/x/y/b"),
            "`**` 落在剩余 pattern 开头即跨段"
        );
    }

    /// 项目权限文件的 `**/*.env` 依赖「双星可跳过前缀段」这一回溯分支。
    #[test]
    fn double_star_prefix_matches_a_nested_leaf() {
        assert!(glob("**/*.env", "/ws/.env"));
        assert!(glob("**/*.env", "ws/sub/.env"));
        assert!(!glob("**/*.env", "ws/sub/a.txt"));
    }

    #[test]
    fn literal_mismatch_fails_without_backtracking() {
        assert!(glob("abc", "abc"));
        assert!(!glob("abc", "abd"));
    }

    /// 大小写敏感性由平台决定：Windows 的规范路径大小写不定，必须放宽匹配。
    #[test]
    fn char_comparison_follows_platform_case_sensitivity() {
        assert_eq!(
            glob("C:/Windows/**", "c:/windows/System32"),
            CASE_INSENSITIVE
        );
    }

    #[test]
    fn workspace_write_matrix_keeps_workspace_edits_and_sensitive_file_asks() {
        let policy = policy(ApprovalLevel::WorkspaceWrite, ToolTier::External);

        // W1：工作区写入放行（`edit` 意图恒落在工作区内，越界在解析阶段即失败）。
        assert_eq!(
            policy.evaluate("edit", "/ws/a.txt"),
            PermissionEffect::Allow
        );

        // W2–W5：敏感文件例外排在 W1 之后，last-match-wins 覆盖宽泛 Allow。
        assert_eq!(policy.evaluate("edit", "/ws/.env"), PermissionEffect::Ask);
        assert_eq!(
            policy.evaluate("edit", "/ws/.env.local"),
            PermissionEffect::Ask
        );
        assert_eq!(
            policy.evaluate("edit", "/ws/.env.example"),
            PermissionEffect::Allow
        );
        assert_eq!(
            policy.evaluate("edit", "/ws/.git/config"),
            PermissionEffect::Ask
        );

        // W1 只覆盖 `edit`：其余动作落回序 1 的 Ask。
        assert_eq!(
            policy.evaluate("delete", "/ws/a.txt"),
            PermissionEffect::Ask
        );
        assert_eq!(policy.evaluate("execute", "ls -la"), PermissionEffect::Ask);
        assert_eq!(
            policy.evaluate("mcp", "mcp:srv-a:abc"),
            PermissionEffect::Ask
        );

        // 读取放行；内部域（输出域 / 临时区）回读恒 Ask。
        assert_eq!(
            policy.evaluate("read", "/ws/a.txt"),
            PermissionEffect::Allow
        );
        for action in ["readToolOutput", "readScratch"] {
            assert_eq!(
                policy.evaluate(action, "/ws/outputs/a.txt"),
                PermissionEffect::Ask,
                "{action} must stay Ask"
            );
        }

        // 硬保护不受档位影响。
        assert_eq!(
            policy.evaluate("edit", "/etc/passwd"),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn user_rules_are_last_match_wins_within_the_layer() {
        let allow = PermissionRule::new("execute", "**", PermissionEffect::Allow)
            .with_origin(RuleOrigin::UserRule);
        let ask = PermissionRule::new("execute", "**", PermissionEffect::Ask)
            .with_origin(RuleOrigin::UserRule);

        let policy = policy_from_agent_mode_with_rules(
            ToolTier::External,
            ApprovalLevel::DangerousOnly,
            &[allow.clone(), ask.clone()],
        );
        assert_eq!(
            policy.evaluate("execute", "ls -la"),
            PermissionEffect::Ask,
            "后写覆盖先写"
        );

        let policy = policy_from_agent_mode_with_rules(
            ToolTier::External,
            ApprovalLevel::DangerousOnly,
            &[ask, allow],
        );
        assert_eq!(
            policy.evaluate("execute", "ls -la"),
            PermissionEffect::Allow
        );
    }

    #[test]
    fn user_rules_relax_or_tighten_the_preset_level() {
        // `dangerous_only` 的写入 Ask 被用户规则放宽。
        let relax = vec![PermissionRule::new("edit", "**", PermissionEffect::Allow)
            .with_origin(RuleOrigin::UserRule)];
        let policy = policy_from_agent_mode_with_rules(
            ToolTier::External,
            ApprovalLevel::DangerousOnly,
            &relax,
        );
        assert_eq!(
            policy.evaluate("edit", "/ws/a.txt"),
            PermissionEffect::Allow
        );

        // `none` 的写入 Allow 被用户规则收紧。
        let tighten = vec![PermissionRule::new("edit", "**", PermissionEffect::Ask)
            .with_origin(RuleOrigin::UserRule)];
        let policy =
            policy_from_agent_mode_with_rules(ToolTier::External, ApprovalLevel::None, &tighten);
        assert_eq!(policy.evaluate("edit", "/ws/a.txt"), PermissionEffect::Ask);

        // `workspace_write` 的敏感文件 Ask 被更具体的用户规则放宽，其余例外不受影响。
        let specific = vec![
            PermissionRule::new("edit", "**/docs/a.md", PermissionEffect::Allow)
                .with_origin(RuleOrigin::UserRule),
        ];
        let policy = policy_from_agent_mode_with_rules(
            ToolTier::External,
            ApprovalLevel::WorkspaceWrite,
            &specific,
        );
        assert_eq!(
            policy.evaluate("edit", "/ws/docs/a.md"),
            PermissionEffect::Allow
        );
        assert_eq!(policy.evaluate("edit", "/ws/.env"), PermissionEffect::Ask);
    }

    #[test]
    fn allow_rules_touching_bottom_line_actions_are_rejected() {
        for action in ["readToolOutput", "readScratch", "mcp", "*", "**"] {
            let rules = vec![PermissionRule::new(action, "*", PermissionEffect::Allow)];
            assert!(
                matches!(
                    validate_rules(&rules),
                    Err(RuleValidationError::AllowTouchesBottomLine { .. })
                ),
                "allow must not match bottom-line action {action}"
            );
        }

        for effect in [PermissionEffect::Ask, PermissionEffect::Deny] {
            let rules = vec![PermissionRule::new("*", "*", effect)];
            assert!(validate_rules(&rules).is_ok(), "non-allow effects pass");
        }
    }

    #[test]
    fn rule_count_and_length_limits_are_rejected() {
        let too_many: Vec<PermissionRule> = (0..APPROVAL_RULES_MAX + 1)
            .map(|i| PermissionRule::new("read", format!("r{i}"), PermissionEffect::Allow))
            .collect();
        assert_eq!(
            validate_rules(&too_many),
            Err(RuleValidationError::TooManyRules {
                count: 65,
                limit: APPROVAL_RULES_MAX,
            })
        );

        let long_action = vec![PermissionRule::new(
            "a".repeat(APPROVAL_RULE_ACTION_MAX + 1),
            "*",
            PermissionEffect::Ask,
        )];
        assert_eq!(
            validate_rules(&long_action),
            Err(RuleValidationError::ActionLength { index: 0, len: 33 })
        );

        let long_resource = vec![PermissionRule::new(
            "read",
            "r".repeat(APPROVAL_RULE_RESOURCE_MAX + 1),
            PermissionEffect::Ask,
        )];
        assert_eq!(
            validate_rules(&long_resource),
            Err(RuleValidationError::ResourceLength { index: 0, len: 257 })
        );

        let reserved = vec![PermissionRule::new(
            "read",
            "tier:os",
            PermissionEffect::Deny,
        )];
        assert_eq!(
            validate_rules(&reserved),
            Err(RuleValidationError::ReservedTierResource {
                index: 0,
                resource: "tier:os".to_string(),
            })
        );

        let boundary: Vec<PermissionRule> = (0..APPROVAL_RULES_MAX)
            .map(|i| PermissionRule::new("read", format!("r{i}"), PermissionEffect::Allow))
            .collect();
        assert!(validate_rules(&boundary).is_ok(), "boundary count passes");
    }
}
