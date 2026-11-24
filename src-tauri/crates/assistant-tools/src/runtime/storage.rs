use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use dashmap::DashMap;

/// 单结果落盘阈值：超过即落盘，与单轮预算无关。
pub const PER_TOOL_LIMIT_CHARS: usize = 50_000;

/// 单轮内联预算：所有内联结果回填文本的合计上限。
pub const PER_ROUND_LIMIT_CHARS: usize = 200_000;

/// 落盘回填预览的头部窗口（字符）。
pub const PREVIEW_HEAD_CHARS: usize = 600;

/// 落盘回填预览的尾部窗口（字符）。
pub const PREVIEW_TAIL_CHARS: usize = 600;

/// 落盘后回填的预览字符数（头尾合计）。
pub const PREVIEW_CHARS: usize = PREVIEW_HEAD_CHARS + PREVIEW_TAIL_CHARS;

/// stdout 落盘上限：触达后继续读取并丢弃（只计数），不杀进程。
pub const MAX_SPILL_BYTES: usize = 60 * 1024 * 1024;

/// stderr 内存环形窗口（字符）：stderr 独占保留区，避免被 stdout 挤掉错误末行。
pub const STDERR_RING_CHARS: usize = 2 * 1024 * 1024;

/// stdout 触达上限后写入文件的标记行（内容事实，唯一允许写进文件的标记）。
pub const SPILL_TRUNCATED_MARKER: &str = "[truncated: output exceeded 60 MiB; total unknown]";

/// 码页探测窗口（字节）：首 4 KiB 判定一次（I56 §8.3）。
pub const ENCODING_PROBE_BYTES: usize = 4096;

/// 落盘文件的编码口径：探测结论的承载，由 `finish` 回传给 `run.rs`（I56 §8.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEncoding {
    /// 原始字节直写（合法 UTF-8；Unix 上非 UTF-8 流也归入此类，非法字节计 `U+FFFD`）。
    Utf8,
    /// Windows 控制台 OEM 码页：流式解码后以 UTF-8 写盘。
    Oem(u32),
}

impl OutputEncoding {
    /// wire 口径标签：`utf8` / `oem<cp>`（如 `oem936`）。
    pub fn label(self) -> String {
        match self {
            OutputEncoding::Utf8 => "utf8".to_string(),
            OutputEncoding::Oem(code_page) => format!("oem{code_page}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    /// 会话 id；用于目录命名空间（I46 compact §2.9 D9）。`None` 归入共享桶。
    pub session_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRef {
    pub path: String,
    pub bytes: usize,
}

#[async_trait]
pub trait ToolResultStorage: Send + Sync {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String>;

    /// 流式入口：先建文件，边写边算头尾窗口，结束时给出引用与窗口。
    async fn open_sink(&self, req: PersistToolResult) -> Result<Box<dyn ToolResultSink>, String>;

    /// 本存储为某个会话分配的落盘目录（回读域）。默认 `None` = 不提供回读。
    ///
    /// `persist` / `open_sink` / 回读边界三处必须取同一个来源，
    /// 禁止各自 `root.join(session_namespace(..))`——那是三个事实源的起点。
    fn namespace_root(&self, _session_id: Option<&str>) -> Option<PathBuf> {
        None
    }

    /// 条目观测（I61 §3.10）：`None` = 本存储不参与存储管理条目。
    ///
    /// 返回 `Err` 表示遍历失败（管理器降级为 `unreadable`），目录不存在属正常空态。
    fn result_stats(&self) -> Option<std::io::Result<DirStats>> {
        None
    }

    /// 条目保留策略：`(天数, 生效时刻秒)`；`None` = 策略未注入（`policy` 两数字填 `null`）。
    fn result_retention(&self) -> Option<(u32, i64)> {
        None
    }

    /// `E3` 的 `A3` 候选收集（T3）：`None` = 本存储不提供带字节的收集入口。
    ///
    /// 同步方法：判据只有 `mtime` 与文件类型，不需要异步 IO；管理器在
    /// `spawn_blocking` 里调用，采样与执行可以共用同一次实现（§3.10 接口形状 1）。
    /// 字节必须在**收集时**取一次：删除后再取长度是取不到的。
    fn collect_expired(&self) -> Option<Vec<ExpiredCandidate>> {
        None
    }

    /// `E3` 的 `A3` 执行（T3）：逐个删除候选并累加字节；`None` = 不提供。
    ///
    /// 必须与 [`Self::collect_expired`] 的判据同源，且「文件已不存在」不算失败（§5.1 第 3 条）。
    fn delete_candidates(&self, _candidates: &[ExpiredCandidate]) -> Option<DeleteReport> {
        None
    }
}

/// `E3` 的回收候选：路径 + 收集时的字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredCandidate {
    pub path: PathBuf,
    pub bytes: u64,
}

/// 候选删除结果：成功条数 / 成功字节 / 失败条数 / 已不存在条数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeleteReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    /// 执行时文件已不存在（并发清理或用户手删）：不算失败，也不计入删除量（§5.1 第 3 条）。
    pub skipped: u64,
}

pub struct FileToolResultStorage {
    root: PathBuf,
    /// 清理保留天数（`0` = 不清理）；由 app 层从配置注入，本类型不读配置。
    retention_days: AtomicU32,
    /// 策略生效时刻（秒）：注入时取一次，条目 `policy.effective_since` 用它。
    effective_since: i64,
}

impl FileToolResultStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            retention_days: AtomicU32::new(0),
            effective_since: now_seconds(),
        }
    }

    /// 注入保留策略；`0` 表示不清理（与 [`cleanup_expired_results`] 口径一致）。
    ///
    /// `effective_since` 由 app 层在同一处配置读取点取一次并同时喂给启动清理，
    /// 两处各自取 `now` 会让条目策略与真实清理截止时刻对不上（I61 §3.3）。
    pub fn with_retention(mut self, retention_days: u32, effective_since: i64) -> Self {
        self.retention_days.store(retention_days, Ordering::SeqCst);
        self.effective_since = effective_since;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 条目观测：与 [`cleanup_expired_results`] 共用 [`for_each_file`]，口径不会漂移。
    pub fn result_stats(&self) -> std::io::Result<DirStats> {
        result_stats(&self.root)
    }

    /// 条目保留策略：`(天数, 生效时刻秒)`。
    pub fn result_retention(&self) -> (u32, i64) {
        (
            self.retention_days.load(Ordering::SeqCst),
            self.effective_since,
        )
    }

    fn result_path(&self, req: &PersistToolResult) -> Result<PathBuf, String> {
        let dir = self
            .namespace_root(req.session_id.as_deref())
            .ok_or_else(|| "tool result storage has no namespace root".to_string())?;
        let file = format!(
            "{}-{}-{}",
            unique_prefix(),
            safe_component(&req.tool_name),
            result_file_name(&req.tool_call_id)
        );
        Ok(dir.join(file))
    }
}

