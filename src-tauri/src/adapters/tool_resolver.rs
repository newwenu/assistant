use assistant_providers::types::ToolSpec;
use assistant_tools::{ToolDefinition, ToolRegistry, ToolTier};

/// 白名单解析：`tier → audience → deny-by-default → whitelist → blacklist`。
///
/// 前两步与排序复用 `ToolRegistry::tool_definitions_for_tier(max_tier)`（`tier <= max_tier`
/// + `audience().agent` + 注册名字典序），此处只补名单过滤，避免两处口径分叉。
///
/// `enabled_tools` 为空即 deny-by-default：返回空集，模型看不到任何工具。
/// `disabled_tools` 是最终否决，同一工具同时出现在两个名单时被排除。
/// 未知工具名静默忽略——名单来自前端内存态，名字过期不应让整轮失败。
pub async fn resolve_available_tools(
    registry: &ToolRegistry,
    enabled_tools: &[String],
    disabled_tools: &[String],
    max_tier: ToolTier,
) -> Vec<ToolSpec> {
    if enabled_tools.is_empty() {
        return Vec::new();
    }

    let enabled: std::collections::HashSet<&str> =
        enabled_tools.iter().map(String::as_str).collect();
    let disabled: std::collections::HashSet<&str> =
        disabled_tools.iter().map(String::as_str).collect();

    registry
        .tool_definitions_for_tier(max_tier)
        .await
        .into_iter()
        .filter(|definition| enabled.contains(definition.id.as_str()))
        .filter(|definition| !disabled.contains(definition.id.as_str()))
        .map(into_tool_spec)
        .collect()
}

fn into_tool_spec(definition: ToolDefinition) -> ToolSpec {
    ToolSpec {
        name: definition.name,
        description: definition.description,
        input_schema: definition.input_schema,
    }
}
