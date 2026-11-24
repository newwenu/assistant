// 域类型 → DTO 桥接（I62 §4.4）：按 Rust 孤儿规则归属本 crate。

use assistant_protocol::*;

impl From<crate::app_conf::ThemeMode> for ThemeModeDto {
    fn from(value: crate::app_conf::ThemeMode) -> Self {
        match value {
            crate::app_conf::ThemeMode::System => Self::System,
            crate::app_conf::ThemeMode::Light => Self::Light,
            crate::app_conf::ThemeMode::Dark => Self::Dark,
        }
    }
}

impl From<crate::app_conf::AppConf> for AppConfDto {
    fn from(c: crate::app_conf::AppConf) -> Self {
        Self {
            theme: c.theme.into(),
            language: c.language,
            hotkey: c.hotkey,
            stay_on_top: c.stay_on_top,
            launch_at_login: c.launch_at_login,
            close_to_tray: c.close_to_tray,
            keybindings: c.keybindings,
            auto_compact: c.auto_compact,
            compact_threshold_percent_of_budget: c.compact_threshold_percent_of_budget,
            compact_summary_prompt: c.compact_summary_prompt,
            compact_summary_max_tokens: c.compact_summary_max_tokens,
            main_window_x: c.main_window_x,
            main_window_y: c.main_window_y,
            main_window_width: c.main_window_width,
            main_window_height: c.main_window_height,
            mini_window_x: c.mini_window_x,
            mini_window_y: c.mini_window_y,
            approval_timeout_secs: c.approval_timeout_secs,
            user_question_timeout_secs: c.user_question_timeout_secs,
            result_retention_days: c.result_retention_days,
            audit_retention_days: c.audit_retention_days,
        }
    }
}

impl From<crate::orb_conf::OrbConf> for OrbConfDto {
    fn from(c: crate::orb_conf::OrbConf) -> Self {
        Self {
            click_through: c.click_through,
            always_on_top: c.always_on_top,
            opacity_percent: c.opacity_percent,
            hide_on_pointer_hover: c.hide_on_pointer_hover,
            hover_delay_ms: c.hover_delay_ms,
            idle_opacity_percent: c.idle_opacity_percent,
            position_x: c.position_x,
            position_y: c.position_y,
        }
    }
}

impl From<crate::mascot_conf::MascotConf> for MascotConfDto {
    fn from(c: crate::mascot_conf::MascotConf) -> Self {
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
