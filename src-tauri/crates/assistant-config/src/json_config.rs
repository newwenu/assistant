use log::error;
use serde::{de::DeserializeOwned, Serialize};
use std::{fs::File, io::Read, path::Path};

use super::atomic_write::{atomic_write, recover_temp};
use super::constant::APP_IDENTIFIER;

pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join(APP_IDENTIFIER);
    Ok(config_dir)
}

pub fn load_json_config<T, F>(
    path: &Path,
    default: F,
    tag: &str,
) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned + Serialize + PartialEq,
    F: FnOnce() -> T,
{
    if let Some(tmp_bytes) = recover_temp(path) {
        if let Ok(config) = serde_json::from_slice::<T>(&tmp_bytes) {
            save_json_config(path, &config)?;
            return Ok(config);
        }
        error!("[{}] temp file invalid, discarding", tag);
    }

    if !path.exists() {
        let config = default();
        save_json_config(path, &config)?;
        return Ok(config);
    }

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match serde_json::from_str::<T>(&contents) {
        Ok(config) => Ok(config),
        Err(e) => {
            error!("[{}] parse error: {}, falling back to defaults", tag, e);
            let fallback = default();
            save_json_config(path, &fallback)?;
            Ok(fallback)
        }
    }
}

pub fn load_json_config_with_migrate<T, F, M>(
    path: &Path,
    default: F,
    migrate: M,
    tag: &str,
) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned + Serialize + PartialEq,
    F: FnOnce() -> T,
    M: Fn(T) -> T,
{
    if let Some(tmp_bytes) = recover_temp(path) {
        if let Ok(config) = serde_json::from_slice::<T>(&tmp_bytes) {
            let migrated = migrate(config);
            save_json_config(path, &migrated)?;
            return Ok(migrated);
        }
        error!("[{}] temp file invalid, discarding", tag);
    }

    if !path.exists() {
        let config = default();
        save_json_config(path, &config)?;
        return Ok(config);
    }

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match serde_json::from_str::<T>(&contents) {
        Ok(config) => {
            let migrated = migrate(config);
            if contents != serde_json::to_string_pretty(&migrated)? {
                save_json_config(path, &migrated)?;
            }
            Ok(migrated)
        }
        Err(e) => {
            error!("[{}] parse error: {}, falling back to defaults", tag, e);
            let fallback = default();
            save_json_config(path, &fallback)?;
            Ok(fallback)
        }
    }
}

pub fn save_json_config<T: Serialize>(
    path: &Path,
    config: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(config)?;
    atomic_write(path, json.as_bytes())?;
    Ok(())
}
