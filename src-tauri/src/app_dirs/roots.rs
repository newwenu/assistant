use std::fmt;
use std::path::{Path, PathBuf};

use assistant_config::constant::APP_IDENTIFIER;

use super::dir_name;
use super::registry::{self, RootScope};

/// 基根解析失败：不做任何回退（不落相对路径、不落当前目录、不落默认值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirError {
    DataDirUnavailable,
    LocalDataDirUnavailable,
}

impl fmt::Display for DirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataDirUnavailable => write!(f, "无法解析应用数据根目录"),
            Self::LocalDataDirUnavailable => write!(f, "无法解析应用本地数据根目录"),
        }
    }
}

impl std::error::Error for DirError {}

/// 受管目录的唯一拼串点：基根与第一层目录名只在这里拼接。
///
/// 只在应用层使用；`crates/` 下的库只接收调用方传入的根路径。
/// 可选能力不在此列：它从 `data()` 取根，目录名自持于自己的 `cmd/<feature>/` 模块，
/// 从而保证关闭特性后这些路径知识不进入编译产物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    data: PathBuf,
    local: PathBuf,
}

impl AppDirs {
    /// 解析基根：`dirs::data_dir()` / `dirs::data_local_dir()` + `APP_IDENTIFIER`。
    ///
    /// 幂等、无副作用，可在 Tauri 初始化之前调用（日志插件构建期、崩溃安装期）。
    pub fn resolve() -> Result<Self, DirError> {
        let data = dirs::data_dir().ok_or(DirError::DataDirUnavailable)?;
        let local = dirs::data_local_dir().ok_or(DirError::LocalDataDirUnavailable)?;
        Ok(Self::with_bases(
            data.join(APP_IDENTIFIER),
            local.join(APP_IDENTIFIER),
        ))
    }

    /// 测试注入：直接给出两个已带标识符的基根。
    pub fn with_bases(data: PathBuf, local: PathBuf) -> Self {
        Self { data, local }
    }

    pub fn data(&self) -> &Path {
        &self.data
    }

    pub fn local(&self) -> &Path {
        &self.local
    }

    pub fn database(&self) -> PathBuf {
        self.path_for(dir_name::DATABASE)
    }

    pub fn attachments(&self) -> PathBuf {
        self.path_for(dir_name::ATTACHMENTS)
    }

    pub fn tool_results(&self) -> PathBuf {
        self.path_for(dir_name::TOOL_RESULTS)
    }

    /// 行索引缓存根（`super_read` 的 `index_root`，I58 §3.5）。
    /// 与 `tool_results()` 同根：都是可重建的工具产物，删掉只会导致重建。
    pub fn indexes(&self) -> PathBuf {
        self.path_for(dir_name::TOOL_INDEX)
    }

    pub fn mascot_assets(&self) -> PathBuf {
        self.path_for(dir_name::MASCOT_ASSETS)
    }

    /// 诊断产物落在 local 根：不进漫游、不进持久引用。
    pub fn logs(&self) -> PathBuf {
        self.path_for(dir_name::LOGS)
    }

    /// 与 `logs()` 同根，诊断数据集中在一处。
    pub fn crashes(&self) -> PathBuf {
        self.path_for(dir_name::CRASHES)
    }

    /// 受管根的唯一拼串点：段序列与基根来自登记表（V19 / G4）。
    ///
    /// 首段在登记表中唯一；`E2` 与 `E1` 共享 `attachments`，`find_by_first_segment`
    /// 返回先登记的 `E1`，因此 `attachments()` 只到首层。
    fn path_for(&self, first_segment: &str) -> PathBuf {
        let root = registry::find_by_first_segment(first_segment)
            .unwrap_or_else(|| panic!("managed root `{first_segment}` missing from registry"));
        let base = match root.scope {
            RootScope::Data => &self.data,
            RootScope::Local => &self.local,
        };
        let mut path = base.clone();
        for segment in root.segments {
            path.push(segment);
        }
        path
    }

    /// asset 协议作用域：`["$APPDATA/mascot-assets/**", "$APPDATA/attachments/*"]`。
    ///
    /// 唯一用途是与 `tauri.conf.json` 的 `assetProtocol.scope.allow` 做相等断言。
    /// `attachments` 用 `*` 而非 `**`：`derived/` 侧车只供后端读取，不经 asset 协议暴露。
    pub fn asset_scopes() -> Vec<String> {
        vec![
            format!("$APPDATA/{}/**", dir_name::MASCOT_ASSETS),
            format!("$APPDATA/{}/*", dir_name::ATTACHMENTS),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn injected() -> AppDirs {
        AppDirs::with_bases(PathBuf::from("/data/root"), PathBuf::from("/local/root"))
    }

    #[test]
    fn maps_every_managed_entry() {
        let dirs = injected();
        assert_eq!(dirs.data(), Path::new("/data/root"));
        assert_eq!(dirs.local(), Path::new("/local/root"));
        assert_eq!(dirs.database(), PathBuf::from("/data/root/assistant.db"));
        assert_eq!(dirs.attachments(), PathBuf::from("/data/root/attachments"));
        assert_eq!(
            dirs.tool_results(),
            PathBuf::from("/data/root/tool-results")
        );
        assert_eq!(dirs.indexes(), PathBuf::from("/data/root/tool-index"));
        assert_eq!(
            dirs.mascot_assets(),
            PathBuf::from("/data/root/mascot-assets")
        );
    }

    #[test]
    fn diagnostics_live_under_local_root() {
        let dirs = injected();
        assert_eq!(dirs.logs(), PathBuf::from("/local/root/logs"));
        assert_eq!(dirs.crashes(), PathBuf::from("/local/root/crashes"));
        assert_ne!(dirs.logs(), dirs.data().join(dir_name::LOGS));
        assert_ne!(dirs.crashes(), dirs.data().join(dir_name::CRASHES));
    }

    #[test]
    fn asset_scopes_cover_previews_but_not_derived() {
        assert_eq!(
            AppDirs::asset_scopes(),
            vec![
                "$APPDATA/mascot-assets/**".to_string(),
                "$APPDATA/attachments/*".to_string(),
            ]
        );
    }

    #[test]
    fn resolve_is_idempotent() {
        let first = AppDirs::resolve().expect("system data dirs are resolvable in tests");
        let second = AppDirs::resolve().expect("system data dirs are resolvable in tests");
        assert_eq!(first, second);
        assert!(first.data().ends_with(APP_IDENTIFIER));
        assert!(first.local().ends_with(APP_IDENTIFIER));
    }
}
