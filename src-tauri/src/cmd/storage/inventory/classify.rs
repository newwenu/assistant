//! 首层分类与单段名校验（§3.11.3）：纯函数、无 IO，便于逐条断言。

use crate::app_dirs::registry::{self, RootScope};
use crate::cmd::storage::entries::EntryView;
use crate::protocol::dto::{StorageInventoryClassDto, StorageLayerDto};

/// 伴生后缀（§3.11.3）：计入扫描字节。对账只对**首层**按名字分类，故首层伴生文件
/// （如 `assistant.db-wal`，其 owner 是只观测自身的文件条目）不带登记值，表现为该节点的
/// `residualBytes`；落在已登记子树内部的伴生文件由 owner 的 `tree_stats` 一并计入。
pub(super) const COMPANION_SUFFIXES: &[&str] = &["-journal", "-wal", "-shm", ".tmp", ".lock"];

/// 首层分类（§3.11.3，互斥且穷尽）：纯函数，便于逐条断言。
///
/// `views` 是本次响应的条目清单：命中清单即 `declared`（条目 id 随视图给），
/// 未命中但属于 [`registry::OPTIONAL_SEGMENTS`] 即 `optionalNotMounted`——
/// 后者**不带条目 id**，因为未挂载分区的条目在 core 侧根本不存在（§7.1「行不存在」）。
pub fn classify(
    name: &str,
    views: &[EntryView],
) -> (StorageInventoryClassDto, Option<&'static str>) {
    if let Some(view) = views.iter().find(|view| {
        view.segments
            .first()
            .is_some_and(|segment| *segment == name)
    }) {
        return (StorageInventoryClassDto::Declared, Some(view.entry));
    }
    if registry::OPTIONAL_SEGMENTS.contains(&name) {
        return (StorageInventoryClassDto::OptionalNotMounted, None);
    }
    if registry::IGNORED_NAMES.contains(&name) {
        return (StorageInventoryClassDto::Ignored, None);
    }
    // 伴生后缀大小写不敏感（§3.11.3）：owner 侧判定（tools / 密码库域 / crash）都先
    // `to_ascii_lowercase()` 跳过伴生文件，管理器若按原样比对，`assistant.db-WAL` 就会从
    // `companion` 掉进 `unregistered`，变成「未登记」假告警。行为等价要求两侧同一口径（V21），
    // 这里对齐 owner 侧。
    let lowered = name.to_ascii_lowercase();
    if COMPANION_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
    {
        return (StorageInventoryClassDto::Companion, None);
    }
    (StorageInventoryClassDto::Unregistered, None)
}

/// 单段名校验（§3.11.3）：单段名不是路径，不得含分隔符 / 上跳 / NUL。
pub(super) fn is_safe_segment(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(|c| c == '\0' || c.is_control())
}

pub(super) fn layer_of(scope: RootScope) -> StorageLayerDto {
    match scope {
        RootScope::Data => StorageLayerDto::Data,
        RootScope::Local => StorageLayerDto::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::registry::{RootCoverage, StatsMode};
    use crate::cmd::storage::entries::views;
    use crate::cmd::storage::section::RiskTier;
    use crate::protocol::dto::{
        ReclaimPolicyDto, StorageEligibilityDto, StorageReclaimActionDto, StorageReclaimReasonDto,
    };

    fn core_views() -> Vec<EntryView> {
        views(&[])
    }

    /// 可选分区首段：读 [`registry::OPTIONAL_SEGMENTS`]，测试里不另写首段字面量（V22）。
    fn optional_segment() -> &'static str {
        registry::OPTIONAL_SEGMENTS[0]
    }

    /// 已挂载可选分区的视图：只造一条单段条目，够分类断言用。
    fn section_view() -> EntryView {
        EntryView {
            entry: "S1",
            core: None,
            key: optional_segment(),
            segments: registry::OPTIONAL_SEGMENTS,
            scope: RootScope::Data,
            coverage: RootCoverage::SingleFile,
            owner: "SecurityDbState",
            inventory: true,
            stats_mode: StatsMode::Deferred,
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            risk: RiskTier::Forbidden,
            note: None,
            section: Some(0),
        }
    }

    #[test]
    fn classification_is_exclusive_and_exhaustive() {
        let core = core_views();
        let optional = optional_segment();
        assert_eq!(
            classify("attachments", &core).0,
            StorageInventoryClassDto::Declared
        );
        assert_eq!(
            classify(optional, &core).0,
            StorageInventoryClassDto::OptionalNotMounted,
            "未挂载分区首段只给 optionalNotMounted，不带条目 id"
        );
        assert_eq!(classify(optional, &core).1, None);
        assert_eq!(
            classify("Thumbs.db", &core).0,
            StorageInventoryClassDto::Ignored
        );
        assert_eq!(
            classify("assistant.db-wal", &core).0,
            StorageInventoryClassDto::Companion
        );
        assert_eq!(
            classify("stray", &core).0,
            StorageInventoryClassDto::Unregistered
        );
        assert_eq!(classify("attachments", &core).1, Some("E1"));
    }

    #[test]
    fn mounted_section_segment_is_declared_with_its_own_entry_id() {
        let mut mounted = core_views();
        mounted.push(section_view());
        let optional = optional_segment();
        assert_eq!(
            classify(optional, &mounted).0,
            StorageInventoryClassDto::Declared
        );
        assert_eq!(
            classify(optional, &mounted).1,
            Some("S1"),
            "条目 id 只由分区自述提供"
        );
    }

    #[test]
    fn companion_suffixes_are_not_unregistered() {
        let core = core_views();
        for name in ["x.tmp", "x.lock", "db-journal", "db-shm"] {
            assert_eq!(
                classify(name, &core).0,
                StorageInventoryClassDto::Companion,
                "{name} 必须归 companion"
            );
        }
    }

    /// 大小写漂移（V21）：owner 侧判定先小写化，管理器必须给出同一结论，
    /// 否则 `assistant.db-WAL` 会被当成「未登记」告警（§3.11.3 主信号）。
    #[test]
    fn companion_suffixes_are_case_insensitive() {
        let core = core_views();
        for name in [
            "assistant.db-WAL",
            "assistant.db-SHM",
            "attachments.TMP",
            "assistant.db-Journal",
        ] {
            assert_eq!(
                classify(name, &core).0,
                StorageInventoryClassDto::Companion,
                "{name} 必须归 companion"
            );
        }
    }

    #[test]
    fn unsafe_segments_are_rejected() {
        for name in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert!(!is_safe_segment(name), "{name:?} 必须被拒绝");
        }
        assert!(is_safe_segment("attachments"));
    }
}
