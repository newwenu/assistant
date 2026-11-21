//! 13 条条目的静态登记与装配（I61 §3.1–§3.6）：纯函数，不做 IO、不读配置。
//!
//! 条目顺序与 `key` / `layer` / `owner` 一律来自 `app_dirs::registry::MANAGED_ROOTS`
//! （G1 / G4）：登记表里没有的 key 不会出现在 DTO 里，这里也不写字面量目录名。

use crate::app_dirs::registry::{self, ManagedRoot, RootScope};
use crate::protocol::dto::{
    AttachmentSummary, ConversationAttachmentSummary, ReclaimPolicyDto, ReclaimViewDto,
    StorageAvailabilityDto, StorageBreakdownDto, StorageByteBasisDto, StorageConfirmLevelDto,
    StorageEligibilityDto, StorageEntryDto, StorageInventoryDto, StorageLayerDto,
    StoragePolicySourceDto, StorageReclaimActionDto, StorageReclaimPlanEntryDto,
    StorageReclaimReasonDto,
};

pub const ENTRY_COUNT: usize = 13;

/// `E12` 是唯一「插件自持、不暴露 stats」的条目，判定 `complete` 时必须排除（§6.1）。
///
/// 用条目 id 而不是下标：登记表顺序调整时不会静默排除错行。
const LOGS_ENTRY: &str = "E12";

/// 一条条目的观测结果；`None` 表示「取不到」，绝不填 `0` 冒充（G6）。
#[derive(Debug, Clone, PartialEq)]
pub struct EntrySample {
    pub bytes: Option<u64>,
    pub count: Option<u64>,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
    pub availability: StorageAvailabilityDto,
    pub availability_reason: Option<String>,
    /// 策略自述覆盖（`E3` / `E11` 由 owner 给，其余用静态表）。
    pub policy: Option<ReclaimPolicyDto>,
    /// 只对 `eligible` 条目有意义；T1 没有 `eligible` 条目，因此恒 `None`。
    pub reclaimable: Option<(u64, u64)>,
}

impl EntrySample {
    /// owner 给出了实测值：`count == 0` 归 `missing`（确实没有，计 `0`）。
    pub fn observed(
        bytes: u64,
        count: u64,
        oldest_at: Option<i64>,
        newest_at: Option<i64>,
    ) -> Self {
        let availability = if count == 0 {
            StorageAvailabilityDto::Missing
        } else {
            StorageAvailabilityDto::Available
        };
        Self {
            bytes: Some(bytes),
            count: Some(count),
            oldest_at,
            newest_at,
            availability,
            availability_reason: None,
            policy: None,
            reclaimable: None,
        }
    }

    /// 目录 / 文件确实不存在：`missing` + 计 `0`。
    pub fn missing() -> Self {
        Self::observed(0, 0, None, None)
    }

    /// 取不到数：`bytes` / `count` 恒 `null`，`availability` 与原因必填。
    pub fn degraded(availability: StorageAvailabilityDto, reason: impl Into<String>) -> Self {
        Self {
            bytes: None,
            count: None,
            oldest_at: None,
            newest_at: None,
            availability,
            availability_reason: Some(reason.into()),
            policy: None,
            reclaimable: None,
        }
    }

    pub fn with_policy(mut self, policy: ReclaimPolicyDto) -> Self {
        self.policy = Some(policy);
        self
    }
}

/// 旧字段与 `E1` 同源的那一份数据（§3.10 第 1 条：一次 `list_all` 同时填五处）。
pub struct BreakdownBase {
    pub total_bytes: u64,
    pub total_count: u32,
    pub orphans: Vec<AttachmentSummary>,
    pub conversations: Vec<ConversationAttachmentSummary>,
    pub shared_bytes: u64,
    pub referenced_once_bytes: u64,
}

/// 静态部分：T1 的资格是「按当时代码真实可执行性」填的（§11 切片期资格表）。
///
/// `note` 只承载结构性的能力缺口说明（前端原文展示）：不写策略数值（上限 / 份数 / 天数）、
/// 路径、时间点或错误原文——这些内容会随配置与版本变化而过时。
struct EntryStatic {
    action: StorageReclaimActionDto,
    eligibility: StorageEligibilityDto,
    reason: StorageReclaimReasonDto,
    policy: ReclaimPolicyDto,
    auto_active: bool,
    counts_self_logs: bool,
    note: Option<&'static str>,
}

