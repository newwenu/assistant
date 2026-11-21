//! 13 条条目的并行采样装配（I61 §3.10）：每条一个独立任务 + 单条 500 ms 超时。
//!
//! 三条纪律：①`E1` 与顶层字段同源，由调用方传入同一份 `list_all` 结果，这里不重扫；
//! ②除 `E1` 外全部并行，单条超时只降级该条（`unavailable` + 「统计超时」），不拖整页；
//! ③超时只是放弃等待，任务线程仍会跑完——因此 owner stats 必须只读、幂等、不持锁。
//!
//! 本模块不读配置：策略一律由 owner 自述（`result_retention` / `audit_policy`），
//! 管理器不得成为第二个配置读取入口（§3.5 硬约束 1）。

use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use tauri::{AppHandle, Manager};

use assistant_storage::attachment_store::{AttachmentEntry, AttachmentStore};
use assistant_tools::os::index_stats;
use assistant_tools::ToolResultStorage;

use crate::adapters::PipelineToolExecution;
use crate::app_dirs::registry::{self, ManagedRoot, RootMount};
use crate::app_dirs::AppDirs;
use crate::protocol::dto::{ReclaimPolicyDto, StorageAvailabilityDto, StoragePolicySourceDto};
use crate::setup::DbState;

use super::entries::{set_sample, unmounted_probe_sample, EntrySample, ENTRY_COUNT};
use super::probe::{ProbeError, ProbeRegistry, ProbeStats};

/// 单条观测上限（§3.10）：超过即降级，绝不阻塞整页。
pub const SAMPLE_TIMEOUT: Duration = Duration::from_millis(500);

