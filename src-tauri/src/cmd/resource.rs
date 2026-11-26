use tauri::command;

use crate::protocol::dto::{
    CoverageDto, CpuBasisDto, MemoryBasisDto, ProcessGroupDto, ProcessSnapshotDto,
    ResourceAvailabilityDto, ResourceCapabilitiesDto, ResourceTabCapabilityDto, ResourceTabDto,
};

const PROCESS_TREE_BACKEND_PENDING: &str =
    "process tree sampling backend is not wired on this platform yet";

const NO_MEMORY_WITHOUT_SAMPLING: &str = "no process memory data without a sampling backend";

/// 只返回各 tab 的可用性与原因，不含任何数字（I76 §6.1、§10.2 V5）。
#[command]
pub fn get_resource_capabilities() -> ResourceCapabilitiesDto {
    ResourceCapabilitiesDto {
        tabs: vec![
            ResourceTabCapabilityDto {
                tab: ResourceTabDto::Storage,
                availability: ResourceAvailabilityDto::Available,
                reason: None,
            },
            ResourceTabCapabilityDto {
                tab: ResourceTabDto::ProcessTree,
                availability: ResourceAvailabilityDto::NotImplemented,
                reason: Some(PROCESS_TREE_BACKEND_PENDING.to_string()),
            },
        ],
    }
}

/// 一次采样的全部结果；取数失败不抛命令错误，返回顶层 `unreadable` + `reason`（I76 §6.1）。
///
/// R0 骨架：采样后端随 R1a 接线，此处只回 `notImplemented`。口径字段取「无合计数字」的组合，
/// 即便被误渲染也不会给出一个错误的可加数字；R1a 起改由采样 crate 按平台常量给出（I76 §2.4）。
#[command]
pub fn get_process_snapshot() -> ProcessSnapshotDto {
    ProcessSnapshotDto {
        availability: ResourceAvailabilityDto::NotImplemented,
        reason: Some(PROCESS_TREE_BACKEND_PENDING.to_string()),
        sampled_at: None,
        root_pid: std::process::id(),
        memory_basis: MemoryBasisDto::WorkingSet,
        memory_additive: false,
        memory_equals_footprint: false,
        cpu_basis: CpuBasisDto::SingleCorePercent,
        coverage: CoverageDto::SubtreeOnly,
        webview_detached: false,
        truncated: false,
        host_thread_count: None,
        self_group: unavailable_group(),
        subtree_group: unavailable_group(),
        entries: Vec::new(),
    }
}

fn unavailable_group() -> ProcessGroupDto {
    ProcessGroupDto {
        pid_count: 0,
        cpu_percent: None,
        memory_bytes: None,
        memory_availability: ResourceAvailabilityDto::Unavailable,
        memory_reason: Some(NO_MEMORY_WITHOUT_SAMPLING.to_string()),
    }
}