fn constant_policy(keep_count: Option<u32>) -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Constant,
        keep_count,
        ..ReclaimPolicyDto::default()
    }
}

fn owner_policy() -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Owner,
        ..ReclaimPolicyDto::default()
    }
}

/// 按条目 id（`E1`→`E13`）取静态部分；条目 id 是对外契约，不依赖登记表顺序。
fn static_for(entry: &str) -> EntryStatic {
    match entry {
        "E1" => EntryStatic {
            action: StorageReclaimActionDto::DeleteOrphans,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::Orphan,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E2" => EntryStatic {
            action: StorageReclaimActionDto::RebuildCache,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::DerivedCache,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E3" => EntryStatic {
            action: StorageReclaimActionDto::SweepExpired,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::Expired,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E4" => EntryStatic {
            action: StorageReclaimActionDto::RebuildCache,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::DerivedCache,
            policy: owner_policy(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E5" => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::UserData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E6" => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E7" => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E8" => EntryStatic {
            action: StorageReclaimActionDto::PruneToLimit,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::OverLimit,
            policy: constant_policy(Some(5)),
            auto_active: true,
            counts_self_logs: false,
            note: None,
        },
        "E9" => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::UserData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        "E10" => EntryStatic {
            action: StorageReclaimActionDto::SweepExpired,
            eligibility: StorageEligibilityDto::OwnerManaged,
            reason: StorageReclaimReasonDto::Transient,
            policy: constant_policy(None),
            auto_active: true,
            counts_self_logs: false,
            note: None,
        },
        "E11" => EntryStatic {
            action: StorageReclaimActionDto::SweepExpired,
            eligibility: StorageEligibilityDto::OwnerManaged,
            reason: StorageReclaimReasonDto::Expired,
            policy: ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                mutable: true,
                ..ReclaimPolicyDto::default()
            },
            auto_active: true,
            counts_self_logs: false,
            note: None,
        },
        "E12" => EntryStatic {
            action: StorageReclaimActionDto::Rotate,
            eligibility: StorageEligibilityDto::OwnerManaged,
            reason: StorageReclaimReasonDto::ExternalOwner,
            policy: constant_policy(Some(3)),
            auto_active: true,
            counts_self_logs: true,
            note: None,
        },
        "E13" => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotImplemented,
            reason: StorageReclaimReasonDto::NoReclaimApi,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: Some("暂未提供清理功能，这里只展示占用情况"),
        },
        other => unreachable!("entry id out of range: {other}"),
    }
}

fn layer_of(scope: RootScope) -> StorageLayerDto {
    match scope {
        RootScope::Data => StorageLayerDto::Data,
        RootScope::Local => StorageLayerDto::Local,
    }
}

/// 资格按「当时代码真实可执行性」定档（§3.1 末条硬规则）。
///
/// 随观测态变化的只有 `E9` / `E7` / `E3`：`E9` 锁定态 `availability = locked`；
/// `E7` 的 `metadata` 失败（`unavailable`，文件正在替换 / 迁移）同样判 `locked`；
/// `E3` 按 owner 自述的保留天数在 `eligible` / `notEligible` 之间切换。
fn eligibility_for(
    root: &ManagedRoot,
    sample: &EntrySample,
    static_part: &EntryStatic,
) -> (StorageEligibilityDto, Option<StorageReclaimReasonDto>) {
    if root.entry == "E9" && sample.availability == StorageAvailabilityDto::Locked {
        return (
            StorageEligibilityDto::Locked,
            Some(StorageReclaimReasonDto::Locked),
        );
    }
    if root.entry == "E7" && sample.availability == StorageAvailabilityDto::Unavailable {
        return (
            StorageEligibilityDto::Locked,
            Some(StorageReclaimReasonDto::Locked),
        );
    }
    if root.entry == "E3" {
        if let Some((eligibility, reason)) = tool_results_eligibility(sample) {
            return (eligibility, Some(reason));
        }
    }
    if root.entry == "E1" {
        if let Some((eligibility, reason)) = orphans_eligibility(sample) {
            return (eligibility, Some(reason));
        }
    }
    if root.entry == "E8" {
        if let Some((eligibility, reason)) = backups_eligibility(sample, static_part) {
            return (eligibility, Some(reason));
        }
    }
    if static_part.eligibility == StorageEligibilityDto::Eligible {
        // `eligible` 的 reason 可空，但统一填该条目的唯一解（§3.1），前端不必分支。
        return (StorageEligibilityDto::Eligible, Some(static_part.reason));
    }
    (static_part.eligibility, Some(static_part.reason))
}

