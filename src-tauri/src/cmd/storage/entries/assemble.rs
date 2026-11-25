//! 条目与整页装配：把样本 + 条目视图折算成 wire DTO（纯函数，不碰磁盘）。
//!
//! 条数不再固定：core 条目恒在，可选分区条目随已挂载分区出现，因此装配以
//! [`EntryView`] 清单为准，清单里没有的条目不会凭空生成占位行（X15 / §3.10）。

use crate::app_dirs::registry::{CoreEntry, RootScope};
use crate::protocol::dto::{
    ReclaimViewDto, StorageAvailabilityDto, StorageBreakdownDto, StorageByteBasisDto,
    StorageEligibilityDto, StorageEntryDto, StorageInventoryDto, StorageLayerDto,
    StorageReclaimPlanEntryDto,
};

use super::eligibility::{confirmation_for, eligibility_for};
use super::{BreakdownBase, EntrySample, EntryView};

fn layer_of(scope: RootScope) -> StorageLayerDto {
    match scope {
        RootScope::Data => StorageLayerDto::Data,
        RootScope::Local => StorageLayerDto::Local,
    }
}

fn build_entry(view: &EntryView, sample: &EntrySample) -> StorageEntryDto {
    let (eligibility, reason) = eligibility_for(view, sample);
    let policy = sample.policy.clone().unwrap_or_else(|| view.policy.clone());
    let reclaimable = if eligibility == StorageEligibilityDto::Eligible {
        sample.reclaimable
    } else {
        None
    };
    StorageEntryDto {
        key: view.key.to_string(),
        layer: layer_of(view.scope),
        owner: view.owner.to_string(),
        bytes: sample.bytes,
        count: sample.count,
        oldest_at: sample.oldest_at,
        newest_at: sample.newest_at,
        availability: sample.availability,
        includes_derived: false,
        counts_self_logs: view.counts_self_logs,
        reclaim: ReclaimViewDto {
            action: view.action,
            eligibility,
            reason,
            policy,
            reclaimable_bytes: reclaimable.map(|(bytes, _)| bytes),
            reclaimable_count: reclaimable.map(|(_, count)| count),
            confirmation: confirmation_for(view, eligibility),
            auto_active: view.auto_active,
            note: view.note.map(str::to_string),
        },
    }
}

/// 判据可算：`available` / `missing` / `locked` 都是明确结论；其余是「取不到」。
///
/// 静态 `notImplemented` 的条目即便观测到了数字也推不出判据，同样不算。
pub fn judgeable(view: &EntryView, sample: &EntrySample) -> bool {
    view.eligibility != StorageEligibilityDto::NotImplemented
        && matches!(
            sample.availability,
            StorageAvailabilityDto::Available
                | StorageAvailabilityDto::Missing
                | StorageAvailabilityDto::Locked
        )
}

/// 计划条目：与整页快照共用同一装配，保证「计划里的数字 = 页面上的数字」。
pub fn build_plan_entry(view: &EntryView, sample: &EntrySample) -> StorageReclaimPlanEntryDto {
    let entry = build_entry(view, sample);
    StorageReclaimPlanEntryDto {
        key: entry.key,
        action: entry.reclaim.action,
        eligibility: entry.reclaim.eligibility,
        reason: entry.reclaim.reason,
        expected_count: entry.reclaim.reclaimable_count,
        expected_bytes: entry.reclaim.reclaimable_bytes,
        policy: entry.reclaim.policy,
        confirmation: entry.reclaim.confirmation,
    }
}

