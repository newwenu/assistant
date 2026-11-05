use std::sync::Arc;

use assistant_agent::kernel::{
    CompactionConfig, EventSink, SessionStates, TurnInputSource, TurnRunner, TurnTasks,
};
use tauri::Manager;

use super::{DbMessageStore, SystemClock, TauriCompactionSink};

/// 用宿主托管的单例状态装配 runner；每次 turn 只换 `sink` 与输入源。
pub fn build_turn_runner(
    app: &tauri::AppHandle,
    sink: Arc<dyn EventSink>,
    input: Arc<dyn TurnInputSource>,
) -> TurnRunner {
    let tasks = app.state::<Arc<TurnTasks>>().inner().clone();
    let states = app.state::<Arc<SessionStates>>().inner().clone();
    TurnRunner::new(
        sink,
        Arc::new(DbMessageStore::new(app.clone())),
        input,
        states,
        tasks,
        Arc::new(SystemClock::new()),
        compaction_config(app),
    )
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
