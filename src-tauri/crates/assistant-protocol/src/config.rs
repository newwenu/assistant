use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 主题模式（封闭集合）。`system` 表示跟随系统，由前端解析为具体值。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ThemeModeDto {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppConfDto {
    pub theme: ThemeModeDto,
    pub language: String,
    pub hotkey: String,
    pub stay_on_top: bool,
    pub launch_at_login: bool,
    pub close_to_tray: bool,
    pub keybindings: std::collections::HashMap<String, String>,
    pub auto_compact: bool,
    pub compact_threshold_percent_of_budget: u8,
    pub compact_summary_prompt: Option<String>,
    pub compact_summary_max_tokens: u16,
    pub main_window_x: Option<i32>,
    pub main_window_y: Option<i32>,
    pub main_window_width: Option<u32>,
    pub main_window_height: Option<u32>,
    pub mini_window_x: Option<i32>,
    pub mini_window_y: Option<i32>,
    pub approval_timeout_secs: Option<u32>,
    pub user_question_timeout_secs: Option<u32>,
    /// 工具结果落盘文件保留天数；`0` 表示禁用清理（I46 compact §2.9 D9）。
    pub result_retention_days: u32,
    /// 审计 JSONL 日文件保留天数；`0` 表示禁用清理（I50 §5 G12）。
    pub audit_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrbConfDto {
    pub click_through: bool,
    pub always_on_top: bool,
    pub scale_percent: u16,
    pub opacity_percent: u8,
    pub hide_on_pointer_hover: bool,
    pub hover_delay_ms: u64,
    pub idle_opacity_percent: u8,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MascotConfDto {
    pub renderer: String,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub rive_idle_pose_rotation: bool,
    pub rive_src: Option<String>,
    pub svg_animated: bool,
    pub gif_src: Option<String>,
    pub gif_border_radius: u32,
}
