//! 进程资源采样（I76）。
//!
//! 分层：平台后端只负责一轮全系统枚举 → `RawSnapshot`（`win.rs` / 后续 `linux.rs`、
//! `macos.rs`），建树、角色判定、CPU 差分、合计全部是纯逻辑，可在任意平台用假快照测试。
//! 一次快照 = 一次枚举 + 一个时刻（I76 §2.2 约束 2）。

use assistant_protocol::{
    CoverageDto, CpuBasisDto, MemoryBasisDto, ProcessEntryDto, ProcessGroupDto, ProcessSnapshotDto,
    ResourceAvailabilityDto, ResourceCapabilitiesDto, ResourceTabCapabilityDto, ResourceTabDto,
};

pub mod classify;
pub mod cpu;
pub mod mock;
pub mod source;
pub mod tree;

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

pub use classify::PlatformProfile;
pub use cpu::CpuDiffCache;
pub use mock::MockSource;
pub use source::{FailingSource, RawProcess, RawSnapshot, SnapshotSource};

/// 前端轮询间隔；同时是 CPU 差分的最小可信窗口上限（I76 §2.5）。
pub const SAMPLE_INTERVAL_MS: u64 = 1000;
/// 超过此间隔无采样则整表丢弃，暂停后恢复不产生尖峰（I76 §4.2）。
pub const CPU_CACHE_IDLE_TIMEOUT_MS: i64 = 3000;
/// 同一进程两次读数的最小间隔，不足则沿用上一拍百分比（I76 §4.2）。
pub const MIN_CPU_DELTA_MS: i64 = 500;
/// 单次快照的条目上限，超出置 `truncated`（I76 §5.3）。
pub const MAX_ENTRIES: usize = 512;
/// 树深度上限，超出置 `truncated`（I76 §5.3）。
pub const MAX_DEPTH: usize = 16;

const NOT_IMPLEMENTED_REASON: &str =
    "process tree sampling backend is not implemented on this platform yet";
const ROOT_MISSING_REASON: &str = "host process is not present in the snapshot";
const NO_ADDITIVE_MEMORY_REASON: &str = "this platform has no additive memory caliber";
const PARTIAL_MEMORY_REASON: &str = "some processes in this group have unreadable memory";

#[cfg(target_os = "windows")]
const PLATFORM_PROFILE: Option<PlatformProfile> = Some(PlatformProfile {
    // `virtual_memory()` == PROCESS_MEMORY_COUNTERS_EX.PrivateUsage（I76 §10.2 V27 已实测）
    memory_basis: MemoryBasisDto::PrivateWorkingSet,
    memory_additive: true,
    memory_equals_footprint: false,
    webview_detached: false,
    webview_families: &["msedgewebview2.exe", "msedge.exe", "chrome.exe"],
});

#[cfg(target_os = "linux")]
const PLATFORM_PROFILE: Option<PlatformProfile> = Some(PlatformProfile {
    // `memory()` 是 `/proc/<pid>/statm` 的 resident，属工作集口径：
    // 共享页逐进程各记一次，求和必然大于物理占用（I76 §2.4）
    memory_basis: MemoryBasisDto::WorkingSet,
    memory_additive: false,
    memory_equals_footprint: false,
    webview_detached: false,
    webview_families: &["WebKitWebProcess", "WebKitNetworkProcess"],
});

#[cfg(target_os = "macos")]
const PLATFORM_PROFILE: Option<PlatformProfile> = Some(PlatformProfile {
    // 逐行取 `proc_pid_rusage` 的 `ri_phys_footprint`，不是 `sysinfo` 的 RSS（I76 §2.4）
    memory_basis: MemoryBasisDto::PhysFootprint,
    memory_additive: true,
    memory_equals_footprint: true,
    // WKWebView 内容进程由 launchd / XPC 托管，通常不在宿主子树内（I76 §2.3）
    webview_detached: true,
    webview_families: &[
        "com.apple.WebKit.WebContent",
        "com.apple.WebKit.Networking",
        "com.apple.WebKit.GPU",
    ],
});

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const PLATFORM_PROFILE: Option<PlatformProfile> = None;

