mod shared;
mod tools;

pub(crate) use shared::{footer, fs_util, scope, walk};
pub use shared::{output, path, process_output, text_encoding};

// 测试夹具：`cargo test` 编 lib 时（cfg(test)）以及其它 package 的集成测试
// （`test-support` feature）都需要它。以 `#[path]` 指向集成测试树里的唯一实现，
// 平台分派只写一次；对集成测试公开，故不能只挂 `#[cfg(test)]`。
#[cfg(any(test, feature = "test-support"))]
#[path = "shared/test_support.rs"]
pub mod test_support;

pub use tools::{
    agent_ask_user, agent_delete, agent_edit, agent_glob, agent_grep, agent_list, agent_mkdir,
    agent_move, agent_read, agent_request_tool, agent_shell, agent_write, fs_link, index_stats,
    purge_index_files, super_edit, super_read, IndexPurgeReport,
};

use std::sync::Arc;

use crate::os::agent_ask_user::QuestionTimeoutSource;
use crate::question::QuestionPrompter;
use crate::r#trait::Tool;
use crate::registry::ToolRegistry;
use crate::sandbox::{SandboxEnforcement, SandboxProbe};
use crate::storage::{ReadReceiptStore, ToolResultStorage};

/// `TOOLBOX_ONLY` 桥接工具：只服务工具箱入口，不进 Agent 白名单。
///
/// 名字即受众声明：本组工具**不可**被模型调用（`find_for_agent` 一律拒绝），
/// 只能经 `os_tool_exec` 按 id 调用。新工具不得混进本函数（I38c §7 注册门禁 / I72 §2.5）。
pub fn register_bridge_tools(registry: &ToolRegistry) {
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

/// 命令执行组：`shell`。
///
/// 两条前置条件缺一即**不注册**（不注册空工具、不降级为无沙箱执行）：
/// 探测结论 `enforcement != none`（门禁判据由调用方传入，这里复核一次），
/// 以及 shell 程序能被解析到绝对路径（否则工具存在但必然报错）。
pub fn register_shell_tools(
    registry: &ToolRegistry,
    storage: Arc<dyn ToolResultStorage>,
    probe: &'static SandboxProbe,
) {
    if probe.enforcement == SandboxEnforcement::None {
        log::warn!(
            "shell not registered: enforcement is none (detail={:?})",
            probe.detail
        );
        return;
    }
    if !agent_shell::spawn::program_available() {
        log::warn!("shell not registered: no usable shell program");
        return;
    }
    registry.register(Arc::new(agent_shell::ShellTool::new(probe, storage)) as Arc<dyn Tool>);
}
