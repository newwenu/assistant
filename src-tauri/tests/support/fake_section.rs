//! 集成测试共享的假分区（I61 §3.10）：core 条目之外的条目由分区自述，不依赖真实 vault。
//!
//! 首段一律从 [`registry::OPTIONAL_SEGMENTS`] 读，测试里不另写首段字面量（V22）。

#![allow(dead_code)]

use std::sync::Arc;

use assistant_app::app_dirs::registry::{self, RootCoverage, RootScope};
use assistant_app::cmd::storage::section::{
    EntryDecl, RiskTier, SectionEntry, SectionError, SectionReclaimReport, SectionStats,
    StorageSection, StorageSectionHandle,
};
use assistant_app::protocol::dto::{
    ReclaimPolicyDto, StorageEligibilityDto, StorageReclaimActionDto, StorageReclaimReasonDto,
};

/// 可选首段事实：唯一来源是 core 登记表，测试不重复字面量。
pub fn optional_segment() -> &'static str {
    registry::OPTIONAL_SEGMENTS[0]
}

/// 分区条目的稳定身份：首段 + 子段在运行期拼出，恒等于 `segments.join("/")`。
pub fn optional_key(child: &str) -> &'static str {
    Box::leak(format!("{}/{child}", optional_segment()).into_boxed_str())
}

pub fn optional_segments(child: &'static str) -> &'static [&'static str] {
    Box::leak(vec![optional_segment(), child].into_boxed_slice())
}

pub fn decl(
    action: StorageReclaimActionDto,
    eligibility: StorageEligibilityDto,
    reason: StorageReclaimReasonDto,
    risk: RiskTier,
) -> EntryDecl {
    EntryDecl {
        action,
        eligibility,
        reason,
        policy: ReclaimPolicyDto::default(),
        auto_active: false,
        counts_self_logs: false,
        risk,
        note: None,
    }
}

pub fn entry(id: &'static str, child: &'static str, decl: EntryDecl) -> SectionEntry {
    SectionEntry {
        entry: id,
        key: optional_key(child),
        segments: optional_segments(child),
        scope: RootScope::Data,
        coverage: RootCoverage::RecursiveTree,
        owner: "FakeOwner",
        inventory: true,
        decl,
    }
}

pub fn entries(list: Vec<SectionEntry>) -> &'static [SectionEntry] {
    Box::leak(list.into_boxed_slice())
}

pub fn actions(list: Vec<StorageReclaimActionDto>) -> &'static [StorageReclaimActionDto] {
    Box::leak(list.into_boxed_slice())
}

pub fn stats(bytes: u64, count: u64) -> SectionStats {
    SectionStats {
        bytes,
        count,
        oldest_at: Some(100),
        newest_at: Some(200),
    }
}

/// 两条假条目：禁止回收（`db`）+ 可回收（`backups`）。
pub fn pair_entries() -> &'static [SectionEntry] {
    entries(vec![
        entry(
            "X1",
            "db",
            decl(
                StorageReclaimActionDto::None,
                StorageEligibilityDto::NotEligible,
                StorageReclaimReasonDto::BusinessData,
                RiskTier::Forbidden,
            ),
        ),
        entry(
            "X2",
            "backups",
            decl(
                StorageReclaimActionDto::PruneToLimit,
                StorageEligibilityDto::Eligible,
                StorageReclaimReasonDto::OverLimit,
                RiskTier::Medium,
            ),
        ),
    ])
}

/// 三条假条目：仓库 / 恢复码 / 审计，覆盖 `Forbidden` 与 `Low` 两档。
pub fn triple_entries() -> &'static [SectionEntry] {
    entries(vec![
        entry(
            "X3",
            "vault",
            decl(
                StorageReclaimActionDto::None,
                StorageEligibilityDto::NotEligible,
                StorageReclaimReasonDto::UserData,
                RiskTier::Forbidden,
            ),
        ),
        entry(
            "X4",
            "recovery",
            decl(
                StorageReclaimActionDto::None,
                StorageEligibilityDto::NotEligible,
                StorageReclaimReasonDto::UserData,
                RiskTier::Forbidden,
            ),
        ),
        entry(
            "X5",
            "audit",
            decl(
                StorageReclaimActionDto::SweepExpired,
                StorageEligibilityDto::Eligible,
                StorageReclaimReasonDto::Expired,
                RiskTier::Low,
            ),
        ),
    ])
}

