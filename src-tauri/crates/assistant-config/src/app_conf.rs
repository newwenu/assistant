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

fn default_vault_auto_lock_minutes() -> u16 {
    5
}

fn default_vault_clipboard_clear_seconds() -> u16 {
    30
}

fn default_vault_cipher_algorithm() -> String {
    "aes256Gcm".to_string()
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
    #[serde(default = "default_compaction_threshold")]
    pub compact_threshold_percent: u8,
    #[serde(default)]
    pub compact_summary_prompt: Option<String>,
    #[serde(default = "default_max_summary_tokens")]
    pub compact_summary_max_tokens: u16,
    #[serde(default = "default_vault_auto_lock_minutes")]
    pub vault_auto_lock_minutes: u16,
    #[serde(default = "default_true")]
    pub vault_lock_on_screen_lock: bool,
    #[serde(default = "default_vault_clipboard_clear_seconds")]
    pub vault_clipboard_clear_seconds: u16,
    #[serde(default = "default_vault_cipher_algorithm")]
    pub vault_cipher_algorithm: String,
    #[serde(default)]
    pub vault_biometric_enabled: bool,
    #[serde(default = "default_true")]
    pub vault_hibp_auto_audit: bool,
    #[serde(default)]
    pub vault_sync_remote_url: Option<String>,
    #[serde(default)]
    pub vault_ssh_auto_add: bool,
    #[serde(default = "default_true")]
    pub vault_ssh_use_pageant: bool,
    #[serde(default = "default_true")]
    pub vault_ssh_use_openssh_agent: bool,
    #[serde(default)]
    pub vault_ssh_auth_sock_override: String,
}

impl AppConf {
    pub fn new() -> Self {
        let mut keybindings = HashMap::new();
        keybindings.insert("toggle_orb".to_string(), "Meta+Shift+O".to_string());
        keybindings.insert("toggle_main".to_string(), "Meta+Shift+M".to_string());
        keybindings.insert("new_chat".to_string(), "Meta+Shift+N".to_string());

        Self {
            theme: "system".to_string(),
            language: "zh-CN".to_string(),
            hotkey: "".to_string(),
            stay_on_top: false,
            launch_at_login: false,
            close_to_tray: true,
            keybindings,
            auto_compact: true,
            compact_threshold_percent: super::constant::COMPACTION_THRESHOLD_PERCENT,
            compact_summary_prompt: None,
            compact_summary_max_tokens: super::constant::MAX_SUMMARY_REQUEST_TOKENS as u16,
            vault_auto_lock_minutes: 5,
            vault_lock_on_screen_lock: true,
            vault_clipboard_clear_seconds: 30,
            vault_cipher_algorithm: "aes256Gcm".to_string(),
            vault_biometric_enabled: false,
            vault_hibp_auto_audit: true,
            vault_sync_remote_url: None,
            vault_ssh_auto_add: false,
            vault_ssh_use_pageant: true,
            vault_ssh_use_openssh_agent: true,
            vault_ssh_auth_sock_override: String::new(),
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
