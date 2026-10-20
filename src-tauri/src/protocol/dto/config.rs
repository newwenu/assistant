use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppConfDto {
    pub theme: String,
    pub language: String,
    pub hotkey: String,
    pub stay_on_top: bool,
    pub launch_at_login: bool,
    pub close_to_tray: bool,
    pub keybindings: std::collections::HashMap<String, String>,
    pub auto_compact: bool,
    pub compact_threshold_percent: u8,
    pub compact_summary_prompt: Option<String>,
    pub compact_summary_max_tokens: u16,
}

impl From<assistant_config::app_conf::AppConf> for AppConfDto {
    fn from(c: assistant_config::app_conf::AppConf) -> Self {
        Self {
            theme: c.theme,
            language: c.language,
            hotkey: c.hotkey,
            stay_on_top: c.stay_on_top,
            launch_at_login: c.launch_at_login,
            close_to_tray: c.close_to_tray,
            keybindings: c.keybindings,
            auto_compact: c.auto_compact,
            compact_threshold_percent: c.compact_threshold_percent,
            compact_summary_prompt: c.compact_summary_prompt,
            compact_summary_max_tokens: c.compact_summary_max_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl From<assistant_config::orb_conf::OrbConf> for OrbConfDto {
    fn from(c: assistant_config::orb_conf::OrbConf) -> Self {
        Self {
            click_through: c.click_through,
            always_on_top: c.always_on_top,
            scale_percent: c.scale_percent,
            opacity_percent: c.opacity_percent,
            hide_on_pointer_hover: c.hide_on_pointer_hover,
            hover_delay_ms: c.hover_delay_ms,
            idle_opacity_percent: c.idle_opacity_percent,
            position_x: c.position_x,
            position_y: c.position_y,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl From<assistant_config::mascot_conf::MascotConf> for MascotConfDto {
    fn from(c: assistant_config::mascot_conf::MascotConf) -> Self {
        Self {
            renderer: c.renderer,
            primary_color: c.primary_color,
            secondary_color: c.secondary_color,
            rive_idle_pose_rotation: c.rive_idle_pose_rotation,
            rive_src: c.rive_src,
            svg_animated: c.svg_animated,
            gif_src: c.gif_src,
            gif_border_radius: c.gif_border_radius,
        }
    }
}
