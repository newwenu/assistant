// 参考: ChatGPT/src-tauri/src/core/conf.rs — AppConf 结构体 + load/save 模式

use log::error;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
};

use std::collections::HashMap;

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
        keybindings.insert("toggle_orb".to_string(), "Super+Shift+O".to_string());
        keybindings.insert("toggle_main".to_string(), "Super+Shift+M".to_string());
        keybindings.insert("new_chat".to_string(), "Super+Shift+N".to_string());

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

        if !path.exists() {
            let config = Self::new();
            config.save()?;
            return Ok(config);
        }

        let mut file = File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        match serde_json::from_str::<Self>(&contents) {
            Ok(mut config) => {
                if config.keybindings.is_empty() && !config.hotkey.is_empty() {
                    config
                        .keybindings
                        .insert("toggle_orb".to_string(), config.hotkey.clone());
                }
                if config.keybindings.is_empty() {
                    let defaults = Self::new();
                    config.keybindings = defaults.keybindings;
                }
                Ok(config)
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

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_conf_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;

        Ok(())
    }
}

impl Default for AppConf {
    fn default() -> Self {
        Self::new()
    }
}