//! stdio 进程装配（P7-C4）：把 `assistant-tools` 的纯协议客户端接到真实子进程上。
//!
//! 进程管理只在这里出现；状态机与注册生命周期全部在 `McpServerManager`，
//! 因此客户端与桥接层保持不依赖 `tokio::process`。

use std::process::Stdio;
use std::sync::Arc;

use assistant_tools::mcp::{
    spawn_stderr_logger, McpCaller, McpClient, McpConnection, McpServerConfig, McpToolDef,
    McpTransport,
};
use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// 生产传输层：每个 server 一个子进程，stdio 承载行分隔 JSON-RPC。
pub struct StdioTransport;

impl StdioTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self, config: &McpServerConfig) -> Result<Arc<dyn McpConnection>, String> {
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // 连接对象被丢弃（含 panic 展开）时兜底杀进程，避免留下孤儿 server。
            .kill_on_drop(true);
        if let Some(cwd) = config.cwd.as_deref() {
            command.current_dir(cwd);
        }
        // 增量注入：不继承 `env_clear()`，server 仍能看到父进程的环境。
        command.envs(&config.env);

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to spawn '{}': {e}", config.command))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to take child stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to take child stdout".to_string())?;
        let stderr = child.stderr.take();

        let mut client = McpClient::new(stdout, stdin);
        if let Some(timeout) = config.call_timeout {
            client = client.with_call_timeout(timeout);
        }

        let stderr_task = stderr.map(|stderr| spawn_stderr_logger(stderr, config.id.clone()));

        Ok(Arc::new(StdioConnection {
            client: Arc::new(client),
            child: Mutex::new(Some(child)),
            stderr_task,
        }))
    }
}

/// 一条 stdio 会话：握手、调用入口与进程回收。
pub struct StdioConnection {
    client: Arc<McpClient>,
    child: Mutex<Option<Child>>,
    stderr_task: Option<tokio::task::JoinHandle<()>>,
}

#[async_trait]
impl McpConnection for StdioConnection {
    async fn handshake(&self) -> Result<Vec<McpToolDef>, String> {
        self.client
            .initialize()
            .await
            .map_err(|e| format!("initialize failed: {e}"))?;
        self.client
            .list_tools()
            .await
            .map_err(|e| format!("tools/list failed: {e}"))
    }

    fn caller(&self) -> Arc<dyn McpCaller> {
        Arc::clone(&self.client) as Arc<dyn McpCaller>
    }

    /// 幂等：先 `start_kill` 再 `wait`，重复调用不会误伤已回收的进程。
    async fn shutdown(&self) {
        let mut guard = self.child.lock().await;
        if let Some(mut child) = guard.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

impl Drop for StdioConnection {
    fn drop(&mut self) {
        // 走到这里说明调用方没走 `shutdown()`（例如注册表重建）：尽力同步杀进程。
        // `try_lock` 失败说明 `shutdown()` 正持有锁并已接管回收，交给它处理。
        if let Ok(mut guard) = self.child.try_lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.start_kill();
            }
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
    }
}
