// 参考: docs/store/I75-startup-layering-spec.md §9 #2 — 启动期唯一配置读盘点

use std::path::Path;

use assistant_config::{app_conf::AppConf, orb_conf::OrbConf, ConfGroup};

/// 启动期配置快照。一次定位目录、逐组 `load_from(显式路径)`，不再各模块自行 `load()`。
#[derive(Clone, Default)]
pub struct BootConf {
    pub app: AppConf,
    pub orb: OrbConf,
}

impl BootConf {
    /// 永不返回 `Err`：任一组的读取失败都回落该组默认值，并把原因交给调用点记降档。
    pub fn load_all() -> (Self, Vec<String>) {
        let mut failures = Vec::new();
        let dir = match <AppConf as ConfGroup>::get_conf_dir() {
            Ok(dir) => dir,
            Err(e) => {
                failures.push(format!("conf dir: {e}"));
                return (Self::default(), failures);
            }
        };

        let app = match AppConf::load_from(&dir.join(<AppConf as ConfGroup>::FILE_NAME)) {
            Ok(conf) => conf,
            Err(e) => {
                failures.push(format!("app: {e}"));
                AppConf::default()
            }
        };
        let orb = match OrbConf::load_from(&dir.join(<OrbConf as ConfGroup>::FILE_NAME)) {
            Ok(conf) => conf,
            Err(e) => {
                failures.push(format!("orb: {e}"));
                OrbConf::default()
            }
        };

        (Self { app, orb }, failures)
    }

    pub fn app_path() -> Option<std::path::PathBuf> {
        conf_path::<AppConf>()
    }

    pub fn orb_path() -> Option<std::path::PathBuf> {
        conf_path::<OrbConf>()
    }
}

fn conf_path<T: ConfGroup>() -> Option<std::path::PathBuf> {
    <T as ConfGroup>::get_conf_dir()
        .ok()
        .map(|dir| join::<T>(&dir))
}

fn join<T: ConfGroup>(dir: &Path) -> std::path::PathBuf {
    dir.join(<T as ConfGroup>::FILE_NAME)
}