/// 未支持平台的占位口径：没有任何数字，取「不可求和」的组合最安全（I76 §2.4）。
const UNSUPPORTED_PROFILE: PlatformProfile = PlatformProfile {
    memory_basis: MemoryBasisDto::WorkingSet,
    memory_additive: false,
    memory_equals_footprint: false,
    webview_detached: false,
    webview_families: &[],
};

fn platform_profile() -> Option<PlatformProfile> {
    PLATFORM_PROFILE
}

/// 各 tab 的可用性与原因，不含任何数字（I76 §6.1、§10.2 V5）。
pub fn capabilities() -> ResourceCapabilitiesDto {
    ResourceCapabilitiesDto {
        tabs: vec![
            ResourceTabCapabilityDto {
                tab: ResourceTabDto::Storage,
                availability: ResourceAvailabilityDto::Available,
                reason: None,
            },
            process_tree_capability(),
        ],
    }
}

fn process_tree_capability() -> ResourceTabCapabilityDto {
    match platform_profile() {
        Some(_) => ResourceTabCapabilityDto {
            tab: ResourceTabDto::ProcessTree,
            availability: ResourceAvailabilityDto::Available,
            reason: None,
        },
        None => ResourceTabCapabilityDto {
            tab: ResourceTabDto::ProcessTree,
            availability: ResourceAvailabilityDto::NotImplemented,
            reason: Some(NOT_IMPLEMENTED_REASON.to_string()),
        },
    }
}

/// 当前平台的采样来源。未实现平台返回恒失败的来源，调用方无需区分。
pub fn default_source() -> Box<dyn SnapshotSource> {
    #[cfg(target_os = "windows")]
    {
        Box::new(win::WinSource::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxSource::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosSource::new())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Box::new(FailingSource::new(NOT_IMPLEMENTED_REASON))
    }
}

/// 一次采样：调用方负责每拍新建或复用来源，并把 `cpu_cache` 跨拍保留（I76 §6.1）。
pub fn build_snapshot(
    root_pid: u32,
    source: &mut dyn SnapshotSource,
    cpu_cache: &mut CpuDiffCache,
) -> ProcessSnapshotDto {
    match platform_profile() {
        Some(profile) => build_with_profile(root_pid, source, cpu_cache, profile),
        None => unavailable_snapshot(
            root_pid,
            ResourceAvailabilityDto::NotImplemented,
            NOT_IMPLEMENTED_REASON,
            UNSUPPORTED_PROFILE,
        ),
    }
}

fn build_with_profile(
    root_pid: u32,
    source: &mut dyn SnapshotSource,
    cpu_cache: &mut CpuDiffCache,
    profile: PlatformProfile,
) -> ProcessSnapshotDto {
    let raw = match source.snapshot() {
        Ok(raw) => raw,
        Err(reason) => {
            return unavailable_snapshot(
                root_pid,
                ResourceAvailabilityDto::Unreadable,
                &reason,
                profile,
            )
        }
    };

    let Some(scan) = tree::collect(root_pid, &raw) else {
        return unavailable_snapshot(
            root_pid,
            ResourceAvailabilityDto::Unreadable,
            ROOT_MISSING_REASON,
            profile,
        );
    };

    let cpu_rows: Vec<(u32, i64, Option<u64>)> = scan
        .rows
        .iter()
        .map(|process| (process.pid, process.start_time, process.cpu_time_ms))
        .collect();
    let cpu_percent = cpu_cache.sample(raw.sampled_at, &cpu_rows);

    // 平台定向取数优先（macOS `proc_pidinfo`），否则用建树层按被过滤线程条目的计数（Linux）。
    let host_thread_count = source
        .host_thread_count(root_pid)
        .or(scan.host_thread_count);

    let mut self_group = GroupAcc::default();
    let mut subtree_group = GroupAcc::default();
    let mut entries = Vec::with_capacity(scan.rows.len());

    for process in &scan.rows {
        let role = classify::role(process, root_pid, profile.webview_families);
        let cpu = cpu_percent
            .get(&(process.pid, process.start_time))
            .copied()
            .flatten();

        subtree_group.push(cpu, process.memory_bytes);
        if classify::is_self_role(role) {
            self_group.push(cpu, process.memory_bytes);
        }

        entries.push(ProcessEntryDto {
            pid: process.pid,
            parent_pid: process.parent_pid,
            role,
            exe_name: process.exe_name.clone(),
            cpu_percent: cpu,
            memory_bytes: process.memory_bytes,
            start_time: process.start_time,
        });
    }

    ProcessSnapshotDto {
        availability: ResourceAvailabilityDto::Available,
        reason: None,
        sampled_at: Some(raw.sampled_at),
        root_pid,
        memory_basis: profile.memory_basis,
        memory_additive: profile.memory_additive,
        memory_equals_footprint: profile.memory_equals_footprint,
        cpu_basis: CpuBasisDto::SingleCorePercent,
        coverage: CoverageDto::SubtreeOnly,
        webview_detached: profile.webview_detached,
        truncated: scan.truncated,
        host_thread_count,
        self_group: self_group.finish(&profile),
        subtree_group: subtree_group.finish(&profile),
        entries,
    }
}

