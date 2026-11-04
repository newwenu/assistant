use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use std::collections::HashMap;

use super::json_config::{get_conf_dir, load_json_config_with_migrate, save_json_config};

fn default_true() -> bool {
    true
}

fn default_compaction_threshold() -> u8 {
    super::constant::COMPACTION_THRESHOLD_PERCENT
}

fn default_max_summary_tokens() -> u16 {
    super::constant::MAX_SUMMARY_REQUEST_TOKENS as u16
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AppConf {
    pub theme: String,
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
    /// 旧版配置键，仅用于迁移读取，不再写出
    #[serde(
        default,
        rename = "compact_threshold_percent",
        skip_serializing_if = "Option::is_none"
    )]
    pub legacy_compact_threshold_percent: Option<u8>,
    #[serde(default)]
    pub compact_summary_prompt: Option<String>,
    #[serde(default = "default_max_summary_tokens")]
    pub compact_summary_max_tokens: u16,
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
pub fn default_keybindings() -> HashMap<String, String> {
    let primary = primary_modifier();
    let mut keybindings = HashMap::new();
    keybindings.insert("toggle_orb".to_string(), format!("{}+Shift+Alt+O", primary));
    keybindings.insert(
        "toggle_main".to_string(),
        format!("{}+Shift+Alt+M", primary),
    );
    keybindings.insert("new_chat".to_string(), format!("{}+Shift+Alt+N", primary));
    keybindings
}

impl AppConf {
    pub fn new() -> Self {
        Self {
            theme: "system".to_string(),
            language: "zh-CN".to_string(),
            hotkey: "".to_string(),
            stay_on_top: false,
            launch_at_login: false,
            close_to_tray: true,
            keybindings: default_keybindings(),
            auto_compact: true,
            compact_threshold_percent_of_budget: super::constant::COMPACTION_THRESHOLD_PERCENT,
            legacy_compact_threshold_percent: None,
            compact_summary_prompt: None,
            compact_summary_max_tokens: super::constant::MAX_SUMMARY_REQUEST_TOKENS as u16,
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

    fn migrate(mut config: Self) -> Self {
        // 旧键 `compact_threshold_percent` 指「占完整窗口的百分比」，新键指「占可用预算的百分比」。
        // 两者分母不同，直接沿用数值属于同名不同义，因此迁移时显式告警并提示重新确认取值。
        if let Some(legacy) = config.legacy_compact_threshold_percent.take() {
            // 仅当新键未显式设置（仍为默认值）时才沿用旧值，避免旧键覆盖用户已确认的新取值
            if config.compact_threshold_percent_of_budget == default_compaction_threshold() {
                log::warn!(
                    "config key `compact_threshold_percent` ({}) 已废弃，语义由「占完整窗口」改为「占可用预算」；旧值已迁移到 compact_threshold_percent_of_budget，实际阈值可能变化，请在设置中确认。",
                    legacy
                );
                config.compact_threshold_percent_of_budget = legacy;
            } else {
                log::warn!(
                    "config key `compact_threshold_percent` ({}) 已废弃并忽略：`compact_threshold_percent_of_budget` 已显式设置为 {}。",
                    legacy,
                    config.compact_threshold_percent_of_budget
                );
            }
        }
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    pub fn validate(&self) -> Result<(), String> {
        let valid_themes = ["system", "light", "dark"];
        if !valid_themes.contains(&self.theme.as_str()) {
            return Err(format!(
                "theme must be system/light/dark, got {}",
                self.theme
            ));
        }
        if self.language.is_empty() {
            return Err("language must not be empty".to_string());
        }
        Ok(())
    }

    pub fn amend(self, patch: serde_json::Value) -> Result<Self, String> {
        let val = serde_json::to_value(&self).map_err(|e| e.to_string())?;
        let mut config: BTreeMap<String, serde_json::Value> =
            serde_json::from_value(val).map_err(|e| e.to_string())?;
        let new_json: BTreeMap<String, serde_json::Value> =
            serde_json::from_value(patch).map_err(|e| e.to_string())?;
        for (k, v) in new_json {
            config.insert(k, v);
        }
        serde_json::from_str::<AppConf>(
            &serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("amend deserialization failed: {}", e))
    }
}

impl Default for AppConf {
    fn default() -> Self {
        Self::new()
    }
}
