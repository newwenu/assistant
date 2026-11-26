//! 静态声明表：按 core 条目身份取动作 / 静态资格 / 原因 / 策略 / 风险档相关字段。

use crate::app_dirs::registry::CoreEntry;
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

/// 按 core 条目身份取静态部分；枚举穷尽匹配，新增 / 删除条目时编译器点名漏改的分支。
///
/// 分区条目没有静态表：`action` / 资格 / `reason` / 策略 / `note` 全由分区自述（§3.1.1）。
pub(super) fn static_for(entry: CoreEntry) -> EntryStatic {
    match entry {
        CoreEntry::Attachments => EntryStatic {
            action: StorageReclaimActionDto::DeleteOrphans,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::Orphan,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::AttachmentsDerived => EntryStatic {
            action: StorageReclaimActionDto::RebuildCache,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::DerivedCache,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::ToolResults => EntryStatic {
            action: StorageReclaimActionDto::SweepExpired,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::Expired,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::ToolIndex => EntryStatic {
            action: StorageReclaimActionDto::RebuildCache,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::DerivedCache,
            policy: owner_policy(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::MascotAssets => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::UserData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::Database => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: None,
        },
        CoreEntry::Logs => EntryStatic {
            action: StorageReclaimActionDto::Rotate,
            eligibility: StorageEligibilityDto::OwnerManaged,
            reason: StorageReclaimReasonDto::ExternalOwner,
            policy: constant_policy(Some(3)),
            auto_active: true,
            counts_self_logs: true,
            note: None,
        },
        CoreEntry::Crashes => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotImplemented,
            reason: StorageReclaimReasonDto::NoReclaimApi,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: Some("暂未提供清理功能，这里只展示占用情况"),
        },
        CoreEntry::WebViewProfile => EntryStatic {
            action: StorageReclaimActionDto::RebuildCache,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::DerivedCache,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: Some("Chromium 缓存可重建，用户态数据不删；运行期由 WebView2 持有句柄"),
        },
        // `E15` / `E17` / `E18`：配置文件归应用自持，但内容是用户业务数据，只展示、不提供回收入口
        // （`action = None` / `NotEligible` / `BusinessData`，§13.4.2）。三条共用同一形状，
        // 用 or-pattern 合并，避免三份重复字面量。
        CoreEntry::Config | CoreEntry::OrbConfig | CoreEntry::MascotConfig => EntryStatic {
            action: StorageReclaimActionDto::None,
            eligibility: StorageEligibilityDto::NotEligible,
            reason: StorageReclaimReasonDto::BusinessData,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: Some("配置文件承载用户设置，只展示占用，不提供清理入口"),
        },
        // `E19` 临时区（I77 §4.5）：会话删除时随会话释放，也可由用户一键清空整根。
        // 静态 `Eligible`（同 `E2` / `E4` / `E14`）：执行器不依赖采样数字，采样失败也不该藏按钮。
        CoreEntry::ToolScratch => EntryStatic {
            action: StorageReclaimActionDto::ClearAll,
            eligibility: StorageEligibilityDto::Eligible,
            reason: StorageReclaimReasonDto::Transient,
            policy: ReclaimPolicyDto::default(),
            auto_active: false,
            counts_self_logs: false,
            note: Some("会话删除时随会话释放；也可在存储管理器一键清空（含其他会话与共享段）"),
        },
    }
}
