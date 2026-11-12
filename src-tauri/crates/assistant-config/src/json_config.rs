use log::error;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

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

/// 严格加载：文件损坏时返回错误，且**不写回**用户配置文件。
///
/// 与 `load_json_config_with_migrate` 的宽松回退不同，本函数用于「配置损坏必须
/// 显式报错、不得静默覆盖用户数据」的场景（capability-gating-spec §2.2.3 / T-C12）。
pub fn load_json_config_strict<T>(path: &Path, tag: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned,
{
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    serde_json::from_str::<T>(&contents).map_err(|e| {
        error!(
            "[{}] parse error: {}, refusing to overwrite user config",
            tag, e
        );
        format!("[{}] config is corrupt: {}", tag, e).into()
    })
}

/// 把 patch 合并进当前配置，并拒绝任何 patch 里未被结构体字段消费的键。
///
/// 配置结构体故意不加 `deny_unknown_fields`（读文件必须向前兼容，旧版本读到新版本
/// 写出的配置不能加载失败），所以这里的严格性只能放在 `amend()` 这一侧：
/// 用 `serde_ignored` 在反序列化时收集被忽略的键路径，只要有就整体报错。
///
/// 不写字段白名单，避免与结构体字段清单形成第二份需要同步维护的副本；也避免
/// 从实例序列化结果推导白名单——那会随 `skip_serializing_if` 与 `Option` 状态漂移。
pub fn amend_json_config<T>(current: &T, patch: serde_json::Value) -> Result<T, String>
where
    T: Serialize + DeserializeOwned,
{
    let current_val = serde_json::to_value(current).map_err(|e| e.to_string())?;
    let mut merged: BTreeMap<String, serde_json::Value> =
        serde_json::from_value(current_val).map_err(|e| e.to_string())?;
    let patch_map: BTreeMap<String, serde_json::Value> =
        serde_json::from_value(patch).map_err(|e| format!("patch must be an object: {}", e))?;
    merged.extend(patch_map);

    let json = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    let mut de = serde_json::Deserializer::from_str(&json);
    let mut ignored: Vec<String> = Vec::new();
    let conf: T = serde_ignored::deserialize(&mut de, |path: serde_ignored::Path<'_>| {
        ignored.push(path.to_string())
    })
    .map_err(|e| format!("amend deserialization failed: {}", e))?;

    if ignored.is_empty() {
        return Ok(conf);
    }

    ignored.sort();
    ignored.dedup();
    Err(format!("unknown config key: {}", ignored.join(", ")))
}
