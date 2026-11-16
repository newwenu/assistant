use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::mcp::bridge::{McpCallOutcome, McpCaller, McpToolDef};

/// 一次失败后的等待基数序列（规格 §4.2）：1s / 2s / 4s / 8s。
const BACKOFF_STEPS: [u32; 4] = [1, 2, 4, 8];
/// 退避上限：即便序列被拉长也不超过 60s（规格 §4.2）。
pub const BACKOFF_CAP: Duration = Duration::from_secs(60);
/// 最多 5 次尝试（首次 + 4 次重试）；超限停在 `Error` 等手动重启。
pub const MAX_ATTEMPTS: u32 = 5;
/// 默认退避基数；单测用 `with_backoff_base` 调到毫秒级。
pub(super) const DEFAULT_BACKOFF_BASE: Duration = Duration::from_secs(1);

/// `attempt` 从 1 起：第 1 次失败后等 1s，第 2 次等 2s，第 3 次 4s，之后 8s。
pub fn backoff_delay(base: Duration, attempt: u32) -> Duration {
    let index = (attempt.saturating_sub(1) as usize).min(BACKOFF_STEPS.len() - 1);
    let multiplier = BACKOFF_STEPS[index];
    base.saturating_mul(multiplier).min(BACKOFF_CAP)
}

/// server 运行时状态（规格 §4.1）。重试次数与错误原因只在内存，不落库（§6.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerStatus {
    /// 无进程；工具不注册（用户禁用）。
    Stopped,
    /// 启动子进程、握手、`tools/list`。
    Starting,
    /// 工具已注册，可调用。
    Ready,
    /// 记录原因；工具保留但调用返回 `Internal`；按退避重试。
    Error,
}

impl McpServerStatus {
    /// 合法迁移表（规格 §4.1）。任意状态都可以被禁用回 `Stopped`。
    ///
    /// 这是状态机的定义本身，管理器每次写入状态都按它校验。
    pub fn allows(self, next: McpServerStatus) -> bool {
        use McpServerStatus::*;
        matches!(
            (self, next),
            (Stopped, Starting)
                | (Stopped, Stopped)
                | (Starting, Ready)
                | (Starting, Error)
                | (Starting, Stopped)
                | (Ready, Starting)
                | (Ready, Error)
                | (Ready, Stopped)
                | (Ready, Ready)
                | (Error, Starting)
                | (Error, Stopped)
                | (Error, Error)
        )
    }
}

impl std::fmt::Display for McpServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            McpServerStatus::Stopped => "stopped",
            McpServerStatus::Starting => "starting",
            McpServerStatus::Ready => "ready",
            McpServerStatus::Error => "error",
        };
        f.write_str(label)
    }
}

/// 一个 server 的启动配置（`mcp_servers` 行的明文投影，C4 负责解密）。
///
/// `config_hash` 由宿主层按明文规范 JSON 的 SHA-256 计算并随行下发；
/// 管理器只透传给工具，用于审批键失效判断（规格 §3.5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerConfig {
    /// 不可变稳定 id；模型工具名的第二段。
    pub id: String,
    /// 仅展示名，可改。
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// `BTreeMap` 保证遍历顺序确定，便于上层做规范 JSON。
    pub env: BTreeMap<String, String>,
    pub config_hash: String,
    /// 单次调用超时覆盖（规格 §4.3）；`None` 用客户端默认 30s。
    pub call_timeout: Option<Duration>,
}

/// 一次连接会话：由传输层产出，管理器只管生命周期与握手结果。
#[async_trait]
pub trait McpConnection: Send + Sync {
    /// `initialize` + `tools/list`：成功返回工具定义快照。
    async fn handshake(&self) -> Result<Vec<McpToolDef>, String>;
    /// 调用入口；工具通过它发 `tools/call`。
    fn caller(&self) -> Arc<dyn McpCaller>;
    /// 杀进程并释放管道；幂等。
    async fn shutdown(&self);
}

/// 传输层：stdio 实现见 C4 宿主侧装配；单测注入假实现。
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&self, config: &McpServerConfig) -> Result<Arc<dyn McpConnection>, String>;
}

/// 工具调用入口的间接层：工具在注册时拿到它，连接替换只改这里，
/// 因此 `Ready → Error → Ready` 的反复不会改动已注册工具名（规格 §4.2 第 4 条）。
///
/// 没有连接时返回 `ServerUnavailable`，桥接层归一化为 `Internal`——
/// 正好对应「`Error` 状态工具保留但调用失败」（§4.1）。
pub struct ServerCaller {
    current: RwLock<Option<Arc<dyn McpCaller>>>,
}

impl ServerCaller {
    pub(super) fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }

    pub(super) async fn set(&self, caller: Arc<dyn McpCaller>) {
        *self.current.write().await = Some(caller);
    }

    pub(super) async fn clear(&self) {
        *self.current.write().await = None;
    }
}

#[async_trait]
impl McpCaller for ServerCaller {
    async fn call_tool(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: Option<&CancellationToken>,
    ) -> McpCallOutcome {
        let caller = self.current.read().await.clone();
        match caller {
            Some(caller) => caller.call_tool(tool, arguments, cancel).await,
            None => McpCallOutcome::ServerUnavailable,
        }
    }
}

/// 对外只读快照：给启停命令与配置页（C6）展示，不含连接句柄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerSnapshot {
    pub id: String,
    pub name: String,
    pub status: McpServerStatus,
    pub last_error: Option<String>,
    pub attempts: u32,
    pub tool_ids: Vec<String>,
}

pub(super) struct ServerEntry {
    pub(super) config: McpServerConfig,
    pub(super) status: McpServerStatus,
    pub(super) last_error: Option<String>,
    pub(super) attempts: u32,
    pub(super) caller: Arc<ServerCaller>,
    pub(super) connection: Option<Arc<dyn McpConnection>>,
    pub(super) tool_ids: Vec<String>,
}

impl ServerEntry {
    pub(super) fn snapshot(&self) -> McpServerSnapshot {
        McpServerSnapshot {
            id: self.config.id.clone(),
            name: self.config.name.clone(),
            status: self.status,
            last_error: self.last_error.clone(),
            attempts: self.attempts,
            tool_ids: self.tool_ids.clone(),
        }
    }
}

/// 状态写入的唯一实现：非法迁移记 warn 但不阻断，避免状态机把管理器卡死。
pub(super) fn transition(entry: &mut ServerEntry, next: McpServerStatus) {
    if !entry.status.allows(next) {
        log::warn!(
            "mcp: illegal state transition {} -> {} for server '{}'",
            entry.status,
            next,
            entry.config.id
        );
    }
    entry.status = next;
}
