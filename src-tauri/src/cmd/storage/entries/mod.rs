//! 条目静态登记与装配（I61 §3.1–§3.6）：纯函数，不做 IO、不读配置。
//!
//! core 条目的 `key` / `layer` / `owner` 一律来自 `app_dirs::registry::MANAGED_ROOTS`（G1 / G4）；
//! 可选分区条目由各分区 `entries()` 自述，core 不写任何功能条目字面量（G17(b)）。
//!
//! 子模块按职责划分：`decl` 静态声明表、`view` 统一条目视图、`eligibility` 资格与确认档位派生、
//! `assemble` 条目与整页装配、`slot` 样本槽位寻址。

mod assemble;
mod decl;
mod eligibility;
mod slot;
mod view;

use crate::protocol::dto::{
    AttachmentSummary, ConversationAttachmentSummary, ReclaimPolicyDto, StorageAvailabilityDto,
    StorageEligibilityDto, StorageReclaimReasonDto,
};

pub use assemble::{assemble_breakdown, build_plan_entry, judgeable};
pub use slot::{index_of_core, index_of_key, sample_for, set_sample};
pub use view::{declared_segments, find_by_entry, find_by_key, views, EntryView};

/// 一条条目的观测结果；`None` 表示「取不到」，绝不填 `0` 冒充（G6）。
#[derive(Debug, Clone, PartialEq)]
pub struct EntrySample {
    pub bytes: Option<u64>,
    pub count: Option<u64>,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
    pub availability: StorageAvailabilityDto,
    /// 策略自述覆盖（`E3` 与分区条目由 owner 给，其余用静态表）。
    pub policy: Option<ReclaimPolicyDto>,
    /// 分区随观测态自述的资格（`None` = 用静态那一档）。
    pub eligibility: Option<(StorageEligibilityDto, StorageReclaimReasonDto)>,
    /// 只对 `eligible` 条目有意义；分区条目本期恒 `None`。
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
            policy: None,
            eligibility: None,
            reclaimable: None,
        }
    }

    /// 目录 / 文件确实不存在：`missing` + 计 `0`。
    pub fn missing() -> Self {
        Self::observed(0, 0, None, None)
    }

    /// 取不到数：`bytes` / `count` 恒 `null`，`availability` 必填（原因文案不上 wire）。
    pub fn degraded(availability: StorageAvailabilityDto) -> Self {
        Self {
            bytes: None,
            count: None,
            oldest_at: None,
            newest_at: None,
            availability,
            policy: None,
            eligibility: None,
            reclaimable: None,
        }
    }

    pub fn with_policy(mut self, policy: ReclaimPolicyDto) -> Self {
        self.policy = Some(policy);
        self
    }

    /// 分区自述的资格：覆盖静态那一档（§3.6 硬约束 5 的「资格」步）。
    pub fn with_eligibility(
        mut self,
        eligibility: StorageEligibilityDto,
        reason: StorageReclaimReasonDto,
    ) -> Self {
        self.eligibility = Some((eligibility, reason));
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
