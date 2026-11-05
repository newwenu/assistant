use serde::{Deserialize, Serialize};

use super::ids::TurnId;

/// 封闭集合：新增 phase 必须同步前端生成类型（`npm run contract:gen`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnPhase {
    Starting,
    Compacting,
    Thinking,
    Streaming,
    ToolCalling,
    AwaitingApproval,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SessionState {
    Idle,
    Processing { turn_id: TurnId, phase: TurnPhase },
    Error { message: String, recoverable: bool },
}
