// 参考: ChatGPT/src-tauri/src/core/conf.rs — AppConf 结构体 + load/save 模式

use log::error;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
};

use std::collections::HashMap;

use super::atomic_write::{atomic_write, recover_temp};
use super::constant::APP_IDENTIFIER;

#[derive(Serialize, Deserialize, Debug, Clone)]
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
        }
    }

    pub fn get_conf_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Cannot determine config directory")?
            .join(APP_IDENTIFIER);
        Ok(config_dir)
    }

    pub fn get_conf_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::get_conf_dir()?.join("config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::get_conf_path()?;

        if let Some(tmp_bytes) = recover_temp(&path) {
            if let Ok(config) = serde_json::from_slice::<Self>(&tmp_bytes) {
                let migrated = Self::migrate(config);
                migrated.save()?;
                return Ok(migrated);
            }
            error!("[AppConf::load] temp file invalid, discarding");
        }

        if !path.exists() {
            let config = Self::new();
            config.save()?;
            return Ok(config);
        }

        let mut file = File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        match serde_json::from_str::<Self>(&contents) {
            Ok(config) => {
                let migrated = Self::migrate(config);
                if contents != serde_json::to_string_pretty(&migrated)? {
                    migrated.save()?;
                }
                Ok(migrated)
            }
            Err(e) => {
                error!(
                    "[AppConf::load] parse error: {}, falling back to defaults",
                    e
                );
                let default = Self::new();
                default.save()?;
                Ok(default)
            }
        }
    }

    fn migrate(mut config: Self) -> Self {
        if config.keybindings.is_empty() && !config.hotkey.is_empty() {
            config
                .keybindings
                .insert("toggle_orb".to_string(), config.hotkey.clone());
        }
        if config.keybindings.is_empty() {
            let defaults = Self::new();
            config.keybindings = defaults.keybindings;
        }
        for binding in config.keybindings.values_mut() {
            *binding = binding.replace("Super", "Meta");
        }
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_conf_path()?;
        let json = serde_json::to_string_pretty(self)?;
        atomic_write(&path, json.as_bytes())?;
        Ok(())
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
}

impl Default for AppConf {
    fn default() -> Self {
        Self::new()
    }
}