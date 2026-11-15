//! MCP server 管理器（P7-C3）：状态机、启停、重连与工具注册。
//!
//! 只做「运行时状态 + 工具注册」；配置的持久化与加解密在宿主层（C4）。
//! 传输通过 [`McpTransport`] 抽象，单测用假连接驱动状态机，不真起子进程。

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use super::bridge::{build_tools, McpCallOutcome, McpCaller, McpToolDef};
use crate::r#trait::Tool;
use crate::registry::ToolRegistry;

/// 一次失败后的等待基数序列（规格 §4.2）：1s / 2s / 4s / 8s。
const BACKOFF_STEPS: [u32; 4] = [1, 2, 4, 8];
/// 退避上限：即便序列被拉长也不超过 60s（规格 §4.2）。
pub const BACKOFF_CAP: Duration = Duration::from_secs(60);
/// 最多 5 次尝试（首次 + 4 次重试）；超限停在 `Error` 等手动重启。
pub const MAX_ATTEMPTS: u32 = 5;
/// 默认退避基数；单测用 `with_backoff_base` 调到毫秒级。
const DEFAULT_BACKOFF_BASE: Duration = Duration::from_secs(1);

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
    fn new() -> Self {
        Self {
            current: RwLock::new(None),
        }
    }

    async fn set(&self, caller: Arc<dyn McpCaller>) {
        *self.current.write().await = Some(caller);
    }

    async fn clear(&self) {
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

struct ServerEntry {
    config: McpServerConfig,
    status: McpServerStatus,
    last_error: Option<String>,
    attempts: u32,
    caller: Arc<ServerCaller>,
    connection: Option<Arc<dyn McpConnection>>,
    tool_ids: Vec<String>,
}

pub struct McpServerManager {
    registry: Arc<ToolRegistry>,
    transport: Arc<dyn McpTransport>,
    servers: Mutex<HashMap<String, ServerEntry>>,
    backoff_base: Duration,
}

impl McpServerManager {
    pub fn new(registry: Arc<ToolRegistry>, transport: Arc<dyn McpTransport>) -> Self {
        Self {
            registry,
            transport,
            servers: Mutex::new(HashMap::new()),
            backoff_base: DEFAULT_BACKOFF_BASE,
        }
    }

    /// 单测把退避基数压到毫秒级，避免真实等待。
    pub fn with_backoff_base(mut self, base: Duration) -> Self {
        self.backoff_base = base;
        self
    }

    pub async fn snapshot(&self, server_id: &str) -> Option<McpServerSnapshot> {
        let servers = self.servers.lock().await;
        servers.get(server_id).map(ServerEntry::snapshot)
    }

    pub async fn snapshots(&self) -> Vec<McpServerSnapshot> {
        let servers = self.servers.lock().await;
        let mut list: Vec<McpServerSnapshot> =
            servers.values().map(ServerEntry::snapshot).collect();
        list.sort_by(|left, right| left.id.cmp(&right.id));
        list
    }

    /// 启用（或按新配置重启）：停旧 → 注销旧工具 → 新配置启动。
    ///
    /// 启动失败只落到 `Error`，不返回 `Err`：调用方（启停命令 / 启动序列）
    /// 只关心最终状态，失败原因从 `last_error` 取。
    pub async fn enable(&self, config: McpServerConfig) -> McpServerStatus {
        self.stop(&config.id).await;

        let caller = Arc::new(ServerCaller::new());
        {
            let mut servers = self.servers.lock().await;
            let entry = servers
                .entry(config.id.clone())
                .or_insert_with(|| ServerEntry {
                    config: config.clone(),
                    status: McpServerStatus::Stopped,
                    last_error: None,
                    attempts: 0,
                    caller: Arc::clone(&caller),
                    connection: None,
                    tool_ids: Vec::new(),
                });
            entry.config = config.clone();
            entry.attempts = 0;
            entry.last_error = None;
            entry.caller = Arc::clone(&caller);
            entry.connection = None;
            entry.tool_ids.clear();
            transition(entry, McpServerStatus::Starting);
        }

        self.start(&config.id, &caller).await
    }

    /// 禁用：先注销工具再杀进程，不留「已注销但仍在跑」窗口（规格 §4.2 第 5 条）。
    pub async fn disable(&self, server_id: &str) {
        self.stop(server_id).await;
    }

    /// 配置不可用（密文解密失败 / 明文非法）：登记为 `Error` 且不注册任何工具。
    ///
    /// 这不是状态迁移（`Stopped → Error` 不在合法表内），而是首次登记的初始状态：
    /// 该 server 从未启动过，但 `enabled` 仍为真，UI 需要看到失败原因（规格 §6.2）。
    pub async fn mark_config_error(&self, id: &str, name: &str, reason: String) {
        let entry = ServerEntry {
            config: McpServerConfig {
                id: id.to_string(),
                name: name.to_string(),
                command: String::new(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
                config_hash: String::new(),
                call_timeout: None,
            },
            status: McpServerStatus::Error,
            last_error: Some(reason),
            attempts: 0,
            caller: Arc::new(ServerCaller::new()),
            connection: None,
            tool_ids: Vec::new(),
        };
        self.servers.lock().await.insert(id.to_string(), entry);
    }

    /// 删除 server 后清掉运行时条目：避免列表里留下没有对应配置行的孤儿状态。
    ///
    /// 调用方必须已经 `disable`（工具注销 + 进程回收），这里只摘条目。
    pub async fn forget(&self, server_id: &str) {
        self.servers.lock().await.remove(server_id);
    }

    /// 断线重连：指数退避，最多 [`MAX_ATTEMPTS`] 次；超限停在 `Error`。
    ///
    /// 重连成功重新 `tools/list`：新快照里消失的工具注销，新增的注册；
    /// 同名工具沿用同一个 `ServerCaller`，故工具名与授权键不变。
    pub async fn reconnect(&self, server_id: &str) -> McpServerStatus {
        let prepared = self
            .with_entry(server_id, |entry| {
                let config = entry.config.clone();
                let caller = Arc::clone(&entry.caller);
                let connection = entry.connection.take();
                transition(entry, McpServerStatus::Starting);
                (config, caller, connection)
            })
            .await;

        let Some((config, caller, connection)) = prepared else {
            return McpServerStatus::Stopped;
        };

        // 旧连接已断：先掐掉调用入口，再回收可能残留的进程，
        // 否则 `Error` 态的工具会打到一条已死的管道上（或打到假成功）。
        caller.clear().await;
        if let Some(connection) = connection {
            connection.shutdown().await;
        }

        let mut attempt = 0u32;
        let mut last_error = String::from("connection attempt failed");
        while attempt < MAX_ATTEMPTS {
            attempt += 1;
            match self.transport.connect(&config).await {
                Ok(connection) => match connection.handshake().await {
                    Ok(defs) => {
                        caller.set(connection.caller()).await;
                        let tool_ids = self.sync_tools(&config, &defs, &caller, server_id).await;
                        self.with_entry(server_id, |entry| {
                            entry.connection = Some(connection);
                            entry.tool_ids = tool_ids;
                            entry.attempts = attempt;
                            entry.last_error = None;
                            transition(entry, McpServerStatus::Ready);
                        })
                        .await;
                        return McpServerStatus::Ready;
                    }
                    Err(message) => {
                        connection.shutdown().await;
                        last_error = message;
                    }
                },
                Err(message) => last_error = message,
            }

            if attempt < MAX_ATTEMPTS {
                tokio::time::sleep(backoff_delay(self.backoff_base, attempt)).await;
            }
        }

        self.with_entry(server_id, |entry| {
            entry.attempts = attempt;
            entry.last_error = Some(last_error.clone());
            transition(entry, McpServerStatus::Error);
        })
        .await;
        McpServerStatus::Error
    }

    /// 启动链路：连接 → 握手 → 注册工具。失败落 `Error`（工具保留语义见 §4.1）。
    async fn start(&self, server_id: &str, caller: &Arc<ServerCaller>) -> McpServerStatus {
        let config = match self
            .with_entry(server_id, |entry| entry.config.clone())
            .await
        {
            Some(config) => config,
            None => return McpServerStatus::Stopped,
        };

        let connection = match self.transport.connect(&config).await {
            Ok(connection) => connection,
            Err(message) => return self.mark_error(server_id, message).await,
        };

        let defs = match connection.handshake().await {
            Ok(defs) => defs,
            Err(message) => {
                connection.shutdown().await;
                return self.mark_error(server_id, message).await;
            }
        };

        caller.set(connection.caller()).await;
        let tool_ids = self.sync_tools(&config, &defs, caller, server_id).await;

        self.with_entry(server_id, |entry| {
            entry.connection = Some(connection);
            entry.tool_ids = tool_ids;
            entry.last_error = None;
            entry.attempts = 0;
            transition(entry, McpServerStatus::Ready);
        })
        .await
        .map(|_| McpServerStatus::Ready)
        .unwrap_or(McpServerStatus::Stopped)
    }

    async fn mark_error(&self, server_id: &str, message: String) -> McpServerStatus {
        self.with_entry(server_id, |entry| {
            entry.last_error = Some(message);
            transition(entry, McpServerStatus::Error);
        })
        .await
        .map(|_| McpServerStatus::Error)
        .unwrap_or(McpServerStatus::Stopped)
    }

    /// 停链路：注销工具 → 断调用入口 → 杀进程（顺序不可调换）。
    async fn stop(&self, server_id: &str) {
        let taken = self
            .with_entry(server_id, |entry| {
                let tool_ids = std::mem::take(&mut entry.tool_ids);
                let connection = entry.connection.take();
                let caller = Arc::clone(&entry.caller);
                entry.attempts = 0;
                transition(entry, McpServerStatus::Stopped);
                (tool_ids, connection, caller)
            })
            .await;

        let Some((tool_ids, connection, caller)) = taken else {
            return;
        };

        for tool_id in &tool_ids {
            self.registry.unregister(tool_id);
        }
        caller.clear().await;
        if let Some(connection) = connection {
            connection.shutdown().await;
        }
    }

    /// 按快照对齐注册表：消失的工具注销，其余覆盖注册（同名 id 不变）。
    async fn sync_tools(
        &self,
        config: &McpServerConfig,
        defs: &[McpToolDef],
        caller: &Arc<ServerCaller>,
        server_id: &str,
    ) -> Vec<String> {
        let tools = build_tools(
            &config.id,
            &config.name,
            &config.config_hash,
            defs,
            caller.clone(),
        );
        let tool_ids: Vec<String> = tools.iter().map(|tool| tool.id().to_string()).collect();

        let previous = self
            .with_entry(server_id, |entry| entry.tool_ids.clone())
            .await
            .unwrap_or_default();
        for stale in previous.iter().filter(|id| !tool_ids.contains(id)) {
            self.registry.unregister(stale);
        }
        for tool in tools {
            self.registry.register(tool);
        }
        tool_ids
    }

    /// 条目写入的唯一入口：所有状态与字段变更都从这里过。
    async fn with_entry<T>(
        &self,
        server_id: &str,
        f: impl FnOnce(&mut ServerEntry) -> T,
    ) -> Option<T> {
        let mut servers = self.servers.lock().await;
        servers.get_mut(server_id).map(f)
    }
}

impl ServerEntry {
    fn snapshot(&self) -> McpServerSnapshot {
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
fn transition(entry: &mut ServerEntry, next: McpServerStatus) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ToolUseContext;
    use crate::mcp::bridge::McpAnnotations;
    use crate::result::ToolErrorKind;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;

    fn def(name: &str) -> McpToolDef {
        McpToolDef {
            name: name.to_string(),
            description: Some(format!("{name} tool")),
            input_schema: None,
            annotations: McpAnnotations::default(),
        }
    }

    fn config(id: &str, config_hash: &str) -> McpServerConfig {
        McpServerConfig {
            id: id.to_string(),
            name: format!("{id} display"),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            cwd: None,
            env: BTreeMap::new(),
            config_hash: config_hash.to_string(),
            call_timeout: None,
        }
    }

    struct StubCaller;

    #[async_trait]
    impl McpCaller for StubCaller {
        async fn call_tool(
            &self,
            _tool: &str,
            _arguments: &Value,
            _cancel: Option<&CancellationToken>,
        ) -> McpCallOutcome {
            McpCallOutcome::Content {
                blocks: Vec::new(),
                is_error: false,
            }
        }
    }

    struct FakeConnection {
        defs: Vec<McpToolDef>,
        caller: Arc<dyn McpCaller>,
        events: Arc<StdMutex<Vec<String>>>,
        registry: Arc<ToolRegistry>,
        prefix: String,
    }

    #[async_trait]
    impl McpConnection for FakeConnection {
        async fn handshake(&self) -> Result<Vec<McpToolDef>, String> {
            Ok(self.defs.clone())
        }

        fn caller(&self) -> Arc<dyn McpCaller> {
            Arc::clone(&self.caller)
        }

        /// 杀进程时检查本 server 的工具是否还在注册表里：
        /// 还在 → 说明「先杀后注销」，顺序错误。
        async fn shutdown(&self) {
            let leaked = self
                .registry
                .tool_ids()
                .iter()
                .any(|id| id.starts_with(&self.prefix));
            let label = if leaked {
                "kill-before-unregister"
            } else {
                "kill-after-unregister"
            };
            self.events.lock().unwrap().push(label.to_string());
        }
    }

    type PlanQueue = HashMap<String, VecDeque<Result<Vec<McpToolDef>, String>>>;

    struct FakeTransport {
        plans: StdMutex<PlanQueue>,
        connects: StdMutex<Vec<String>>,
        events: Arc<StdMutex<Vec<String>>>,
        registry: Arc<ToolRegistry>,
    }

    impl FakeTransport {
        fn new(registry: Arc<ToolRegistry>) -> Self {
            Self {
                plans: StdMutex::new(HashMap::new()),
                connects: StdMutex::new(Vec::new()),
                events: Arc::new(StdMutex::new(Vec::new())),
                registry,
            }
        }

        fn plan(&self, server_id: &str, defs: Vec<McpToolDef>) {
            self.plans
                .lock()
                .unwrap()
                .entry(server_id.to_string())
                .or_default()
                .push_back(Ok(defs));
        }

        fn plan_failure(&self, server_id: &str, message: &str) {
            self.plans
                .lock()
                .unwrap()
                .entry(server_id.to_string())
                .or_default()
                .push_back(Err(message.to_string()));
        }

        fn attempts(&self, server_id: &str) -> usize {
            self.connects
                .lock()
                .unwrap()
                .iter()
                .filter(|id| id.as_str() == server_id)
                .count()
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl McpTransport for FakeTransport {
        async fn connect(
            &self,
            config: &McpServerConfig,
        ) -> Result<Arc<dyn McpConnection>, String> {
            self.connects.lock().unwrap().push(config.id.clone());
            let next = self
                .plans
                .lock()
                .unwrap()
                .get_mut(&config.id)
                .and_then(VecDeque::pop_front);
            match next {
                Some(Ok(defs)) => Ok(Arc::new(FakeConnection {
                    defs,
                    caller: Arc::new(StubCaller),
                    events: Arc::clone(&self.events),
                    registry: Arc::clone(&self.registry),
                    prefix: format!("mcp__{}__", config.id),
                })),
                Some(Err(message)) => Err(message),
                None => Err("no scripted connection".to_string()),
            }
        }
    }

    fn harness() -> (Arc<ToolRegistry>, Arc<FakeTransport>, McpServerManager) {
        let registry = Arc::new(ToolRegistry::new());
        let transport = Arc::new(FakeTransport::new(Arc::clone(&registry)));
        let manager = McpServerManager::new(
            Arc::clone(&registry),
            Arc::clone(&transport) as Arc<dyn McpTransport>,
        )
        .with_backoff_base(Duration::from_millis(1));
        (registry, transport, manager)
    }

    /// `DashMap` 遍历无序，断言前先排序。
    fn sorted_ids(registry: &ToolRegistry) -> Vec<String> {
        let mut ids = registry.tool_ids();
        ids.sort();
        ids
    }

    #[test]
    fn state_machine_transition_table() {
        use McpServerStatus::*;
        assert!(Stopped.allows(Starting), "禁用 → 启动");
        assert!(!Stopped.allows(Ready), "不能跳过 Starting");
        assert!(Starting.allows(Ready));
        assert!(Starting.allows(Error));
        assert!(Ready.allows(Error), "断线");
        assert!(Ready.allows(Starting), "配置变更重启");
        assert!(Error.allows(Starting), "重试");
        assert!(Ready.allows(Stopped), "用户禁用");
        assert!(Error.allows(Stopped), "用户禁用");
        assert!(Ready.allows(Ready), "重复 Ready 幂等");
    }

    #[test]
    fn backoff_delay_follows_the_documented_sequence() {
        let base = Duration::from_secs(1);
        assert_eq!(backoff_delay(base, 1), Duration::from_secs(1));
        assert_eq!(backoff_delay(base, 2), Duration::from_secs(2));
        assert_eq!(backoff_delay(base, 3), Duration::from_secs(4));
        assert_eq!(backoff_delay(base, 4), Duration::from_secs(8));
        assert_eq!(
            backoff_delay(base, 5),
            Duration::from_secs(8),
            "序列封顶 8s"
        );
        assert_eq!(
            backoff_delay(Duration::from_secs(60), 2),
            BACKOFF_CAP,
            "整体上限 60s"
        );
    }

    #[tokio::test]
    async fn enable_registers_tools_only_after_handshake() {
        let (registry, transport, manager) = harness();
        transport.plan("srv", vec![def("alpha"), def("beta")]);

        let status = manager.enable(config("srv", "hash-1")).await;
        assert_eq!(status, McpServerStatus::Ready);

        let snapshot = manager.snapshot("srv").await.expect("snapshot");
        assert_eq!(snapshot.status, McpServerStatus::Ready);
        assert_eq!(snapshot.tool_ids, vec!["mcp__srv__alpha", "mcp__srv__beta"]);
        assert_eq!(snapshot.last_error, None);
        assert_eq!(snapshot.attempts, 0);

        for name in ["alpha", "beta"] {
            let id = format!("mcp__srv__{name}");
            assert!(registry.find(&id).is_some(), "{id} 应已注册");
            assert!(registry.find_by_name(&id).is_some(), "{id} 名字映射应存在");
        }
    }

    #[tokio::test]
    async fn handshake_failure_marks_error_and_isolates_other_servers() {
        let (registry, transport, manager) = harness();
        transport.plan_failure("bad", "spawn failed");
        transport.plan("good", vec![def("gamma")]);

        assert_eq!(
            manager.enable(config("bad", "hash-bad")).await,
            McpServerStatus::Error
        );
        let bad = manager.snapshot("bad").await.expect("snapshot");
        assert_eq!(bad.status, McpServerStatus::Error);
        assert_eq!(bad.last_error.as_deref(), Some("spawn failed"));
        assert!(bad.tool_ids.is_empty(), "握手失败不注册任何工具");
        assert!(registry.tool_ids().is_empty());

        assert_eq!(
            manager.enable(config("good", "hash-good")).await,
            McpServerStatus::Ready
        );
        assert!(registry.find("mcp__good__gamma").is_some());
    }

    #[tokio::test]
    async fn disable_unregisters_tools_before_killing_the_process() {
        let (registry, transport, manager) = harness();
        transport.plan("srv", vec![def("alpha")]);

        manager.enable(config("srv", "hash-1")).await;
        assert!(registry.find("mcp__srv__alpha").is_some());

        manager.disable("srv").await;

        let snapshot = manager.snapshot("srv").await.expect("snapshot");
        assert_eq!(snapshot.status, McpServerStatus::Stopped);
        assert!(snapshot.tool_ids.is_empty());
        assert!(registry.tool_ids().is_empty());
        assert_eq!(
            transport.events(),
            vec!["kill-after-unregister".to_string()],
            "必须注销后才杀进程"
        );
    }

    #[tokio::test]
    async fn error_state_keeps_tools_and_calls_fail_as_internal() {
        let (registry, transport, manager) = harness();
        transport.plan("srv", vec![def("alpha")]);
        assert_eq!(
            manager.enable(config("srv", "hash-1")).await,
            McpServerStatus::Ready
        );

        // 后续 5 次连接全部失败（队列里没有脚本），退避基数已压到 1ms。
        let status = manager.reconnect("srv").await;
        assert_eq!(status, McpServerStatus::Error);
        assert_eq!(transport.attempts("srv"), 1 + MAX_ATTEMPTS as usize);

        let snapshot = manager.snapshot("srv").await.expect("snapshot");
        assert_eq!(snapshot.status, McpServerStatus::Error);
        assert_eq!(snapshot.attempts, MAX_ATTEMPTS);
        assert_eq!(
            snapshot.tool_ids,
            vec!["mcp__srv__alpha"],
            "Error 态保留工具"
        );
        assert!(snapshot.last_error.is_some());

        let registered = registry.find("mcp__srv__alpha").expect("tool kept");
        let result = registered
            .tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .expect("execute never errors out");
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Internal));
    }

    #[tokio::test]
    async fn reconnect_keeps_tool_ids_and_drops_tools_missing_from_snapshot() {
        let (registry, transport, manager) = harness();
        transport.plan("srv", vec![def("alpha"), def("beta")]);
        manager.enable(config("srv", "hash-1")).await;

        transport.plan("srv", vec![def("alpha"), def("beta")]);
        assert_eq!(manager.reconnect("srv").await, McpServerStatus::Ready);
        assert_eq!(
            sorted_ids(&registry),
            vec!["mcp__srv__alpha", "mcp__srv__beta"]
        );

        transport.plan("srv", vec![def("alpha")]);
        assert_eq!(manager.reconnect("srv").await, McpServerStatus::Ready);
        assert_eq!(
            sorted_ids(&registry),
            vec!["mcp__srv__alpha"],
            "消失的工具要注销"
        );
        let snapshot = manager.snapshot("srv").await.expect("snapshot");
        assert_eq!(snapshot.tool_ids, vec!["mcp__srv__alpha"]);
    }

    #[tokio::test]
    async fn reconnect_on_unknown_server_is_a_noop() {
        let (_, _, manager) = harness();
        assert_eq!(manager.reconnect("ghost").await, McpServerStatus::Stopped);
        assert!(manager.snapshots().await.is_empty());
    }

    #[tokio::test]
    async fn config_change_restarts_with_new_hash_but_same_tool_id() {
        let (registry, transport, manager) = harness();
        transport.plan("srv", vec![def("alpha")]);
        manager.enable(config("srv", "hash-1")).await;

        let first = registry.find("mcp__srv__alpha").expect("tool");
        let intents = first
            .tool
            .permission_intents(&json!({}), &ToolUseContext::default())
            .expect("intents");
        assert_eq!(intents[0].resources, vec!["mcp:srv:hash-1"]);

        transport.plan("srv", vec![def("alpha")]);
        manager.enable(config("srv", "hash-2")).await;

        let second = registry.find("mcp__srv__alpha").expect("tool");
        assert_eq!(second.id, first.id, "工具 id 不随配置哈希变化");
        let intents = second
            .tool
            .permission_intents(&json!({}), &ToolUseContext::default())
            .expect("intents");
        assert_eq!(
            intents[0].resources,
            vec!["mcp:srv:hash-2"],
            "新哈希让旧审批失效"
        );
        assert_eq!(
            transport.events(),
            vec!["kill-after-unregister".to_string()]
        );
    }
}