/// `E1` 的资格随孤儿数量变化（§3.1）：有孤儿才 `eligible`，无孤儿 `notEligible` + `referenced`。
///
/// 孤儿数量只取 `reclaimable`（与顶层孤儿列表同源）。三态必须分开：`Some(n>0)` 才是
/// `orphan`；`Some(0)` 是「算出来确实没有孤儿」→ `referenced`；`None` 是「引用集合取不到、
/// 判据不可算」→ `unknown`（§3.4 Q2）。**不得**把 `None` 当 0 条孤儿，那会把「不知道」
/// 说成「仍被引用」。
fn orphans_eligibility(
    sample: &EntrySample,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => {
            let judged = match sample.reclaimable {
                Some((_, count)) if count > 0 => (
                    StorageEligibilityDto::Eligible,
                    StorageReclaimReasonDto::Orphan,
                ),
                Some(_) => (
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::Referenced,
                ),
                None => (
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::Unknown,
                ),
            };
            Some(judged)
        }
        StorageAvailabilityDto::NotImplemented => Some((
            StorageEligibilityDto::NotImplemented,
            StorageReclaimReasonDto::NoReclaimApi,
        )),
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}

/// `E3` 的资格随 owner 自述的保留天数变化（§3.1）：`> 0` 才 `eligible`。
///
/// 观测不可用时不推断：缺入口 → `notImplemented`，取不到数 → `notEligible` + `unknown`
/// （判据不可算，§3.4 的 Q2），绝不因「大概能删」而提前标 `eligible`。
fn tool_results_eligibility(
    sample: &EntrySample,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => {
            let days = sample
                .policy
                .as_ref()
                .and_then(|policy| policy.retention_days)
                .unwrap_or(0);
            if days > 0 {
                Some((
                    StorageEligibilityDto::Eligible,
                    StorageReclaimReasonDto::Expired,
                ))
            } else {
                Some((
                    StorageEligibilityDto::NotEligible,
                    StorageReclaimReasonDto::PolicyDisabled,
                ))
            }
        }
        StorageAvailabilityDto::NotImplemented => Some((
            StorageEligibilityDto::NotImplemented,
            StorageReclaimReasonDto::NoReclaimApi,
        )),
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}

/// `E8` 的资格随备份份数变化（§3.1）：超过 `keep` 才 `eligible`，否则 `notEligible`。
///
/// `reason` 在该行只有一个词 `overLimit`（§3.1 表格）：`notEligible` 时读作
/// 「已在上限内、无可裁剪」，因此两个分支同填，不另造词。
/// 备份计数走目录列举、不依赖解锁，故 `locked` 态不降级（`SecurityDbState` 无锁态观测）。
fn backups_eligibility(
    sample: &EntrySample,
    static_part: &EntryStatic,
) -> Option<(StorageEligibilityDto, StorageReclaimReasonDto)> {
    match sample.availability {
        StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing => {
            let keep = static_part.policy.keep_count.unwrap_or(1);
            let count = sample.count.unwrap_or(0);
            let eligibility = if count > u64::from(keep) {
                StorageEligibilityDto::Eligible
            } else {
                StorageEligibilityDto::NotEligible
            };
            Some((eligibility, StorageReclaimReasonDto::OverLimit))
        }
        StorageAvailabilityDto::NotImplemented => Some((
            StorageEligibilityDto::NotImplemented,
            StorageReclaimReasonDto::NoReclaimApi,
        )),
        _ => Some((
            StorageEligibilityDto::NotEligible,
            StorageReclaimReasonDto::Unknown,
        )),
    }
}

