//! CPU 差分缓存（I76 §2.5、§4.2）。
//!
//! 键是 `pid + startTime`：PID 复用会让同一个 PID 指向新进程，单用 PID 会把两个进程的
//! 累计时间相减，得到负值或虚假尖峰。缓存随实例走，由命令层用
//! `OnceLock<Mutex<CpuDiffCache>>` 持有，禁止挂到启动分层的资源表。

use std::collections::{HashMap, HashSet};

use crate::{CPU_CACHE_IDLE_TIMEOUT_MS, MIN_CPU_DELTA_MS};

#[derive(Debug, Clone, Copy, PartialEq)]
struct CpuEntry {
    cpu_time_ms: u64,
    /// 上一次被采纳读数的时刻（不是缓存创建时刻）。
    sampled_at: i64,
    percent: Option<f32>,
}

/// 单核百分比差分器。
#[derive(Debug, Default)]
pub struct CpuDiffCache {
    entries: HashMap<(u32, i64), CpuEntry>,
    last_call_at: Option<i64>,
}

impl CpuDiffCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 输入 `(pid, startTime, 累计 CPU 毫秒)`，输出同一键上的单核百分比。
    ///
    /// - 首拍（键未见）→ `None`，禁止填 `0`（I76 §4.2）
    /// - 与上次采纳读数间隔 `< MIN_CPU_DELTA_MS` → 沿用上一拍百分比，不重算
    /// - 间隔超过 `CPU_CACHE_IDLE_TIMEOUT_MS` → 整表丢弃（暂停后恢复不产生尖峰）
    /// - 本次快照里不存在的键 → 淘汰，避免 PID 复用时读到陈旧的 `startTime`
    pub fn sample(
        &mut self,
        sampled_at: i64,
        rows: &[(u32, i64, Option<u64>)],
    ) -> HashMap<(u32, i64), Option<f32>> {
        if let Some(last) = self.last_call_at {
            if sampled_at - last > CPU_CACHE_IDLE_TIMEOUT_MS {
                self.entries.clear();
            }
        }
        self.last_call_at = Some(sampled_at);

        let mut result = HashMap::with_capacity(rows.len());
        let mut alive: HashSet<(u32, i64)> = HashSet::with_capacity(rows.len());

        for &(pid, start_time, cpu_time_ms) in rows {
            let key = (pid, start_time);
            alive.insert(key);

            let Some(cpu_time_ms) = cpu_time_ms else {
                self.entries.remove(&key);
                result.insert(key, None);
                continue;
            };

            let percent = match self.entries.get(&key).copied() {
                None => {
                    self.entries.insert(
                        key,
                        CpuEntry {
                            cpu_time_ms,
                            sampled_at,
                            percent: None,
                        },
                    );
                    None
                }
                Some(previous) => {
                    let wall = sampled_at - previous.sampled_at;
                    if wall < MIN_CPU_DELTA_MS {
                        previous.percent
                    } else {
                        // 累计时间倒退只可能来自平台时钟异常，饱和到 0 而不是给负值。
                        let delta = cpu_time_ms.saturating_sub(previous.cpu_time_ms);
                        let percent = (delta as f64 / wall as f64 * 100.0) as f32;
                        self.entries.insert(
                            key,
                            CpuEntry {
                                cpu_time_ms,
                                sampled_at,
                                percent: Some(percent),
                            },
                        );
                        Some(percent)
                    }
                }
            };

            result.insert(key, percent);
        }

        self.entries.retain(|key, _| alive.contains(key));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V13：首拍全部 `null`，禁止填 `0`（I76 §10.2 V13）。
    #[test]
    fn v13_first_sample_is_null() {
        let mut cache = CpuDiffCache::new();
        let result = cache.sample(1_000, &[(100, 50, Some(1_200))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(None));
    }

    #[test]
    fn second_sample_diffs_over_wall_clock() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000))]);
        // 1000ms 内多用 500ms CPU → 50%
        let result = cache.sample(2_000, &[(100, 50, Some(1_500))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(Some(50.0)));
    }

    /// 超过单核 100% 也要如实给出（多线程进程）。
    #[test]
    fn percent_can_exceed_one_hundred() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(0))]);
        let result = cache.sample(2_000, &[(100, 50, Some(3_000))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(Some(300.0)));
    }

    /// 间隔不足最小窗口 → 沿用上一拍百分比，且不覆盖上次采纳读数。
    #[test]
    fn too_short_interval_reuses_previous_percent() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000))]);
        cache.sample(2_000, &[(100, 50, Some(1_500))]);

        let result = cache.sample(2_400, &[(100, 50, Some(1_900))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(Some(50.0)));

        // 从 2000ms 的读数继续差分：2000→3000 用了 1000ms 累计时间里的 500ms
        let result = cache.sample(3_000, &[(100, 50, Some(2_000))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(Some(50.0)));
    }

    /// PID 复用：`startTime` 变了就是新键，首拍仍是 `null`，不与旧进程相减。
    #[test]
    fn reused_pid_gets_fresh_key() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(9_000))]);
        let result = cache.sample(2_000, &[(100, 80, Some(100))]);
        assert_eq!(result.get(&(100, 80)).copied(), Some(None));
        assert!(!result.contains_key(&(100, 50)));
    }

    /// 长时间无采样 → 整表丢弃，恢复后首拍是 `null` 而不是尖峰。
    #[test]
    fn idle_timeout_drops_cache() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000))]);
        let result = cache.sample(
            1_000 + CPU_CACHE_IDLE_TIMEOUT_MS + 1,
            &[(100, 50, Some(1_200))],
        );
        assert_eq!(result.get(&(100, 50)).copied(), Some(None));
    }

    /// 消失的键被淘汰。
    #[test]
    fn missing_rows_are_evicted() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000)), (200, 60, Some(1_000))]);
        cache.sample(2_000, &[(100, 50, Some(1_500))]);
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key(&(100, 50)));
    }

    /// 累计时间倒退不给负值。
    #[test]
    fn backwards_cumulative_time_saturates() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000))]);
        let result = cache.sample(2_000, &[(100, 50, Some(500))]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(Some(0.0)));
    }

    /// 取不到累计时间的行输出 `null`，且不留陈旧缓存。
    #[test]
    fn unreadable_cpu_time_is_null() {
        let mut cache = CpuDiffCache::new();
        cache.sample(1_000, &[(100, 50, Some(1_000))]);
        let result = cache.sample(2_000, &[(100, 50, None)]);
        assert_eq!(result.get(&(100, 50)).copied(), Some(None));
        assert!(cache.entries.is_empty());
    }
}
