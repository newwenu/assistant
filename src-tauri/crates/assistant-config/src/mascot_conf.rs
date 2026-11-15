use serde::{Deserialize, Serialize};

use super::conf_group::{ConfGroup, LoadPolicy};

fn default_renderer() -> String {
    "svg".to_string()
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
}

impl ConfGroup for MascotConf {
    const GROUP: &'static str = "mascot";
    const LOG_TAG: &'static str = "MascotConf";
    const FILE_NAME: &'static str = "mascot-config.json";
    const LOAD_POLICY: LoadPolicy = LoadPolicy::Lenient;

    fn validate(&self) -> Result<(), String> {
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
}

crate::conf_group_shims!(MascotConf);

impl Default for MascotConf {
    fn default() -> Self {
        Self::new()
    }
}
