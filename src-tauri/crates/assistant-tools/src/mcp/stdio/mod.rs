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

use std::time::Duration;

mod client;
mod codec;
mod stderr;
mod transport;

pub use client::{McpClient, RpcFailure};
pub use stderr::spawn_stderr_logger;

/// 单次 `tools/call` 的默认超时（规格 §4.3）。
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// 握手 / `tools/list` 的默认超时（规格 §4.2）。
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// v1 只声明 tools 能力，不涉及 resources / prompts / sampling（规格 §9 已定稿）。
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// `tools/list` 分页上限：防止 server 返回循环 cursor 让启动永不结束。
const MAX_TOOL_PAGES: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::bridge::{McpCallOutcome, McpCaller};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{
        duplex, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, DuplexStream, ReadHalf,
        WriteHalf,
    };
    use tokio_util::sync::CancellationToken;

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
