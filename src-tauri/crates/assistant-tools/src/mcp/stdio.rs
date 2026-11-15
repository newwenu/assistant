//! MCP stdio 客户端（P7-C2）：JSON-RPC 2.0 行分隔读写。
//!
//! 本模块只负责协议与请求分发，**不管理进程**（进程启动 / 重启 / kill 属于 C3）。
//! 传输入参是 `AsyncRead` / `AsyncWrite`，因此单测可以用 `tokio::io::duplex`
//! 起假 server，无需真的拉起子进程。
//!
//! 崩溃隔离约束见规格 §4.4：
//! - 解析路径零 `panic`（不 `unwrap` / 不直接索引 / 不做无检查切片）；
//! - 非法帧只丢弃该帧，读取循环继续；
//! - 读取任务结束（EOF / 出错 / panic）时，drop guard 把在途请求收敛为
//!   `ServerUnavailable`，调用方不会永久等待。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::bridge::{McpCallOutcome, McpCaller, McpToolDef};

/// 单次 `tools/call` 的默认超时（规格 §4.3）。
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// 握手 / `tools/list` 的默认超时（规格 §4.2）。
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// v1 只声明 tools 能力，不涉及 resources / prompts / sampling（规格 §9 已定稿）。
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// 单帧上限：超过即丢弃。防止畸形 server 用超长行撑爆内存。
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// `tools/list` 分页上限：防止 server 返回循环 cursor 让启动永不结束。
const MAX_TOOL_PAGES: usize = 100;
/// stderr 单行截断长度：日志只留可诊断前缀，不整段回放。
const STDERR_LINE_LIMIT: usize = 200;

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<RpcReply>>>>;
type SharedWriter = Arc<tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;

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

enum RpcReply {
    Result(Value),
    Error { code: i64, message: String },
    Closed,
}

/// 读取任务结束时收敛在途请求。
///
/// 用 `Drop` 而不是在循环末尾收尾：任务可能因 panic 或取消直接结束，
/// 只有 `Drop` 在两条路径上都会跑（规格 §4.4 硬约束二）。
struct PendingGuard {
    pending: PendingMap,
    closed: Arc<AtomicBool>,
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
fn with_pending<T>(
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
            if pages > MAX_TOOL_PAGES {
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

/// 把 server 的 stderr 收敛成日志。
///
/// 只记 `debug` 且逐行截断：stderr 内容不受我们控制，无法可靠过滤密钥，
/// 因此不做整段回放，也不拼进协议日志。
pub fn spawn_stderr_logger<R>(stderr: R, server_id: String) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    let line: String = line.trim_end().chars().take(STDERR_LINE_LIMIT).collect();
                    log::debug!("mcp[{server_id}] stderr: {line}");
                }
                Err(_) => break,
            }
        }
    })
}

async fn send_frame(writer: &SharedWriter, frame: &Value) -> bool {
    let line = match serde_json::to_string(frame) {
        Ok(line) => line,
        Err(_) => return false,
    };
    let mut guard = writer.lock().await;
    if guard.write_all(line.as_bytes()).await.is_err() {
        return false;
    }
    if guard.write_all(b"\n").await.is_err() {
        return false;
    }
    guard.flush().await.is_ok()
}

async fn read_loop(
    mut reader: BufReader<Box<dyn AsyncRead + Send + Unpin>>,
    pending: PendingMap,
    writer: SharedWriter,
    closed: Arc<AtomicBool>,
) {
    let _guard = PendingGuard {
        pending: Arc::clone(&pending),
        closed: Arc::clone(&closed),
    };

    let mut buf = Vec::new();
    loop {
        buf.clear();
        let read = match reader.read_until(b'\n', &mut buf).await {
            Ok(read) => read,
            Err(error) => {
                log::warn!("mcp: stdout read failed: {error}");
                break;
            }
        };
        if read == 0 {
            break;
        }
        if buf.len() > MAX_FRAME_BYTES {
            log::warn!("mcp: dropped oversized frame ({} bytes)", buf.len());
            continue;
        }
        let line = match std::str::from_utf8(&buf) {
            Ok(line) => line.trim(),
            Err(_) => {
                log::warn!("mcp: dropped non-utf8 frame");
                continue;
            }
        };
        if line.is_empty() {
            continue;
        }
        let frame: Value = match serde_json::from_str(line) {
            Ok(frame) => frame,
            Err(_) => {
                log::warn!("mcp: dropped malformed frame");
                continue;
            }
        };
        if frame.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            log::warn!("mcp: dropped frame without jsonrpc 2.0");
            continue;
        }

        let id = frame.get("id").and_then(Value::as_u64);
        let method = frame.get("method").and_then(Value::as_str);
        match (id, method) {
            (Some(id), None) => deliver_reply(&pending, id, &frame),
            (Some(id), Some(method)) => {
                // server → client 请求：v1 不做 resources / prompts / sampling，
                // 必须明确回错，否则 server 会一直等这个 id。
                let reply = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("method not supported by this client: {method}"),
                    },
                });
                if !send_frame(&writer, &reply).await {
                    log::warn!("mcp: failed to reply to server request {method}");
                }
            }
            (None, _) => {}
        }
    }
}

