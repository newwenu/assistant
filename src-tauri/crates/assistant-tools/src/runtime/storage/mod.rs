mod naming;
mod preview;
mod receipt;
mod retention;
mod sink;
mod tree;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use self::naming::{result_file_name, safe_component, unique_prefix};
use self::sink::FileSpillSink;

pub use naming::{scratch_root_for, session_namespace};
pub use preview::{
    persisted_reference, persisted_reference_from_tail, select_persist_targets,
    select_persist_targets_with_limits, PER_ROUND_LIMIT_CHARS, PER_TOOL_LIMIT_CHARS, PREVIEW_CHARS,
    PREVIEW_HEAD_CHARS, PREVIEW_TAIL_CHARS,
};
pub use receipt::{
    InMemoryReadReceiptStore, ReadReceipt, ReadReceiptKey, ReadReceiptStore,
    DEFAULT_LINE_WINDOW_MAX_CHARS,
};
pub use retention::{
    cleanup_expired_results, collect_expired, delete_candidates, CleanupReport, DeleteReport,
    ExpiredCandidate,
};
pub use sink::{
    OutputEncoding, OutputStream, SpilledOutput, ToolResultSink, ENCODING_PROBE_BYTES,
    MAX_SPILL_BYTES, SPILL_TRUNCATED_MARKER, STDERR_RING_CHARS,
};
pub(crate) use tree::{dir_tree_stats, purge_tree_files};
pub use tree::{result_stats, DirStats, PurgeReport};

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
    /// 绝对路径：内部 IO、条目统计与测试都以它为入口（I73 §3.6 R4）。
    pub path: String,
    pub bytes: usize,
}

impl PersistedRef {
    /// 模型可见的引用（I73 §3.6 R1）：由 `path` 的文件名派生，不新增字段、不改落盘格式。
    ///
    /// 映射是结构性恒等式而非查表——落盘文件平铺在会话命名空间目录下，
    /// 因此 `tool_output_root.join(file_name)` 就是原路径（I73 §3.7）。
    pub fn model_ref(&self) -> String {
        let file_name = Path::new(&self.path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone());
        crate::os::path::tool_output_ref(&file_name)
    }
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

    /// 条目观测：与 [`cleanup_expired_results`] 共用 `for_each_file`，口径不会漂移。
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

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since_epoch| i64::try_from(since_epoch.as_secs()).ok())
        .unwrap_or(0)
}
