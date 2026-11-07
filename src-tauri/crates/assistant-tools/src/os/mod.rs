pub mod agent_delete;
pub mod agent_edit;
pub mod agent_glob;
pub mod agent_grep;
pub mod agent_list;
pub mod agent_mkdir;
pub mod agent_move;
pub mod agent_read;
pub mod agent_write;
pub mod fs_link;
pub mod output;
pub mod path;
pub mod process_output;
pub mod text_encoding;

use std::sync::Arc;

use crate::r#trait::Tool;
use crate::registry::ToolRegistry;
use crate::storage::ReadReceiptStore;

/// `TOOLBOX_ONLY` 桥接工具：只服务工具箱入口，不进 Agent 白名单。
/// 新工具不得混进本函数（见 I38c §7 注册门禁）。
pub fn register_os_tools(registry: &ToolRegistry) {
    registry.register(Arc::new(fs_link::FsLinkTool::new()) as Arc<dyn Tool>);
}

pub fn register_readonly_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry.register(Arc::new(agent_read::AgentReadTool::new(receipts)) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_list::AgentListTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_glob::AgentGlobTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_grep::AgentGrepTool::new()) as Arc<dyn Tool>);
}

/// 修改组：`write` / `edit` / `mkdir` / `move`。
///
/// 与危险组分开注册，让调用方按阶段门禁逐个放开（P5 生产注册只到只读组）。
pub fn register_write_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry
        .register(Arc::new(agent_write::AgentWriteTool::new(receipts.clone())) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_edit::AgentEditTool::new(receipts.clone())) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_mkdir::AgentMkdirTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_move::AgentMoveTool::new(receipts)) as Arc<dyn Tool>);
}

/// 危险组：`delete`。`destructive = true`，审批门禁未上线前不得加入生产注册表。
pub fn register_dangerous_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry.register(Arc::new(agent_delete::AgentDeleteTool::new(receipts)) as Arc<dyn Tool>);
}