#[async_trait]
impl ToolResultStorage for FileToolResultStorage {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String> {
        let path = self.result_path(&req)?;
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "tool result path has no parent".to_string())?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let bytes = req.content.len();
        tokio::fs::write(&path, req.content.as_bytes())
            .await
            .map_err(|error| format!("write result failed: {}", error))?;
        Ok(PersistedRef {
            path: path.display().to_string(),
            bytes,
        })
    }

    async fn open_sink(&self, req: PersistToolResult) -> Result<Box<dyn ToolResultSink>, String> {
        let path = self.result_path(&req)?;
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "tool result path has no parent".to_string())?;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let handle = tokio::fs::File::create(&path)
            .await
            .map_err(|error| format!("create result file failed: {}", error))?;
        let handle = handle.into_std().await;
        Ok(Box::new(FileSpillSink::new(path, handle)))
    }

    fn namespace_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
        Some(self.root.join(session_namespace(session_id)))
    }

    fn result_stats(&self) -> Option<std::io::Result<DirStats>> {
        Some(result_stats(&self.root))
    }

    fn result_retention(&self) -> Option<(u32, i64)> {
        Some((
            self.retention_days.load(Ordering::SeqCst),
            self.effective_since,
        ))
    }

    fn collect_expired(&self) -> Option<Vec<ExpiredCandidate>> {
        Some(collect_expired(
            &self.root,
            self.retention_days.load(Ordering::SeqCst),
        ))
    }

    fn delete_candidates(&self, candidates: &[ExpiredCandidate]) -> Option<DeleteReport> {
        Some(delete_candidates(candidates))
    }
}

/// 会话命名空间：会话 id 的 FNV-1a 64 位摘要（不引第三方哈希库，跨进程稳定）。
///
/// 不用 `DefaultHasher`：它的算法与密钥未承诺跨版本稳定，而定位符要跨重启有效。
/// 空 id / `None` 归入 `shared`，与 Agent 会话互不干扰。
pub fn session_namespace(session_id: Option<&str>) -> String {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return "shared".to_string();
    };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// 文件名的唯一前缀：纳秒时间戳 + 进程内自增序号。
///
/// 只保证「同一目录内不撞名」，不承担不可猜测性：定位符本身不是凭据，
/// 越权访问由工作区边界与审批约束。
fn unique_prefix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    format!("{:012x}{:04x}", nanos & 0xffff_ffff_ffff, seq & 0xffff)
}

/// 工具名只作可读提示，非法字符一律替换，避免路径分隔符与保留字符。
fn safe_component(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.chars().take(32).collect()
    }
}

/// 启动清理结果：只用于日志与测试断言，不参与任何业务判定。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: usize,
    pub failed: usize,
}

/// 按 `mtime` 年龄清理落盘目录（I46 compact §2.9 D9，门禁 10）。
///
/// 约束：只删常规文件、不跟随符号链接、失败只计数不抛出、`retention_days == 0`
/// 完全禁用。目录本身不修剪（OQ6 定稿：只删文件、保留空目录）。
/// 旧布局（根目录下的扁平 `<call_id>.txt`）同样按年龄清理，不做迁移。
pub fn cleanup_expired_results(root: &Path, retention_days: u32) -> CleanupReport {
    let mut report = CleanupReport::default();
    if retention_days == 0 {
        return report;
    }
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return report;
    };
    sweep_directory(root, cutoff, &mut report);
    report
}

/// `E3` 的候选收集：与 [`cleanup_expired_results`] 同一判据、同一遍历，额外带字节。
///
/// `retention_days == 0` 时返回空集（策略禁用，不产生任何候选）；字节取收集时刻的
/// `metadata.len()`，执行时不再重算（删完就取不到）。
pub fn collect_expired(root: &Path, retention_days: u32) -> Vec<ExpiredCandidate> {
    if retention_days == 0 {
        return Vec::new();
    }
    let Some(cutoff) = retention_cutoff(retention_days) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for_each_file(
        root,
        &mut |path, meta| {
            if is_expired(meta, cutoff) {
                candidates.push(ExpiredCandidate {
                    path: path.to_path_buf(),
                    bytes: meta.len(),
                });
            }
        },
        &mut |_path, _error| {},
    );
    candidates
}

/// `E3` 的候选删除：逐条删除并累加收集时的字节。
///
/// 「文件已不存在」计入 `skipped`（并发启动清理 / 用户手删），不算失败；
/// 其余错误只计数不抛出，单条失败不影响其余候选（§5.1 第 2、3 条）。
pub fn delete_candidates(candidates: &[ExpiredCandidate]) -> DeleteReport {
    let mut report = DeleteReport::default();
    for candidate in candidates {
        match std::fs::remove_file(&candidate.path) {
            Ok(()) => {
                report.removed = report.removed.saturating_add(1);
                report.removed_bytes = report.removed_bytes.saturating_add(candidate.bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.skipped = report.skipped.saturating_add(1);
            }
            Err(error) => {
                log::warn!("reclaim tool results: remove candidate failed: {}", error);
                report.failed = report.failed.saturating_add(1);
            }
        }
    }
    report
}

/// 过期判据（唯一来源）：`mtime` 严格早于截止时刻。
///
/// `mtime` 取不到或落在未来都判「未过期」（fail-closed，§5.5）：宁可少删，
/// 不因时钟回拨 / 文件系统精度问题误删刚写入的结果。
fn is_expired(meta: &std::fs::Metadata, cutoff: SystemTime) -> bool {
    meta.modified().map(|mtime| mtime < cutoff).unwrap_or(false)
}

/// 受管目录的观测快照（I61 §3.10）：条目与清理共用同一口径。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirStats {
    pub bytes: u64,
    pub count: u64,
    /// 目录内文件 `mtime` 最早 / 最晚时刻（秒）；无文件时两端为 `None`，不是 `0`。
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
}

