use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::json_config::{
    amend_json_config, get_conf_dir as conf_dir_root, load_conf, save_json_config,
};

/// 配置加载策略（三态）。选择在编译期由 `ConfGroup::LOAD_POLICY` 决定，
/// 运行时不再分派。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPolicy {
    /// 损坏即报错，且**不写回**用户配置文件。
    Strict,
    /// 损坏回落默认值并写回。
    Lenient,
    /// 损坏回落默认值并写回；解析成功时执行迁移，迁移结果有变化才写回。
    LenientWithMigrate,
}

/// 一个配置组的完整生命周期契约。
///
/// 只声明「策略」与「特性专属逻辑」，目录解析、临时文件恢复、宽松/严格回退、
/// 原子写入这些机制全部由 [`load_conf`] / [`save_json_config`] 承担。
pub trait ConfGroup: Serialize + DeserializeOwned + Clone + PartialEq + Default + Sized {
    /// 分组短名（门禁与日志用，如 `app`）。
    const GROUP: &'static str;
    /// 日志前缀，与迁移前 `[AppConf]` 等形态保持一致。
    const LOG_TAG: &'static str;
    /// 磁盘文件名，与迁移前保持一致。
    const FILE_NAME: &'static str;
    /// 加载策略。
    const LOAD_POLICY: LoadPolicy;

    /// 旧配置接管：主文件不存在时从旧位置读取；`None` 表示无旧配置可用。
    fn adopt_legacy(_path: &Path) -> Option<Self> {
        None
    }

    /// 字段迁移。仅 `LoadPolicy::LenientWithMigrate` 会调用。
    fn migrate(self) -> Self {
        self
    }

    /// 字段级校验。`Strict` 在加载期调用，[`apply_patch`] 在写回前调用。
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    fn get_conf_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        conf_dir_root()
    }

    fn get_conf_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(Self::get_conf_dir()?.join(Self::FILE_NAME))
    }

    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::get_conf_path()?)
    }

    fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        load_conf::<Self>(path)
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::get_conf_path()?)
    }

    fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        save_json_config(path, self)
    }

    fn amend(self, patch: Value) -> Result<Self, String> {
        amend_json_config(&self, patch)
    }
}

/// 读-改-校验-写的唯一入口，返回 `(旧值, 新值)` 供调用方计算副作用差异。
///
/// 写回发生在返回之前，因此调用方拿到的 `next` 一定已落盘。
pub fn apply_patch<C: ConfGroup>(patch: Value) -> Result<(C, C), String> {
    let prev = C::load().map_err(|e| e.to_string())?;
    let next = prev.clone().amend(patch)?;
    next.validate()?;
    next.save().map_err(|e| e.to_string())?;
    Ok((prev, next))
}

/// 为配置组生成与迁移前同名的固有方法，保持既有调用点零改动。
///
/// 委托目标一律写全限定形式 `<$t as ConfGroup>::method`，避免固有方法与
/// trait 方法同名时在 trait 实现体内形成自递归。
#[macro_export]
macro_rules! conf_group_shims {
    ($t:ty) => {
        impl $t {
            pub fn get_conf_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::get_conf_dir()
            }

            pub fn get_conf_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::get_conf_path()
            }

            pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::load()
            }

            pub fn load_from(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::load_from(path)
            }

            pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::save(self)
            }

            pub fn save_to(
                &self,
                path: &std::path::Path,
            ) -> Result<(), Box<dyn std::error::Error>> {
                <$t as $crate::ConfGroup>::save_to(self, path)
            }

            pub fn validate(&self) -> Result<(), String> {
                <$t as $crate::ConfGroup>::validate(self)
            }

            pub fn amend(self, patch: serde_json::Value) -> Result<Self, String> {
                <$t as $crate::ConfGroup>::amend(self, patch)
            }
        }
    };
}
