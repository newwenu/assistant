use serde::Serialize;

use super::round_executor::ToolErrorClass;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEvent {
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: ToolEventPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum ToolEventPayload {
    Started {
        args_preview: Option<String>,
    },
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolCompact>,
    },
    Failed {
        error: String,
        error_kind: ToolErrorClass,
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolCompact>,
    },
}

/// 内核侧的中性信封：与工具实现无关，只承载渲染所需事实（I46 compact §3.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCompact {
    pub head: String,
    pub truncated: Option<CompactTruncated>,
    pub persisted: Option<CompactPersisted>,
    pub error_kind: Option<ToolErrorClass>,
    pub anchor: Option<CompactAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CompactUnit {
    Lines,
    Items,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactTruncated {
    pub unit: CompactUnit,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactPersisted {
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactAnchor {
    pub path: String,
    pub ranges: Vec<CompactLineRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactLineRange {
    pub start: u64,
    pub end: u64,
}
