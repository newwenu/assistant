use serde_json::Value;
use tokio::io::AsyncWriteExt;

use super::client::{with_pending, PendingMap, SharedWriter};

/// 单帧上限：超过即丢弃。防止畸形 server 用超长行撑爆内存。
pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

pub(super) enum RpcReply {
    Result(Value),
    Error { code: i64, message: String },
    Closed,
}

pub(super) async fn send_frame(writer: &SharedWriter, frame: &Value) -> bool {
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

pub(super) fn deliver_reply(pending: &PendingMap, id: u64, frame: &Value) {
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
