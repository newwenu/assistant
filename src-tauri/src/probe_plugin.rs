// 参考: docs/store/I24-feature-extension-spec.md §6 — 探针插件：全部 cfg 收敛在本文件内部

use tauri::plugin::TauriPlugin;

#[cfg(feature = "probe")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "probe")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeConf {
    pub probe_flag: bool,
    pub probe_text: String,
}

#[cfg(feature = "probe")]
impl ProbeConf {
    fn new() -> Self {
        Self {
            probe_flag: false,
            probe_text: String::new(),
        }
    }

    fn path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        Ok(assistant_config::json_config::get_conf_dir()?.join("probe-config.json"))
    }

    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        assistant_config::json_config::load_json_config(&Self::path()?, Self::new, "ProbeConf")
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        assistant_config::json_config::save_json_config(&Self::path()?, self)
    }

    fn amend(self, patch: serde_json::Value) -> Result<Self, String> {
        let val = serde_json::to_value(&self).map_err(|e| e.to_string())?;
        let mut config: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_value(val).map_err(|e| e.to_string())?;
        let new_json: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_value(patch).map_err(|e| e.to_string())?;
        for (key, value) in new_json {
            config.insert(key, value);
        }
        serde_json::from_str::<ProbeConf>(
            &serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("amend deserialization failed: {}", e))
    }
}

#[cfg(feature = "probe")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeConfDto {
    pub probe_flag: bool,
    pub probe_text: String,
}

#[cfg(feature = "probe")]
impl From<ProbeConf> for ProbeConfDto {
    fn from(conf: ProbeConf) -> Self {
        Self {
            probe_flag: conf.probe_flag,
            probe_text: conf.probe_text,
        }
    }
}

#[cfg(feature = "probe")]
#[tauri::command]
pub fn get_probe_conf() -> Result<ProbeConfDto, String> {
    ProbeConf::load()
        .map(ProbeConfDto::from)
        .map_err(|e| e.to_string())
}

#[cfg(feature = "probe")]
#[tauri::command]
pub fn probe_echo(payload: String) -> Result<String, String> {
    Ok(format!("probe:{}", payload))
}

#[cfg(feature = "probe")]
pub fn apply_conf(_app: &tauri::AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let conf = ProbeConf::load().map_err(|e| e.to_string())?;
    let merged = conf.amend(patch)?;
    merged.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(feature = "probe")]
pub fn init() -> TauriPlugin<tauri::Wry> {
    crate::cmd::register_conf_applier("probe", apply_conf as crate::cmd::ConfApplier);
    tauri::plugin::Builder::<tauri::Wry>::new("probe")
        .invoke_handler(tauri::generate_handler![get_probe_conf, probe_echo])
        .build()
}

#[cfg(not(feature = "probe"))]
pub fn init() -> TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::<tauri::Wry>::new("probe").build()
}
