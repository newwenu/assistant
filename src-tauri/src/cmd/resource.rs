use std::sync::{Mutex, OnceLock};

use assistant_resource_probe::{
    build_snapshot, capabilities, default_source, CpuDiffCache, FailingSource,
};
use tauri::command;

use crate::protocol::dto::{ProcessSnapshotDto, ResourceCapabilitiesDto};

/// CPU 差分缓存必须跨拍存活，且只活在本特性自己的静态里（I76 §4.2）：
/// 挂到启动分层的资源表会让暂停/重启用共享生命周期污染采样窗口。
static CPU_CACHE: OnceLock<Mutex<CpuDiffCache>> = OnceLock::new();

/// 只返回各 tab 的可用性与原因，不含任何数字（I76 §6.1、§10.2 V5）。
#[command]
pub fn get_resource_capabilities() -> ResourceCapabilitiesDto {
    capabilities()
}

/// 一次采样的全部结果；取数失败不抛命令错误，返回顶层 `unreadable` + `reason`（I76 §6.1）。
///
/// 枚举是阻塞调用（Windows 一轮全量约 10ms 量级），放到阻塞线程池执行，不占异步运行时的核。
/// 根 PID 由命令层注入为宿主 PID，采样 crate 不自己猜（I76 §6.1）。
#[command]
pub async fn get_process_snapshot() -> ProcessSnapshotDto {
    let root_pid = std::process::id();
    tauri::async_runtime::spawn_blocking(move || sample(root_pid))
        .await
        .unwrap_or_else(|error| {
            // 采样任务异常（panic 等）并入同一条降级路径，仍返回结构化快照。
            let mut source = FailingSource::new(&format!("sampling task failed: {error}"));
            build_snapshot(root_pid, &mut source, &mut CpuDiffCache::new())
        })
}

fn sample(root_pid: u32) -> ProcessSnapshotDto {
    let mut source = default_source();
    let cache = CPU_CACHE.get_or_init(|| Mutex::new(CpuDiffCache::new()));
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        // 上一拍 panic 留下的毒锁仍可继续用：缓存只是数值，不影响正确性。
        Err(poisoned) => poisoned.into_inner(),
    };
    build_snapshot(root_pid, source.as_mut(), &mut guard)
}
