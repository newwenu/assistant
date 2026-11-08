//! 快捷键绑定的解析策略。
//!
//! 这里只处理「用哪条绑定、能不能用」的决策，不涉及任何 UI 框架的具体快捷键类型：
//! 解析动作由调用方以 `parse` 回调注入，因此本模块可在无 GUI 依赖的环境下单测。

use std::collections::HashMap;

/// 一条已解析、可注册的绑定。
pub struct ResolvedBinding<A, S> {
    /// 配置查找键，同时用于告警定位
    pub id: &'static str,
    /// 实际生效的绑定文本
    pub binding: String,
    /// 解析后的绑定，具体类型由调用方决定
    pub shortcut: S,
    /// 按下时执行的回调
    pub action: A,
}

/// 一条未能注册的绑定及原因。
pub struct BindingIssue {
    pub id: &'static str,
    pub binding: String,
    pub error: String,
}

/// 把配置与默认值解析成可注册项。
///
/// 规则：
/// - 配置里显式清空（空字符串）表示用户主动禁用，不回落默认值；
/// - 配置缺失时使用默认值；
/// - 解析失败、重复绑定都记录为 issue，不注册该条；
/// - 重复判定使用归一化后的字符串，后出现的条目让位给先出现的。
pub fn resolve_bindings<A: Copy, S>(
    specs: &[(&'static str, A)],
    configured: &HashMap<String, String>,
    defaults: &HashMap<String, String>,
    parse: impl Fn(&str) -> Option<S>,
) -> (Vec<ResolvedBinding<A, S>>, Vec<BindingIssue>) {
    let mut resolved = Vec::new();
    let mut issues = Vec::new();
    let mut seen: HashMap<String, &'static str> = HashMap::new();

    for (id, action) in specs {
        let binding = match configured.get(*id) {
            Some(raw) if raw.trim().is_empty() => continue,
            Some(raw) => raw.trim().to_string(),
            None => match defaults.get(*id) {
                Some(default) => default.clone(),
                None => {
                    issues.push(BindingIssue {
                        id,
                        binding: String::new(),
                        error: "no default binding".to_string(),
                    });
                    continue;
                }
            },
        };

        let Some(shortcut) = parse(&binding) else {
            issues.push(BindingIssue {
                id,
                binding,
                error: "invalid binding".to_string(),
            });
            continue;
        };

        let normalized = binding.to_lowercase().replace(' ', "");
        if let Some(existing) = seen.get(&normalized) {
            issues.push(BindingIssue {
                id,
                binding,
                error: format!("duplicate of {}", existing),
            });
            continue;
        }

        seen.insert(normalized, id);
        resolved.push(ResolvedBinding {
            id,
            binding,
            shortcut,
            action: *action,
        });
    }

    (resolved, issues)
}
