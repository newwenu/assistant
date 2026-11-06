use serde::{Deserialize, Serialize};
use std::path::Path;

use super::json_config::{amend_json_config, get_conf_dir, load_json_config, save_json_config};

fn default_idle_opacity() -> u8 {
    85
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrbConf {
    pub click_through: bool,
    pub always_on_top: bool,
    pub scale_percent: u16,
    pub opacity_percent: u8,
    pub hide_on_pointer_hover: bool,
    pub hover_delay_ms: u64,
    #[serde(default = "default_idle_opacity")]
    pub idle_opacity_percent: u8,
    #[serde(default)]
    pub position_x: Option<i32>,
    #[serde(default)]
    pub position_y: Option<i32>,
}

impl OrbConf {
    pub fn new() -> Self {
        Self {
            click_through: false,
            always_on_top: true,
            scale_percent: 100,
            opacity_percent: 100,
            hide_on_pointer_hover: false,
            hover_delay_ms: 0,
            idle_opacity_percent: 85,
            position_x: None,
            position_y: None,
        }
    }

    pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        get_conf_dir()
    }

    pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(get_conf_dir()?.join("orb-config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        load_json_config(path, Self::new, "OrbConf")
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.scale_percent < 10 || self.scale_percent > 300 {
            return Err(format!(
                "scale_percent must be 10-300, got {}",
                self.scale_percent
            ));
        }
        if self.opacity_percent < 10 || self.opacity_percent > 100 {
            return Err(format!(
                "opacity_percent must be 10-100, got {}",
                self.opacity_percent
            ));
        }
        if self.idle_opacity_percent < 10 || self.idle_opacity_percent > 100 {
            return Err(format!(
                "idle_opacity_percent must be 10-100, got {}",
                self.idle_opacity_percent
            ));
        }
        if self.hover_delay_ms > 5000 {
            return Err(format!(
                "hover_delay_ms must be 0-5000, got {}",
                self.hover_delay_ms
            ));
        }
        Ok(())
    }

    pub fn amend(self, patch: serde_json::Value) -> Result<Self, String> {
        amend_json_config(&self, patch)
    }
}

impl Default for OrbConf {
    fn default() -> Self {
        Self::new()
    }
}