/// 与 [`cleanup_expired_results`] 同一套遍历规则（递归 / 跳符号链接 / 跳非常规文件），
/// 但不按年龄过滤：统计的是整个条目当前占用。
pub fn result_stats(root: &Path) -> std::io::Result<DirStats> {
    dir_tree_stats(root)
}

/// 整棵子树的只读统计：crate 内所有「受管目录条目」共用同一份遍历实现。
///
/// 任何调用方自己写 `read_dir` 循环都会让统计口径与回收口径漂移（I61 G4），
/// 因此这里只留一个入口，外部只能经由它拿数字。
pub(crate) fn dir_tree_stats(root: &Path) -> std::io::Result<DirStats> {
    let mut stats = DirStats::default();
    let mut first_error: Option<std::io::Error> = None;
    for_each_file(
        root,
        &mut |path, meta| {
            if !is_companion_file(path) {
                accumulate_stats(&mut stats, meta);
            }
        },
        &mut |path, error| {
            if first_error.is_none() {
                first_error = Some(traversal_error(path, error));
            }
        },
    );
    match first_error {
        Some(error) => Err(error),
        None => Ok(stats),
    }
}

/// 按后缀批量删除的计数：成功条数 / 成功字节 / 失败条数 / 判据排除条数 / 目录读失败数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
    /// 遍历时读目录失败（权限 / 文件正在替换）：一个候选都没处理成功时据此判 `failed`。
    pub read_failures: u64,
}

/// 受管目录的「按后缀批量删除」：与 [`dir_tree_stats`] 共用 [`for_each_file`]。
///
/// 只删 `suffix` 命中的常规文件（大小写不敏感）；符号链接 / 非常规文件 / 子目录不参与，
/// 因此「统计多少」与「能删多少」不会各说一套（I61 G4）。目录不存在是正常空态，
/// 单条删除失败只计数、不中断。
pub(crate) fn purge_tree_files(root: &Path, suffix: &str) -> PurgeReport {
    let suffix = suffix.to_ascii_lowercase();
    let mut report = PurgeReport::default();
    for_each_file(
        root,
        &mut |path, meta| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                report.skipped = report.skipped.saturating_add(1);
                return;
            };
            if !name.to_ascii_lowercase().ends_with(&suffix) {
                return;
            }
            match std::fs::remove_file(path) {
                Ok(()) => {
                    report.removed = report.removed.saturating_add(1);
                    report.removed_bytes = report.removed_bytes.saturating_add(meta.len());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.skipped = report.skipped.saturating_add(1);
                }
                Err(_) => report.failed = report.failed.saturating_add(1),
            }
        },
        &mut |_path, _error| {
            report.read_failures = report.read_failures.saturating_add(1);
        },
    );
    report
}

/// 受管目录的统一遍历：递归、跳符号链接、跳非常规文件。
///
/// 清理与统计必须共用本函数：任何一处自己写 `read_dir` 循环，
/// 「统计多少」与「能删多少」就会漂移（I61 G4）。
///
/// 目录不存在是正常空态（按需创建），静默返回；其余失败交给 `on_error`，
/// 由调用方决定是记日志还是向上返回——本函数本身永不中断遍历。
fn for_each_file(
    dir: &Path,
    on_file: &mut dyn FnMut(&Path, &std::fs::Metadata),
    on_error: &mut dyn FnMut(&Path, &std::io::Error),
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            on_error(dir, &error);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                on_error(dir, &error);
                continue;
            }
        };
        let path = entry.path();
        // `symlink_metadata` 不跟随链接：符号链接按「非常规文件」跳过，绝不删目标。
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                on_error(&path, &error);
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            for_each_file(&path, on_file, on_error);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        on_file(&path, &meta);
    }
}

/// 伴生文件（SQLite 边车 / 部分写入残留）不计入统计；清理仍会遍历到它们（§3.9）。
const COMPANION_SUFFIXES: &[&str] = &["-journal", "-wal", "-shm", ".tmp", ".lock"];

fn is_companion_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    COMPANION_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn accumulate_stats(stats: &mut DirStats, meta: &std::fs::Metadata) {
    stats.bytes = stats.bytes.saturating_add(meta.len());
    stats.count = stats.count.saturating_add(1);
    if let Ok(modified) = meta.modified() {
        if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
            let seconds = i64::try_from(since_epoch.as_secs()).unwrap_or(i64::MAX);
            stats.oldest_at = Some(stats.oldest_at.map_or(seconds, |cur| cur.min(seconds)));
            stats.newest_at = Some(stats.newest_at.map_or(seconds, |cur| cur.max(seconds)));
        }
    }
}

fn traversal_error(path: &Path, error: &std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!("scan '{}' failed: {}", path.display(), error),
    )
}

fn retention_cutoff(retention_days: u32) -> Option<SystemTime> {
    let span = Duration::from_secs(u64::from(retention_days).saturating_mul(86_400));
    SystemTime::now().checked_sub(span)
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since_epoch| i64::try_from(since_epoch.as_secs()).ok())
        .unwrap_or(0)
}

