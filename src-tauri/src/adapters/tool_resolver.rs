use assistant_providers::types::ToolSpec;
use assistant_tools::{ToolDefinition, ToolRegistry, ToolTier};

/// 白名单解析：`tier → audience → deny-by-default → 身份过滤 → 名单过滤`。
///
/// 前两步与排序复用 `ToolRegistry::tool_definitions_for_tier(max_tier)`（`tier <= max_tier`
/// + `audience().agent` + 注册名字典序），此处只补名单过滤，避免两处口径分叉。
///
/// `enabled_tools` 为空即 deny-by-default：返回空集，模型看不到任何工具。
/// `disabled_tools` 是最终否决，同一工具同时出现在两个名单时被排除。
/// 未知工具名静默忽略——名单来自前端内存态，名字过期不应让整轮失败。
///
/// 伴随工具（I58 §3.6）：配置路径是**身份过滤**而不是关系展开。`companion_of != None`
/// 的 id 一律不进候选集，与基工具是否也在名单里无关；`disabled_tools` 里的伴随 id
/// 同样被忽略（既不阻断也不生效）。超级工具能否被调用只由运行时申请决定。
///
/// 元工具（`request_tool`）没有特殊待遇：它是普通可勾选条目，勾了才注入、被否决就排除。
/// 「模型能不能申请升级」由此完全落在用户配置上（§3.6）。
pub async fn resolve_available_tools(
    registry: &ToolRegistry,
    enabled_tools: &[String],
    disabled_tools: &[String],
    max_tier: ToolTier,
) -> Vec<ToolSpec> {
    if enabled_tools.is_empty() {
        return Vec::new();
    }

    let enabled: std::collections::HashSet<String> =
        registry.exclude_companions(&enabled_tools.iter().cloned().collect());
    let disabled: std::collections::HashSet<&str> =
        disabled_tools.iter().map(String::as_str).collect();

    registry
        .tool_definitions_for_tier(max_tier)
        .await
        .into_iter()
        .filter(|definition| enabled.contains(&definition.id))
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
