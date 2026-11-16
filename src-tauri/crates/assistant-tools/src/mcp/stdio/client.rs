use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::mcp::bridge::{McpCallOutcome, McpCaller, McpToolDef};

use super::codec::{send_frame, RpcReply};
use super::transport::read_loop;
use super::{DEFAULT_CALL_TIMEOUT, DEFAULT_HANDSHAKE_TIMEOUT, PROTOCOL_VERSION};

pub(crate) type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcReply>>>>;
pub(crate) type SharedWriter = Arc<tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;

/// 一次 JSON-RPC 请求的失败原因（客户端内部使用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcFailure {
    /// 管道已关闭 / 写入失败：server 进程已不可用。
    Closed,
    /// 本地超时（与 `exec::invoke` 的统一 deadline 是两层，见 §4.3）。
    Timeout,
    /// turn 取消（用户停止）：只中止本次请求，进程与工具保留（§4.3）。
    Cancelled,
    /// 帧结构不合法（缺 `protocolVersion`、分页失控等）。
    Protocol(String),
    /// server 返回的 JSON-RPC error。
    Remote { code: i64, message: String },
}

impl RpcFailure {
    fn message(&self) -> String {
        match self {
            RpcFailure::Closed => "MCP server connection is closed".to_string(),
            RpcFailure::Timeout => "MCP request timed out".to_string(),
            RpcFailure::Cancelled => "MCP request was cancelled".to_string(),
            RpcFailure::Protocol(message) => message.clone(),
            RpcFailure::Remote { code, message } => {
                format!("MCP server error {code}: {message}")
            }
        }
    }
}

impl std::fmt::Display for RpcFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

/// 读取任务结束时收敛在途请求。
///
/// 用 `Drop` 而不是在循环末尾收尾：任务可能因 panic 或取消直接结束，
/// 只有 `Drop` 在两条路径上都会跑（规格 §4.4 硬约束二）。
pub(crate) struct PendingGuard {
    pub(crate) pending: PendingMap,
    pub(crate) closed: Arc<AtomicBool>,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        let senders: Vec<oneshot::Sender<RpcReply>> = with_pending(&self.pending, |map| {
            map.drain().map(|(_, sender)| sender).collect()
        });
        for sender in senders {
            let _ = sender.send(RpcReply::Closed);
        }
    }
}

