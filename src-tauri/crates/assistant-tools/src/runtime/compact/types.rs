use crate::result::ToolErrorKind;

/// head 单行上限（字符）：单行 + 160 字符是信封的硬约束（I46 compact §3.1）。
pub const HEAD_MAX_CHARS: usize = 160;
/// subject 上限（字符）：超长按中间省略。
pub const SUBJECT_MAX_CHARS: usize = 80;
/// `argsPreview` 上限（字符）：Started 首帧只带参数摘要。
pub const ARGS_PREVIEW_MAX_CHARS: usize = 200;
/// 单个参数值上限（字符）：避免一个长文本把整条预览挤满。
pub(super) const PREVIEW_VALUE_MAX_CHARS: usize = 60;
pub(super) const HEAD_SEPARATOR: &str = " · ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedUnit {
    Lines,
    Items,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncatedInfo {
    pub unit: TruncatedUnit,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedInfo {
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocAnchor {
    pub path: String,
    pub ranges: Vec<LineRange>,
}

/// 一次调用的紧凑信封：展示与历史重载的唯一事实源（I46 compact §3.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResultCompact {
    pub head: String,
    pub truncated: Option<TruncatedInfo>,
    pub persisted: Option<PersistedInfo>,
    pub error_kind: Option<ToolErrorKind>,
    pub anchor: Option<DocAnchor>,
}
