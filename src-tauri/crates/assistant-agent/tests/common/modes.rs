//! `AgentModeRow` 最小构造：只保留决定行为的字段，其余取缺省值。
//!
//! 与 storage 的 `AgentModeRow` 字段增删解耦：新增字段不必逐个改测试。

#![allow(dead_code)]

use assistant_storage::agent_modes::AgentModeRow;

pub fn mode_row(kind: &str) -> AgentModeRow {
    AgentModeRow {
        id: format!("mode-{kind}"),
        name: kind.to_string(),
        kind: kind.to_string(),
        ..Default::default()
    }
}