fn sweep_directory(dir: &Path, cutoff: SystemTime, report: &mut CleanupReport) {
    // 计数用 `Cell`：遍历的两个回调各自捕获同一份计数，不产生双重可变借用。
    let removed = std::cell::Cell::new(0usize);
    let failed = std::cell::Cell::new(0usize);
    for_each_file(
        dir,
        &mut |path, meta| {
            let expired = is_expired(meta, cutoff);
            if !expired {
                return;
            }
            match std::fs::remove_file(path) {
                Ok(()) => removed.set(removed.get() + 1),
                Err(error) => {
                    log::warn!(
                        "cleanup tool results: remove '{}' failed: {}",
                        path.display(),
                        error
                    );
                    failed.set(failed.get() + 1);
                }
            }
        },
        &mut |path, error| {
            log::warn!(
                "cleanup tool results: scan '{}' failed: {}",
                path.display(),
                error
            );
            failed.set(failed.get() + 1);
        },
    );
    report.removed += removed.get();
    report.failed += failed.get();
}

/// 选择本轮需要落盘的结果。
///
/// 规则：单结果超 `PER_TOOL_LIMIT_CHARS` 直接落盘；其余按字符数**降序**累加，
/// 累计超出 `PER_ROUND_LIMIT_CHARS` 的才落盘。降序保证了落盘数量最小：
/// 先把最大的挪出内联预算，剩下的内联总量才可能装得下。
///
/// 入参与返回都按原始顺序，调用方无需重排。
pub fn select_persist_targets(sizes: &[usize]) -> Vec<bool> {
    select_persist_targets_with_limits(sizes, &[])
}

/// 同上，但允许逐条覆盖单结果阈值（`None` = 用全局 `PER_TOOL_LIMIT_CHARS`）。
///
/// `limits` 短于 `sizes` 时缺位按 `None` 处理：调用方少给一条不该 panic，
/// 也不该跳过落盘判定。单轮预算不受覆盖影响，仍是硬上限。
pub fn select_persist_targets_with_limits(sizes: &[usize], limits: &[Option<usize>]) -> Vec<bool> {
    let mut persist = vec![false; sizes.len()];
    let mut candidates: Vec<usize> = Vec::new();

    for (index, &size) in sizes.iter().enumerate() {
        let limit = limits
            .get(index)
            .copied()
            .flatten()
            .unwrap_or(PER_TOOL_LIMIT_CHARS);
        if size > limit {
            persist[index] = true;
        } else {
            candidates.push(index);
        }
    }

    candidates.sort_by(|&left, &right| sizes[right].cmp(&sizes[left]));

    let mut inline_budget = 0usize;
    for index in candidates {
        let size = sizes[index];
        if inline_budget + size > PER_ROUND_LIMIT_CHARS {
            persist[index] = true;
        } else {
            inline_budget += size;
        }
    }

    persist
}

/// 落盘后的回填文本：引用 + 头 `PREVIEW_HEAD_CHARS` + 省略标记 + 尾 `PREVIEW_TAIL_CHARS`。
///
/// 尾部窗口不可省：工具自己的省略 footer（`[truncated: ...]` / `[encoding: ...]`）
/// 与命令行类输出的错误末行都在原文末尾，只留头等于把它们全部丢掉。
///
/// 标记行**不得复用** `[truncated:` 语法：那是「工具结果被采集侧截断」的专用口径，
/// 复用会让 `compact.truncated` 把「存储预览被裁」误读成「结果不完整」。
pub fn persisted_reference(path: &str, content: &str) -> String {
    let total = content.chars().count();
    if total <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, content);
    }

    let head: String = content.chars().take(PREVIEW_HEAD_CHARS).collect();
    let tail: String = content.chars().skip(total - PREVIEW_TAIL_CHARS).collect();
    let omitted = total - PREVIEW_HEAD_CHARS - PREVIEW_TAIL_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n{}\n[… {} chars omitted; full output in the file above …]\n{}",
        path, head, omitted, tail
    )
}

/// 流式落盘后的回填文本：标签 + 省略标记 + 尾窗口（**不取头窗口**）。
///
/// 命令输出的信息密度集中在末尾（失败栈、测试摘要、退出前的现场），头部多是启动横幅
/// 与进度条重绘——因此预览整块取自文件末，不与头部分享预算。整段生产者
/// [`persisted_reference`] 保留头尾窗口：那里预览的是文档，开头就是内容本身。
///
/// 标签与省略标记必须与 [`persisted_reference`] **逐字节一致**：
/// 两个生产者共用同一个 `parse_persisted_tag` 回退路径。
pub fn persisted_reference_from_tail(path: &str, tail: &str, total_chars: usize) -> String {
    if total_chars <= PREVIEW_CHARS {
        return format!("<persisted-output path=\"{}\">\n{}", path, tail);
    }

    let omitted = total_chars - PREVIEW_CHARS;
    format!(
        "<persisted-output path=\"{}\">\n[… {} chars omitted; full output in the file above …]\n{}",
        path, omitted, tail
    )
}

/// 输出流选择：决定字节进落盘文件还是 stderr 环形窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// 流式落盘 sink：同步签名，调用方是 blocking 池里的管道读取循环（无法 `await`）。
///
/// 并发写入由持有方串行化（`Arc<Mutex<Option<Box<dyn ToolResultSink>>>>`）：
/// 临界区只有一次 `write`，内部不取其它锁、不做跨进程等待，因此不会与「读管道」互相卡死。
pub trait ToolResultSink: Send {
    /// 逐块写入原始字节；`stream` 决定落盘区还是 stderr 环形窗口。
    ///
    /// **永不因超限返回 `Err`**：超限后只计数（stdout 丢弃 / stderr 环形覆盖），
    /// 仍继续消费管道——否则子进程写阻塞，读管道的一方会等一个永不到来的 EOF。
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String>;

    /// 同步收尾（追加 stderr 段 / 删空文件 / 解码尾窗口），由调用方置于 `spawn_blocking`。
    fn finish(self: Box<Self>) -> Result<SpilledOutput, String>;
}

