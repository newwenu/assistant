// 参考: BongoCat/crates/bongocat-config/src/config_schema/overlay.rs — OverlayConfig 字段设计

use log::error;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
};

use super::atomic_write::{atomic_write, recover_temp};
use super::constant::APP_IDENTIFIER;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrbConf {
    pub click_through: bool,
    pub always_on_top: bool,
    pub scale_percent: u16,
    pub opacity_percent: u8,
    pub hide_on_pointer_hover: bool,
    pub hover_delay_ms: u64,
    pub corner_radius_percent: u8,
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
            corner_radius_percent: 50,
            position_x: None,
            position_y: None,
        }
    }

    pub fn get_conf_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Cannot determine config directory")?
            .join(APP_IDENTIFIER);
        Ok(config_dir)
    }

    pub fn get_conf_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::get_conf_dir()?.join("orb-config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::get_conf_path()?;

        if let Some(tmp_bytes) = recover_temp(&path) {
            if let Ok(config) = serde_json::from_slice::<Self>(&tmp_bytes) {
                config.save()?;
                return Ok(config);
            }
            error!("[OrbConf::load] temp file invalid, discarding");
        }

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
                error!(
                    "[OrbConf::load] parse error: {}, falling back to defaults",
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
        let json = serde_json::to_string_pretty(self)?;
        atomic_write(&path, json.as_bytes())?;
        Ok(())
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
        if self.corner_radius_percent > 100 {
            return Err(format!(
                "corner_radius_percent must be 0-100, got {}",
                self.corner_radius_percent
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
}

impl Default for OrbConf {
    fn default() -> Self {
        Self::new()
    }
}