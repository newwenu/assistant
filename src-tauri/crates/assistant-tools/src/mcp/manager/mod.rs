//! MCP server 管理器（P7-C3）：状态机、启停、重连与工具注册。
//!
//! 只做「运行时状态 + 工具注册」；配置的持久化与加解密在宿主层（C4）。
//! 传输通过 [`McpTransport`] 抽象，单测用假连接驱动状态机，不真起子进程。

mod lifecycle;
mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::registry::ToolRegistry;

use types::{ServerEntry, DEFAULT_BACKOFF_BASE};

pub use types::*;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ToolUseContext;
    use crate::mcp::bridge::{McpAnnotations, McpCallOutcome, McpCaller, McpToolDef};
    use crate::registry::ToolRegistry;
    use crate::result::ToolErrorKind;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::sync::Mutex as StdMutex;
    use tokio_util::sync::CancellationToken;

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