/// 流式落盘的收尾结果：引用、尾窗口与全流计数。
#[derive(Debug, Clone)]
pub struct SpilledOutput {
    /// 空输出为 `None`（文件已删除）。
    pub reference: Option<PersistedRef>,
    /// 文件末 `PREVIEW_CHARS` 字符（`total_chars` 不足时即全文；
    /// `truncated` 时以 `[truncated: …]` 结尾）。
    pub preview: String,
    /// 已写入文件的字符数（`truncated` 时为截至上限的计数）。
    pub total_chars: usize,
    /// 子进程总产出字节（精确，含被丢弃部分）。
    pub output_bytes: usize,
    /// 子进程总产出行数（精确，含被丢弃部分）。
    pub output_lines: usize,
    /// stdout 触达 `MAX_SPILL_BYTES`。
    pub truncated: bool,
    /// stderr 超过 `STDERR_RING_CHARS`。
    pub stderr_truncated: bool,
    /// 本次落盘实际采用的编码口径（探测结论，只读回传，见 I56 §8.3）。
    pub output_encoding: OutputEncoding,
}

/// 文件 sink：stdout 边读边落盘，stderr 进有界环形窗口，`finish` 时统一收尾。
struct FileSpillSink {
    path: PathBuf,
    file: Option<std::fs::File>,
    stdout_bytes: usize,
    stdout_total_bytes: usize,
    stdout_lines: usize,
    stdout_last_byte: Option<u8>,
    stdout_tail: VecDeque<char>,
    stderr_total_bytes: usize,
    stderr_lines: usize,
    stderr_total_chars: usize,
    stderr_last_byte: Option<u8>,
    stderr_ring: VecDeque<char>,
    /// 码页判定结论：`None` = 尚未判定，两路原始字节暂存于 pending。
    encoding: Option<OutputEncoding>,
    /// 判定前的 stdout 原始字节（首 4 KiB 窗口，判定时一次性冲洗）。
    stdout_pending: Vec<u8>,
    /// 判定前的 stderr 原始字节（同上；任一路满 4 KiB 即触发判定）。
    stderr_pending: Vec<u8>,
    stdout_codec: Option<StreamCodec>,
    stderr_codec: Option<StreamCodec>,
    truncated: bool,
    stderr_truncated: bool,
    file_chars: usize,
    file_ends_with_newline: bool,
    /// `finish` 是否已接手收尾。`false` 且 sink 被 drop = 调用方在收尾前放弃
    /// （turn 取消丢弃 future），此时落盘文件是无人引用的半成品，必须删除。
    finished: bool,
}

/// 单路输出编解码器：`Raw` 直写原始字节、字符由增量 UTF-8 解码器推进；
/// `Transcode` 按 OEM 码页流式解码（跨块保留未配对尾字节）后以 UTF-8 写盘。
enum StreamCodec {
    Raw(Utf8Decoder),
    Transcode(Box<encoding_rs::Decoder>),
}

impl StreamCodec {
    fn new(encoding: OutputEncoding) -> Self {
        match encoding {
            OutputEncoding::Utf8 => StreamCodec::Raw(Utf8Decoder::default()),
            OutputEncoding::Oem(code_page) => StreamCodec::Transcode(Box::new(
                encoding_for_code_page(code_page).new_decoder_without_bom_handling(),
            )),
        }
    }
}

impl FileSpillSink {
    fn new(path: PathBuf, file: std::fs::File) -> Self {
        Self {
            path,
            file: Some(file),
            stdout_bytes: 0,
            stdout_total_bytes: 0,
            stdout_lines: 0,
            stdout_last_byte: None,
            stdout_tail: VecDeque::with_capacity(PREVIEW_CHARS + 1),
            stderr_total_bytes: 0,
            stderr_lines: 0,
            stderr_total_chars: 0,
            stderr_last_byte: None,
            stderr_ring: VecDeque::new(),
            encoding: None,
            stdout_pending: Vec::new(),
            stderr_pending: Vec::new(),
            stdout_codec: None,
            stderr_codec: None,
            truncated: false,
            stderr_truncated: false,
            file_chars: 0,
            file_ends_with_newline: false,
            finished: false,
        }
    }

    fn push_stdout_tail_char(&mut self, ch: char) {
        if self.stdout_tail.len() >= PREVIEW_CHARS {
            self.stdout_tail.pop_front();
        }
        self.stdout_tail.push_back(ch);
    }

    fn push_stderr_char(&mut self, ch: char) {
        self.stderr_total_chars += 1;
        if self.stderr_ring.len() >= STDERR_RING_CHARS {
            self.stderr_ring.pop_front();
            self.stderr_truncated = true;
        }
        self.stderr_ring.push_back(ch);
    }

    /// 首 4 KiB 判定一次编码：取两路 pending 中较长的一路作为样本，
    /// 判定后立即把两路 pending 冲洗进各自的 codec（I56 §8.3「判定只做一次」）。
    fn maybe_decide_encoding(&mut self) -> Result<(), String> {
        if self.encoding.is_some() {
            return Ok(());
        }
        let probe = if self.stderr_pending.len() > self.stdout_pending.len() {
            &self.stderr_pending
        } else if self.stdout_pending.is_empty() {
            return Ok(());
        } else {
            &self.stdout_pending
        };
        let encoding = probe_encoding(probe);
        self.encoding = Some(encoding);
        self.stdout_codec = Some(StreamCodec::new(encoding));
        self.stderr_codec = Some(StreamCodec::new(encoding));
        let stdout_pending = std::mem::take(&mut self.stdout_pending);
        if !stdout_pending.is_empty() {
            self.write_stdout_decoded(&stdout_pending)?;
        }
        let stderr_pending = std::mem::take(&mut self.stderr_pending);
        if !stderr_pending.is_empty() {
            self.write_stderr_decoded(&stderr_pending)?;
        }
        Ok(())
    }

