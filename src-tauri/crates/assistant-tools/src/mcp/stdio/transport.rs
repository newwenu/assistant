use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

use super::client::{PendingGuard, PendingMap, SharedWriter};
use super::codec::{deliver_reply, send_frame, MAX_FRAME_BYTES};

pub(super) async fn read_loop(
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
