//! `TurnRunner` 用例共用的轮上下文构造。

#![allow(dead_code)]

use assistant_agent::kernel::{OnBusy, TurnContext, TurnTasks};

/// 在 `TurnTasks` 上登记一轮并返回上下文；占位行 id 固定为 `assistant-1`。
///
/// `turn_index` 传 `None`（存量分支点语义），需要坐标的用例自行覆写。
pub fn started_ctx(tasks: &TurnTasks, conversation_id: &str) -> TurnContext {
    let (turn_id, _token) = tasks
        .begin(conversation_id, OnBusy::Replace)
        .expect("begin");
    TurnContext::new(conversation_id, turn_id, "assistant-1", None)
}