type SampleFuture = Pin<Box<dyn Future<Output = (&'static str, EntrySample)> + Send>>;

fn timeout_sample() -> EntrySample {
    EntrySample::degraded(StorageAvailabilityDto::Unavailable, "统计超时")
}

/// IO 错误类别：`availabilityReason` 只放用户能懂的类别，不放整条错误文案或内部枚举名。
fn io_reason(error: &std::io::Error) -> String {
    use std::io::ErrorKind;
    let text = match error.kind() {
        ErrorKind::PermissionDenied => "没有访问权限",
        ErrorKind::NotFound => "文件不存在",
        ErrorKind::StorageFull => "磁盘空间不足",
        ErrorKind::InvalidData => "文件内容异常",
        _ => "读取失败",
    };
    text.to_string()
}

fn from_dir_stats(stats: impl StatsView) -> EntrySample {
    EntrySample::observed(
        stats.bytes(),
        stats.count(),
        stats.oldest_at(),
        stats.newest_at(),
    )
}

fn from_io<T: StatsView>(result: std::io::Result<T>) -> EntrySample {
    match result {
        Ok(stats) => from_dir_stats(stats),
        Err(error) => EntrySample::degraded(StorageAvailabilityDto::Unreadable, io_reason(&error)),
    }
}

/// 各 crate 的同形状观测结构（避免管理器依赖它们的类型）统一取值。
trait StatsView {
    fn bytes(&self) -> u64;
    fn count(&self) -> u64;
    fn oldest_at(&self) -> Option<i64>;
    fn newest_at(&self) -> Option<i64>;
}

macro_rules! impl_stats_view {
    ($ty:path) => {
        impl StatsView for $ty {
            fn bytes(&self) -> u64 {
                self.bytes
            }
            fn count(&self) -> u64 {
                self.count
            }
            fn oldest_at(&self) -> Option<i64> {
                self.oldest_at
            }
            fn newest_at(&self) -> Option<i64> {
                self.newest_at
            }
        }
    };
}

impl_stats_view!(crate::app_dirs::fs_stats::DirStats);
impl_stats_view!(assistant_tools::DirStats);
impl_stats_view!(assistant_crash::DirStats);

/// `E1`：与顶层字段同源，直接由调用方给的那份 `list_all` 结果派生。
///
/// 可回收量 = 孤儿（未被任何会话引用）的字节与条数：判据与 `build_base` 同源
/// （同一份 `entries` + 同一份引用集合），因此页面上「孤儿合计」与 `E1` 的
/// `reclaimableBytes` 恒等。引用集合取不到时留 `null` → 资格按不可算降级（fail-closed）。
///
/// 这里只算数字，不做资格判定：`eligible` / `reason` / 确认档位全在 `entries.rs` 按登记表
/// 与规格派生；执行时删不删由 owner 在附件 ID 锁内重算引用集合决定（§3.3），本函数不参与。
fn attachment_sample(
    entries: &[AttachmentEntry],
    referenced: Option<&HashSet<String>>,
) -> EntrySample {
    let bytes = entries.iter().map(|entry| entry.bytes).sum();
    let count = u64::try_from(entries.len()).unwrap_or(u64::MAX);
    let oldest = entries
        .iter()
        .map(|entry| i64::try_from(entry.created_at).unwrap_or(i64::MAX))
        .min();
    let newest = entries
        .iter()
        .map(|entry| i64::try_from(entry.created_at).unwrap_or(i64::MAX))
        .max();
    let mut sample = EntrySample::observed(bytes, count, oldest, newest);
    if let Some(referenced) = referenced {
        let orphans = entries
            .iter()
            .filter(|entry| !referenced.contains(&entry.id));
        let orphan_bytes = orphans.clone().map(|entry| entry.bytes).sum();
        let orphan_count = u64::try_from(orphans.count()).unwrap_or(u64::MAX);
        sample.reclaimable = Some((orphan_bytes, orphan_count));
    }
    sample
}

fn probe_sample(stats: ProbeStats) -> EntrySample {
    let availability = match (stats.count, stats.bytes) {
        (Some(0), _) => StorageAvailabilityDto::Missing,
        (Some(_), Some(_)) => StorageAvailabilityDto::Available,
        _ => StorageAvailabilityDto::Unreadable,
    };
    let availability_reason = if availability == StorageAvailabilityDto::Unreadable {
        Some("该条目未提供这项数据".to_string())
    } else {
        None
    };
    EntrySample {
        bytes: stats.bytes,
        count: stats.count,
        oldest_at: stats.oldest_at,
        newest_at: stats.newest_at,
        availability,
        availability_reason,
        policy: None,
        reclaimable: None,
    }
}

/// provider 的封闭三值映射到 `availability` + 固定文案（§3.10 / §7.1）。
fn probe_error(error: ProbeError) -> EntrySample {
    let (availability, reason) = match error {
        ProbeError::Locked => (StorageAvailabilityDto::Locked, "尚未解锁"),
        ProbeError::Unavailable => (StorageAvailabilityDto::Unavailable, "文件正在替换 / 迁移"),
        ProbeError::Unreadable => (StorageAvailabilityDto::Unreadable, "读取失败"),
    };
    EntrySample::degraded(availability, reason)
}

/// 一行 `Feature(capability)` 的采样：**按能力名找 provider、再按该行 key 取数**。
///
/// 归属只由登记表声明（G18(d)）：绝不「遍历全部 provider 取第一个 `Some`」，
/// 那样新增一个 provider 就会抢走别人的条目。
pub fn probe_sample_for(probes: Option<&ProbeRegistry>, root: &ManagedRoot) -> EntrySample {
    let capability = match root.mount {
        RootMount::Feature(capability) => capability,
        RootMount::Always => {
            return EntrySample::degraded(
                StorageAvailabilityDto::NotImplemented,
                "该条目无需单独统计",
            )
        }
    };
    let Some(probe) = probes.and_then(|probes| probes.find(capability)) else {
        return unmounted_probe_sample();
    };
    match probe.stats(root.key) {
        None => EntrySample::degraded(StorageAvailabilityDto::NotImplemented, "该条目暂未提供统计"),
        Some(Ok(stats)) => probe_sample(stats),
        Some(Err(error)) => probe_error(error),
    }
}

/// `E11` 的策略自述：按登记表该行的能力名找 provider，不看实现是谁。
pub fn audit_policy_of(probes: Option<&ProbeRegistry>) -> Option<(u32, i64)> {
    let root = registry::find_by_entry("E11")?;
    let capability = match root.mount {
        RootMount::Feature(capability) => capability,
        RootMount::Always => return None,
    };
    probes
        .and_then(|probes| probes.find(capability))?
        .audit_policy()
}

async fn timed<F>(entry: &'static str, work: F) -> (&'static str, EntrySample)
where
    F: Future<Output = EntrySample>,
{
    match tokio::time::timeout(SAMPLE_TIMEOUT, work).await {
        Ok(sample) => (entry, sample),
        Err(_) => (entry, timeout_sample()),
    }
}

/// 阻塞型采样统一入口：`spawn_blocking` + 超时；超时后线程跑完才退出（不持锁即可）。
async fn timed_blocking<F>(entry: &'static str, work: F) -> (&'static str, EntrySample)
where
    F: FnOnce() -> EntrySample + Send + 'static,
{
    let handle = tauri::async_runtime::spawn_blocking(work);
    match tokio::time::timeout(SAMPLE_TIMEOUT, handle).await {
        Ok(Ok(sample)) => (entry, sample),
        Ok(Err(_)) => (
            entry,
            EntrySample::degraded(StorageAvailabilityDto::Unreadable, "统计失败"),
        ),
        Err(_) => (entry, timeout_sample()),
    }
}

fn sample_tool_results(storage: &Arc<dyn ToolResultStorage>) -> EntrySample {
    let stats = match storage.result_stats() {
        None => {
            return EntrySample::degraded(
                StorageAvailabilityDto::NotImplemented,
                "该存储方式不支持统计占用",
            )
        }
        Some(Ok(stats)) => stats,
        Some(Err(error)) => {
            return EntrySample::degraded(StorageAvailabilityDto::Unreadable, io_reason(&error))
        }
    };
    let mut sample = from_dir_stats(stats);
    if let Some((days, effective_since)) = storage.result_retention() {
        sample = sample.with_policy(config_policy(days, effective_since));
        // 可回收量 = 过期候选的条数与字节：与执行侧同一判据、同一遍历（`collect_expired`）。
        if days > 0 {
            if let Some(candidates) = storage.collect_expired() {
                let bytes = candidates.iter().map(|candidate| candidate.bytes).sum();
                let count = u64::try_from(candidates.len()).unwrap_or(u64::MAX);
                sample.reclaimable = Some((bytes, count));
            }
        }
    }
    sample
}

fn sample_index(root: &Path) -> EntrySample {
    match index_stats(root) {
        Ok(stats) => {
            let mut sample = from_dir_stats(stats);
            // `E4` 的可回收量就是全部索引文件：owner 判定「可重建」，管理器不自判。
            sample.reclaimable = Some((sample.bytes.unwrap_or(0), sample.count.unwrap_or(0)));
            sample
        }
        Err(error) => EntrySample::degraded(StorageAvailabilityDto::Unreadable, io_reason(&error)),
    }
}

/// `E2`：与 `derived_stats` 同源（`list_derived` 是唯一遍历点），孤儿即可回收量。
fn derived_sample(entries: &[assistant_storage::attachment_store::DerivedEntry]) -> EntrySample {
    let bytes = entries.iter().map(|entry| entry.bytes).sum();
    let count = u64::try_from(entries.len()).unwrap_or(u64::MAX);
    let oldest = entries.iter().filter_map(|entry| entry.modified_at).min();
    let newest = entries.iter().filter_map(|entry| entry.modified_at).max();
    let mut sample = EntrySample::observed(bytes, count, oldest, newest);
    let orphans = entries.iter().filter(|entry| entry.orphan);
    let orphan_bytes = orphans.clone().map(|entry| entry.bytes).sum();
    let orphan_count = u64::try_from(orphans.count()).unwrap_or(u64::MAX);
    sample.reclaimable = Some((orphan_bytes, orphan_count));
    sample
}

fn sample_mascot(root: &Path) -> EntrySample {
    from_io(crate::cmd::mascot_asset::asset_stats(root))
}

fn sample_crashes(root: &Path) -> EntrySample {
    from_io(assistant_crash::dump_stats(root))
}

/// `E6`：主库只读 `metadata`；失败按 `unavailable`（文件正在替换 / 迁移，§7.1 特例 2）。
fn sample_database(db: &DbState) -> EntrySample {
    match db.db_file_stats() {
        Ok(stats) => from_dir_stats(stats),
        Err(_) => EntrySample::degraded(StorageAvailabilityDto::Unavailable, "文件正在替换 / 迁移"),
    }
}

/// `E2`：owner 返回的错误只取类别，不把整条 IO 文案塞进 `availabilityReason`。
pub fn attachment_reason(error: &assistant_storage::attachment_store::AttachmentError) -> String {
    match error {
        assistant_storage::attachment_store::AttachmentError::Io(io) => io_reason(io),
        _ => "文件状态异常".to_string(),
    }
}

fn config_policy(days: u32, effective_since: i64) -> ReclaimPolicyDto {
    ReclaimPolicyDto {
        source: StoragePolicySourceDto::Config,
        retention_days: Some(days),
        effective_since: Some(effective_since),
        mutable: true,
        ..ReclaimPolicyDto::default()
    }
}

/// 采样输入：句柄一律由调用方给，采样层不碰 `AppHandle`（计划命令复用同一实现）。
///
/// `probes` / `tool_results` 用 `Arc` 句柄而非引用：阻塞型 owner 要搬进
/// `spawn_blocking`，闭包必须 `'static`；`db` 用可克隆的 `DbState`
/// （内含 `Arc<Mutex<Connection>>` + 文件路径，只读 `metadata`）。
pub struct SampleInputs<'a> {
    pub dirs: &'a AppDirs,
    pub store: &'a AttachmentStore,
    pub entries: &'a [AttachmentEntry],
    /// `E1` 的孤儿判据：调用方已加载的引用集合（与顶层字段同一份，不重查 DB）。
    pub referenced: Option<&'a HashSet<String>>,
    pub probes: Option<Arc<ProbeRegistry>>,
    pub tool_results: Option<Arc<dyn ToolResultStorage>>,
    pub db: Option<DbState>,
}

