use std::path::{Path, PathBuf};

use async_trait::async_trait;
use dashmap::DashMap;

/// 单结果落盘阈值：超过即落盘，与单轮预算无关。
pub const PER_TOOL_LIMIT_CHARS: usize = 50_000;

/// 单轮内联预算：所有内联结果回填文本的合计上限。
pub const PER_ROUND_LIMIT_CHARS: usize = 200_000;

/// 落盘后回填的预览字符数。
pub const PREVIEW_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
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
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|error| format!("create result dir failed: {}", error))?;
        let path = self.root.join(result_file_name(&req.tool_call_id));
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

/// 落盘后的回填文本：引用 + 前 `PREVIEW_CHARS` 字符。
pub fn persisted_reference(path: &str, content: &str) -> String {
    let preview: String = content.chars().take(PREVIEW_CHARS).collect();
    format!("<persisted-output path=\"{}\">\n{}", path, preview)
}

/// 跨轮去重的键：同一会话、同一路径、同一行范围。
///
/// `session_id` 来自 `ToolUseContext.session_id`；ToolBox 链路为 `None`，
/// 所有 ToolBox 调用归入同一桶，不会与 Agent 会话互相抑制。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReadReceiptKey {
    pub session_id: Option<String>,
    pub path: String,
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
}

/// 已返回过的读取记录。`revision` 变化即失效，必须重读。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadReceipt {
    pub revision: String,
    pub tool_call_id: String,
}

/// 跨轮去重端口（P2 定义，P5 由 `read` / `edit` 使用）。
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
