//! Windows 后端：`sysinfo` 一轮全量枚举（I76 §2.2、§2.4、§6.4）。
//!
//! 取数口径已由开发机探针实测定案（I76 §10.2 V27）：
//! `Process::virtual_memory()` 即 `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`，
//! 与 `memoryBasis = privateWorkingSet` / `memoryAdditive = true` 对应。

use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::source::{RawProcess, RawSnapshot, SnapshotSource};

pub struct WinSource {
    /// `System::new()` 是 `Result`；失败时保留 `None`，每次采样如实返回 `Err`，
    /// 由调用方转 `unreadable`，不伪造空快照。
    system: Option<System>,
}

impl WinSource {
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
        }
    }
}

impl Default for WinSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for WinSource {
    fn snapshot(&mut self) -> Result<RawSnapshot, String> {
        let system = self
            .system
            .as_mut()
            .ok_or_else(|| "system snapshot source unavailable".to_string())?;

        // 一轮全量枚举：内存、CPU、命令行一次取齐；不取磁盘/网络/用户/环境变量。
        // 命令行默认不刷新，必须显式 Always（I76 §2.3 第 ③ 层）。
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_cpu()
                .with_cmd(UpdateKind::Always)
                .without_tasks(),
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
                // 0 只可能来自取数失败（活进程私有工作集不会是 0 字节），按「读不到」处理。
                memory_bytes: nonzero(process.virtual_memory()),
                cpu_time_ms: Some(process.accumulated_cpu_time()),
                is_thread: process.thread_kind().is_some(),
            })
            .collect();

        Ok(RawSnapshot {
            sampled_at,
            processes,
            // Windows 拿不到「宿主线程数」的独立口径，交给建树层按线程条目计数。
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