/// 装配整页快照：条目清单与样本一一对齐，顺序即清单顺序，`inventory` 的数字绝不回流条目（§3.11.5）。
pub fn assemble_breakdown(
    base: BreakdownBase,
    views: &[EntryView],
    samples: &[EntrySample],
    sampled_at: i64,
    inventory: StorageInventoryDto,
) -> StorageBreakdownDto {
    let entries: Vec<StorageEntryDto> = views
        .iter()
        .zip(samples.iter())
        .map(|(view, sample)| build_entry(view, sample))
        .collect();
    let complete = views.iter().zip(samples.iter()).all(|(view, sample)| {
        // `E12` 日志 / `E14` WebView profile 在采样期间持续写入，数字必然滞后，
        // 判 `complete` 时必须排除（§6.1 / §13.3.6）。
        // 用 core 枚举比较而不是 `E` 字面量：登记表顺序调整时不会静默排除错行。
        matches!(view.core, Some(CoreEntry::Logs | CoreEntry::WebViewProfile))
            || matches!(
                sample.availability,
                StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing
            )
    });
    StorageBreakdownDto {
        total_bytes: base.total_bytes,
        total_count: base.total_count,
        orphans: base.orphans,
        conversations: base.conversations,
        sampled_at,
        byte_basis: StorageByteBasisDto::FileLength,
        shared_bytes: base.shared_bytes,
        referenced_once_bytes: base.referenced_once_bytes,
        entries,
        inventory,
        complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::registry::RootCoverage;
    use crate::cmd::storage::entries::set_sample;
    use crate::cmd::storage::entries::views;
    use crate::cmd::storage::section::RiskTier;
    use crate::protocol::dto::{
        ReclaimPolicyDto, StorageConfirmLevelDto, StorageEligibilityDto, StorageInventoryStatusDto,
        StoragePolicySourceDto, StorageReclaimActionDto, StorageReclaimReasonDto,
    };

    const OPTIONAL: &str = "optional-zone";

    #[allow(clippy::too_many_arguments)]
    fn section_entry(
        entry: &'static str,
        key: &'static str,
        segments: &'static [&'static str],
        risk: RiskTier,
        action: StorageReclaimActionDto,
        eligibility: StorageEligibilityDto,
        reason: StorageReclaimReasonDto,
        auto_active: bool,
    ) -> EntryView {
        EntryView {
            entry,
            core: None,
            key,
            segments,
            scope: RootScope::Data,
            coverage: RootCoverage::RecursiveTree,
            owner: "SecurityDb",
            inventory: true,
            action,
            eligibility,
            reason,
            policy: ReclaimPolicyDto::default(),
            auto_active,
            counts_self_logs: false,
            risk,
            note: None,
            section: Some(0),
        }
    }

    /// core 条目 + 一个假分区（数据库 / 备份 / 仓库三条）：验证分区条目走同一装配路径。
    ///
    /// 假条目 id 用 `S1`–`S3`：真实安全域条目由功能 crate 自述，
    /// core 侧（含测试）不写这些 id 字面量（G17(b) / R-09 验收）。
    fn test_views() -> Vec<EntryView> {
        let mut list = views(&[]);
        list.push(section_entry(
            "S1",
            "optional-zone/db",
            &[OPTIONAL, "db"],
            RiskTier::Forbidden,
            StorageReclaimActionDto::None,
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::BusinessData,
            false,
        ));
        list.push(section_entry(
            "S2",
            "optional-zone/backups",
            &[OPTIONAL, "backups"],
            RiskTier::Medium,
            StorageReclaimActionDto::PruneToLimit,
            StorageEligibilityDto::Eligible,
            StorageReclaimReasonDto::OverLimit,
            true,
        ));
        list.push(section_entry(
            "S3",
            "optional-zone/repo",
            &[OPTIONAL, "repo"],
            RiskTier::Forbidden,
            StorageReclaimActionDto::None,
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::UserData,
            false,
        ));
        list
    }

    fn observed() -> EntrySample {
        EntrySample::observed(1024, 1, Some(100), Some(200))
    }

    fn samples(views: &[EntryView]) -> Vec<EntrySample> {
        let mut samples = vec![observed(); views.len()];
        set_sample(
            views,
            &mut samples,
            "E12",
            EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        );
        set_sample(
            views,
            &mut samples,
            "E3",
            observed().with_policy(ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                retention_days: Some(30),
                effective_since: Some(100),
                mutable: true,
                ..ReclaimPolicyDto::default()
            }),
        );
        set_sample(
            views,
            &mut samples,
            "S2",
            EntrySample::observed(1024, 6, Some(100), Some(200)).with_policy(ReclaimPolicyDto {
                source: StoragePolicySourceDto::Constant,
                keep_count: Some(5),
                ..ReclaimPolicyDto::default()
            }),
        );
        samples
    }

    fn base() -> BreakdownBase {
        BreakdownBase {
            total_bytes: 0,
            total_count: 0,
            orphans: Vec::new(),
            conversations: Vec::new(),
            shared_bytes: 0,
            referenced_once_bytes: 0,
        }
    }

    fn assemble(views: &[EntryView], samples: &[EntrySample]) -> StorageBreakdownDto {
        assemble_breakdown(base(), views, samples, 100, StorageInventoryDto::default())
    }

    #[test]
    fn entries_follow_view_list_in_order() {
        let views = test_views();
        let dto = assemble(&views, &samples(&views));
        assert_eq!(dto.entries.len(), views.len());
        for (index, entry) in dto.entries.iter().enumerate() {
            assert_eq!(
                entry.key, views[index].key,
                "第 {index} 条 key 必须等于视图"
            );
            assert_eq!(entry.owner, views[index].owner);
            assert_eq!(entry.layer, layer_of(views[index].scope));
            assert!(!entry.includes_derived, "includesDerived 恒 false");
        }
        assert_eq!(dto.sampled_at, 100);
    }

    #[test]
    fn core_eligibility_matches_static_table() {
        let views = test_views();
        let dto = assemble(&views, &samples(&views));
        let eligible_keys = [
            "attachments/derived",
            "tool-results",
            "tool-index",
            "optional-zone/backups",
            // `E14`：资格恒 `eligible`（否则前端无入口，待执行标记永远写不进），
            // 运行期不删由执行结果 `scheduled` 表达（§13.3.5）。
            "EBWebView",
        ];
        for entry in &dto.entries {
            if eligible_keys.contains(&entry.key.as_str()) {
                assert_eq!(
                    entry.reclaim.eligibility,
                    StorageEligibilityDto::Eligible,
                    "{} 资格",
                    entry.key
                );
                continue;
            }
            assert_ne!(
                entry.reclaim.eligibility,
                StorageEligibilityDto::Eligible,
                "{} 静态档不该是 eligible",
                entry.key
            );
            assert!(
                entry.reclaim.reason.is_some(),
                "{} 非 eligible 必须给 reason",
                entry.key
            );
            assert_eq!(
                entry.reclaim.confirmation,
                StorageConfirmLevelDto::None,
                "{} 非 eligible 确认档位恒 none",
                entry.key
            );
            assert!(
                entry.reclaim.reclaimable_bytes.is_none()
                    && entry.reclaim.reclaimable_count.is_none(),
                "{} 非 eligible 不得给可回收数字",
                entry.key
            );
        }
        // `E12` 由插件自持：静态档恒 `ownerManaged`，观测不到统计也不改判资格。
        let logs = dto
            .entries
            .iter()
            .find(|entry| entry.key == "logs")
            .unwrap();
        assert_eq!(
            logs.reclaim.eligibility,
            StorageEligibilityDto::OwnerManaged
        );
        assert_eq!(
            logs.reclaim.reason,
            Some(StorageReclaimReasonDto::ExternalOwner)
        );
    }

    /// `E1` / `E3` 的资格随观测态变化：判据不满足时降级，且 `reason` 用 §3.1 指定的那个词。
    #[test]
    fn dynamic_eligibility_follows_policy_and_orphans() {
        let views = test_views();
        let mut samples = samples(&views);

        // `E1` 引用集合取不到（`reclaimable = None`）→ 判据不可算，`notEligible` + `unknown`。
        let dto = assemble(&views, &samples);
        let e1 = dto
            .entries
            .iter()
            .find(|entry| entry.key == "attachments")
            .unwrap();
        assert_eq!(e1.reclaim.reason, Some(StorageReclaimReasonDto::Unknown));

        // `E1` 算出来确实没有孤儿 → `notEligible` + `referenced`。
        set_sample(
            &views,
            &mut samples,
            "E1",
            EntrySample {
                reclaimable: Some((0, 0)),
                ..observed()
            },
        );
        let dto = assemble(&views, &samples);
        let e1 = dto
            .entries
            .iter()
            .find(|entry| entry.key == "attachments")
            .unwrap();
        assert_eq!(e1.reclaim.eligibility, StorageEligibilityDto::NotEligible);
        assert_eq!(e1.reclaim.reason, Some(StorageReclaimReasonDto::Referenced));

        // `E1` 有孤儿 → `eligible` + `orphan` + `strongConfirm`（风险档由条目决定）。
        set_sample(
            &views,
            &mut samples,
            "E1",
            EntrySample {
                reclaimable: Some((512, 2)),
                ..observed()
            },
        );
        let dto = assemble(&views, &samples);
        let e1 = dto
            .entries
            .iter()
            .find(|entry| entry.key == "attachments")
            .unwrap();
        assert_eq!(e1.reclaim.eligibility, StorageEligibilityDto::Eligible);
        assert_eq!(e1.reclaim.reason, Some(StorageReclaimReasonDto::Orphan));
        assert_eq!(
            e1.reclaim.confirmation,
            StorageConfirmLevelDto::StrongConfirm
        );
        assert_eq!(e1.reclaim.reclaimable_bytes, Some(512));
        assert_eq!(e1.reclaim.reclaimable_count, Some(2));

        // `E3` 保留天数 `0`（策略禁用）→ `notEligible` + `policyDisabled`。
        set_sample(
            &views,
            &mut samples,
            "E3",
            observed().with_policy(ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                retention_days: Some(0),
                effective_since: Some(100),
                mutable: true,
                ..ReclaimPolicyDto::default()
            }),
        );
        let dto = assemble(&views, &samples);
        let e3 = dto
            .entries
            .iter()
            .find(|entry| entry.key == "tool-results")
            .unwrap();
        assert_eq!(e3.reclaim.eligibility, StorageEligibilityDto::NotEligible);
        assert_eq!(
            e3.reclaim.reason,
            Some(StorageReclaimReasonDto::PolicyDisabled)
        );
    }

    /// 分区条目：资格由分区自述覆盖静态档，确认档位由风险档派生（§3.6 硬约束 5–6）。
    #[test]
    fn section_entries_use_self_report_and_risk_tier() {
        let views = test_views();
        let mut samples = samples(&views);

        // 中等风险 + 分区自述 `eligible` → `confirm`。
        set_sample(
            &views,
            &mut samples,
            "S2",
            EntrySample::observed(1024, 6, Some(100), Some(200))
                .with_eligibility(
                    StorageEligibilityDto::Eligible,
                    StorageReclaimReasonDto::OverLimit,
                )
                .with_policy(ReclaimPolicyDto {
                    source: StoragePolicySourceDto::Constant,
                    keep_count: Some(5),
                    ..ReclaimPolicyDto::default()
                }),
        );
        // 禁止档 + 分区自述 `eligible` → 仍不给确认入口（`none`）。
        set_sample(
            &views,
            &mut samples,
            "S3",
            observed().with_eligibility(
                StorageEligibilityDto::Eligible,
                StorageReclaimReasonDto::UserData,
            ),
        );

        let dto = assemble(&views, &samples);
        let backups = dto
            .entries
            .iter()
            .find(|entry| entry.key == "optional-zone/backups")
            .unwrap();
        assert_eq!(
            backups.reclaim.confirmation,
            StorageConfirmLevelDto::Confirm
        );
        assert_eq!(
            backups.reclaim.policy.keep_count,
            Some(5),
            "策略自述原样透出"
        );

        let repo = dto
            .entries
            .iter()
            .find(|entry| entry.key == "optional-zone/repo")
            .unwrap();
        assert_eq!(repo.reclaim.eligibility, StorageEligibilityDto::Eligible);
        assert_eq!(
            repo.reclaim.confirmation,
            StorageConfirmLevelDto::None,
            "禁止档不给确认入口"
        );
    }

    #[test]
    fn complete_excludes_only_logs_entry() {
        let views = test_views();
        let dto = assemble(&views, &samples(&views));
        assert!(dto.complete, "除 E12 外全部 available 时应为 true");

        let mut degraded = samples(&views);
        set_sample(
            &views,
            &mut degraded,
            "E3",
            EntrySample::degraded(StorageAvailabilityDto::Unreadable),
        );
        let dto = assemble(&views, &degraded);
        assert!(!dto.complete, "任一条目 unreadable 时 complete = false");
    }

    #[test]
    fn degraded_availability_keeps_partition_declared_eligibility() {
        let views = test_views();
        let mut degraded = samples(&views);

        set_sample(
            &views,
            &mut degraded,
            "S3",
            EntrySample::degraded(StorageAvailabilityDto::Locked).with_eligibility(
                StorageEligibilityDto::Locked,
                StorageReclaimReasonDto::Locked,
            ),
        );
        set_sample(
            &views,
            &mut degraded,
            "S1",
            EntrySample::degraded(StorageAvailabilityDto::Unavailable).with_eligibility(
                StorageEligibilityDto::NotEligible,
                StorageReclaimReasonDto::Unknown,
            ),
        );
        let dto = assemble(&views, &degraded);

        let repo = dto
            .entries
            .iter()
            .find(|entry| entry.key == "optional-zone/repo")
            .unwrap();
        assert_eq!(repo.availability, StorageAvailabilityDto::Locked);
        assert_eq!(repo.reclaim.eligibility, StorageEligibilityDto::Locked);
        assert_eq!(
            repo.reclaim.reason,
            Some(StorageReclaimReasonDto::Locked),
            "锁态由分区自述，core 只转发"
        );

        let db = dto
            .entries
            .iter()
            .find(|entry| entry.key == "optional-zone/db")
            .unwrap();
        assert_eq!(db.availability, StorageAvailabilityDto::Unavailable);
        assert_eq!(
            db.reclaim.eligibility,
            StorageEligibilityDto::NotEligible,
            "`unavailable` 不得被 core 判成 `locked`（§7.1 特例 2）"
        );
        assert_eq!(
            db.reclaim.reason,
            Some(StorageReclaimReasonDto::Unknown),
            "观测失败按「判据不可算」给 unknown"
        );
    }

    #[test]
    fn inventory_never_feeds_entries() {
        let views = test_views();
        let inventory = StorageInventoryDto {
            status: StorageInventoryStatusDto::Fresh,
            scanned_at: Some(9_999),
            unregistered_bytes: Some(4_000_000),
            ..StorageInventoryDto::default()
        };
        let dto = assemble_breakdown(base(), &views, &samples(&views), 100, inventory.clone());
        assert_eq!(
            dto.entries[0].bytes,
            Some(1024),
            "扫描数字不得改写条目 bytes"
        );
        assert_eq!(dto.sampled_at, 100, "条目时刻与扫描时刻分离");
        assert_eq!(dto.inventory, inventory);
    }

    #[test]
    fn empty_view_list_assembles_zero_entries() {
        let dto = assemble_breakdown(base(), &[], &[], 100, StorageInventoryDto::default());
        assert!(dto.entries.is_empty());
        assert!(dto.complete);
    }
}
