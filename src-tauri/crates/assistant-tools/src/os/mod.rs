mod shared;
mod tools;

pub(crate) use shared::{footer, fs_util, scope, walk};
pub use shared::{output, path, process_output, text_encoding};

#[cfg(test)]
pub(crate) use shared::test_support;

pub use tools::{
    agent_ask_user, agent_delete, agent_edit, agent_glob, agent_grep, agent_list, agent_mkdir,
    agent_move, agent_read, agent_request_tool, agent_write, fs_link, super_edit, super_read,
};

use std::sync::Arc;

use crate::os::agent_ask_user::QuestionTimeoutSource;
use crate::question::QuestionPrompter;
use crate::r#trait::Tool;
use crate::registry::ToolRegistry;
use crate::storage::ReadReceiptStore;

/// `TOOLBOX_ONLY` 桥接工具：只服务工具箱入口，不进 Agent 白名单。
/// 新工具不得混进本函数（见 I38c §7 注册门禁）。
pub fn register_os_tools(registry: &ToolRegistry) {
    registry.register(Arc::new(fs_link::FsLinkTool::new()) as Arc<dyn Tool>);
}

pub fn register_readonly_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry.register(Arc::new(agent_read::AgentReadTool::new(receipts.clone())) as Arc<dyn Tool>);
    // 按行索引读取（I58 §3.2）：与 `read` 同属只读组，共用同一份读取回执。
    registry.register(Arc::new(super_read::SuperReadTool::new(receipts)) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_list::AgentListTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_glob::AgentGlobTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_grep::AgentGrepTool::new()) as Arc<dyn Tool>);
    // 能力发现（I58 §3.1）：只读、无审批，属只读组；目录由 Pipeline 注入，
    // 因此本组不持有注册表。
    registry.register(Arc::new(agent_request_tool::AgentRequestTool::new()) as Arc<dyn Tool>);
}

/// 修改组：`write` / `edit` / `mkdir` / `move`。
///
/// 与危险组分开注册，让调用方按阶段门禁逐个放开（P5 生产注册只到只读组）。
pub fn register_write_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry
        .register(Arc::new(agent_write::AgentWriteTool::new(receipts.clone())) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_edit::AgentEditTool::new(receipts.clone())) as Arc<dyn Tool>);
    // 按行地址替换（I58 §3.3）：与 `edit` 同组、同审批键，随 `edit` 一起放开。
    registry.register(Arc::new(super_edit::SuperEditTool::new(receipts.clone())) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_mkdir::AgentMkdirTool::new()) as Arc<dyn Tool>);
    registry.register(Arc::new(agent_move::AgentMoveTool::new(receipts)) as Arc<dyn Tool>);
}

/// 危险组：`delete`。`destructive = true`，审批门禁未上线前不得加入生产注册表。
pub fn register_dangerous_tools(registry: &ToolRegistry, receipts: Arc<dyn ReadReceiptStore>) {
    registry.register(Arc::new(agent_delete::AgentDeleteTool::new(receipts)) as Arc<dyn Tool>);
}

/// 交互组：`ask_user`。必须同时给出提问通道与超时来源；
/// 没有 UI 的场合调用方不应调用本函数（`QuestionPrompter` 缺省即 fail-closed）。
pub fn register_interactive_tools(
    registry: &ToolRegistry,
    prompter: Arc<dyn QuestionPrompter>,
    timeout: QuestionTimeoutSource,
) {
    registry.register(
        Arc::new(agent_ask_user::AgentAskUserTool::new(prompter, timeout)) as Arc<dyn Tool>,
    );
}