/// 确认档位：第一步看用户能否发起，第二步才按风险档；T1 无 `eligible` 条目。
///
/// 风险档（§3.6 硬约束 5）：`E1` 高 → `strongConfirm`；`E2` / `E3` / `E4` / `E8` 低 / 中 → `confirm`。
fn confirmation_for(
    root: &ManagedRoot,
    eligibility: StorageEligibilityDto,
) -> StorageConfirmLevelDto {
    if eligibility != StorageEligibilityDto::Eligible {
        return StorageConfirmLevelDto::None;
    }
    if root.entry == "E1" {
        StorageConfirmLevelDto::StrongConfirm
    } else {
        StorageConfirmLevelDto::Confirm
    }
}

fn build_entry(root: &ManagedRoot, sample: &EntrySample) -> StorageEntryDto {
    let static_part = static_for(root.entry);
    let (eligibility, reason) = eligibility_for(root, sample, &static_part);
    let policy = sample.policy.clone().unwrap_or(static_part.policy);
    let reclaimable = if eligibility == StorageEligibilityDto::Eligible {
        sample.reclaimable
    } else {
        None
    };
    StorageEntryDto {
        key: root.key.to_string(),
        layer: layer_of(root.scope),
        owner: root.owner.to_string(),
        bytes: sample.bytes,
        count: sample.count,
        oldest_at: sample.oldest_at,
        newest_at: sample.newest_at,
        availability: sample.availability,
        availability_reason: sample.availability_reason.clone(),
        includes_derived: false,
        counts_self_logs: static_part.counts_self_logs,
        reclaim: ReclaimViewDto {
            action: static_part.action,
            eligibility,
            reason,
            policy,
            reclaimable_bytes: reclaimable.map(|(bytes, _)| bytes),
            reclaimable_count: reclaimable.map(|(_, count)| count),
            confirmation: confirmation_for(root, eligibility),
            auto_active: static_part.auto_active,
            note: static_part.note.map(str::to_string),
        },
    }
}

/// key → 样本槽位：请求与采样只认登记表 key，位置由登记表现算（不写下标常量）。
pub fn index_of_key(key: &str) -> Option<usize> {
    registry::MANAGED_ROOTS
        .iter()
        .position(|root| root.key == key)
}

/// 按登记项取样本；登记表里没有的 key 属于调用方 bug（先经 `index_of_key` 校验）。
pub fn sample_for<'a>(
    samples: &'a [EntrySample; ENTRY_COUNT],
    root: &ManagedRoot,
) -> &'a EntrySample {
    let slot = index_of_key(root.key).expect("登记表 key 必须可定位");
    &samples[slot]
}

/// 按条目 id（`E1`..`E13`）覆写样本：条目 id 是对外契约，槽位现算。
pub fn set_sample(samples: &mut [EntrySample; ENTRY_COUNT], entry: &str, sample: EntrySample) {
    let root = registry::find_by_entry(entry).expect("登记表必须包含该条目");
    let slot = index_of_key(root.key).expect("登记表 key 必须可定位");
    samples[slot] = sample;
}

/// 判据可算：`available` / `missing` / `locked` 都是明确结论；其余是「取不到」。
///
/// 静态 `notImplemented` 的条目即便观测到了数字也推不出判据，同样不算。
pub fn judgeable(root: &ManagedRoot, sample: &EntrySample) -> bool {
    static_for(root.entry).eligibility != StorageEligibilityDto::NotImplemented
        && matches!(
            sample.availability,
            StorageAvailabilityDto::Available
                | StorageAvailabilityDto::Missing
                | StorageAvailabilityDto::Locked
        )
}

