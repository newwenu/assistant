//! macOS 后端：`sysinfo` 枚举进程与命令行，内存与宿主线程数走 `libc`（I76 §2.2、§2.4、§6.4）。
//!
//! 两处必须绕开 `sysinfo`：
//! - 逐行内存取 `proc_pid_rusage(RUSAGE_INFO_V2)` 的 `ri_phys_footprint`；
//!   `sysinfo` 的 `memory()` 是 `PROC_PIDTASKINFO.pti_resident_size`（RSS），属工作集口径，
//!   **不可用**（I76 §2.4 取数约束 2）。
//! - 宿主线程数取 `proc_pidinfo(PROC_PIDTASKINFO)` 的 `pti_threadnum`；macOS 的 `tasks()`
//!   恒 `None`，故这是对单个 PID 的定向读取，不构成第二次全系统枚举（I76 §5.3）。

use std::mem::MaybeUninit;
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::source::{RawProcess, RawSnapshot, SnapshotSource};

pub struct MacosSource {
    /// `System::new()` 是 `Result`；失败时保留 `None`，每次采样如实返回 `Err`，
    /// 由调用方转 `unreadable`，不伪造空快照。
    system: Option<System>,
}

impl MacosSource {
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
        }
    }
}

impl Default for MacosSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for MacosSource {
    fn snapshot(&mut self) -> Result<RawSnapshot, String> {
        let system = self
            .system
            .as_mut()
            .ok_or_else(|| "system snapshot source unavailable".to_string())?;

        // 一轮全量枚举：CPU 与命令行一次取齐。内存不经 `sysinfo`（见模块注释），
        // 也不取 `tasks`（macOS 恒 `None`，开着只是白做，I76 §2.3）。
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
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
                // 镜像名取基础枚举值（`exe()` 的文件名），家族判定用
                // `com.apple.WebKit.*` 全名（I76 §2.3 第 ② 层）。
                exe_name: process.name().to_string_lossy().into_owned(),
                cmd: process
                    .cmd()
                    .iter()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect(),
                memory_bytes: phys_footprint(pid.as_u32()),
                cpu_time_ms: Some(process.accumulated_cpu_time()),
                is_thread: process.thread_kind().is_some(),
            })
            .collect();

        Ok(RawSnapshot {
            sampled_at,
            processes,
            host_thread_count: None,
        })
    }

    fn host_thread_count(&self, root_pid: u32) -> Option<u32> {
        host_threads(root_pid)
    }
}

/// `ri_phys_footprint`：与 `memoryBasis = physFootprint` / `memoryEqualsFootprint = true` 对应。
///
/// 取不到（权限不足、进程已退出）返回 `None`，**禁止**退回 `sysinfo` 的 RSS——
/// 那会把工作集口径混进「物理占用」列（I76 §2.4）。
fn phys_footprint(pid: u32) -> Option<u64> {
    // SAFETY: `proc_pid_rusage` 只写 `rusage_info_v2` 缓冲区；指针在调用期间有效，
    // 且仅在返回 0 时读取。
    unsafe {
        let mut info = MaybeUninit::<libc::rusage_info_v2>::zeroed();
        let mut buffer: libc::rusage_info_t = info.as_mut_ptr().cast();
        let result = libc::proc_pid_rusage(pid as libc::c_int, libc::RUSAGE_INFO_V2, &mut buffer);
        if result != 0 {
            return None;
        }
        let info = info.assume_init();
        (info.ri_phys_footprint > 0).then_some(info.ri_phys_footprint)
    }
}

/// 宿主线程数：`proc_pidinfo(PROC_PIDTASKINFO)` 的 `pti_threadnum`。
fn host_threads(pid: u32) -> Option<u32> {
    // SAFETY: 同上，`proc_taskinfo` 全为整数成员，零初始化的读取是安全的；
    // 仅在返回值等于结构体大小时读取。
    unsafe {
        let mut info = MaybeUninit::<libc::proc_taskinfo>::zeroed();
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let result = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if result != size {
            return None;
        }
        let info = info.assume_init();
        // 0 视为取不到，禁止填 0 冒充（I76 §5.3）。
        (info.pti_threadnum > 0).then_some(info.pti_threadnum as u32)
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}
