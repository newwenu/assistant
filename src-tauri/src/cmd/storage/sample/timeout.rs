//! 超时包装：单条观测超过预算即降级，绝不阻塞整页（I61 §3.10 + I79 §4.4）。
//!
//! 预算分两档：`Inline` 用 [`SAMPLE_TIMEOUT`]，超时即「声明说谎」——追加 `log::warn!`
//! 暴露登记表填错档或 owner 实现退化（INV10）；`Deferred` 的计划强制采样用
//! [`PLAN_SAMPLE_TIMEOUT`]，只是计划预算，不是档位声明。

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::protocol::dto::StorageAvailabilityDto;

use super::super::entries::EntrySample;
use super::{PLAN_SAMPLE_TIMEOUT, SAMPLE_TIMEOUT};

pub(super) type SampleFuture = Pin<Box<dyn Future<Output = (&'static str, EntrySample)> + Send>>;

/// 单条采样的超时预算（I79 §5.1）。
#[derive(Debug, Clone, Copy)]
pub(super) struct Budget {
    pub(super) timeout: Duration,
    /// `true` = 该条声明 `Inline`，超时即声明说谎探针（INV10）。
    pub(super) declared_inline: bool,
}

impl Budget {
    /// 页面档 `Inline`：预算 `SAMPLE_TIMEOUT`，超时要 warn。
    pub(super) fn inline() -> Self {
        Self {
            timeout: SAMPLE_TIMEOUT,
            declared_inline: true,
        }
    }

    /// 计划档 `Deferred` 强制现场采样：预算 `PLAN_SAMPLE_TIMEOUT`。
    pub(super) fn plan() -> Self {
        Self {
            timeout: PLAN_SAMPLE_TIMEOUT,
            declared_inline: false,
        }
    }

    /// 页面档 `Deferred` 的兜底（统计缓存未装配，正常启动不会发生）：
    /// 仍按页面预算，但不是档位声明，超时不 warn。
    pub(super) fn deferred_page() -> Self {
        Self {
            timeout: SAMPLE_TIMEOUT,
            declared_inline: false,
        }
    }
}

fn timeout_sample(entry: &'static str, budget: Budget) -> EntrySample {
    if budget.declared_inline {
        log::warn!(
            "条目 {entry} 声明 Inline 但观测超过 {:?}：改档或修 owner，不要调大超时",
            budget.timeout
        );
    }
    EntrySample::degraded(StorageAvailabilityDto::Unavailable)
}

pub(super) async fn timed<F>(
    entry: &'static str,
    budget: Budget,
    work: F,
) -> (&'static str, EntrySample)
where
    F: Future<Output = EntrySample>,
{
    match tokio::time::timeout(budget.timeout, work).await {
        Ok(sample) => (entry, sample),
        Err(_) => (entry, timeout_sample(entry, budget)),
    }
}

/// 阻塞型采样统一入口：`spawn_blocking` + 超时；超时后线程跑完才退出（不持锁即可）。
pub(super) async fn timed_blocking<F>(
    entry: &'static str,
    budget: Budget,
    work: F,
) -> (&'static str, EntrySample)
where
    F: FnOnce() -> EntrySample + Send + 'static,
{
    let handle = tauri::async_runtime::spawn_blocking(work);
    match tokio::time::timeout(budget.timeout, handle).await {
        Ok(Ok(sample)) => (entry, sample),
        Ok(Err(error)) => {
            log::debug!("阻塞采样失败: {error}");
            (
                entry,
                EntrySample::degraded(StorageAvailabilityDto::Unreadable),
            )
        }
        Err(_) => (entry, timeout_sample(entry, budget)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_sample_is_unavailable_without_numbers() {
        let sample = timeout_sample("E2", Budget::plan());
        assert_eq!(sample.availability, StorageAvailabilityDto::Unavailable);
        assert_eq!(sample.bytes, None);
        assert_eq!(sample.count, None);
    }

    #[test]
    fn budgets_follow_stats_mode() {
        assert_eq!(Budget::inline().timeout, SAMPLE_TIMEOUT);
        assert!(Budget::inline().declared_inline);
        assert_eq!(Budget::plan().timeout, PLAN_SAMPLE_TIMEOUT);
        assert!(!Budget::plan().declared_inline);
        assert_eq!(Budget::deferred_page().timeout, SAMPLE_TIMEOUT);
        assert!(!Budget::deferred_page().declared_inline);
    }
}