/// 五条假条目：覆盖「禁止回收 / 可回收 / 观测失败」三种静态档，用于 golden 装配。
pub fn five_entries() -> &'static [SectionEntry] {
    let mut list = Vec::new();
    list.extend_from_slice(pair_entries());
    list.extend_from_slice(triple_entries());
    entries(list)
}

/// 一个自述了 `PruneToLimit` 的假分区：驱动计划与执行的分区入口。
pub fn backups_handle(bytes: u64, count: u64, removed: u64) -> StorageSectionHandle {
    let list = entries(vec![entry(
        "X2",
        "backups",
        decl(
            StorageReclaimActionDto::PruneToLimit,
            StorageEligibilityDto::Eligible,
            StorageReclaimReasonDto::OverLimit,
            RiskTier::Medium,
        ),
    )]);
    mount_with(
        "fake-backups",
        list,
        stats(bytes, count),
        actions(vec![StorageReclaimActionDto::PruneToLimit]),
        SectionReclaimReport {
            removed,
            removed_bytes: bytes,
            failed: 0,
        },
    )
}

/// 观测态可替换的假分区：`stats` 走固定值，`actions` / `reclaim` 由调用方声明。
pub fn mount_with(
    id: &'static str,
    entries: &'static [SectionEntry],
    stats: SectionStats,
    actions: &'static [StorageReclaimActionDto],
    report: SectionReclaimReport,
) -> StorageSectionHandle {
    Arc::new(FakeSection {
        id,
        entries,
        stats,
        actions,
        report,
    })
}

/// 五条条目 + 默认观测值（1 KiB / 3 个文件）。
pub fn mount(id: &'static str, entries: &'static [SectionEntry]) -> StorageSectionHandle {
    mount_with(
        id,
        entries,
        stats(1024, 3),
        &[],
        SectionReclaimReport::default(),
    )
}

/// 首段不在 `OPTIONAL_SEGMENTS` 的假分区：注册必须被拒，且不出现在清单里。
pub fn foreign_handle() -> StorageSectionHandle {
    let segments: &'static [&'static str] = Box::leak(vec!["elsewhere"].into_boxed_slice());
    let list = entries(vec![SectionEntry {
        entry: "Y1",
        key: "elsewhere",
        segments,
        scope: RootScope::Data,
        coverage: RootCoverage::RecursiveTree,
        owner: "FakeOwner",
        inventory: true,
        decl: decl(
            StorageReclaimActionDto::None,
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::BusinessData,
            RiskTier::Forbidden,
        ),
    }]);
    mount("fake-foreign", list)
}

pub struct FakeSection {
    id: &'static str,
    entries: &'static [SectionEntry],
    stats: SectionStats,
    actions: &'static [StorageReclaimActionDto],
    report: SectionReclaimReport,
}

impl StorageSection for FakeSection {
    fn id(&self) -> &'static str {
        self.id
    }

    fn entries(&self) -> &'static [SectionEntry] {
        self.entries
    }

    fn stats(&self, key: &'static str) -> Option<Result<SectionStats, SectionError>> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|_| Ok(self.stats))
    }

    fn reclaim_actions(&self) -> &'static [StorageReclaimActionDto] {
        self.actions
    }

    fn reclaim(
        &self,
        key: &'static str,
        action: StorageReclaimActionDto,
        _policy: Option<ReclaimPolicyDto>,
    ) -> Option<Result<SectionReclaimReport, SectionError>> {
        self.entries.iter().find(|entry| entry.key == key)?;
        if !self.actions.contains(&action) {
            return None;
        }
        Some(Ok(self.report))
    }
}
