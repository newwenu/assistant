pub mod agent_glob;
pub mod agent_grep;
pub mod agent_list;
pub mod agent_read;
pub mod agent_write;
pub mod fs_link;
pub mod output;
pub mod path;
pub mod process_output;

use std::sync::Arc;

use crate::r#trait::Tool;
use crate::registry::ToolRegistry;
use crate::storage::ReadReceiptStore;

pub fn register_os_tools(registry: &ToolRegistry) {
    registry.register(Arc::new(fs_link::FsLinkTool::new()) as Arc<dyn Tool>);
}

pub fn register_readonly_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry.register(Arc::new(agent_read::AgentReadTool::new(receipts)) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_list::AgentListTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_glob::AgentGlobTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_grep::AgentGrepTool::new()) as Arc<dyn Tool>);
}
