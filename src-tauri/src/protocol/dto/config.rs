use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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
    pub compact_threshold_percent_of_budget: u8,
    pub compact_summary_prompt: Option<String>,
    pub compact_summary_max_tokens: u16,
    pub main_window_x: Option<i32>,
    pub main_window_y: Option<i32>,
    pub main_window_width: Option<u32>,
    pub main_window_height: Option<u32>,
    pub mini_window_x: Option<i32>,
    pub mini_window_y: Option<i32>,
    pub workspace_root: Option<String>,
    pub approval_timeout_secs: Option<u32>,
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
            compact_threshold_percent_of_budget: c.compact_threshold_percent_of_budget,
            compact_summary_prompt: c.compact_summary_prompt,
            compact_summary_max_tokens: c.compact_summary_max_tokens,
            main_window_x: c.main_window_x,
            main_window_y: c.main_window_y,
            main_window_width: c.main_window_width,
            main_window_height: c.main_window_height,
            mini_window_x: c.mini_window_x,
            mini_window_y: c.mini_window_y,
            workspace_root: c.workspace_root,
            approval_timeout_secs: c.approval_timeout_secs,
        }
    }
}

/// 保险库加密算法（封闭集合，线格式与前端联合一致）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum VaultCipherAlgorithmDto {
    Aes256Gcm,
    Chacha20Poly1305,
}

impl From<assistant_config::vault_conf::VaultCipherAlgorithm> for VaultCipherAlgorithmDto {
    fn from(value: assistant_config::vault_conf::VaultCipherAlgorithm) -> Self {
        match value {
            assistant_config::vault_conf::VaultCipherAlgorithm::Aes256Gcm => Self::Aes256Gcm,
            assistant_config::vault_conf::VaultCipherAlgorithm::Chacha20Poly1305 => {
                Self::Chacha20Poly1305
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultConfDto {
    pub vault_auto_lock_minutes: u16,
    pub vault_lock_on_screen_lock: bool,
    pub vault_clipboard_clear_seconds: u16,
    pub vault_cipher_algorithm: VaultCipherAlgorithmDto,
    pub vault_hibp_auto_audit: bool,
    pub vault_sync_remote_url: Option<String>,
    pub vault_ssh_auto_add: bool,
    pub vault_ssh_use_pageant: bool,
    pub vault_ssh_use_openssh_agent: bool,
    pub vault_ssh_auth_sock_override: String,
    pub vault_sync_last_at: Option<i64>,
    pub vault_sync_ssh_hosts: Vec<String>,
}

impl From<assistant_config::vault_conf::VaultConf> for VaultConfDto {
    fn from(c: assistant_config::vault_conf::VaultConf) -> Self {
        Self {
            vault_auto_lock_minutes: c.vault_auto_lock_minutes,
            vault_lock_on_screen_lock: c.vault_lock_on_screen_lock,
            vault_clipboard_clear_seconds: c.vault_clipboard_clear_seconds,
            vault_cipher_algorithm: c.vault_cipher_algorithm.into(),
            vault_hibp_auto_audit: c.vault_hibp_auto_audit,
            vault_sync_remote_url: c.vault_sync_remote_url,
            vault_ssh_auto_add: c.vault_ssh_auto_add,
            vault_ssh_use_pageant: c.vault_ssh_use_pageant,
            vault_ssh_use_openssh_agent: c.vault_ssh_use_openssh_agent,
            vault_ssh_auth_sock_override: c.vault_ssh_auth_sock_override,
            vault_sync_last_at: c.vault_sync_last_at,
            vault_sync_ssh_hosts: c.vault_sync_ssh_hosts,
        }
    }
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
