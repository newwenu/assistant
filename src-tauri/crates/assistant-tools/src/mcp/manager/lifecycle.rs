use std::collections::BTreeMap;
use std::sync::Arc;

use crate::mcp::bridge::{build_tools, McpToolDef};
use crate::r#trait::Tool;

use super::types::{
    backoff_delay, transition, McpServerConfig, McpServerStatus, ServerCaller, ServerEntry,
    MAX_ATTEMPTS,
};
use super::McpServerManager;

impl McpServerManager {
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
}
