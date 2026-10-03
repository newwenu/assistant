// 参考: ChatGPT/src-tauri/src/core/conf.rs — AppConf 结构体 + load/save 模式

use log::error;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
};

use super::constant::APP_IDENTIFIER;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConf {
    pub theme: String,
    pub language: String,
    pub hotkey: String,
    pub stay_on_top: bool,
}

impl AppConf {
    pub fn new() -> Self {
        Self {
            theme: "system".to_string(),
            language: "zh-CN".to_string(),
            hotkey: "CommandOrControl+K".to_string(),
            stay_on_top: false,
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

        match serde_json::from_str(&contents) {
            Ok(config) => Ok(config),
            Err(e) => {
                error!("[AppConf::load] parse error: {}, falling back to defaults", e);
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