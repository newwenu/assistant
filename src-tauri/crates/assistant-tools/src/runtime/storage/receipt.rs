use std::path::Path;

use async_trait::async_trait;
use dashmap::DashMap;

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