    fn write_stdout(&mut self, chunk: &[u8]) -> Result<(), String> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.stdout_total_bytes += chunk.len();
        self.stdout_lines += count_newlines(chunk);
        self.stdout_last_byte = chunk.last().copied();
        if self.truncated {
            return Ok(());
        }
        if self.encoding.is_none() {
            self.stdout_pending.extend_from_slice(chunk);
            if self.stdout_pending.len() >= ENCODING_PROBE_BYTES {
                self.maybe_decide_encoding()?;
            }
            return Ok(());
        }
        self.write_stdout_decoded(chunk)
    }

    fn write_stdout_decoded(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut codec = self
            .stdout_codec
            .take()
            .ok_or_else(|| "result sink encoding not decided".to_string())?;
        let result = match &mut codec {
            StreamCodec::Raw(decoder) => self.write_stdout_raw(bytes, decoder),
            StreamCodec::Transcode(decoder) => self.write_stdout_transcoded(bytes, decoder),
        };
        self.stdout_codec = Some(codec);
        result
    }

    fn write_stdout_raw(&mut self, chunk: &[u8], decoder: &mut Utf8Decoder) -> Result<(), String> {
        let remaining = MAX_SPILL_BYTES.saturating_sub(self.stdout_bytes);
        let take = remaining.min(chunk.len());
        let written = &chunk[..take];
        {
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| "result sink already finished".to_string())?;
            file.write_all(written)
                .map_err(|error| format!("write result failed: {}", error))?;
        }
        self.stdout_bytes += written.len();
        decoder.feed(written, |ch| {
            self.file_chars += 1;
            self.push_stdout_tail_char(ch);
        });
        self.file_ends_with_newline = written.last() == Some(&b'\n');
        if take < chunk.len() {
            self.truncated = true;
        }
        Ok(())
    }

    fn write_stdout_transcoded(
        &mut self,
        chunk: &[u8],
        decoder: &mut encoding_rs::Decoder,
    ) -> Result<(), String> {
        let mut text = String::new();
        let _ = decoder.decode_to_string(chunk, &mut text, false);
        self.write_stdout_text(&text)
    }

    /// 转码路径的写盘：文件字节上限按 UTF-8 文本计，截断点回退到字符边界。
    fn write_stdout_text(&mut self, text: &str) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }
        let remaining = MAX_SPILL_BYTES.saturating_sub(self.stdout_bytes);
        let mut cut = remaining.min(text.len());
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let written = &text[..cut];
        if !written.is_empty() {
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| "result sink already finished".to_string())?;
            file.write_all(written.as_bytes())
                .map_err(|error| format!("write result failed: {}", error))?;
            self.stdout_bytes += written.len();
            for ch in written.chars() {
                self.file_chars += 1;
                self.push_stdout_tail_char(ch);
            }
            self.file_ends_with_newline = written.ends_with('\n');
        }
        if cut < text.len() {
            self.truncated = true;
        }
        Ok(())
    }

    fn write_stderr(&mut self, chunk: &[u8]) -> Result<(), String> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.stderr_total_bytes += chunk.len();
        self.stderr_lines += count_newlines(chunk);
        self.stderr_last_byte = chunk.last().copied();
        if self.encoding.is_none() {
            self.stderr_pending.extend_from_slice(chunk);
            if self.stderr_pending.len() >= ENCODING_PROBE_BYTES {
                self.maybe_decide_encoding()?;
            }
            return Ok(());
        }
        self.write_stderr_decoded(chunk)
    }

    fn write_stderr_decoded(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut codec = self
            .stderr_codec
            .take()
            .ok_or_else(|| "result sink encoding not decided".to_string())?;
        match &mut codec {
            StreamCodec::Raw(decoder) => {
                decoder.feed(bytes, |ch| self.push_stderr_char(ch));
            }
            StreamCodec::Transcode(decoder) => {
                let mut text = String::new();
                let _ = decoder.decode_to_string(bytes, &mut text, false);
                for ch in text.chars() {
                    self.push_stderr_char(ch);
                }
            }
        }
        self.stderr_codec = Some(codec);
        Ok(())
    }

    /// 收口：转码路径把未配对尾字节按 1 字符冲掉，直写路径把不完整 UTF-8 尾字节计 1 字符。
    fn flush_codec_leftovers(&mut self) -> Result<(), String> {
        if let Some(mut codec) = self.stdout_codec.take() {
            match &mut codec {
                StreamCodec::Raw(decoder) => {
                    if !std::mem::take(&mut decoder.pending).is_empty() {
                        self.file_chars += 1;
                        self.push_stdout_tail_char('\u{FFFD}');
                    }
                }
                StreamCodec::Transcode(decoder) => {
                    let mut text = String::new();
                    let _ = decoder.decode_to_string(&[], &mut text, true);
                    self.write_stdout_text(&text)?;
                }
            }
            self.stdout_codec = Some(codec);
        }
        if let Some(mut codec) = self.stderr_codec.take() {
            match &mut codec {
                StreamCodec::Raw(decoder) => {
                    if !std::mem::take(&mut decoder.pending).is_empty() {
                        self.push_stderr_char('\u{FFFD}');
                    }
                }
                StreamCodec::Transcode(decoder) => {
                    let mut text = String::new();
                    let _ = decoder.decode_to_string(&[], &mut text, true);
                    for ch in text.chars() {
                        self.push_stderr_char(ch);
                    }
                }
            }
            self.stderr_codec = Some(codec);
        }
        Ok(())
    }
}

/// 未收尾即被 drop 的兜底：删除落盘文件。
///
/// 触发路径是「调用方放弃」——turn 取消时 agent 侧丢弃 future，`finish` 永不执行，
/// 而文件里只有半截输出且无任何引用指向它，留着就是无主残留。
/// 正常路径 `finish` 已置 `finished`，此处直接返回，行为逐字节不变。
impl Drop for FileSpillSink {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        // 先关句柄再删：Windows 下打开的文件不能被删除。
        self.file = None;
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::debug!(
                "abandoned result spill cleanup skipped ({}): {}",
                self.path.display(),
                error
            ),
        }
    }
}

impl ToolResultSink for FileSpillSink {
    fn write(&mut self, stream: OutputStream, chunk: &[u8]) -> Result<(), String> {
        match stream {
            OutputStream::Stdout => self.write_stdout(chunk),
            OutputStream::Stderr => self.write_stderr(chunk),
        }
    }

