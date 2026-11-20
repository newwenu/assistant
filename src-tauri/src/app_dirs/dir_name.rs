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
pub const MASCOT_ASSETS: &str = "mascot-assets";
pub const LOGS: &str = "logs";
pub const CRASHES: &str = "crashes";

/// 常量显式清单：V19 靠它枚举，新增常量必须同时进 `ALL` 与 `MANAGED_ROOTS`。
#[cfg(test)]
pub const ALL: &[&str] = &[
    DATABASE,
    ATTACHMENTS,
    TOOL_RESULTS,
    TOOL_INDEX,
    MASCOT_ASSETS,
    LOGS,
    CRASHES,
];