fn unavailable_snapshot(
    root_pid: u32,
    availability: ResourceAvailabilityDto,
    reason: &str,
    profile: PlatformProfile,
) -> ProcessSnapshotDto {
    ProcessSnapshotDto {
        availability,
        reason: Some(reason.to_string()),
        sampled_at: None,
        root_pid,
        memory_basis: profile.memory_basis,
        memory_additive: profile.memory_additive,
        memory_equals_footprint: profile.memory_equals_footprint,
        cpu_basis: CpuBasisDto::SingleCorePercent,
        coverage: CoverageDto::SubtreeOnly,
        webview_detached: profile.webview_detached,
        truncated: false,
        host_thread_count: None,
        self_group: unavailable_group(reason),
        subtree_group: unavailable_group(reason),
        entries: Vec::new(),
    }
}

fn unavailable_group(reason: &str) -> ProcessGroupDto {
    ProcessGroupDto {
        pid_count: 0,
        cpu_percent: None,
        memory_bytes: None,
        memory_availability: ResourceAvailabilityDto::Unavailable,
        memory_reason: Some(reason.to_string()),
    }
}

#[derive(Default)]
struct GroupAcc {
    pid_count: u32,
    cpu_sum: f32,
    cpu_any: bool,
    memory_sum: u64,
    memory_missing: bool,
}

impl GroupAcc {
    fn push(&mut self, cpu: Option<f32>, memory: Option<u64>) {
        self.pid_count += 1;
        if let Some(percent) = cpu {
            self.cpu_sum += percent;
            self.cpu_any = true;
        }
        if let Some(bytes) = memory {
            self.memory_sum += bytes;
        } else {
            self.memory_missing = true;
        }
    }