    fn finish(mut self: Box<Self>) -> Result<SpilledOutput, String> {
        // 收尾职责已交给本函数：之后的 `Box` drop 不得再删文件（含早退分支，
        // 与收尾前放弃的路径分开——那条由 `Drop` 负责）。
        self.finished = true;
        // 流提前 EOF 时在这里做唯一一次判定（不足 4 KiB 的样本也用），
        // 保证「判定只做一次、结论只由 sink 产出」（I56 §8.3）。
        self.maybe_decide_encoding()?;
        self.flush_codec_leftovers()?;
        let output_encoding = self.encoding.unwrap_or(OutputEncoding::Utf8);
        let mut file = self
            .file
            .take()
            .ok_or_else(|| "result sink already finished".to_string())?;

        let mut appendix = String::new();
        let mut ends_with_newline = self.file_ends_with_newline;
        let mut has_content = self.file_chars > 0;
        if self.truncated {
            if has_content && !ends_with_newline {
                appendix.push('\n');
            }
            appendix.push_str(SPILL_TRUNCATED_MARKER);
            ends_with_newline = false;
            has_content = true;
        }
        let stderr_body: Option<String> = if self.stderr_ring.is_empty() {
            None
        } else {
            Some(self.stderr_ring.iter().collect())
        };
        if let Some(body) = stderr_body.as_deref() {
            if has_content && !ends_with_newline {
                appendix.push('\n');
            }
            let header = if self.stderr_truncated {
                format!(
                    "--- stderr (last {} of {} chars) ---",
                    self.stderr_ring.len(),
                    self.stderr_total_chars
                )
            } else {
                "--- stderr ---".to_string()
            };
            appendix.push_str(&header);
            appendix.push('\n');
            appendix.push_str(body);
        }

        let appendix_chars = appendix.chars().count();
        self.file_chars += appendix_chars;
        if !appendix.is_empty() {
            file.write_all(appendix.as_bytes())
                .map_err(|error| format!("write result failed: {}", error))?;
        }
        file.flush()
            .map_err(|error| format!("flush result failed: {}", error))?;
        drop(file);

        if self.file_chars == 0 {
            let _ = std::fs::remove_file(&self.path);
            return Ok(SpilledOutput {
                reference: None,
                preview: String::new(),
                total_chars: 0,
                output_bytes: self.stdout_total_bytes + self.stderr_total_bytes,
                output_lines: self.total_lines(),
                truncated: self.truncated,
                stderr_truncated: self.stderr_truncated,
                output_encoding,
            });
        }

        let stdout_tail: String = self.stdout_tail.iter().collect();
        let mut preview = last_n_chars(&format!("{}{}", stdout_tail, appendix), PREVIEW_CHARS);
        // stdout 触达上限的标记行已被 stderr 段挤出尾窗口时，在 `preview` 末尾补一行：
        // 文件里仍只有一处，`parse_truncation` 靠尾窗口 `rfind` 判定。
        if self.truncated && !preview.ends_with(SPILL_TRUNCATED_MARKER) {
            preview.push('\n');
            preview.push_str(SPILL_TRUNCATED_MARKER);
        }

        let bytes = std::fs::metadata(&self.path)
            .map(|meta| meta.len() as usize)
            .map_err(|error| format!("stat result file failed: {}", error))?;

        Ok(SpilledOutput {
            reference: Some(PersistedRef {
                path: self.path.display().to_string(),
                bytes,
            }),
            preview,
            total_chars: self.file_chars,
            output_bytes: self.stdout_total_bytes + self.stderr_total_bytes,
            output_lines: self.total_lines(),
            truncated: self.truncated,
            stderr_truncated: self.stderr_truncated,
            output_encoding,
        })
    }
}

impl FileSpillSink {
    fn total_lines(&self) -> usize {
        let mut lines = self.stdout_lines + self.stderr_lines;
        if self.stdout_total_bytes > 0 && self.stdout_last_byte != Some(b'\n') {
            lines += 1;
        }
        if self.stderr_total_bytes > 0 && self.stderr_last_byte != Some(b'\n') {
            lines += 1;
        }
        lines
    }
}

/// 增量 UTF-8 解码器：分块边界可能切在多字节序列中间，禁止逐块 `from_utf8_lossy` 后拼窗口。
///
/// 非法序列计 1 字符（不 panic），不完整尾字节留到下一块；流结束时由调用方按 1 字符收口。
#[derive(Default)]
struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    fn feed<F: FnMut(char)>(&mut self, chunk: &[u8], mut on_char: F) {
        if chunk.is_empty() && self.pending.is_empty() {
            return;
        }
        let mut buffer = Vec::with_capacity(self.pending.len() + chunk.len());
        buffer.append(&mut self.pending);
        buffer.extend_from_slice(chunk);
        let mut offset = 0;
        while offset < buffer.len() {
            match std::str::from_utf8(&buffer[offset..]) {
                Ok(rest) => {
                    for ch in rest.chars() {
                        on_char(ch);
                    }
                    offset = buffer.len();
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        if let Ok(text) = std::str::from_utf8(&buffer[offset..offset + valid]) {
                            for ch in text.chars() {
                                on_char(ch);
                            }
                        }
                        offset += valid;
                    }
                    match error.error_len() {
                        Some(len) => {
                            on_char('\u{FFFD}');
                            offset += len;
                        }
                        None => {
                            self.pending.extend_from_slice(&buffer[offset..]);
                            offset = buffer.len();
                        }
                    }
                }
            }
        }
    }
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

/// 首 4 KiB 的编码判定（I56 §8.3 表格）。
///
/// 合法 UTF-8（允许末尾不完整序列）一律直写；只有**确定非法**的流才在 Windows 上
/// 按 OEM 码页转码，Unix 侧保持原始字节（非 UTF-8 流可能是任意编码，猜错即乱码）。
fn probe_encoding(bytes: &[u8]) -> OutputEncoding {
    match std::str::from_utf8(bytes) {
        Ok(_) => OutputEncoding::Utf8,
        Err(error) if error.error_len().is_none() => OutputEncoding::Utf8,
        Err(_) => oem_encoding(),
    }
}

