use serde::{Deserialize, Serialize};
use std::path::Path;

use std::collections::HashMap;

use super::json_config::{
    amend_json_config, get_conf_dir, load_json_config_with_migrate, save_json_config,
};

fn default_true() -> bool {
    true
}

fn default_compaction_threshold() -> u8 {
    super::constant::COMPACTION_THRESHOLD_PERCENT
}

fn default_max_summary_tokens() -> u16 {
    super::constant::MAX_SUMMARY_REQUEST_TOKENS as u16
}

/// 主题模式（封闭集合）。线格式 `system` / `light` / `dark`，与协议层
/// `ThemeModeDto` 及前端联合类型一致；非法值在反序列化阶段即被拒绝。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AppConf {
    pub theme: ThemeMode,
    pub language: String,
    pub hotkey: String,
    pub stay_on_top: bool,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub close_to_tray: bool,
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub auto_compact: bool,
    /// 可用预算（窗口扣除输出预留与安全裕度）的使用比例，不是占完整窗口的比例
    #[serde(default = "default_compaction_threshold")]
    pub compact_threshold_percent_of_budget: u8,
    #[serde(default)]
    pub compact_summary_prompt: Option<String>,
    #[serde(default = "default_max_summary_tokens")]
    pub compact_summary_max_tokens: u16,
    /// normal 形态的窗口位置（物理像素）
    #[serde(default)]
    pub main_window_x: Option<i32>,
    #[serde(default)]
    pub main_window_y: Option<i32>,
    /// normal 形态的窗口尺寸（逻辑像素）
    #[serde(default)]
    pub main_window_width: Option<u32>,
    #[serde(default)]
    pub main_window_height: Option<u32>,
    /// mini 形态的窗口位置（物理像素）；mini 尺寸固定，不存宽高
    #[serde(default)]
    pub mini_window_x: Option<i32>,
    #[serde(default)]
    pub mini_window_y: Option<i32>,
    /// 审批等待上限（秒）。`None` → 默认 120s；`Some(0)` 非法（审批必须有时限）。
    #[serde(default)]
    pub approval_timeout_secs: Option<u32>,
    /// `ask_user` 等待用户作答的上限（秒）。`None` → 默认 180s；
    /// `Some(0)` → 不设 deadline（只由取消终止），因此不像审批那样拒绝 0。
    #[serde(default)]
    pub user_question_timeout_secs: Option<u32>,
}

/// 平台主修饰键：macOS 用 Command，其余用 Control。
pub fn primary_modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "Meta"
    } else {
        "Control"
    }
}

/// 默认快捷键的唯一来源（Rust 注册与前端默认值必须保持一致）。
///
/// 统一使用四键组合，避开浏览器、编辑器和系统的高频占用：
/// `Ctrl+Shift+N`（浏览器无痕/新建窗口）、`Ctrl+Shift+O`（编辑器跳转符号）、
/// `Ctrl+Shift+M`（部分 IDE 重构）等都不再作为默认值。
///
/// 第二项是线格式模板，`{primary}` 由 [`primary_modifier`] 在运行时替换；
/// 前端通过 `npm run contract:gen` 生成的 `src/types/shortcutDefaults.generated.ts`
/// 读取同一份模板，因此增删快捷键只需改这张表。
pub static DEFAULT_SHORTCUT_PATTERNS: &[(&str, &str)] = &[
    ("toggle_orb", "{primary}+Shift+Alt+O"),
    ("toggle_main", "{primary}+Shift+Alt+M"),
    ("new_chat", "{primary}+Shift+Alt+N"),
];

pub fn default_keybindings() -> HashMap<String, String> {
    let primary = primary_modifier();
    DEFAULT_SHORTCUT_PATTERNS
        .iter()
        .map(|(id, pattern)| (id.to_string(), pattern.replace("{primary}", primary)))
        .collect()
}

impl AppConf {
    pub fn new() -> Self {
        Self {
            theme: ThemeMode::System,
            language: "zh-CN".to_string(),
            hotkey: "".to_string(),
            stay_on_top: false,
            launch_at_login: false,
            close_to_tray: true,
            keybindings: default_keybindings(),
            auto_compact: true,
            compact_threshold_percent_of_budget: super::constant::COMPACTION_THRESHOLD_PERCENT,
            compact_summary_prompt: None,
            compact_summary_max_tokens: super::constant::MAX_SUMMARY_REQUEST_TOKENS as u16,
            main_window_x: None,
            main_window_y: None,
            main_window_width: None,
            main_window_height: None,
            mini_window_x: None,
            mini_window_y: None,
            approval_timeout_secs: None,
            user_question_timeout_secs: None,
        }
    }

    pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        get_conf_dir()
    }

    pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(get_conf_dir()?.join("config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        load_json_config_with_migrate(path, Self::new, Self::migrate, "AppConf")
    }

    /// 配置迁移入口。当前没有待执行的迁移，仅保留空实现与下方模板。
    ///
    /// 模板（字段语义变更或键改名时按此实现，迁移窗口结束后连同旧字段一起删除）：
    /// 1. 旧键以 `#[serde(default, rename = "旧键名")] Option<T>` 字段接收，并加
    ///    `skip_serializing_if = "Option::is_none"`，保证未迁移时旧键不会被写回；
    /// 2. 在此 `take()` 旧值：仅当新键仍为默认值时才沿用旧值，否则保留用户已确认的新值；
    /// 3. 两条分支都 `log::warn!` 说明取值来源与可能的变化，提示用户到设置里确认；
    /// 4. 旧版本不再有用户后，删除旧字段与本段逻辑，恢复成空实现。
    ///
    /// 已移除的迁移：`compact_threshold_percent` → `compact_threshold_percent_of_budget`
    /// （分母由「完整窗口」改为「可用预算」，I29 引入，2026-11-06 关闭迁移窗口）。
    fn migrate(config: Self) -> Self {
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.language.is_empty() {
            return Err("language must not be empty".to_string());
        }
        if self.approval_timeout_secs == Some(0) {
            return Err("approval_timeout_secs must be greater than 0".to_string());
        }
        Ok(())
    }

    pub fn amend(self, patch: serde_json::Value) -> Result<Self, String> {
        amend_json_config(&self, patch)
    }
}

impl Default for AppConf {
    fn default() -> Self {
        Self::new()
    }
}