/// 计划条目：与整页快照共用同一装配，保证「计划里的数字 = 页面上的数字」。
pub fn build_plan_entry(root: &ManagedRoot, sample: &EntrySample) -> StorageReclaimPlanEntryDto {
    let entry = build_entry(root, sample);
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

/// 装配整页快照：13 条恒存在、顺序固定，`inventory` 的数字绝不回流条目（§3.11.5）。
pub fn assemble_breakdown(
    base: BreakdownBase,
    samples: &[EntrySample; ENTRY_COUNT],
    sampled_at: i64,
    inventory: StorageInventoryDto,
) -> StorageBreakdownDto {
    let entries: Vec<StorageEntryDto> = registry::MANAGED_ROOTS
        .iter()
        .zip(samples.iter())
        .map(|(root, sample)| build_entry(root, sample))
        .collect();
    let complete = registry::MANAGED_ROOTS
        .iter()
        .zip(samples.iter())
        .all(|(root, sample)| {
            root.entry == LOGS_ENTRY
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

/// 关闭特性构建 / 插件未装配时 `E7`–`E11` 的统一降级：点名「可选能力未启用」。
pub fn unmounted_probe_sample() -> EntrySample {
    EntrySample::degraded(StorageAvailabilityDto::NotImplemented, "可选功能未启用")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::dto::StorageInventoryStatusDto;

    fn samples() -> [EntrySample; ENTRY_COUNT] {
        let mut samples: [EntrySample; ENTRY_COUNT] =
            std::array::from_fn(|_| EntrySample::observed(1024, 1, Some(100), Some(200)));
        set_sample(
            &mut samples,
            "E12",
            EntrySample::degraded(
                StorageAvailabilityDto::NotImplemented,
                "该组件不提供占用统计",
            ),
        );
        // `E3` 的资格随 owner 自述的保留天数变化：`> 0` 才 `eligible`。
        set_sample(
            &mut samples,
            "E3",
            EntrySample::observed(1024, 1, Some(100), Some(200)).with_policy(ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                retention_days: Some(30),
                effective_since: Some(100),
                mutable: true,
                ..ReclaimPolicyDto::default()
            }),
        );
        // `E8` 的资格随备份份数变化：超过上限才 `eligible`。
        set_sample(
            &mut samples,
            "E8",
            EntrySample::observed(1024, 6, Some(100), Some(200)),
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

    #[test]
    fn entries_are_thirteen_in_registry_order() {
        let dto = assemble_breakdown(base(), &samples(), 100, StorageInventoryDto::default());
        assert_eq!(dto.entries.len(), ENTRY_COUNT);
        for (index, entry) in dto.entries.iter().enumerate() {
            let root = &registry::MANAGED_ROOTS[index];
            assert_eq!(entry.key, root.key, "第 {index} 条 key 必须等于登记表");
            assert_eq!(entry.owner, root.owner);
            assert_eq!(entry.layer, layer_of(root.scope));
            assert!(!entry.includes_derived, "13 条 includesDerived 恒 false");
        }
        assert_eq!(dto.sampled_at, 100);
    }

    #[test]
    fn t3_eligibility_matches_slice_table() {
        let dto = assemble_breakdown(base(), &samples(), 100, StorageInventoryDto::default());
        for (index, entry) in dto.entries.iter().enumerate() {
            let expected = match index {
                12 => StorageEligibilityDto::NotImplemented,
                0 | 4 | 5 | 6 | 8 => StorageEligibilityDto::NotEligible,
                9..=11 => StorageEligibilityDto::OwnerManaged,
                1 | 2 | 3 | 7 => StorageEligibilityDto::Eligible,
                _ => unreachable!(),
            };
            assert_eq!(entry.reclaim.eligibility, expected, "E{} 资格", index + 1);
            if expected != StorageEligibilityDto::Eligible {
                assert!(
                    entry.reclaim.reason.is_some(),
                    "E{} 非 eligible 必须给 reason",
                    index + 1
                );
                assert_eq!(
                    entry.reclaim.confirmation,
                    StorageConfirmLevelDto::None,
                    "E{} 非 eligible 确认档位恒 none",
                    index + 1
                );
                assert!(
                    entry.reclaim.reclaimable_bytes.is_none()
                        && entry.reclaim.reclaimable_count.is_none(),
                    "E{} 非 eligible 不得给可回收数字",
                    index + 1
                );
            }
        }
        assert!(
            dto.entries[12].reclaim.note.is_some(),
            "notImplemented 必须给出能力缺口说明"
        );
    }

    /// `E1` / `E3` / `E8` 的资格随观测态变化：判据不满足时降级，且 `reason` 用 §3.1 指定的那个词。
    #[test]
    fn t3_dynamic_eligibility_follows_policy_and_count() {
        // `E1` 引用集合取不到（`reclaimable = None`）→ 判据不可算，`notEligible` + `unknown`。
        assert_eq!(
            assemble_breakdown(base(), &samples(), 100, StorageInventoryDto::default()).entries[0]
                .reclaim
                .reason,
            Some(StorageReclaimReasonDto::Unknown)
        );
        // `E1` 算出来确实没有孤儿（`reclaimable = Some((0, 0))`）→ `notEligible` + `referenced`。
        {
            let mut e1 = samples();
            e1[0].reclaimable = Some((0, 0));
            let dto = assemble_breakdown(base(), &e1, 100, StorageInventoryDto::default());
            assert_eq!(
                dto.entries[0].reclaim.eligibility,
                StorageEligibilityDto::NotEligible
            );
            assert_eq!(
                dto.entries[0].reclaim.reason,
                Some(StorageReclaimReasonDto::Referenced)
            );
        }
        // `E1` 有孤儿 → `eligible` + `orphan` + `strongConfirm`（风险档由条目决定）。
        {
            let mut e1 = samples();
            e1[0].reclaimable = Some((512, 2));
            let dto = assemble_breakdown(base(), &e1, 100, StorageInventoryDto::default());
            assert_eq!(
                dto.entries[0].reclaim.eligibility,
                StorageEligibilityDto::Eligible
            );
            assert_eq!(
                dto.entries[0].reclaim.reason,
                Some(StorageReclaimReasonDto::Orphan)
            );
            assert_eq!(
                dto.entries[0].reclaim.confirmation,
                StorageConfirmLevelDto::StrongConfirm
            );
            assert_eq!(dto.entries[0].reclaim.reclaimable_bytes, Some(512));
            assert_eq!(dto.entries[0].reclaim.reclaimable_count, Some(2));
        }

        let mut samples = samples();
        // `E3` 保留天数 `0`（策略禁用）→ `notEligible` + `policyDisabled`。
        set_sample(
            &mut samples,
            "E3",
            EntrySample::observed(1024, 1, Some(100), Some(200)).with_policy(ReclaimPolicyDto {
                source: StoragePolicySourceDto::Config,
                retention_days: Some(0),
                effective_since: Some(100),
                mutable: true,
                ..ReclaimPolicyDto::default()
            }),
        );
        // `E8` 备份份数不超限 → `notEligible`，`reason` 仍是该行唯一的 `overLimit`。
        set_sample(
            &mut samples,
            "E8",
            EntrySample::observed(1024, 5, Some(100), Some(200)),
        );
        let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());

        assert_eq!(
            dto.entries[2].reclaim.eligibility,
            StorageEligibilityDto::NotEligible
        );
        assert_eq!(
            dto.entries[2].reclaim.reason,
            Some(StorageReclaimReasonDto::PolicyDisabled)
        );
        assert_eq!(
            dto.entries[7].reclaim.eligibility,
            StorageEligibilityDto::NotEligible
        );
        assert_eq!(
            dto.entries[7].reclaim.reason,
            Some(StorageReclaimReasonDto::OverLimit)
        );
    }

    /// golden（§11 切片期资格表，T3 结束）：`E1` 有孤儿 → `eligible` + `strongConfirm`；
    /// `E3` 保留天数 > 0 → `eligible` + `confirm`；`E8` 备份数 > `keep` → `eligible` + `confirm`。
    ///
    /// 这条断言锁死「切片期资格」与「确认档位」两件事：T3 之前三条都是 `notImplemented`，
    /// T3 之后只要判据成立就必须进 `eligible`，且风险档由条目而非动作决定（§3.6 硬约束 6）。
    #[test]
    fn t3_golden_slice_table_marks_e1_e3_e8_eligible() {
        let mut samples = samples();
        samples[0].reclaimable = Some((512, 2));
        let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());

        let expectations = [
            (
                0,
                "attachments",
                StorageReclaimReasonDto::Orphan,
                StorageConfirmLevelDto::StrongConfirm,
            ),
            (
                2,
                "tool-results",
                StorageReclaimReasonDto::Expired,
                StorageConfirmLevelDto::Confirm,
            ),
            (
                7,
                "security/backups",
                StorageReclaimReasonDto::OverLimit,
                StorageConfirmLevelDto::Confirm,
            ),
        ];
        for (index, key, reason, confirmation) in expectations {
            let entry = &dto.entries[index];
            assert_eq!(entry.key, key);
            assert_eq!(
                entry.reclaim.eligibility,
                StorageEligibilityDto::Eligible,
                "{} 在 T3 结束必须 eligible",
                key
            );
            assert_eq!(entry.reclaim.reason, Some(reason), "{} reason", key);
            assert_eq!(
                entry.reclaim.confirmation, confirmation,
                "{} 的确认档位由风险档决定",
                key
            );
        }
    }

    #[test]
    fn eligible_entries_require_reclaimable_numbers() {
        let mut samples = samples();
        samples[1].reclaimable = Some((512, 2));
        samples[3].reclaimable = Some((1024, 1));
        let dto = assemble_breakdown(base(), &samples, 100, StorageInventoryDto::default());

        for index in [1, 3] {
            let entry = &dto.entries[index];
            assert_eq!(entry.reclaim.eligibility, StorageEligibilityDto::Eligible);
            assert_eq!(
                entry.reclaim.reason,
                Some(StorageReclaimReasonDto::DerivedCache)
            );
            assert_eq!(entry.reclaim.confirmation, StorageConfirmLevelDto::Confirm);
            assert_eq!(
                entry.reclaim.reclaimable_bytes,
                Some(if index == 1 { 512 } else { 1024 })
            );
            assert_eq!(
                entry.reclaim.reclaimable_count,
                Some(if index == 1 { 2 } else { 1 })
            );
        }
    }

    #[test]
    fn complete_excludes_only_logs_entry() {
        let dto = assemble_breakdown(base(), &samples(), 100, StorageInventoryDto::default());
        assert!(dto.complete, "除 E12 外全部 available 时应为 true");

        let mut degraded = samples();
        degraded[2] = EntrySample::degraded(StorageAvailabilityDto::Unreadable, "io");
        let dto = assemble_breakdown(base(), &degraded, 100, StorageInventoryDto::default());
        assert!(!dto.complete, "任一条目 unreadable 时 complete = false");
    }

    #[test]
    fn e7_and_e9_downgrade_to_locked_eligibility() {
        let mut degraded = samples();

        degraded[8] = EntrySample::degraded(StorageAvailabilityDto::Locked, "尚未解锁");
        degraded[6] =
            EntrySample::degraded(StorageAvailabilityDto::Unavailable, "文件正在替换 / 迁移");
        let dto = assemble_breakdown(base(), &degraded, 100, StorageInventoryDto::default());

        for index in [6, 8] {
            assert_eq!(
                dto.entries[index].reclaim.eligibility,
                StorageEligibilityDto::Locked,
                "E{} 降级后资格必须 locked",
                index + 1
            );
            assert_eq!(
                dto.entries[index].reclaim.reason,
                Some(StorageReclaimReasonDto::Locked),
                "E{} 降级后 reason 必须 locked",
                index + 1
            );
        }
        assert_eq!(
            dto.entries[8].availability,
            StorageAvailabilityDto::Locked,
            "E9 可用性与资格同为 locked"
        );
        assert_eq!(
            dto.entries[6].availability,
            StorageAvailabilityDto::Unavailable,
            "E7 可用性仍是 unavailable，不得与资格统一"
        );

        let unlocked = assemble_breakdown(base(), &samples(), 100, StorageInventoryDto::default());
        assert_eq!(
            unlocked.entries[6].reclaim.eligibility,
            StorageEligibilityDto::NotEligible,
            "E7 正常态不因锁定变 locked"
        );
        assert_eq!(
            unlocked.entries[6].reclaim.reason,
            Some(StorageReclaimReasonDto::BusinessData)
        );
    }

    #[test]
    fn inventory_never_feeds_entries() {
        let inventory = StorageInventoryDto {
            status: StorageInventoryStatusDto::Fresh,
            scanned_at: Some(9_999),
            unregistered_bytes: Some(4_000_000),
            ..StorageInventoryDto::default()
        };
        let dto = assemble_breakdown(base(), &samples(), 100, inventory.clone());
        assert_eq!(
            dto.entries[0].bytes,
            Some(1024),
            "扫描数字不得改写条目 bytes"
        );
        assert_eq!(dto.sampled_at, 100, "条目时刻与扫描时刻分离");
        assert_eq!(dto.inventory, inventory);
    }
}
