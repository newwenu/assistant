// 受管目录登记表：core 8 条资源监视条目（`E1`–`E6` / `E12` / `E13`）的唯一来源。
//
// 本模块只描述「哪个基根、哪一段、归哪条条目、由谁提供、是否参与对账」，
// 不做路径拼接（拼接在 `roots.rs`）、不读配置、不访问磁盘。
// 新增受管目录 = 加一个 `dir_name` 常量且加一条登记；只做前者由 V19 单测直接判红。
//
// 可选功能条目（`E7`–`E11`）**不在这里**：它们由功能 crate 自述（见
// `cmd::storage::section`），core 只保留首段事实 [`OPTIONAL_SEGMENTS`]（G16 / G19）。

use super::dir_name;
use crate::cmd::storage::section::RiskTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootScope {
    Data,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCoverage {
    /// 单文件（如 `assistant.db`）。
    SingleFile,
    /// 只统计该目录的第一层文件，不含子目录（如 `attachments`、`tool-index`）。
    SelfFilesOnly,
    /// 递归整棵子树（如 `attachments/derived`）。
    RecursiveTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedRoot {
    /// `E1`–`E6` / `E12` / `E13`。
    pub entry: &'static str,
    /// 稳定身份（不是路径；等于 `segments` 用 `/` 连接）。
    pub key: &'static str,
    pub scope: RootScope,
    /// 段序列；首段即对账单位。
    pub segments: &'static [&'static str],
    pub coverage: RootCoverage,
    pub owner: &'static str,
    /// 风险档（§3.6 硬约束 5–6）：core 只据此派生 `confirmation`，不进 wire。
    pub risk: RiskTier,
    /// 该 scope 是否参与余量对账；本期 8 条全 `true`（OQ11）。
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
        risk: RiskTier::High,
        inventory: true,
    },
    ManagedRoot {
        entry: "E2",
        key: "attachments/derived",
        scope: RootScope::Data,
        segments: &[dir_name::ATTACHMENTS, "derived"],
        coverage: RootCoverage::RecursiveTree,
        owner: "AttachmentStore",
        risk: RiskTier::Low,
        inventory: true,
    },
    ManagedRoot {
        entry: "E3",
        key: "tool-results",
        scope: RootScope::Data,
        segments: &[dir_name::TOOL_RESULTS],
        coverage: RootCoverage::RecursiveTree,
        owner: "FileToolResultStorage",
        risk: RiskTier::Medium,
        inventory: true,
    },
    ManagedRoot {
        entry: "E4",
        key: "tool-index",
        scope: RootScope::Data,
        segments: &[dir_name::TOOL_INDEX],
        coverage: RootCoverage::SelfFilesOnly,
        owner: "SuperIndex",
        risk: RiskTier::Low,
        inventory: true,
    },
    ManagedRoot {
        entry: "E5",
        key: "mascot-assets",
        scope: RootScope::Data,
        segments: &[dir_name::MASCOT_ASSETS],
        coverage: RootCoverage::RecursiveTree,
        owner: "MascotAssets",
        risk: RiskTier::High,
        inventory: true,
    },
    ManagedRoot {
        entry: "E6",
        key: "assistant.db",
        scope: RootScope::Data,
        segments: &[dir_name::DATABASE],
        coverage: RootCoverage::SingleFile,
        owner: "DbState",
        risk: RiskTier::Forbidden,
        inventory: true,
    },
    ManagedRoot {
        entry: "E12",
        key: "logs",
        scope: RootScope::Local,
        segments: &[dir_name::LOGS],
        coverage: RootCoverage::RecursiveTree,
        owner: "tauri-plugin-log",
        risk: RiskTier::Forbidden,
        inventory: true,
    },
    ManagedRoot {
        entry: "E13",
        key: "crashes",
        scope: RootScope::Local,
        segments: &[dir_name::CRASHES],
        coverage: RootCoverage::RecursiveTree,
        owner: "assistant-crash",
        risk: RiskTier::Low,
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

const EXPECTED_ENTRY_COUNT: usize = 8;

/// core 8 条的显式 id 集合（删条即判红，不靠 `for index in 1..=13` 推算）。
const CORE_ENTRY_IDS: &[&str] = &["E1", "E2", "E3", "E4", "E5", "E6", "E12", "E13"];

/// 按 `key` 取登记项；未登记返回 `None`（管理器不得为未登记 key 产出条目，G4）。
pub fn find(key: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS.iter().find(|root| root.key == key)
}

/// 按条目 id（core 8 条）取登记项：条目 id 是对外契约，槽位由登记表现算。
pub fn find_by_entry(entry: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS.iter().find(|root| root.entry == entry)
}

/// 按首段取登记项；首段唯一（`E2` 与 `E1` 共享 `attachments`，返回先登记的 `E1`）。
///
/// 只覆盖 core 8 条：可选首段（`OPTIONAL_SEGMENTS`）不命中登记表，由分区声明判定。
pub fn find_by_first_segment(segment: &str) -> Option<&'static ManagedRoot> {
    MANAGED_ROOTS
        .iter()
        .find(|root| root.segments.first() == Some(&segment))
}

/// 登记表自检（V19 三条）：纯函数、接受切片，测试可传缺项负例。
///
/// 1. core 8 条 `entry` 集合完整且 `key == segments.join("/")`，`key` 不重复；
/// 2. `dir_name` 每个常量都必须有对应登记（首段命中）；
/// 3. 每条登记首段必须来自 `dir_name` 常量（core 8 条全是无条件挂载）。
///
/// 可选条目的 `key` / `entry` 校验归 `SectionRegistry::register`，这里不重复。
pub fn validate_registry(roots: &[ManagedRoot], dir_names: &[&str]) -> Result<(), String> {
    if roots.len() != EXPECTED_ENTRY_COUNT {
        return Err(format!(
            "registered roots count {} != {}",
            roots.len(),
            EXPECTED_ENTRY_COUNT
        ));
    }

    for entry in CORE_ENTRY_IDS {
        if !roots.iter().any(|root| root.entry == *entry) {
            return Err(format!("missing entry {entry}"));
        }
    }
    if roots.len() != CORE_ENTRY_IDS.len() {
        return Err("core entry id set has duplicates".to_string());
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

    /// V22：`owner` 只是展示标签，不得冒充路由 token（feature 名 / 可选 crate 类型名）。
    #[test]
    fn owner_labels_are_not_routing_tokens() {
        const FEATURE_NAMES: &[&str] = &["vault"];
        const OPTIONAL_TYPE_MARKERS: &[&str] = &["Probe", "Section", "Vault"];
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
                !OPTIONAL_TYPE_MARKERS
                    .iter()
                    .any(|marker| root.owner.contains(marker)),
                "{} 的 owner `{}` 不得是实现类型名形态",
                root.entry,
                root.owner
            );
            assert!(
                !FEATURE_NAMES.contains(&root.owner),
                "{} 的 owner `{}` 不得等于 feature 名",
                root.entry,
                root.owner
            );
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

    /// G17(b) / V24：core 源码不感知具体可选功能——可选首段拼成路径字面量与旧能力名零命中。
    #[test]
    fn core_sources_have_no_optional_entry_knowledge() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = vec![manifest.join("src/app_dirs/registry.rs")];
        for sub in [
            "src/cmd/storage/sample",
            "src/cmd/storage/entries",
            "src/cmd/storage/inventory",
            "src/cmd/storage/reclaim",
        ] {
            collect_rs_files(&manifest.join(sub), &mut files);
        }
        // 扫描范围包含本文件：命中串必须拼接构造，不能在源码里逐字出现。
        let optional_path = ["security", "/"].concat();
        let legacy_name = ["SECURITY", "_OBSERVABILITY"].concat();
        let offenders: Vec<String> = files
            .iter()
            .filter(|path| {
                std::fs::read_to_string(path)
                    .map(|text| text.contains(&optional_path) || text.contains(&legacy_name))
                    .unwrap_or(false)
            })
            .map(|path| path.display().to_string())
            .collect();
        assert!(
            offenders.is_empty(),
            "core 不得感知可选条目（可选首段路径 / 旧能力名），违规文件: {offenders:?}"
        );
    }
}
