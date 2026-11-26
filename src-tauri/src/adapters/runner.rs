use std::sync::Arc;

use assistant_agent::agent::round_executor::ToolExecutionPort;
use assistant_agent::kernel::{
    CompactionConfig, EventSink, SessionStates, TurnInputSource, TurnRunner, TurnTasks,
};
use tauri::Manager;

use super::{DbMessageStore, SystemTimeSource, TauriCompactionSink};

/// 用宿主托管的单例状态装配 runner；每次 turn 只换 `sink` 与输入源。
///
/// `tool_exec` 为 `Some` 时使用调用方按会话有效工具配置派生的执行器（C7），
/// `None` 时经 `access::tool_exec` 取宿主托管的默认策略执行器（聊天等无工具配置的场景）。
/// 未就绪返回 `NotReady`，由调用点显式降级（C6：不 panic、不隐式装配）。
pub fn build_turn_runner(
    app: &tauri::AppHandle,
    sink: Arc<dyn EventSink>,
    input: Arc<dyn TurnInputSource>,
    tool_exec: Option<Arc<dyn ToolExecutionPort>>,
) -> Result<TurnRunner, crate::boot::CapErr> {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    let states = app.state::<Arc<SessionStates>>().inner().clone();
    let tool_exec: Arc<dyn ToolExecutionPort> = match tool_exec {
        Some(exec) => exec,
        None => crate::boot::tool_exec(app)?,
    };
    Ok(TurnRunner::new(
        sink,
        Arc::new(DbMessageStore::new(app.clone())),
        input,
        states,
        tasks,
        Arc::new(SystemTimeSource::new()),
        compaction_config(app),
    )
    .with_tool_exec(tool_exec))
}

/// 终止式压缩的判定参数来自 `AppConf`，执行者由适配层提供。
fn compaction_config(app: &tauri::AppHandle) -> Option<CompactionConfig> {
    let (auto_compact, threshold_percent) = assistant_config::app_conf::AppConf::load()
        .map(|c| (c.auto_compact, c.compact_threshold_percent_of_budget))
        .unwrap_or((
            true,
            assistant_config::constant::COMPACTION_THRESHOLD_PERCENT,
        ));
    Some(CompactionConfig {
        sink: Arc::new(TauriCompactionSink::new(app.clone())),
        auto_compact,
        threshold_percent,
    })
}
