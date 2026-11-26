//! Linux 后端：`sysinfo` 一轮全量枚举（I76 §2.2、§2.4、§6.4）。
//!
//! 与 Windows 的两处差异：
//! - 逐行内存取 `memory()`（`/proc/<pid>/statm` 的 resident）→ `memoryBasis = workingSet`、
//!   `memoryAdditive = false`（共享页重复计数，求和不可当总量）；
//! - **必须保留 `with_tasks`**：线程条目会作为独立进程条目并入同一次快照，由 `tree.rs`
//!   建父子表前过滤，被过滤的条目数即宿主线程数（I76 §2.2「线程条目」、§5.3）。

use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::source::{RawProcess, RawSnapshot, SnapshotSource};

pub struct LinuxSource {
    /// `System::new()` 是 `Result`；失败时保留 `None`，每次采样如实返回 `Err`，
    /// 由调用方转 `unreadable`，不伪造空快照。
    system: Option<System>,
}

impl LinuxSource {
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
        }
    }
}

impl Default for LinuxSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for LinuxSource {
    fn snapshot(&mut self) -> Result<RawSnapshot, String> {
        let system = self
            .system
            .as_mut()
            .ok_or_else(|| "system snapshot source unavailable".to_string())?;

        // 一轮全量枚举：内存、CPU、命令行一次取齐；`with_tasks` 是宿主线程数的唯一来源，
        // 关掉后 `tasks()` 恒 `None`（I76 §2.3 刷新项收窄）。
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_cpu()
                .with_cmd(UpdateKind::Always)
                .with_tasks(),
        );

        let sampled_at = now_millis();
        let processes = system
            .processes()
            .iter()
            .map(|(pid, process)| RawProcess {
                pid: pid.as_u32(),
                parent_pid: process.parent().map(Pid::as_u32).unwrap_or(0),
                start_time: process.start_time() as i64,
                // 镜像名取基础枚举值；`exe()` 在精简刷新下恒 `None`（I76 §2.3 第 ② 层）。
                exe_name: process.name().to_string_lossy().into_owned(),
                cmd: process
                    .cmd()
                    .iter()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
                // 0 只可能来自取数失败（活进程常驻集不会是 0 字节），按「读不到」处理。
                memory_bytes: nonzero(process.memory()),
                cpu_time_ms: Some(process.accumulated_cpu_time()),
                // `thread_kind()` 非空即 `/proc/<pid>/task/*` 条目，建父子表前必须过滤。
                is_thread: process.thread_kind().is_some(),
            })
            .collect();

        Ok(RawSnapshot {
            sampled_at,
            processes,
            // 宿主线程数由建树层按被过滤的线程条目计数（等于 `tasks()` 长度）。
            host_thread_count: None,
        })
    }
}

fn nonzero(bytes: u64) -> Option<u64> {
    (bytes > 0).then_some(bytes)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}
