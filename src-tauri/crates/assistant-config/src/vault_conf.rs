use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::json_config::{get_conf_dir, load_json_config_with_migrate, save_json_config};

fn default_true() -> bool {
    true
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
pub struct VaultConf {
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
    #[serde(default)]
    pub vault_sync_last_at: Option<i64>,
    #[serde(default)]
    pub vault_sync_ssh_hosts: Vec<String>,
}

impl VaultConf {
    pub fn new() -> Self {
        Self {
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
            vault_sync_last_at: None,
            vault_sync_ssh_hosts: Vec::new(),
        }
    }

    pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        get_conf_dir()
    }

    pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(get_conf_dir()?.join("vault-config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            if let Some(legacy) = Self::migrate_from_app_conf(path) {
                legacy.save_to(path)?;
                return Ok(legacy);
            }
        }
        load_json_config_with_migrate(path, Self::new, Self::migrate, "VaultConf")
    }

    fn migrate_from_app_conf(vault_path: &Path) -> Option<Self> {
        let legacy_path = vault_path.parent()?.join("config.json");
        let contents = std::fs::read_to_string(legacy_path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
        serde_json::from_value(value).ok()
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
        let valid_ciphers = ["aes256Gcm", "chacha20Poly1305"];
        if !valid_ciphers.contains(&self.vault_cipher_algorithm.as_str()) {
            return Err(format!(
                "vault_cipher_algorithm must be aes256Gcm/chacha20Poly1305, got {}",
                self.vault_cipher_algorithm
            ));
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
        serde_json::from_str::<VaultConf>(
            &serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("amend deserialization failed: {}", e))
    }
}

impl Default for VaultConf {
    fn default() -> Self {
        Self::new()
    }
}
