use async_trait::async_trait;

/// 工具目录的一项：`request_tool` 的输出单元。
///
/// `id` 是内部唯一标识（排序键），`name` 是模型可见的调用名，`manual` 是手册正文
/// （`Tool::manual()` 优先，缺省回退 `Tool::description()`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub manual: String,
}

/// `request_tool` 的唯一能力来源：可发现的工具清单与手册正文。
///
/// 由 `ToolPipeline` 在每次调用前注入 `ToolUseContext::tool_catalog`，
/// 注册表是唯一实现。工具自身**不持有**注册表：`Arc<ToolRegistry>` 里装着
/// `Arc<dyn Tool>`，反向持有会让 `Arc` 成环。
///
/// 只读契约：本 trait 没有写入口，调用方无法借此改变注册表或本轮工具集。
#[async_trait]
pub trait ToolCatalog: Send + Sync {
    /// 全部可被模型调用的工具（`audience.agent`），按 `id` 字典序。
    async fn entries(&self) -> Vec<CatalogEntry>;
}

impl std::fmt::Debug for dyn ToolCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolCatalog")
    }
}
