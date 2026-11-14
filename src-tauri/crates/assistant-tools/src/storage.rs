use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
}

pub struct FileToolResultStorage {
    root: PathBuf,
}

impl FileToolResultStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl ToolResultStorage for FileToolResultStorage {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String> {
        let namespace = session_namespace(req.session_id.as_deref());
        let dir = self.root.join(namespace);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let file = format!(
            "{}-{}-{}",
            unique_prefix(),
            safe_component(&req.tool_name),
            result_file_name(&req.tool_call_id)
        );
        let path = dir.join(file);
        let bytes = req.content.len();
        tokio::fs::write(&path, req.content.as_bytes())
            .await
            .map_err(|error| format!("write result failed: {}", error))?;
        Ok(PersistedRef {
            path: path.display().to_string(),
            bytes,
        })
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

fn retention_cutoff(retention_days: u32) -> Option<SystemTime> {
    let span = Duration::from_secs(u64::from(retention_days).saturating_mul(86_400));
    SystemTime::now().checked_sub(span)
}

fn sweep_directory(dir: &Path, cutoff: SystemTime, report: &mut CleanupReport) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            log::warn!(
                "cleanup tool results: read dir '{}' failed: {}",
                dir.display(),
                error
            );
            report.failed += 1;
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: bad entry in '{}': {}",
                    dir.display(),
                    error
                );
                report.failed += 1;
                continue;
            }
        };
        let path = entry.path();
        // `symlink_metadata` 不跟随链接：符号链接按「非常规文件」跳过，绝不删目标。
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: stat '{}' failed: {}",
                    path.display(),
                    error
                );
                report.failed += 1;
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            sweep_directory(&path, cutoff, report);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let expired = meta.modified().map(|mtime| mtime < cutoff).unwrap_or(false);
        if !expired {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(error) => {
                log::warn!(
                    "cleanup tool results: remove '{}' failed: {}",
                    path.display(),
                    error
                );
                report.failed += 1;
            }
        }
    }
}

/// 选择本轮需要落盘的结果。
///
/// 规则：单结果超 `PER_TOOL_LIMIT_CHARS` 直接落盘；其余按字符数**降序**累加，
/// 累计超出 `PER_ROUND_LIMIT_CHARS` 的才落盘。降序保证了落盘数量最小：
/// 先把最大的挪出内联预算，剩下的内联总量才可能装得下。
///
/// 入参与返回都按原始顺序，调用方无需重排。
pub fn select_persist_targets(sizes: &[usize]) -> Vec<bool> {
    let mut persist = vec![false; sizes.len()];
    let mut candidates: Vec<usize> = Vec::new();

    for (index, &size) in sizes.iter().enumerate() {
        if size > PER_TOOL_LIMIT_CHARS {
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
}

impl ReadReceipt {
    /// 默认回执：UTF-8 无 BOM。适用于新建文件与尚未探测编码的路径。
    pub fn utf8(revision: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            revision: revision.into(),
            tool_call_id: tool_call_id.into(),
            encoding: "UTF-8".to_string(),
            has_bom: false,
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
