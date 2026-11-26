use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 页内 tab 身份。`tabs` 恒含这两项且 `tab` 不重复（I76 §6.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResourceTabDto {
    Storage,
    ProcessTree,
}

/// 可用性封闭词表（I76 §2.7）：四个取值全部被使用。
/// 与存储侧 `StorageAvailabilityDto` 同形不同集，不复用、不互传值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResourceAvailabilityDto {
    Available,
    Unavailable,
    NotImplemented,
    Unreadable,
}

/// 进程角色（I76 §2.3）：决定该行归入自身合计还是全树合计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProcessRoleDto {
    Host,
    WebviewBrowser,
    WebviewRenderer,
    WebviewGpu,
    WebviewUtility,
    WebviewUnknown,
    OtherChild,
}

/// 逐行内存口径（I76 §2.4）：前端据此决定列名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MemoryBasisDto {
    WorkingSet,
    PrivateWorkingSet,
    PhysFootprint,
}

/// CPU 口径：单逻辑核百分比，可超过 100（I76 §2.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CpuBasisDto {
    SingleCorePercent,
}

/// 覆盖范围：只覆盖本进程子树，不是采样失败（I76 §2.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CoverageDto {
    SubtreeOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCapabilitiesDto {
    /// 平铺数组而非 map：前端按 `tab` 取值遍历，不猜键名。
    pub tabs: Vec<ResourceTabCapabilityDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTabCapabilityDto {
    pub tab: ResourceTabDto,
    pub availability: ResourceAvailabilityDto,
    /// 键恒在；`available` 时为 `null`。
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshotDto {
    /// 顶层状态；非 `available` 时 `entries` 为空、两个组数字为 `null`。
    pub availability: ResourceAvailabilityDto,
    pub reason: Option<String>,
    /// 一次遍历一个时刻；非 `available` 时为 `null`（不填 `0`）。
    pub sampled_at: Option<i64>,
    /// 采样根 = 宿主 PID，由命令层注入。
    pub root_pid: u32,
    pub memory_basis: MemoryBasisDto,
    /// 内存指标是否可求和（I76 §2.4）。
    pub memory_additive: bool,
    /// 内存合计是否≈物理占用（I76 §2.4）；与 `memory_additive` 相互独立，禁止合并。
    pub memory_equals_footprint: bool,
    pub cpu_basis: CpuBasisDto,
    pub coverage: CoverageDto,
    /// 本平台 WebView 进程由系统托管、不属本应用子树（本期仅 macOS 为 `true`）。
    pub webview_detached: bool,
    pub truncated: bool,
    /// 仅宿主；取不到为 `null`，禁止填 `0`（I76 §5.3）。
    pub host_thread_count: Option<u32>,
    pub self_group: ProcessGroupDto,
    pub subtree_group: ProcessGroupDto,
    /// 树序、一行一进程、`pid` 不重复。
    pub entries: Vec<ProcessEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroupDto {
    pub pid_count: u32,
    /// 按 `cpu_basis` 求和；首拍 `null`，禁止填 `0`。
    pub cpu_percent: Option<f32>,
    /// 按 `memory_basis` 求和；取不到为 `null`，禁止填 `0`。
    pub memory_bytes: Option<u64>,
    /// 只取 `available` / `unavailable`。
    pub memory_availability: ResourceAvailabilityDto,
    /// `unavailable` 时必须非空。
    pub memory_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEntryDto {
    pub pid: u32,
    /// 原值；前端不据此重建树（树序由后端出）。
    pub parent_pid: u32,
    pub role: ProcessRoleDto,
    /// 开放集合，前端兜底不窄化；不含命令行全文。
    pub exe_name: String,
    pub cpu_percent: Option<f32>,
    pub memory_bytes: Option<u64>,
    /// 进程创建时刻；PID 复用校验与建父子链的必需字段，必须随附。
    pub start_time: i64,
}