fn deliver_reply(pending: &PendingMap, id: u64, frame: &Value) {
    let reply = match frame.get("error") {
        Some(error) => RpcReply::Error {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unspecified MCP error")
                .to_string(),
        },
        None => RpcReply::Result(frame.get("result").cloned().unwrap_or(Value::Null)),
    };

    let sender = with_pending(pending, |map| map.remove(&id));
    match sender {
        Some(sender) => {
            let _ = sender.send(reply);
        }
        None => log::debug!("mcp: dropped reply for unknown request id {id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{duplex, DuplexStream, ReadHalf, WriteHalf};

    enum Action {
        Reply(Value),
        ReplyError { code: i64, message: String },
        Raw(Vec<u8>),
        Silence,
        Close,
    }

    type ServerReader = ReadHalf<DuplexStream>;
    type ServerWriter = WriteHalf<DuplexStream>;

    fn harness() -> (McpClient, ServerReader, ServerWriter) {
        let (client_io, server_io) = duplex(256 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client_io);
        let (server_reader, server_writer) = tokio::io::split(server_io);
        let client = McpClient::new(client_reader, client_writer)
            .with_call_timeout(Duration::from_secs(2))
            .with_handshake_timeout(Duration::from_secs(2));
        (client, server_reader, server_writer)
    }

    async fn write_json<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Value) -> bool {
        let Ok(line) = serde_json::to_string(frame) else {
            return false;
        };
        if writer.write_all(line.as_bytes()).await.is_err() {
            return false;
        }
        if writer.write_all(b"\n").await.is_err() {
            return false;
        }
        writer.flush().await.is_ok()
    }

    fn spawn_server<F>(
        reader: ServerReader,
        mut writer: ServerWriter,
        handler: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Fn(&str, &Value) -> Vec<Action> + Send + 'static,
    {
        tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let read = match reader.read_until(b'\n', &mut buf).await {
                    Ok(read) => read,
                    Err(_) => break,
                };
                if read == 0 {
                    break;
                }
                let line = String::from_utf8_lossy(&buf);
                let frame: Value = match serde_json::from_str(line.trim()) {
                    Ok(frame) => frame,
                    Err(_) => continue,
                };
                let id = frame.get("id").and_then(Value::as_u64);
                let method = frame
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();

                let mut stop = false;
                for action in handler(&method, &frame) {
                    match action {
                        Action::Reply(result) => {
                            let Some(id) = id else { continue };
                            let reply = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                            if !write_json(&mut writer, &reply).await {
                                stop = true;
                                break;
                            }
                        }
                        Action::ReplyError { code, message } => {
                            let Some(id) = id else { continue };
                            let reply = json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": { "code": code, "message": message },
                            });
                            if !write_json(&mut writer, &reply).await {
                                stop = true;
                                break;
                            }
                        }
                        Action::Raw(bytes) => {
                            if writer.write_all(&bytes).await.is_err()
                                || writer.write_all(b"\n").await.is_err()
                            {
                                stop = true;
                                break;
                            }
                            let _ = writer.flush().await;
                        }
                        Action::Silence => {}
                        Action::Close => {
                            stop = true;
                            break;
                        }
                    }
                }
                if stop {
                    break;
                }
            }
        })
    }

    async fn join_server(task: tokio::task::JoinHandle<()>) {
        let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    }

    #[tokio::test]
    async fn handshake_and_paginated_tools_list() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, frame| match method {
            "initialize" => vec![Action::Reply(
                json!({ "protocolVersion": "2024-11-05", "capabilities": { "tools": {} } }),
            )],
            "tools/list" => {
                let cursor = frame
                    .get("params")
                    .and_then(|params| params.get("cursor"))
                    .and_then(Value::as_str);
                match cursor {
                    None => vec![Action::Reply(json!({
                        "tools": [
                            { "name": "alpha", "description": "first" },
                            { "name": "gamma" },
                        ],
                        "nextCursor": "page-2",
                    }))],
                    Some("page-2") => vec![Action::Reply(json!({
                        "tools": [{ "name": "beta" }],
                    }))],
                    _ => vec![Action::Reply(json!({ "tools": [] }))],
                }
            }
            _ => vec![Action::Silence],
        });

        client.initialize().await.expect("handshake");
        let defs = client.list_tools().await.expect("tools/list");

        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, "alpha");
        assert_eq!(defs[0].description.as_deref(), Some("first"));
        assert_eq!(defs[1].name, "gamma");
        assert_eq!(defs[2].name, "beta");
        assert!(defs[2].description.is_none());

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn malformed_tool_definition_is_skipped() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "initialize" => vec![Action::Reply(json!({ "protocolVersion": "2024-11-05" }))],
            "tools/list" => vec![Action::Reply(json!({
                "tools": [{ "name": "ok" }, { "description": "missing name" }],
            }))],
            _ => vec![Action::Silence],
        });

        client.initialize().await.expect("handshake");
        let defs = client.list_tools().await.expect("tools/list");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "ok");

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn tool_call_maps_text_content() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::Reply(json!({
                "content": [
                    { "type": "text", "text": "hello" },
                    { "type": "image", "data": "BASE64" },
                ],
                "isError": false,
            }))],
            _ => vec![Action::Silence],
        });

        let outcome = client
            .call_tool("search", &json!({ "q": "rust" }), None)
            .await;
        match outcome {
            McpCallOutcome::Content { blocks, is_error } => {
                assert_eq!(blocks.len(), 2);
                assert!(!is_error);
            }
            other => panic!("expected content outcome, got {other:?}"),
        }

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn tool_call_is_error_maps_to_content_error() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::Reply(json!({
                "content": [{ "type": "text", "text": "failed" }],
                "isError": true,
            }))],
            _ => vec![Action::Silence],
        });

        let outcome = client.call_tool("search", &json!({}), None).await;
        match outcome {
            McpCallOutcome::Content { is_error, .. } => assert!(is_error),
            other => panic!("expected content outcome, got {other:?}"),
        }

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn method_not_found_maps_to_tool_not_found() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::ReplyError {
                code: -32601,
                message: "method not found".to_string(),
            }],
            _ => vec![Action::Silence],
        });

        assert_eq!(
            client.call_tool("ghost", &json!({}), None).await,
            McpCallOutcome::ToolNotFound
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn unknown_tool_message_maps_to_tool_not_found() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::ReplyError {
                code: -32602,
                message: "unknown tool: ghost".to_string(),
            }],
            _ => vec![Action::Silence],
        });

        assert_eq!(
            client.call_tool("ghost", &json!({}), None).await,
            McpCallOutcome::ToolNotFound
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn other_remote_errors_map_to_internal() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::ReplyError {
                code: -32000,
                message: "server exploded".to_string(),
            }],
            _ => vec![Action::Silence],
        });

        match client.call_tool("search", &json!({}), None).await {
            McpCallOutcome::Internal { message } => {
                assert!(message.contains("server exploded"), "{message}")
            }
            other => panic!("expected internal outcome, got {other:?}"),
        }

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn malformed_frames_are_skipped_and_reading_continues() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "initialize" => vec![
                Action::Raw(b"this is not json".to_vec()),
                Action::Raw(vec![0xff, 0xfe, 0x00]),
                Action::Raw(b"{\"jsonrpc\":\"1.0\",\"id\":1}".to_vec()),
                Action::Reply(json!({ "protocolVersion": "2024-11-05" })),
            ],
            "tools/list" => vec![
                Action::Raw(b"{\"jsonrpc\":\"2.0\",\"id\":999,\"result\":{}}".to_vec()),
                Action::Reply(json!({ "tools": [{ "name": "ok" }] })),
            ],
            _ => vec![Action::Silence],
        });

        client
            .initialize()
            .await
            .expect("handshake survives bad frames");
        let defs = client
            .list_tools()
            .await
            .expect("tools/list survives bad frames");
        assert_eq!(defs.len(), 1);

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn eof_converges_in_flight_requests() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::Close],
            _ => vec![Action::Silence],
        });

        let outcome = client.call_tool("boom", &json!({}), None).await;
        assert_eq!(outcome, McpCallOutcome::ServerUnavailable);
        assert!(client.is_closed());

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn server_requests_are_rejected_with_method_not_found() {
        let (client, reader, mut writer) = harness();
        let server = tokio::spawn(async move {
            let request = json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "sampling/createMessage",
            });
            let Ok(line) = serde_json::to_string(&request) else {
                return Value::Null;
            };
            if writer.write_all(line.as_bytes()).await.is_err()
                || writer.write_all(b"\n").await.is_err()
            {
                return Value::Null;
            }
            let _ = writer.flush().await;

            let mut reader = BufReader::new(reader);
            let mut buf = Vec::new();
            if reader.read_until(b'\n', &mut buf).await.is_err() {
                return Value::Null;
            }
            serde_json::from_slice(&buf).unwrap_or(Value::Null)
        });

        let reply = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("server should receive a reply")
            .expect("server task");

        assert_eq!(reply.get("id").and_then(Value::as_u64), Some(7));
        assert_eq!(
            reply.get("error").and_then(|e| e.get("code")),
            Some(&json!(-32601))
        );

        drop(client);
    }

    #[tokio::test]
    async fn request_after_close_is_server_unavailable() {
        let (client, reader, writer) = harness();
        let server = spawn_server(reader, writer, |_, _| vec![Action::Close]);

        client
            .initialize()
            .await
            .expect_err("handshake on closed pipe");
        assert_eq!(
            client.call_tool("search", &json!({}), None).await,
            McpCallOutcome::ServerUnavailable
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn timeout_maps_to_timeout_outcome_and_client_stays_usable() {
        let (client, reader, writer) = harness();
        let client = client.with_call_timeout(Duration::from_millis(200));
        let server = spawn_server(reader, writer, |method, _| match method {
            "tools/call" => vec![Action::Silence],
            "tools/list" => vec![Action::Reply(json!({ "tools": [] }))],
            _ => vec![Action::Silence],
        });

        assert_eq!(
            client.call_tool("slow", &json!({}), None).await,
            McpCallOutcome::Timeout
        );
        assert!(
            client.list_tools().await.is_ok(),
            "超时只中止本次请求，客户端仍可用"
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn cancellation_returns_cancelled_and_keeps_client_usable() {
        let (client, reader, writer) = harness();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let server_seen = Arc::clone(&seen);
        let server = spawn_server(reader, writer, move |method, _| {
            server_seen.lock().unwrap().push(method.to_string());
            match method {
                "tools/list" => vec![Action::Reply(json!({ "tools": [] }))],
                _ => vec![Action::Silence],
            }
        });

        let token = CancellationToken::new();
        let cancel_token = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_token.cancel();
        });

        assert_eq!(
            client.call_tool("slow", &json!({}), Some(&token)).await,
            McpCallOutcome::Cancelled
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            client.list_tools().await.is_ok(),
            "取消只中止本次请求，后续调用照常可用"
        );

        let methods = seen.lock().unwrap().clone();
        assert!(
            methods
                .iter()
                .any(|method| method == "notifications/cancelled"),
            "取消时应尽力通知 server，实际收到: {methods:?}"
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn already_cancelled_token_short_circuits_without_sending() {
        let (client, reader, writer) = harness();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let server_seen = Arc::clone(&seen);
        let server = spawn_server(reader, writer, move |method, _| {
            server_seen.lock().unwrap().push(method.to_string());
            vec![Action::Silence]
        });

        let token = CancellationToken::new();
        token.cancel();

        assert_eq!(
            client.call_tool("slow", &json!({}), Some(&token)).await,
            McpCallOutcome::Cancelled
        );

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            seen.lock().unwrap().is_empty(),
            "已取消的令牌不应再发出 tools/call"
        );

        drop(client);
        join_server(server).await;
    }

    #[tokio::test]
    async fn stderr_logger_does_not_panic_on_non_utf8() {
        let (mut writer, reader) = duplex(4096);
        let task = spawn_stderr_logger(reader, "srv-1".to_string());
        let _ = writer.write_all(&[0xff, 0xfe, b'\n']).await;
        let _ = writer.write_all(b"plain line\n").await;
        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    }
}
