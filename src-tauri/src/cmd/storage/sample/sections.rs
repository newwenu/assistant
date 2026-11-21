//! 可选分区采样：把分区自述的观测 / 策略 / 资格折算成 core 的 `EntrySample`。
//!
//! 这里只做「分区 → 通用样本」的搬运，不判断分区是谁、也不写任何条目 `key`：
//! 条目清单来自 `StorageSection::entries`，取数与判据来自 `StorageSection::stats` /
//! `StorageSection::policy` / `StorageSection::eligibility`（G17(b) / X15）。

use crate::cmd::storage::entries::EntrySample;
use crate::cmd::storage::section::{SectionError, SectionSample, StorageSectionHandle};

/// 单条分区条目的观测：取数失败按分区自述的类别降级，不填 `0` 冒充（G6）。
pub(super) fn section_sample(section: &StorageSectionHandle, key: &'static str) -> EntrySample {
    let observed = match section.stats(key) {
        Some(observed) => observed,
        // 清单由分区自己给，取不到自己声明的 key 属分区实现 bug：只降级该条，不 panic。
        None => {
            log::error!("分区未提供条目 {key} 的占用统计");
            Err(SectionError::Unreadable)
        }
    };
    let policy = section.policy(key);
    let mut sample = match observed {
        Ok(stats) => {
            EntrySample::observed(stats.bytes, stats.count, stats.oldest_at, stats.newest_at)
        }
        Err(error) => EntrySample::degraded(error.availability()),
    };
    let judged = SectionSample {
        observed,
        policy: policy.clone(),
    };
    if let Some((eligibility, reason)) = section.eligibility(key, &judged) {
        sample = sample.with_eligibility(eligibility, reason);
    }
    if let Some(policy) = policy {
        sample = sample.with_policy(policy);
    }
    sample
}