/// 锁被 poison 时继续使用内部值：投递应答不该因别的线程 panic 而停摆。
pub(crate) fn with_pending<T>(
    pending: &Mutex<HashMap<u64, oneshot::Sender<RpcReply>>>,
    f: impl FnOnce(&mut HashMap<u64, oneshot::Sender<RpcReply>>) -> T,
) -> T {
    match pending.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

/// stdio MCP 客户端。
pub struct McpClient {
    writer: SharedWriter,
    pending: PendingMap,
    next_id: AtomicU64,
    closed: Arc<AtomicBool>,
    call_timeout: Duration,
    handshake_timeout: Duration,
    reader_task: tokio::task::JoinHandle<()>,
}

impl McpClient {
    /// `reader` 通常是子进程 stdout，`writer` 是子进程 stdin。
    pub fn new<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        let writer: SharedWriter = Arc::new(tokio::sync::Mutex::new(Box::new(writer)));
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));

        let reader_task = tokio::spawn(read_loop(
            BufReader::new(Box::new(reader) as Box<dyn AsyncRead + Send + Unpin>),
            Arc::clone(&pending),
            Arc::clone(&writer),
            Arc::clone(&closed),
        ));

        Self {
            writer,
            pending,
            next_id: AtomicU64::new(1),
            closed,
            call_timeout: DEFAULT_CALL_TIMEOUT,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            reader_task,
        }
    }

    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_timeout = timeout;
        self
    }

    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// 握手：`initialize` + `notifications/initialized`。
    pub async fn initialize(&self) -> Result<(), RpcFailure> {
        let result = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "assistant",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
                self.handshake_timeout,
                None,
            )
            .await?;

        if result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(RpcFailure::Protocol(
                "initialize response is missing protocolVersion".to_string(),
            ));
        }

        self.notify("notifications/initialized", json!({})).await
    }

    /// `tools/list` 全量快照：按 `cursor` 分页收集。
    ///
    /// 单个工具定义解析失败只跳过该工具（记 warn），不影响整份快照。
    pub async fn list_tools(&self) -> Result<Vec<McpToolDef>, RpcFailure> {
        let mut defs = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0usize;

        loop {
            pages += 1;
            if pages > super::MAX_TOOL_PAGES {
                return Err(RpcFailure::Protocol(
                    "tools/list pagination exceeded the page limit".to_string(),
                ));
            }

            let params = match &cursor {
                Some(value) => json!({ "cursor": value }),
                None => json!({}),
            };
            let result = self
                .request("tools/list", params, self.handshake_timeout, None)
                .await?;

            let tools = result
                .get("tools")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for item in tools {
                match serde_json::from_value::<McpToolDef>(item) {
                    Ok(def) => defs.push(def),
                    Err(error) => {
                        log::warn!("mcp: skipped malformed tool definition: {error}")
                    }
                }
            }

            let next = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_string);
            match next {
                Some(next) if cursor.as_ref() != Some(&next) => cursor = Some(next),
                _ => break,
            }
        }

        Ok(defs)
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), RpcFailure> {
        let frame = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if send_frame(&self.writer, &frame).await {
            Ok(())
        } else {
            Err(RpcFailure::Closed)
        }
    }

    /// 尽力通知 server：本次请求已被取消（规格 §4.3）。
    ///
    /// 失败只记 warn：server 可能已经退出，取消通知不是调用成败的一部分。
    async fn notify_cancelled(&self, request_id: u64) {
        let frame = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": request_id },
        });
        if !send_frame(&self.writer, &frame).await {
            log::warn!("mcp: failed to send notifications/cancelled for request {request_id}");
        }
    }

    async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: Option<&CancellationToken>,
    ) -> Result<Value, RpcFailure> {
        if self.is_closed() {
            return Err(RpcFailure::Closed);
        }
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(RpcFailure::Cancelled);
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        with_pending(&self.pending, |map| {
            map.insert(id, sender);
        });

        let frame = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if !send_frame(&self.writer, &frame).await {
            with_pending(&self.pending, |map| map.remove(&id));
            return Err(RpcFailure::Closed);
        }

        let wait = tokio::time::timeout(timeout, receiver);
        let reply = match cancel {
            Some(token) => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        with_pending(&self.pending, |map| map.remove(&id));
                        self.notify_cancelled(id).await;
                        return Err(RpcFailure::Cancelled);
                    }
                    reply = wait => reply,
                }
            }
            None => wait.await,
        };

        match reply {
            Ok(Ok(RpcReply::Result(value))) => Ok(value),
            Ok(Ok(RpcReply::Error { code, message })) => Err(RpcFailure::Remote { code, message }),
            Ok(Ok(RpcReply::Closed)) => Err(RpcFailure::Closed),
            Ok(Err(_)) => {
                with_pending(&self.pending, |map| map.remove(&id));
                Err(RpcFailure::Closed)
            }
            Err(_) => {
                // 超时后必须摘掉条目：迟到的应答只会被丢弃，不能留下悬挂 sender。
                with_pending(&self.pending, |map| map.remove(&id));
                Err(RpcFailure::Timeout)
            }
        }
    }

    async fn invoke_tool(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: Option<&CancellationToken>,
    ) -> McpCallOutcome {
        let outcome = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
                self.call_timeout,
                cancel,
            )
            .await;

        match outcome {
            Ok(result) => McpCallOutcome::Content {
                blocks: result
                    .get("content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                is_error: result
                    .get("isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            Err(RpcFailure::Closed) => McpCallOutcome::ServerUnavailable,
            Err(RpcFailure::Timeout) => McpCallOutcome::Timeout,
            Err(RpcFailure::Cancelled) => McpCallOutcome::Cancelled,
            Err(RpcFailure::Remote { code, message }) => {
                // -32601 = method not found；部分 server 用业务错误文案表达「未知工具」。
                if code == -32601
                    || message.contains("unknown tool")
                    || message.contains("Unknown tool")
                {
                    McpCallOutcome::ToolNotFound
                } else {
                    McpCallOutcome::Internal { message }
                }
            }
            Err(failure) => McpCallOutcome::Internal {
                message: failure.message(),
            },
        }
    }
}

#[async_trait]
impl McpCaller for McpClient {
    async fn call_tool(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: Option<&CancellationToken>,
    ) -> McpCallOutcome {
        self.invoke_tool(tool, arguments, cancel).await
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        self.reader_task.abort();
    }
}
