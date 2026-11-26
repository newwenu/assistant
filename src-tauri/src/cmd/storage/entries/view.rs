//! 条目统一视图：core 条目来自登记表，可选分区条目来自各分区 `entries()` 自述。
//!
//! 装配、采样、对账、回收都只认这一种视图，不各自重建「core 行 / 分区行」两套结构
//! （X15）。`section` 只在回收路由时用（按 `key` 找到声明它的分区），core 条目为 `None`。

use crate::app_dirs::registry::{self, CoreEntry, ManagedRoot, RootCoverage, RootScope, StatsMode};
use crate::cmd::storage::section::{RiskTier, SectionEntry, StorageSectionHandle};
use crate::protocol::dto::{
    ReclaimPolicyDto, StorageEligibilityDto, StorageReclaimActionDto, StorageReclaimReasonDto,
};

use super::decl::static_for;

#[derive(Debug, Clone)]
pub struct EntryView {
    /// 对外调试标签：core 条目等于 [`CoreEntry::id`]，分区条目由分区自述。
    pub entry: &'static str,
    /// core 条目身份（§4.3）：`Some` 时 `entry` 恒等于 `core.id()`；分区条目为 `None`。
    ///
    /// 按条目分支一律匹配本字段：core 是封闭枚举（编译期穷尽），分区由 `section` 路由。
    pub core: Option<CoreEntry>,
    pub key: &'static str,
    pub segments: &'static [&'static str],
    pub scope: RootScope,
    pub coverage: RootCoverage,
    /// 只做展示，不参与路由（G17(a)）。
    pub owner: &'static str,
    /// 该 scope 是否参与余量对账。
    pub inventory: bool,
    /// 采样档（I79 §4.2）：core 取登记表、分区取自述；采样与装配只认这一份清单。
    pub stats_mode: StatsMode,
    pub action: StorageReclaimActionDto,
    /// 不随观测态变化的那一档资格；动态部分在 `EntrySample::eligibility`。
    pub eligibility: StorageEligibilityDto,
    pub reason: StorageReclaimReasonDto,
    pub policy: ReclaimPolicyDto,
    pub auto_active: bool,
    pub counts_self_logs: bool,
    /// 风险档：core 据此派生 `confirmation`（§3.6 硬约束 5–6），不进 wire。
    pub risk: RiskTier,
    pub note: Option<&'static str>,
    /// 所属分区在快照里的下标；core 条目为 `None`。
    pub section: Option<usize>,
}

fn core_view(root: &ManagedRoot) -> EntryView {
    let static_part = static_for(root.entry);
    EntryView {
        entry: root.entry.id(),
        core: Some(root.entry),
        key: root.key,
        segments: root.segments,
        scope: root.scope,
        coverage: root.coverage,
        owner: root.owner,
        inventory: root.inventory,
        stats_mode: root.stats_mode,
        action: static_part.action,
        eligibility: static_part.eligibility,
        reason: static_part.reason,
        policy: static_part.policy,
        auto_active: static_part.auto_active,
        counts_self_logs: static_part.counts_self_logs,
        risk: root.risk,
        note: static_part.note,
        section: None,
    }
}

fn section_view(index: usize, entry: &SectionEntry) -> EntryView {
    EntryView {
        entry: entry.entry,
        core: None,
        key: entry.key,
        segments: entry.segments,
        scope: entry.scope,
        coverage: entry.coverage,
        owner: entry.owner,
        inventory: entry.inventory,
        stats_mode: entry.stats_mode,
        action: entry.decl.action,
        eligibility: entry.decl.eligibility,
        reason: entry.decl.reason,
        policy: entry.decl.policy.clone(),
        auto_active: entry.decl.auto_active,
        counts_self_logs: entry.decl.counts_self_logs,
        risk: entry.decl.risk,
        note: entry.decl.note,
        section: Some(index),
    }
}

/// core 条目按登记表顺序在前，各分区条目按 `entries()` 快照顺序紧随其后。
///
/// 清单只在这里生成一次（X15）：未注册的分区不贡献任何条目，因此「未挂载 = 行不存在」。
pub fn views(sections: &[StorageSectionHandle]) -> Vec<EntryView> {
    let mut views: Vec<EntryView> = registry::MANAGED_ROOTS.iter().map(core_view).collect();
    for (index, section) in sections.iter().enumerate() {
        views.extend(
            section
                .entries()
                .iter()
                .map(|entry| section_view(index, entry)),
        );
    }
    views
}

/// 按 `key` 定位视图：core 与分区条目同一入口（X15），不再有两套查找。
pub fn find_by_key<'v>(views: &'v [EntryView], key: &str) -> Option<&'v EntryView> {
    views.iter().find(|view| view.key == key)
}

/// 按条目 id 定位视图。
pub fn find_by_entry<'v>(views: &'v [EntryView], entry: &str) -> Option<&'v EntryView> {
    views.iter().find(|view| view.entry == entry)
}

/// 已挂载分区声明的首段集合：对账据此判 `declared` / `optionalNotMounted`。
pub fn declared_segments(views: &[EntryView]) -> Vec<&'static str> {
    views
        .iter()
        .filter(|view| view.section.is_some())
        .filter_map(|view| view.segments.first().copied())
        .collect()
}
