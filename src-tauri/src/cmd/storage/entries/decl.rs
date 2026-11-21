//! 静态声明表：按条目 id 取动作 / 静态资格 / 原因 / 策略 / 风险档相关字段。

use crate::protocol::dto::{
    ReclaimPolicyDto, StorageEligibilityDto, StoragePolicySourceDto, StorageReclaimActionDto,
    StorageReclaimReasonDto,
};

/// 静态部分：T1 的资格是「按当时代码真实可执行性」填的（§11 切片期资格表）。
///
/// `note` 只承载结构性的能力缺口说明（前端原文展示）：不写策略数值（上限 / 份数 / 天数）、
/// 路径、时间点或错误原文——这些内容会随配置与版本变化而过时。
pub(super) struct EntryStatic {
    pub(super) action: StorageReclaimActionDto,
    pub(super) eligibility: StorageEligibilityDto,
    pub(super) reason: StorageReclaimReasonDto,
    pub(super) policy: ReclaimPolicyDto,
    pub(super) auto_active: bool,
    pub(super) counts_self_logs: bool,
    pub(super) note: Option<&'static str>,
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

/// 按条目 id 取静态部分（core 8 条）；条目 id 是对外契约，不依赖登记表顺序。
///
/// 分区条目没有静态表：`action` / 资格 / `reason` / 策略 / `note` 全由分区自述（§3.1.1）。
pub(super) fn static_for(entry: &str) -> EntryStatic {
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
        "E12" => EntryStatic {
            action: StorageReclaimActionDto::Rotate,
            eligibility: StorageEligibilityDto::OwnerManaged,
            reason: StorageReclaimReasonDto::ExternalOwner,
            policy: constant_policy(Some(3)),
            auto_active: true,
            counts_self_logs: true,
            note: Some("该组件不提供占用统计"),
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
