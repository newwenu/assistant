// 受管目录登记表：13 条资源监视条目（E1–E13）的唯一来源。
//
// 本模块只描述「哪个基根、哪一段、归哪条条目、由谁提供、是否参与对账」，
// 不做路径拼接（拼接在 `roots.rs`）、不读配置、不访问磁盘。
// 新增受管目录 = 加一个 `dir_name` 常量且加一条登记；只做前者由 V19 单测直接判红。

use super::dir_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootScope {
    Data,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCoverage {
    /// 单文件（如 `assistant.db`、`security/security.db`）。
    SingleFile,
    /// 只统计该目录的第一层文件，不含子目录（如 `attachments`、`tool-index`）。
    SelfFilesOnly,
    /// 递归整棵子树（如 `attachments/derived`、`security/backups`）。
    RecursiveTree,
}

/// 可选能力的唯一槽位名（G17 / G18）：能力名只作路由 token，不进 DTO。
///
/// 它**不是**实现类型名、**不是** feature 名：换实现 / 改类型名 / 特性退役时
/// 登记表与 provider 两侧都不用动。全仓该字面量只允许出现在本常量的定义与引用（V24）。
pub const SECURITY_OBSERVABILITY: &str = "security-observability";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootMount {
    /// 无条件能力：两种构建都挂载。
    Always,
    /// 可选能力：关闭特性后该段可能仍在盘上，对账归 `optionalNotMounted`。
    ///
    /// 载荷是能力名（见 [`SECURITY_OBSERVABILITY`]），不是特性名：对账与采样都按
    /// 「该能力的 provider 是否注册」判定，core 不需要知道是哪个 crate 实现的。
    Feature(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedRoot {
    /// `E1`..`E13`。
    pub entry: &'static str,
    /// 稳定身份（不是路径；等于 `segments` 用 `/` 连接）。
    pub key: &'static str,
    pub scope: RootScope,
    /// 段序列；首段即对账单位。
    pub segments: &'static [&'static str],
    pub coverage: RootCoverage,
    pub owner: &'static str,
    pub mount: RootMount,
    /// 该 scope 是否参与余量对账；本期 13 条全 `true`（OQ11）。
    pub inventory: bool,
}

pub const MANAGED_ROOTS: &[ManagedRoot] = &[
    ManagedRoot {
        entry: "E1",
        key: "attachments",
        scope: RootScope::Data,
        segments: &[dir_name::ATTACHMENTS],
        coverage: RootCoverage::SelfFilesOnly,
        owner: "AttachmentStore",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E2",
        key: "attachments/derived",
        scope: RootScope::Data,
        segments: &[dir_name::ATTACHMENTS, "derived"],
        coverage: RootCoverage::RecursiveTree,
        owner: "AttachmentStore",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E3",
        key: "tool-results",
        scope: RootScope::Data,
        segments: &[dir_name::TOOL_RESULTS],
        coverage: RootCoverage::RecursiveTree,
        owner: "FileToolResultStorage",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E4",
        key: "tool-index",
        scope: RootScope::Data,
        segments: &[dir_name::TOOL_INDEX],
        coverage: RootCoverage::SelfFilesOnly,
        owner: "SuperIndex",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E5",
        key: "mascot-assets",
        scope: RootScope::Data,
        segments: &[dir_name::MASCOT_ASSETS],
        coverage: RootCoverage::RecursiveTree,
        owner: "MascotAssets",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E6",
        key: "assistant.db",
        scope: RootScope::Data,
        segments: &[dir_name::DATABASE],
        coverage: RootCoverage::SingleFile,
        owner: "DbState",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E7",
        key: "security/security.db",
        scope: RootScope::Data,
        segments: &["security", "security.db"],
        coverage: RootCoverage::SingleFile,
        owner: "SecurityDb",
        mount: RootMount::Feature(SECURITY_OBSERVABILITY),
        inventory: true,
    },
    ManagedRoot {
        entry: "E8",
        key: "security/backups",
        scope: RootScope::Data,
        segments: &["security", "backups"],
        coverage: RootCoverage::RecursiveTree,
        owner: "SecurityDb",
        mount: RootMount::Feature(SECURITY_OBSERVABILITY),
        inventory: true,
    },
    ManagedRoot {
        entry: "E9",
        key: "security/vault",
        scope: RootScope::Data,
        segments: &["security", "vault"],
        coverage: RootCoverage::RecursiveTree,
        owner: "SecurityRepo",
        mount: RootMount::Feature(SECURITY_OBSERVABILITY),
        inventory: true,
    },
    ManagedRoot {
        entry: "E10",
        key: "security/recovery",
        scope: RootScope::Data,
        segments: &["security", "recovery"],
        coverage: RootCoverage::RecursiveTree,
        owner: "SecurityRecovery",
        mount: RootMount::Feature(SECURITY_OBSERVABILITY),
        inventory: true,
    },
    ManagedRoot {
        entry: "E11",
        key: "security/audit",
        scope: RootScope::Data,
        segments: &["security", "audit"],
        coverage: RootCoverage::RecursiveTree,
        owner: "SecurityDb",
        mount: RootMount::Feature(SECURITY_OBSERVABILITY),
        inventory: true,
    },
    ManagedRoot {
        entry: "E12",
        key: "logs",
        scope: RootScope::Local,
        segments: &[dir_name::LOGS],
        coverage: RootCoverage::RecursiveTree,
        owner: "tauri-plugin-log",
        mount: RootMount::Always,
        inventory: true,
    },
    ManagedRoot {
        entry: "E13",
        key: "crashes",
        scope: RootScope::Local,
        segments: &[dir_name::CRASHES],
        coverage: RootCoverage::RecursiveTree,
        owner: "assistant-crash",
        mount: RootMount::Always,
        inventory: true,
    },
];

pub const IGNORED_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// 可选功能分区的首段磁盘事实（G19(c)）：core 保留的**唯一**可选功能知识。
///
/// 只声明「该首段可能出现在盘上、归某个可选分区」，**不写条目、不写策略、不写实现**：
/// 条目清单、资格判据、回收入口全在分区自述；分区是否挂载只看
/// `cmd::storage::section::SectionRegistry` 里有没有对应分区，不看 `#[cfg]`。
pub const OPTIONAL_SEGMENTS: &[&str] = &["security"];

const EXPECTED_ENTRY_COUNT: usize = 13;

/// 按 `key` 取登记项；未登记返回 `None`（管理器不得为未登记 key 产出条目，G4）。
pub fn find(key: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS.iter().find(|root| root.key == key)
}

/// 按条目 id（`E1`..`E13`）取登记项：条目 id 是对外契约，槽位由登记表现算。
pub fn find_by_entry(entry: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS.iter().find(|root| root.entry == entry)
}

/// 登记表声明的可选能力名集合（去重、保持登记顺序）。
pub fn feature_capabilities() -> Vec<&'static str> {
    let mut capabilities = Vec::new();
    for root in MANAGED_ROOTS {
        if let RootMount::Feature(capability) = root.mount {
            if !capabilities.contains(&capability) {
                capabilities.push(capability);
            }
        }
    }
    capabilities
}

/// 按首段取登记项；首段唯一（`E2` 与 `E1` 共享 `attachments`，返回先登记的 `E1`）。
pub fn find_by_first_segment(segment: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS
        .iter()
        .find(|root| root.segments.first() == Some(&segment))
}

/// 登记表自检（V19 三条）：纯函数、接受切片，测试可传缺项负例。
///
/// 1. 13 条 `entry` 集合完整且 `key == segments.join("/")`，`key` 不重复；
/// 2. `dir_name` 每个常量都必须有对应登记（首段命中）；
/// 3. 无条件挂载的登记首段必须来自 `dir_name` 常量；可选能力（`Feature`）豁免
///    （其目录名自持于能力模块，如 `security/`）。
pub fn validate_registry(roots: &[ManagedRoot], dir_names: &[&str]) -> Result<(), String> {
    if roots.len() != EXPECTED_ENTRY_COUNT {
        return Err(format!(
            "registered roots count {} != {}",
            roots.len(),
            EXPECTED_ENTRY_COUNT
        ));
    }

    for index in 1..=EXPECTED_ENTRY_COUNT {
        let entry = format!("E{index}");
        if !roots.iter().any(|root| root.entry == entry) {
            return Err(format!("missing entry {entry}"));
        }
    }

    let mut keys = std::collections::HashSet::new();
    for root in roots {
        if root.segments.is_empty() {
            return Err(format!("entry {} has empty segments", root.entry));
        }
        let joined = root.segments.join("/");
        if joined != root.key {
            return Err(format!(
                "entry {} key `{}` != segments `{}`",
                root.entry, root.key, joined
            ));
        }
        if !keys.insert(root.key) {
            return Err(format!("duplicate key {}", root.key));
        }
    }

    let first_segments: std::collections::HashSet<&str> = roots
        .iter()
        .filter_map(|root| root.segments.first().copied())
        .collect();
    for name in dir_names {
        if !first_segments.contains(name) {
            return Err(format!("dir_name `{name}` has no registered root"));
        }
    }

    for root in roots {
        let first = root.segments[0];
        if let RootMount::Feature(capability) = root.mount {
            if capability.is_empty() {
                return Err(format!("entry {} has empty capability", root.entry));
            }
            continue;
        }
        if !dir_names.contains(&first) {
            return Err(format!(
                "entry {} first segment `{}` has no dir_name constant",
                root.entry, first
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::AppDirs;
    use std::path::{Path, PathBuf};

    fn injected() -> AppDirs {
        AppDirs::with_bases(PathBuf::from("/data/root"), PathBuf::from("/local/root"))
    }

    #[test]
    fn registry_is_consistent() {
        validate_registry(MANAGED_ROOTS, dir_name::ALL).expect("registry must be consistent");
    }

    #[test]
    fn missing_registration_is_rejected() {
        let partial = &MANAGED_ROOTS[..MANAGED_ROOTS.len() - 1];
        assert!(validate_registry(partial, dir_name::ALL).is_err());
    }

    #[test]
    fn missing_dir_name_constant_is_rejected() {
        let without_logs: Vec<&str> = dir_name::ALL
            .iter()
            .copied()
            .filter(|name| *name != dir_name::LOGS)
            .collect();
        assert!(validate_registry(MANAGED_ROOTS, &without_logs).is_err());
    }

    #[test]
    fn duplicate_key_is_rejected() {
        let mut roots: Vec<ManagedRoot> = MANAGED_ROOTS.to_vec();
        roots[1].key = roots[0].key;
        roots[1].segments = roots[0].segments;
        assert!(validate_registry(&roots, dir_name::ALL).is_err());
    }

    #[test]
    fn segments_first_matches_roots_getters() {
        let dirs = injected();
        let expected: &[(&str, &Path)] = &[
            ("attachments", Path::new("/data/root/attachments")),
            ("tool-results", Path::new("/data/root/tool-results")),
            ("tool-index", Path::new("/data/root/tool-index")),
            ("mascot-assets", Path::new("/data/root/mascot-assets")),
            ("assistant.db", Path::new("/data/root/assistant.db")),
            ("logs", Path::new("/local/root/logs")),
            ("crashes", Path::new("/local/root/crashes")),
        ];

        for (key, path) in expected.iter().copied() {
            let root = find(key).expect("registered key must exist");
            let actual = match key {
                "attachments" => dirs.attachments(),
                "tool-results" => dirs.tool_results(),
                "tool-index" => dirs.indexes(),
                "mascot-assets" => dirs.mascot_assets(),
                "assistant.db" => dirs.database(),
                "logs" => dirs.logs(),
                "crashes" => dirs.crashes(),
                other => panic!("unexpected key {other}"),
            };
            assert_eq!(actual, path);
            assert_eq!(
                root.segments.first().copied(),
                Some(root.key.split('/').next().unwrap())
            );
        }
    }

    /// V22：`owner` 只是展示标签，不得冒充路由 token（能力名 / feature 名 / crate 类型名）。
    #[test]
    fn owner_labels_are_not_routing_tokens() {
        const FEATURE_NAMES: &[&str] = &["vault"];
        for root in MANAGED_ROOTS {
            assert!(
                !root.owner.contains("::"),
                "{} 的 owner `{}` 不得含路径分隔符",
                root.entry,
                root.owner
            );
            assert!(
                !root.owner.contains("assistant_vault"),
                "{} 的 owner `{}` 不得是可选项 crate 类型名",
                root.entry,
                root.owner
            );
            assert!(
                !root.owner.contains("VaultStorageProbe"),
                "{} 的 owner `{}` 不得是实现类型名",
                root.entry,
                root.owner
            );
            assert!(
                !FEATURE_NAMES.contains(&root.owner),
                "{} 的 owner `{}` 不得等于 feature 名",
                root.entry,
                root.owner
            );
            assert_ne!(root.owner, SECURITY_OBSERVABILITY);
        }
    }

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// V24：能力名字面量只能出现在本常量的定义处（其余位置一律引用常量）。
    #[test]
    fn capability_literal_only_lives_in_its_constant() {
        assert!(!SECURITY_OBSERVABILITY.contains("::"), "不得是类型路径");
        assert!(
            !SECURITY_OBSERVABILITY.chars().any(char::is_uppercase),
            "不得是类型名"
        );
        assert_ne!(SECURITY_OBSERVABILITY, "vault", "不得是 feature 名");
        assert_ne!(SECURITY_OBSERVABILITY, "VaultProbe", "不得是类型名");

        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        for sub in ["src", "tests", "crates"] {
            collect_rs_files(&manifest.join(sub), &mut files);
        }
        let offenders: Vec<String> = files
            .iter()
            .filter(|path| !path.ends_with("app_dirs/registry.rs"))
            .filter(|path| {
                std::fs::read_to_string(path)
                    .map(|text| text.contains(SECURITY_OBSERVABILITY))
                    .unwrap_or(false)
            })
            .map(|path| path.display().to_string())
            .collect();
        assert!(
            offenders.is_empty(),
            "能力名字面量只能出现在 `const SECURITY_OBSERVABILITY` 定义处，违规文件: {offenders:?}"
        );
    }
}
