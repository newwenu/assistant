// 受管目录的第一层目录名与文件名：全仓库唯一字面量。
//
// 前端与 `tauri.conf.json` 不重复这些字符串，由漂移测试比对（T2）。
// 库内部的子布局（`derived/`、`backups/`、`rive/` 等）不在此列。
//
// 这里只放无条件能力的目录名。可选能力自持其目录名，落在自己的 `cmd/<feature>/` 模块内：
// 装配契约要求 `#[cfg(feature = ...)]` 只出现在 `cmd/mod.rs` 与同名 `*_plugin.rs`，本模块不参与特性门控。

pub const DATABASE: &str = "assistant.db";
pub const ATTACHMENTS: &str = "attachments";
pub const TOOL_RESULTS: &str = "tool-results";
pub const TOOL_INDEX: &str = "tool-index";
/// shell 临时区基根（会话级单目录，I73 §3.8 / W5）。
///
/// 命名判据见规格 §3.8「域根与命名」：含 `scratch` 与代码词汇一致、与 scheme 同名、
/// 前缀粒度是 `tool-` 而非 `shell-`、在存储管理列表里与 `tool-index` / `tool-results` 相邻。
pub const TOOL_SCRATCH: &str = "tool-scratch";
pub const MASCOT_ASSETS: &str = "mascot-assets";
pub const LOGS: &str = "logs";
pub const CRASHES: &str = "crashes";
/// WebView2 的 profile 目录（Windows 专属；其它平台恒 `missing`，I67 §13.3.7）。
pub const WEBVIEW_PROFILE: &str = "EBWebView";

/// 常量显式清单：V19 靠它枚举，新增常量必须同时进 `ALL` 与 `MANAGED_ROOTS`。
#[cfg(test)]
pub const ALL: &[&str] = &[
    DATABASE,
    ATTACHMENTS,
    TOOL_RESULTS,
    TOOL_INDEX,
    TOOL_SCRATCH,
    MASCOT_ASSETS,
    LOGS,
    CRASHES,
    WEBVIEW_PROFILE,
];