    /// 内存合计受 `memory_additive` 与「组内是否有不可读行」双重约束：
    /// 任一条件不满足就不出数字，并把原因写进 `memory_reason`（I76 §2.4、§5.4）。
    /// CPU 合计不受此约束（I76 §5.2 约束 4）。
    fn finish(&self, profile: &PlatformProfile) -> ProcessGroupDto {
        let (memory_bytes, memory_availability, memory_reason) = if !profile.memory_additive {
            (
                None,
                ResourceAvailabilityDto::Unavailable,
                Some(NO_ADDITIVE_MEMORY_REASON.to_string()),
            )
        } else if self.memory_missing {
            (
                None,
                ResourceAvailabilityDto::Unavailable,
                Some(PARTIAL_MEMORY_REASON.to_string()),
            )
        } else {
            (
                Some(self.memory_sum),
                ResourceAvailabilityDto::Available,
                None,
            )
        };

        ProcessGroupDto {
            pid_count: self.pid_count,
            cpu_percent: self.cpu_any.then_some(self.cpu_sum),
            memory_bytes,
            memory_availability,
            memory_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_protocol::ProcessRoleDto;

    const ROOT: u32 = 100;

    fn fixture() -> RawSnapshot {
        RawSnapshot {
            sampled_at: 1_000,
            processes: vec![
                RawProcess::new(ROOT, 1, 50, "assistant-app.exe")
                    .with_memory(100)
                    .with_cpu_time(1_000),
                RawProcess::new(200, ROOT, 60, "msedgewebview2.exe")
                    .with_cmd(&["msedgewebview2.exe", "--type=renderer"])
                    .with_memory(200)
                    .with_cpu_time(500),
                RawProcess::new(210, 200, 70, "msedgewebview2.exe")
                    .with_cmd(&["msedgewebview2.exe", "--type=gpu-process"])
                    .with_memory(300)
                    .with_cpu_time(200),
                RawProcess::new(300, ROOT, 60, "node.exe")
                    .with_cmd(&["node.exe"])
                    .with_memory(50)
                    .with_cpu_time(100),
            ],
            host_thread_count: None,
        }
    }

    fn tab(
        capabilities: &ResourceCapabilitiesDto,
        target: ResourceTabDto,
    ) -> ResourceTabCapabilityDto {
        capabilities
            .tabs
            .iter()
            .find(|capability| capability.tab == target)
            .cloned()
            .expect("tab present")
    }

    /// V5：能力声明只有可用性与原因，不含任何数字（I76 §10.2 V5）。
    #[test]
    fn v5_capabilities_carry_no_numbers() {
        let capabilities = capabilities();
        assert_eq!(capabilities.tabs.len(), 2);

        let storage = tab(&capabilities, ResourceTabDto::Storage);
        assert_eq!(storage.availability, ResourceAvailabilityDto::Available);
        assert!(storage.reason.is_none());

        let process_tree = tab(&capabilities, ResourceTabDto::ProcessTree);
        match platform_profile() {
            Some(_) => {
                assert_eq!(
                    process_tree.availability,
                    ResourceAvailabilityDto::Available
                );
                assert!(process_tree.reason.is_none());
            }
            None => {
                assert_eq!(
                    process_tree.availability,
                    ResourceAvailabilityDto::NotImplemented
                );
                assert!(process_tree.reason.is_some_and(|reason| !reason.is_empty()));
            }
        }
    }

    /// V6：两个 tab 恒在且 `tab` 不重复（I76 §10.2 V6）。
    #[test]
    fn v6_tabs_are_unique_and_complete() {
        let capabilities = capabilities();
        let mut tabs: Vec<ResourceTabDto> = capabilities.tabs.iter().map(|item| item.tab).collect();
        tabs.sort_by_key(|item| match item {
            ResourceTabDto::Storage => 0,
            ResourceTabDto::ProcessTree => 1,
        });
        assert_eq!(
            tabs,
            vec![ResourceTabDto::Storage, ResourceTabDto::ProcessTree]
        );
    }

    /// V7：一次快照只有一个时刻，所有行同拍（I76 §10.2 V7）。
    #[test]
    fn v7_single_sampled_at_for_all_rows() {
        let raw = fixture();
        let mut source = MockSource::new(raw.clone());
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());

        assert_eq!(snapshot.availability, ResourceAvailabilityDto::Available);
        assert_eq!(snapshot.sampled_at, Some(raw.sampled_at));
        assert_eq!(snapshot.root_pid, ROOT);
        assert_eq!(snapshot.entries.len(), raw.processes.len());
        for (entry, process) in snapshot.entries.iter().zip(raw.processes.iter()) {
            assert_eq!(entry.pid, process.pid);
            assert_eq!(entry.start_time, process.start_time);
        }
    }

    /// V8 的端到端形态：树序父在子前、根在最前（I76 §10.2 V8）。
    #[test]
    fn v8_entries_are_preorder() {
        let mut source = MockSource::new(fixture());
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());
        let pids: Vec<u32> = snapshot.entries.iter().map(|entry| entry.pid).collect();
        assert_eq!(pids, vec![ROOT, 200, 210, 300]);
        assert_eq!(snapshot.entries[0].role, ProcessRoleDto::Host);
    }

