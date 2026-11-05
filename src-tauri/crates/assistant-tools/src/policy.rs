use std::fmt;

use serde::{Deserialize, Serialize};

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
        }
    }

    pub fn matches(&self, action: &str, resource: &str) -> bool {
        pattern_matches(&self.action, action) && pattern_matches(&self.resource, resource)
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

pub fn default_global_rules() -> Vec<PermissionRule> {
    vec![
        PermissionRule::new("*", "*", PermissionEffect::Ask),
        PermissionRule::new("read", "*", PermissionEffect::Allow),
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
