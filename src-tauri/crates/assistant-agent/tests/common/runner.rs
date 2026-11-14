//! `TurnRunner` 装配：统一 `conv-1` 会话的输入源、状态表与时钟注入。
//!
//! 只装配端口与 fixture，不驱动任何逻辑。`states` 随装配结果一并返回，
//! 让用例能继续断言 runner 实际持有的同一份会话状态。

#![allow(dead_code)]

use std::sync::Arc;

use crate::common::fixtures::fixture_input_with_models;
use assistant_agent::kernel::ports::test_doubles::{
    FixedClock, InMemoryStore, RecordingSink, ScriptedAdapter, ScriptedResponse,
};
use assistant_agent::kernel::ports::EventSink;
use assistant_agent::kernel::{Clock, CompactionConfig, SessionStates, TurnRunner, TurnTasks};
use assistant_providers::base::BaseProviderAdapter;

/// 装配产物：runner 与它持有的会话状态表。
pub struct Assembled {
    pub runner: TurnRunner,
    pub states: Arc<SessionStates>,
}

/// 默认装配：会话 `conv-1`、空模型元数据、不启用压缩。
pub fn build<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
    clock: Arc<dyn Clock>,
) -> Assembled {
    build_with_options(adapter, sink, store, tasks, clock, "[]", None)
}

/// 指定模型元数据（如 `contextWindow`）与压缩配置的装配。
pub fn build_with_options<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
    clock: Arc<dyn Clock>,
    models_json: &str,
    compaction: Option<CompactionConfig>,
) -> Assembled {
    let states = Arc::new(SessionStates::new());
    let runner = TurnRunner::new(
        sink,
        store,
        Arc::new(fixture_input_with_models("conv-1", adapter, models_json)),
        states.clone(),
        tasks,
        clock,
        compaction,
    );
    Assembled { runner, states }
}

/// 用例侧持有的一整套装配结果，便于直接取 `sink` / `store` / `tasks` 断言。
pub struct Harness<S: EventSink + 'static> {
    pub runner: TurnRunner,
    pub sink: Arc<S>,
    pub store: Arc<InMemoryStore>,
    pub tasks: Arc<TurnTasks>,
    pub states: Arc<SessionStates>,
}

/// 最常用装配：录制式 sink、内存 store、新任务表、固定时钟。
pub fn harness(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: RecordingSink,
    store: InMemoryStore,
) -> Harness<RecordingSink> {
    harness_with(
        adapter,
        Arc::new(sink),
        Arc::new(store),
        Arc::new(TurnTasks::new()),
    )
}

pub fn harness_with<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
) -> Harness<S> {
    harness_with_options(adapter, sink, store, tasks, "[]", None)
}

/// 允许指定 provider 的模型元数据（如 `contextWindow`）与压缩配置，用于验证终止式压缩触发。
pub fn harness_with_options<S: EventSink + 'static>(
    adapter: Arc<dyn BaseProviderAdapter>,
    sink: Arc<S>,
    store: Arc<InMemoryStore>,
    tasks: Arc<TurnTasks>,
    models_json: &str,
    compaction: Option<CompactionConfig>,
) -> Harness<S> {
    let assembled = build_with_options(
        adapter,
        sink.clone(),
        store.clone(),
        tasks.clone(),
        Arc::new(FixedClock::default()),
        models_json,
        compaction,
    );
    Harness {
        runner: assembled.runner,
        sink,
        store,
        tasks,
        states: assembled.states,
    }
}

/// 脚本化 provider 的装配：用例只关心「按脚本产出什么」，不关心端口接线。
pub fn scripted_harness(scripts: Vec<ScriptedResponse>) -> Harness<RecordingSink> {
    let adapter: Arc<dyn BaseProviderAdapter> = Arc::new(ScriptedAdapter::new(scripts));
    harness(adapter, RecordingSink::new(), InMemoryStore::new())
}