/// 采样整页：`E1` 由调用方给，其余 12 条并行；全部条目共用一个 `sampledAt`（调用方取）。
///
/// `dirs` 由调用方解析并传入：`AppDirs::resolve()` 失败属装配级错误，
/// 由命令整体返回 `Err`（与 `E1` 同级），不在采样层伪装成「条目不可用」。
pub async fn collect(
    app: &AppHandle,
    dirs: &AppDirs,
    store: &AttachmentStore,
    entries: &[AttachmentEntry],
    referenced: Option<&HashSet<String>>,
) -> [EntrySample; ENTRY_COUNT] {
    let probes: Option<Arc<ProbeRegistry>> = app
        .try_state::<Arc<ProbeRegistry>>()
        .map(|state| state.inner().clone());
    let tool_results: Option<Arc<dyn ToolResultStorage>> = app
        .try_state::<Arc<PipelineToolExecution>>()
        .map(|state| state.storage().clone());
    let db: Option<DbState> = app
        .try_state::<DbState>()
        .map(|state| state.inner().clone());

    collect_with(SampleInputs {
        dirs,
        store,
        entries,
        referenced,
        probes,
        tool_results,
        db,
    })
    .await
}

/// 采样实现本体：只依赖 [`SampleInputs`]，计划命令与 `get_storage_breakdown` 共用。
pub async fn collect_with(inputs: SampleInputs<'_>) -> [EntrySample; ENTRY_COUNT] {
    let SampleInputs {
        dirs,
        store,
        entries,
        referenced,
        probes,
        tool_results: storage,
        db,
    } = inputs;

    let mut samples: [EntrySample; ENTRY_COUNT] = std::array::from_fn(|_| EntrySample::missing());
    set_sample(&mut samples, "E1", attachment_sample(entries, referenced));

    let mut tasks: Vec<SampleFuture> = Vec::new();

    // E2：派生侧车目录（owner 自持 `derived/`，管理器不拼路径）。
    {
        let store = store.clone();
        tasks.push(Box::pin(timed("E2", async move {
            match store.list_derived().await {
                Ok(entries) => derived_sample(&entries),
                Err(error) => EntrySample::degraded(
                    StorageAvailabilityDto::Unreadable,
                    attachment_reason(&error),
                ),
            }
        })));
    }

    // E3：工具结果落盘（策略由 owner 自述，含注入时刻）。
    match storage {
        Some(storage) => tasks.push(Box::pin(timed_blocking("E3", move || {
            sample_tool_results(&storage)
        }))),
        None => set_sample(
            &mut samples,
            "E3",
            EntrySample::degraded(StorageAvailabilityDto::NotImplemented, "工具功能未启用"),
        ),
    }

    // E4 / E5 / E13：应用自有目录，各走 owner 的公开入口。
    {
        let index_root = dirs.indexes();
        tasks.push(Box::pin(timed_blocking("E4", move || {
            sample_index(&index_root)
        })));
        let mascot_root = dirs.mascot_assets();
        tasks.push(Box::pin(timed_blocking("E5", move || {
            sample_mascot(&mascot_root)
        })));
        let crash_root = dirs.crashes();
        tasks.push(Box::pin(timed_blocking("E13", move || {
            sample_crashes(&crash_root)
        })));
    }

    // E6：主库文件，只 `metadata`、不开库。
    match db {
        Some(db) => tasks.push(Box::pin(timed_blocking("E6", move || sample_database(&db)))),
        None => set_sample(
            &mut samples,
            "E6",
            EntrySample::degraded(StorageAvailabilityDto::Unavailable, "数据库暂不可用"),
        ),
    }

    // E7–E11：登记表声明的每一行可选能力，按「能力名找 provider、按该行 key 取数」分派。
    for root in registry::MANAGED_ROOTS {
        if !matches!(root.mount, RootMount::Feature(_)) {
            continue;
        }
        let probes = probes.clone();
        tasks.push(Box::pin(timed_blocking(root.entry, move || {
            probe_sample_for(probes.as_deref(), root)
        })));
    }

    // E12：日志组件不提供占用统计，恒为常量降级。
    set_sample(
        &mut samples,
        "E12",
        EntrySample::degraded(
            StorageAvailabilityDto::NotImplemented,
            "该组件不提供占用统计",
        ),
    );

    let audit_policy = audit_policy_of(probes.as_deref());

    for (entry, sample) in join_all(tasks).await {
        set_sample(&mut samples, entry, sample);
    }
    // E11 的策略自述单独挂：stats 取不到不影响策略展示，策略取不到则两数字保持 `null`。
    if let Some((days, effective_since)) = audit_policy {
        if let Some(root) = registry::find_by_entry("E11") {
            let mut sample = super::entries::sample_for(&samples, root).clone();
            sample = sample.with_policy(config_policy(days, effective_since));
            set_sample(&mut samples, "E11", sample);
        }
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::super::probe::StorageOwnerProbe;
    use super::*;

    fn entry(id: &str, bytes: u64, created_at: u64) -> AttachmentEntry {
        AttachmentEntry {
            id: id.to_string(),
            bytes,
            created_at,
        }
    }

    #[test]
    fn attachment_sample_matches_list_all() {
        let entries = vec![entry("a.png", 10, 100), entry("b.pdf", 20, 200)];
        let sample = attachment_sample(&entries, None);
        assert_eq!(sample.bytes, Some(30));
        assert_eq!(sample.count, Some(2));
        assert_eq!(sample.oldest_at, Some(100));
        assert_eq!(sample.newest_at, Some(200));
        assert_eq!(sample.availability, StorageAvailabilityDto::Available);
    }

    /// `E1` 的可回收量只算孤儿：引用集合里的 ID 不计入。
    #[test]
    fn attachment_sample_reclaimable_counts_only_orphans() {
        let entries = vec![entry("a.png", 10, 100), entry("b.pdf", 20, 200)];
        let referenced: HashSet<String> = ["a.png".to_string()].into_iter().collect();
        let sample = attachment_sample(&entries, Some(&referenced));
        assert_eq!(sample.reclaimable, Some((20, 1)), "只算未被引用的那份");
        let none: HashSet<String> = HashSet::new();
        let sample = attachment_sample(&entries, Some(&none));
        assert_eq!(sample.reclaimable, Some((30, 2)), "全无引用时全部可回收");
    }

    #[test]
    fn empty_attachments_are_missing_not_degraded() {
        let sample = attachment_sample(&[], None);
        assert_eq!(sample.bytes, Some(0));
        assert_eq!(sample.count, Some(0));
        assert_eq!(sample.availability, StorageAvailabilityDto::Missing);
        assert!(sample.availability_reason.is_none());
    }

    #[test]
    fn probe_samples_keep_null_for_unknown_dimensions() {
        let stats = ProbeStats {
            bytes: Some(10),
            count: None,
            oldest_at: None,
            newest_at: None,
        };
        let sample = probe_sample(stats);
        assert_eq!(sample.bytes, Some(10));
        assert_eq!(sample.count, None, "取不到的数不得填 0");
        assert_eq!(sample.availability, StorageAvailabilityDto::Unreadable);
    }

    #[test]
    fn probe_errors_map_to_fixed_availability() {
        assert_eq!(
            probe_error(ProbeError::Locked).availability,
            StorageAvailabilityDto::Locked
        );
        assert_eq!(
            probe_error(ProbeError::Unavailable).availability,
            StorageAvailabilityDto::Unavailable
        );
        assert_eq!(
            probe_error(ProbeError::Unreadable).availability,
            StorageAvailabilityDto::Unreadable
        );
    }

    #[test]
    fn timeout_sample_is_unavailable_with_reason() {
        let sample = timeout_sample();
        assert_eq!(sample.availability, StorageAvailabilityDto::Unavailable);
        assert_eq!(sample.availability_reason.as_deref(), Some("统计超时"));
        assert_eq!(sample.bytes, None);
        assert_eq!(sample.count, None);
    }

    struct FakeProbe {
        capability: &'static str,
        served: &'static str,
    }

    impl StorageOwnerProbe for FakeProbe {
        fn capability(&self) -> &'static str {
            self.capability
        }

        fn stats(&self, key: &'static str) -> Option<Result<ProbeStats, ProbeError>> {
            if key != self.served {
                return None;
            }
            Some(Ok(ProbeStats {
                bytes: Some(1024),
                count: Some(1),
                oldest_at: Some(100),
                newest_at: Some(200),
            }))
        }

        fn audit_policy(&self) -> Option<(u32, i64)> {
            None
        }
    }

    fn registry_with(probe: FakeProbe) -> Arc<ProbeRegistry> {
        let registry = Arc::new(ProbeRegistry::new());
        registry
            .register(Arc::new(probe))
            .expect("首次注册必须成功");
        registry
    }

    #[test]
    fn feature_rows_route_by_capability_then_key() {
        let registry = registry_with(FakeProbe {
            capability: registry::SECURITY_OBSERVABILITY,
            served: "security/security.db",
        });
        let served = registry::find("security/security.db").expect("E7 必须登记");
        let unserved = registry::find("security/audit").expect("E11 必须登记");

        let sample = probe_sample_for(Some(&registry), served);
        assert_eq!(sample.bytes, Some(1024), "登记表 key 必须直达 provider");
        assert_eq!(sample.availability, StorageAvailabilityDto::Available);

        let sample = probe_sample_for(Some(&registry), unserved);
        assert_eq!(
            sample.availability,
            StorageAvailabilityDto::NotImplemented,
            "provider 不认的 key 不得兜底成空目录"
        );
        assert_eq!(sample.bytes, None);
    }

    #[test]
    fn unregistered_capability_degrades_every_feature_row() {
        let registry = Arc::new(ProbeRegistry::new());
        let rows: Vec<&ManagedRoot> = registry::MANAGED_ROOTS
            .iter()
            .filter(|root| matches!(root.mount, RootMount::Feature(_)))
            .collect();
        assert_eq!(rows.len(), 5, "E7–E11 五条都声明了可选能力");
        for root in rows {
            let sample = probe_sample_for(Some(&registry), root);
            assert_eq!(
                sample.availability,
                StorageAvailabilityDto::NotImplemented,
                "{} 未装配能力必须降级",
                root.entry
            );
            assert_eq!(sample.bytes, None);
        }
        let sample = probe_sample_for(None, registry::find_by_entry("E7").unwrap());
        assert_eq!(sample.availability, StorageAvailabilityDto::NotImplemented);
    }

    #[test]
    fn always_rows_never_route_through_provider() {
        let registry = registry_with(FakeProbe {
            capability: registry::SECURITY_OBSERVABILITY,
            served: "attachments/derived",
        });
        let root = registry::find("attachments/derived").expect("E2 必须登记");
        let sample = probe_sample_for(Some(&registry), root);
        assert_eq!(
            sample.availability,
            StorageAvailabilityDto::NotImplemented,
            "无条件能力行不得走 provider 分派"
        );
    }

    #[test]
    fn audit_policy_follows_registry_capability() {
        struct PolicyProbe;
        impl StorageOwnerProbe for PolicyProbe {
            fn capability(&self) -> &'static str {
                registry::SECURITY_OBSERVABILITY
            }
            fn stats(&self, _key: &'static str) -> Option<Result<ProbeStats, ProbeError>> {
                None
            }
            fn audit_policy(&self) -> Option<(u32, i64)> {
                Some((30, 500))
            }
        }

        let registry = Arc::new(ProbeRegistry::new());
        registry.register(Arc::new(PolicyProbe)).unwrap();
        assert_eq!(audit_policy_of(Some(&registry)), Some((30, 500)));
        assert_eq!(audit_policy_of(None), None);
    }

    /// V22：全量 provider 认下登记表声明的**每一个** `Feature` 行 key，其余 key 一律 `None`。
    struct FullProbe;

    impl StorageOwnerProbe for FullProbe {
        fn capability(&self) -> &'static str {
            registry::SECURITY_OBSERVABILITY
        }

        fn stats(&self, key: &'static str) -> Option<Result<ProbeStats, ProbeError>> {
            let declared = registry::MANAGED_ROOTS
                .iter()
                .any(|root| matches!(root.mount, RootMount::Feature(_)) && root.key == key);
            declared.then_some(Ok(ProbeStats {
                bytes: Some(2048),
                count: Some(2),
                oldest_at: Some(100),
                newest_at: Some(300),
            }))
        }

        fn audit_policy(&self) -> Option<(u32, i64)> {
            Some((30, 500))
        }
    }

    #[test]
    fn every_feature_row_is_answered_by_its_declared_key() {
        let probes = Arc::new(ProbeRegistry::new());
        probes.register(Arc::new(FullProbe)).unwrap();

        let registered = probes.capabilities();
        for capability in registry::feature_capabilities() {
            assert!(
                registered.contains(&capability),
                "登记表声明的能力名 `{capability}` 必须在注册表内"
            );
        }

        let feature_rows: Vec<&ManagedRoot> = registry::MANAGED_ROOTS
            .iter()
            .filter(|root| matches!(root.mount, RootMount::Feature(_)))
            .collect();
        assert_eq!(feature_rows.len(), 5, "E7–E11 五条可选能力行");
        for root in feature_rows {
            let sample = probe_sample_for(Some(&probes), root);
            assert_eq!(
                sample.availability,
                StorageAvailabilityDto::Available,
                "{} 的 key `{}` 必须被 provider 认下",
                root.entry,
                root.key
            );
            assert_eq!(sample.bytes, Some(2048));
            assert_eq!(sample.count, Some(2));
        }

        assert!(
            FullProbe.stats("logs").is_none(),
            "非本 provider 的 key 必须返回 None"
        );
        assert!(
            FullProbe.stats("security/unknown").is_none(),
            "未登记的 key 必须返回 None"
        );
    }
}
