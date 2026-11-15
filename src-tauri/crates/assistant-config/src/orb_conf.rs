use serde::{Deserialize, Serialize};

use super::conf_group::{ConfGroup, LoadPolicy};

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
}

impl ConfGroup for OrbConf {
    const GROUP: &'static str = "orb";
    const LOG_TAG: &'static str = "OrbConf";
    const FILE_NAME: &'static str = "orb-config.json";
    const LOAD_POLICY: LoadPolicy = LoadPolicy::Lenient;

    fn validate(&self) -> Result<(), String> {
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
}

crate::conf_group_shims!(OrbConf);

impl Default for OrbConf {
    fn default() -> Self {
        Self::new()
    }
}
