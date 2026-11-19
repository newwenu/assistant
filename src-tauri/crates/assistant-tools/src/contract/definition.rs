use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::r#trait::ToolTier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    /// 内部唯一标识，注册表键与白名单匹配依据。
    pub id: String,
    /// 发给模型的调用名（`ToolSpec.name`），默认与 `id` 相同。
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl ToolDefinition {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// 工具装配清单条目：不含 `input_schema`，只够前端按 tier 分组渲染多选。
///
/// 与 `ToolDefinition` 分开：装配面板不需要 schema（体积大且无展示价值），
/// 而 `ToolDefinition` 是发给模型的载荷，两者用途不同，不能共用结构。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDescriptor {
    /// 内部唯一标识，也是白名单 `enabled_tools` / `disabled_tools` 的取值。
    pub id: String,
    /// 发给模型的调用名，面板展示用。
    pub name: String,
    pub description: String,
    pub tier: ToolTier,
    /// 伴随工具的基工具 id（`Tool::companion_of`）。`Some` 表示本工具没有独立身份：
    /// 不单独勾选、不因基工具被勾选而注入，只是**跟随基工具获得准入**（可被模型申请），
    /// 面板据此渲染为基工具下的从属行（I58 §3.6）。
    pub companion_of: Option<String>,
}