    /// V12：每拍只枚举一次，不按字段/子树分次枚举（I76 §10.2 V12）。
    #[test]
    fn v12_single_enumeration_per_snapshot() {
        let mut source = MockSource::new(fixture());
        let mut cache = CpuDiffCache::new();
        let _ = build_with_profile(ROOT, &mut source, &mut cache, test_profile());
        assert_eq!(source.calls(), 1);
    }

    /// 取数失败 → 顶层 `unreadable`，`entries` 为空、组数字全 `null`，不抛错（I76 §6.1）。
    #[test]
    fn failing_source_yields_unreadable() {
        let mut source = MockSource::failing("refresh failed");
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());

        assert_eq!(snapshot.availability, ResourceAvailabilityDto::Unreadable);
        assert_eq!(snapshot.reason.as_deref(), Some("refresh failed"));
        assert!(snapshot.sampled_at.is_none());
        assert!(snapshot.entries.is_empty());
        assert!(snapshot.self_group.memory_bytes.is_none());
        assert!(snapshot.subtree_group.cpu_percent.is_none());
        assert!(snapshot.subtree_group.memory_reason.is_some());
    }

    /// 根进程不在快照 → `unreadable`（宿主已退出，不给空树）。
    #[test]
    fn missing_root_yields_unreadable() {
        let mut raw = fixture();
        raw.processes.retain(|process| process.pid != ROOT);
        let mut source = MockSource::new(raw);
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());
        assert_eq!(snapshot.availability, ResourceAvailabilityDto::Unreadable);
        assert_eq!(snapshot.reason.as_deref(), Some(ROOT_MISSING_REASON));
    }

    /// 合计口径：自身组只含宿主与 WebView，全树组含全部（I76 §2.3、§5.4）。
    #[test]
    fn groups_split_self_and_subtree() {
        let mut source = MockSource::new(fixture());
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());

        assert_eq!(snapshot.self_group.pid_count, 3);
        assert_eq!(snapshot.self_group.memory_bytes, Some(600));
        assert_eq!(snapshot.subtree_group.pid_count, 4);
        assert_eq!(snapshot.subtree_group.memory_bytes, Some(650));
        // 首拍 CPU 必然为 null，禁止填 0。
        assert!(snapshot.self_group.cpu_percent.is_none());
        assert!(snapshot.subtree_group.cpu_percent.is_none());
    }

    /// `memoryAdditive == false`：合计位无数字，逐行照常（I76 §5.4、V14）。
    #[test]
    fn non_additive_platform_hides_group_memory() {
        let mut source = MockSource::new(fixture());
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, non_additive_profile());

        assert!(!snapshot.memory_additive);
        assert!(snapshot.subtree_group.memory_bytes.is_none());
        assert_eq!(
            snapshot.subtree_group.memory_availability,
            ResourceAvailabilityDto::Unavailable
        );
        assert!(snapshot.subtree_group.memory_reason.is_some());
        // 逐行仍给数字。
        assert!(snapshot
            .entries
            .iter()
            .all(|entry| entry.memory_bytes.is_some()));
    }

    /// 组内有不可读内存行 → 合计不出数字（不把读不到当 0）。
    #[test]
    fn unreadable_member_hides_group_memory() {
        let mut raw = fixture();
        raw.processes[3].memory_bytes = None;
        let mut source = MockSource::new(raw);
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());

        assert!(snapshot.subtree_group.memory_bytes.is_none());
        assert!(snapshot.subtree_group.memory_reason.is_some());
        // 自身组不受影响。
        assert_eq!(snapshot.self_group.memory_bytes, Some(600));
    }

    /// 与平台无关的可求和口径（Windows 形态），让合计断言在任何 CI 上成立。
    fn test_profile() -> PlatformProfile {
        PlatformProfile {
            memory_basis: MemoryBasisDto::PrivateWorkingSet,
            memory_additive: true,
            memory_equals_footprint: false,
            webview_detached: false,
            webview_families: &["msedgewebview2.exe"],
        }
    }

    fn non_additive_profile() -> PlatformProfile {
        PlatformProfile {
            memory_basis: MemoryBasisDto::WorkingSet,
            memory_additive: false,
            memory_equals_footprint: false,
            webview_detached: false,
            webview_families: &["msedgewebview2.exe"],
        }
    }

    fn macos_profile() -> PlatformProfile {
        PlatformProfile {
            memory_basis: MemoryBasisDto::PhysFootprint,
            memory_additive: true,
            memory_equals_footprint: true,
            webview_detached: true,
            webview_families: &[
                "com.apple.WebKit.WebContent",
                "com.apple.WebKit.Networking",
                "com.apple.WebKit.GPU",
            ],
        }
    }

    /// V26：macOS 口径为 `physFootprint` / 等于物理占用 / `webviewDetached` 为真；
    /// 由系统托管的 WebKit 进程即便镜像名命中家族也不得进 `entries`（I76 §2.3、§2.4、§10.2 V26）。
    #[test]
    fn v26_detached_webview_stays_out_of_entries() {
        let mut raw = fixture();
        // 镜像名命中 macOS 家族，但父进程（launchd）不在快照内 → 归属层不可达。
        raw.processes.push(
            RawProcess::new(900, 1, 80, "com.apple.WebKit.WebContent")
                .with_cmd(&["com.apple.WebKit.WebContent"])
                .with_memory(400)
                .with_cpu_time(50),
        );
        let mut source = MockSource::new(raw);
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, macos_profile());

        assert_eq!(snapshot.memory_basis, MemoryBasisDto::PhysFootprint);
        assert!(snapshot.memory_additive);
        assert!(snapshot.memory_equals_footprint);
        assert!(snapshot.webview_detached);
        assert!(!snapshot.entries.iter().any(|entry| entry.pid == 900));
        // 子树内没有命中 macOS 家族的进程，故自身组只含宿主。
        assert_eq!(snapshot.self_group.pid_count, 1);
        assert_eq!(snapshot.self_group.memory_bytes, Some(100));
        assert_eq!(snapshot.subtree_group.pid_count, 4);
        assert_eq!(snapshot.subtree_group.memory_bytes, Some(650));
    }

    /// 真实平台常量必须与规格口径一致（Linux / macOS 由 CI 真机跑，I76 §10.2 V14、V26）。
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_profile_matches_spec() {
        let profile = platform_profile().expect("linux backend is available");
        assert_eq!(profile.memory_basis, MemoryBasisDto::WorkingSet);
        assert!(!profile.memory_additive);
        assert!(!profile.memory_equals_footprint);
        assert!(!profile.webview_detached);
        assert_eq!(
            profile.webview_families,
            &["WebKitWebProcess", "WebKitNetworkProcess"]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_matches_spec() {
        let profile = platform_profile().expect("macos backend is available");
        assert_eq!(profile.memory_basis, MemoryBasisDto::PhysFootprint);
        assert!(profile.memory_additive);
        assert!(profile.memory_equals_footprint);
        assert!(profile.webview_detached);
        assert_eq!(
            profile.webview_families,
            &[
                "com.apple.WebKit.WebContent",
                "com.apple.WebKit.Networking",
                "com.apple.WebKit.GPU",
            ]
        );
    }

    /// 平台定向线程数优先于建树层兜底（I76 §5.3）。
    #[test]
    fn source_thread_count_wins_over_tree_fallback() {
        struct ThreadedSource {
            raw: RawSnapshot,
            threads: Option<u32>,
        }

        impl SnapshotSource for ThreadedSource {
            fn snapshot(&mut self) -> Result<RawSnapshot, String> {
                Ok(self.raw.clone())
            }

            fn host_thread_count(&self, _root_pid: u32) -> Option<u32> {
                self.threads
            }
        }

        let mut source = ThreadedSource {
            raw: fixture(),
            threads: Some(42),
        };
        let mut cache = CpuDiffCache::new();
        let snapshot = build_with_profile(ROOT, &mut source, &mut cache, test_profile());
        assert_eq!(snapshot.host_thread_count, Some(42));
    }
}
