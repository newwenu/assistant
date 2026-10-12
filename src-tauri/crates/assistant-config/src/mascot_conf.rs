use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::json_config::{get_conf_dir, load_json_config, save_json_config};

fn default_renderer() -> String {
    "rive".to_string()
}

fn default_true() -> bool {
    true
}

fn default_gif_border_radius() -> u32 {
    50
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MascotConf {
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(default)]
    pub primary_color: Option<String>,
    #[serde(default)]
    pub secondary_color: Option<String>,
    #[serde(default)]
    pub rive_idle_pose_rotation: bool,
    #[serde(default)]
    pub rive_src: Option<String>,
    #[serde(default = "default_true")]
    pub svg_animated: bool,
    #[serde(default)]
    pub gif_src: Option<String>,
    #[serde(default = "default_gif_border_radius")]
    pub gif_border_radius: u32,
}

impl MascotConf {
    pub fn new() -> Self {
        Self {
            renderer: default_renderer(),
            primary_color: None,
            secondary_color: None,
            rive_idle_pose_rotation: false,
            rive_src: None,
            svg_animated: true,
            gif_src: None,
            gif_border_radius: default_gif_border_radius(),
        }
    }

    pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        get_conf_dir()
    }

    pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(get_conf_dir()?.join("mascot-config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        load_json_config(path, Self::new, "MascotConf")
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    pub fn validate(&self) -> Result<(), String> {
        let valid_renderers = ["rive", "svg", "gif", "disabled"];
        if !valid_renderers.contains(&self.renderer.as_str()) {
            return Err(format!(
                "renderer must be one of {:?}, got {}",
                valid_renderers, self.renderer
            ));
        }
        if let Some(ref c) = self.primary_color {
            if !c.starts_with('#') || c.len() != 7 {
                return Err(format!("primary_color must be #RRGGBB, got {}", c));
            }
        }
        if let Some(ref c) = self.secondary_color {
            if !c.starts_with('#') || c.len() != 7 {
                return Err(format!("secondary_color must be #RRGGBB, got {}", c));
            }
        }
        if let Some(ref s) = self.gif_src {
            if s.is_empty() {
                return Err("gif_src must not be empty string".to_string());
            }
        }
        if let Some(ref s) = self.rive_src {
            if s.is_empty() {
                return Err("rive_src must not be empty string".to_string());
            }
        }
        if self.gif_border_radius > 50 {
            return Err(format!(
                "gif_border_radius must be 0..=50, got {}",
                self.gif_border_radius
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
        serde_json::from_str::<MascotConf>(
            &serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("amend deserialization failed: {}", e))
    }
}

impl Default for MascotConf {
    fn default() -> Self {
        Self::new()
    }
}