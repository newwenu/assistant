//! 超时包装：单条观测超过 `SAMPLE_TIMEOUT` 即降级，绝不阻塞整页（§3.10）。

use std::future::Future;
use std::pin::Pin;

use crate::protocol::dto::StorageAvailabilityDto;

use super::super::entries::EntrySample;
use super::SAMPLE_TIMEOUT;

pub(super) type SampleFuture = Pin<Box<dyn Future<Output = (&'static str, EntrySample)> + Send>>;

fn timeout_sample() -> EntrySample {
    EntrySample::degraded(StorageAvailabilityDto::Unavailable)
}

pub(super) async fn timed<F>(entry: &'static str, work: F) -> (&'static str, EntrySample)
where
    F: Future<Output = EntrySample>,
{
    match tokio::time::timeout(SAMPLE_TIMEOUT, work).await {
        Ok(sample) => (entry, sample),
        Err(_) => (entry, timeout_sample()),
    }
}

/// 阻塞型采样统一入口：`spawn_blocking` + 超时；超时后线程跑完才退出（不持锁即可）。
pub(super) async fn timed_blocking<F>(entry: &'static str, work: F) -> (&'static str, EntrySample)
where
    F: FnOnce() -> EntrySample + Send + 'static,
{
    let handle = tauri::async_runtime::spawn_blocking(work);
    match tokio::time::timeout(SAMPLE_TIMEOUT, handle).await {
        Ok(Ok(sample)) => (entry, sample),
        Ok(Err(error)) => {
            log::debug!("阻塞采样失败: {error}");
            (
                entry,
                EntrySample::degraded(StorageAvailabilityDto::Unreadable),
            )
        }
        Err(_) => (entry, timeout_sample()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_sample_is_unavailable_without_numbers() {
        let sample = timeout_sample();
        assert_eq!(sample.availability, StorageAvailabilityDto::Unavailable);
        assert_eq!(sample.bytes, None);
        assert_eq!(sample.count, None);
    }
}