#[cfg(windows)]
fn oem_encoding() -> OutputEncoding {
    // SAFETY: `GetOEMCP` 无参数、无副作用，只读取当前控制台 OEM 码页。
    OutputEncoding::Oem(unsafe { windows::Win32::Globalization::GetOEMCP() })
}

#[cfg(not(windows))]
fn oem_encoding() -> OutputEncoding {
    OutputEncoding::Utf8
}

/// 码页 → `encoding_rs` 编码。未列入的码页回退 `windows-1252`
/// （字节可解、不 panic；探测已排除合法 UTF-8，回退只影响扩展字符的精确性）。
fn encoding_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding {
    use encoding_rs::{
        BIG5, EUC_KR, GBK, IBM866, SHIFT_JIS, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
        WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
        WINDOWS_874,
    };
    match code_page {
        866 => IBM866,
        874 => WINDOWS_874,
        932 => SHIFT_JIS,
        936 => GBK,
        949 => EUC_KR,
        950 => BIG5,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        _ => WINDOWS_1252,
    }
}

fn last_n_chars(text: &str, n: usize) -> String {
    let total = text.chars().count();
    if total <= n {
        return text.to_string();
    }
    text.chars().skip(total - n).collect()
}

/// 读取回执的键：同一会话、同一路径。
///
/// 行范围**不参与键**：回执不承担去重，重复 `read` 每次都完整回传并覆盖刷新同一条回执。
///
/// `session_id` 来自 `ToolUseContext.session_id`；ToolBox 链路为 `None`，
/// 所有 ToolBox 调用归入同一桶，不会与 Agent 会话互相抑制。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReadReceiptKey {
    pub session_id: Option<String>,
    pub path: String,
}

/// 行窗口「每行字符上限」的默认值（I57 §3.5）。
///
/// `read` 的 `MAX_LINE_CHARS` 与本模块 `ReadReceipt::utf8` 的默认宽度共用此常量，
/// 避免「闸门文案里的宽度」与「实际切行宽度」两处各写一个 `2000` 而漂移。
pub const DEFAULT_LINE_WINDOW_MAX_CHARS: usize = 2000;

/// 已读取过的文件记录。`revision` 变化即失效，`edit` / `write` 据此拒绝「未读过或已过期」。
///
/// `encoding` / `has_bom` 是写侧（`write` / `edit`）唯一的编码来源：
/// 它们只消费、不重判（I38c §14.2），因此必须由 `read` 成功时写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadReceipt {
    pub revision: String,
    pub tool_call_id: String,
    /// 实际生效编码的规范名（WHATWG label，如 `UTF-8` / `GBK`）。
    pub encoding: String,
    /// 文件是否带 BOM；写回时据此补回。
    pub has_bom: bool,
    /// 本次读取中**有行被切**的行数（I57 §3.5）。`0` 表示完整读到可见段；
    /// `> 0` 时 `edit` 拒绝写入——被切掉的尾部无法与 `old_string` 逐字校验。
    pub truncated_lines: usize,
    /// 本次读取使用的每行宽度，只供 `edit` 闸门文案归因（§6.3 的 `{width}`）。
    ///
    /// 在 A 内恒为 [`DEFAULT_LINE_WINDOW_MAX_CHARS`]、不产生任何分支；
    /// 它是为 I58 的 `super_read` 预留的——B 写自己的宽度，`edit` 文案自动正确。
    pub line_window_max_chars: usize,
}

impl ReadReceipt {
    /// 默认回执：UTF-8 无 BOM。适用于新建文件与尚未探测编码的路径。
    pub fn utf8(revision: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            tool_call_id: tool_call_id.into(),
            encoding: "UTF-8".to_string(),
            has_bom: false,
            truncated_lines: 0,
            line_window_max_chars: DEFAULT_LINE_WINDOW_MAX_CHARS,
        }
    }
}

/// 读取回执端口（P2 定义，P5 由 `read` / `edit` / `write` 使用）。
///
/// 只承担两件事：`edit` / `write` 的「本会话读过该文件」闸门，以及编码结论继承。
/// **不承担去重**：`read` 不做 `lookup` 判断是否跳过回传。
#[async_trait]
pub trait ReadReceiptStore: Send + Sync {
    async fn lookup(&self, key: &ReadReceiptKey) -> Option<ReadReceipt>;
    async fn record(&self, key: ReadReceiptKey, receipt: ReadReceipt);

    /// 按组件级前缀失效：移除 `path` 自身及其子树下的所有回执。
    ///
    /// 供 `delete` / `move` 在成功后调用，覆盖「删除后重建且长度 + mtime 恰好相同」
    /// 时修订号无法识别的情形（见 `os/path::is_component_prefix`）。
    async fn invalidate_path(&self, path: &Path);
}

/// 进程内回执存储（`dashmap`）。生命周期由构造方决定，跨会话共享同一实例。
pub struct InMemoryReadReceiptStore {
    receipts: DashMap<ReadReceiptKey, ReadReceipt>,
}

impl InMemoryReadReceiptStore {
    pub fn new() -> Self {
        Self {
            receipts: DashMap::new(),
        }
    }
}

impl Default for InMemoryReadReceiptStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReadReceiptStore for InMemoryReadReceiptStore {
    async fn lookup(&self, key: &ReadReceiptKey) -> Option<ReadReceipt> {
        self.receipts.get(key).map(|entry| entry.value().clone())
    }

    async fn record(&self, key: ReadReceiptKey, receipt: ReadReceipt) {
        self.receipts.insert(key, receipt);
    }

    async fn invalidate_path(&self, path: &Path) {
        self.receipts
            .retain(|key, _| !crate::os::path::is_component_prefix(Path::new(&key.path), path));
    }
}

fn result_file_name(tool_call_id: &str) -> String {
    let sanitized: String = tool_call_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() || sanitized.chars().all(|ch| ch == '_') {
        "result.txt".to_string()
    } else {
        format!("{}.txt", sanitized)
    }
}
