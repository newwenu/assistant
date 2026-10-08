use serde::{Deserialize, Serialize};
use std::path::Path;

use std::collections::HashMap;

use super::json_config::{get_conf_dir, load_json_config_with_migrate, save_json_config};

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
}

impl Default for AppConf {
    fn default() -> Self {
        Self::new()
    }
}
