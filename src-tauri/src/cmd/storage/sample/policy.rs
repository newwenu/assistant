//! 策略快照：策略一律由 owner 自述，管理器只做「自述 → DTO」的搬运，不填默认值。

use crate::protocol::dto::{ReclaimPolicyDto, StoragePolicySourceDto};

pub(super) fn config_policy(days: u32, effective_since: i64) -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Config,
        retention_days: Some(days),
        effective_since: Some(effective_since),
        mutable: true,
        ..ReclaimPolicyDto::default()
    }
}
