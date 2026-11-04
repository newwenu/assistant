pub mod fs_link;
pub mod path;
pub mod process_output;

use std::sync::Arc;

use crate::r#trait::Tool;
use crate::registry::ToolRegistry;

pub fn register_os_tools(registry: &ToolRegistry) {
    registry.register(Arc::new(fs_link::FsLinkTool::new()) as Arc<dyn Tool>);
}
