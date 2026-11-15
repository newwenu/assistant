//! dev-mcp：C5 端到端夹具用的 stdio MCP server。
//!
//! 契约严格对齐 `assistant-tools/src/mcp/stdio.rs`：
//! - 行分隔 JSON-RPC 2.0；stdout 只写协议帧，日志一律走 stderr
//! - `initialize` 响应必须带 `protocolVersion`（客户端校验这一项）
//! - 未知工具返回 `-32601` + `unknown tool: X`（映射到 `ToolNotFound`）
//! - 每个 `tools/call` 独立线程，慢工具不阻塞其它请求
//! - `notifications/cancelled` 只记日志，不中断在途调用（验证「取消不杀进程」）
//! - 非法帧只影响自己，服务继续读下一帧
//!
//! 这个二进制只服务于 `tests/mcp_e2e.rs`，不进 Tauri bundle。

use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2024-11-05";
/// 客户端 `MAX_FRAME_BYTES = 8 MiB`；留出 JSON 包装开销，避免整帧被丢弃。
const MAX_FLOOD_CHARS: usize = 8 * 1024 * 1024 - 128 * 1024;
const MAX_APPEND_FILE_SEGMENT: usize = 64;

fn log(message: &str) {
    eprintln!("[dev-mcp] {message}");
}

fn emit(frame: &Value) {
    let text = serde_json::to_string(frame).unwrap_or_else(|_| "{}".to_string());
    let mut out = io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn emit_raw(line: &str) {
    let mut out = io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn reply_result(id: &Value, result: Value) {
    emit(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
}

fn reply_error(id: &Value, code: i64, message: &str) {
    emit(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }));
}

fn text_result(text: String, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

fn tools() -> Value {
    json!([
        {
            "name": "dev_echo",
            "description": "Echo the received arguments back as compact JSON text.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": true,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "dev_append",
            "description": "Append one line to a single-segment file under the work dir.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Single-segment file name, no separators." },
                    "line": { "type": "string", "description": "Text to append." },
                },
                "required": ["file", "line"],
                "additionalProperties": false,
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": true,
                "openWorldHint": false,
            },
        },
        {
            "name": "dev_fail",
            "description": "Always answer with isError=true.",
            "inputSchema": {
                "type": "object",
                "properties": { "reason": { "type": "string" } },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "dev_sleep",
            "description": "Sleep for the requested milliseconds, then answer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ms": { "type": "integer", "minimum": 0, "maximum": 600000 },
                },
                "required": ["ms"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "dev_flood",
            "description": "Return a large text payload to exercise the output budget.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bytes": { "type": "integer", "minimum": 0 },
                    "marker": { "type": "string" },
                },
                "required": ["bytes"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "dev_crash",
            "description": "Terminate the server process without answering.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true },
        },
        {
            "name": "dev_bad_frame",
            "description": "Emit one malformed frame, then answer normally.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
    ])
}

/// 单次 `tools/call` 的结局：`Err` 走 JSON-RPC error 通道，`Ok` 是 `result`。
type ToolReply = Result<Value, (i64, String)>;

fn invalid_params(detail: &str) -> (i64, String) {
    (-32602, format!("invalid params: {detail}"))
}

fn non_negative_int(arguments: &Value, key: &str) -> Result<u64, (i64, String)> {
    match arguments.get(key) {
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) => Ok(value),
            None => Err(invalid_params(&format!(
                "{key} must be a non-negative integer"
            ))),
        },
        _ => Err(invalid_params(&format!(
            "{key} must be a non-negative integer"
        ))),
    }
}

fn check_file_segment(name: Option<&str>) -> Result<String, (i64, String)> {
    let Some(name) = name else {
        return Err(invalid_params("file must be a non-empty string"));
    };
    if name.is_empty() {
        return Err(invalid_params("file must be a non-empty string"));
    }
    if name.chars().count() > MAX_APPEND_FILE_SEGMENT {
        return Err(invalid_params("file name is too long"));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(invalid_params("file must be a single path segment"));
    }
    if name.starts_with('.') {
        return Err(invalid_params("file must not start with a dot"));
    }
    Ok(name.to_string())
}

fn run_tool(name: &str, arguments: &Value, work_dir: &Path) -> ToolReply {
    match name {
        "dev_echo" => Ok(text_result(
            serde_json::to_string(arguments).unwrap_or_else(|_| "{}".to_string()),
            false,
        )),

        "dev_append" => {
            let file = check_file_segment(arguments.get("file").and_then(Value::as_str))?;
            let Some(line) = arguments.get("line").and_then(Value::as_str) else {
                return Err(invalid_params("line must be a string"));
            };
            let path = work_dir.join(&file);
            let appended = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut handle| handle.write_all(format!("{line}\n").as_bytes()));
            match appended {
                Ok(()) => Ok(text_result(format!("appended to {file}"), false)),
                Err(error) => Ok(text_result(format!("append failed: {error}"), true)),
            }
        }

        "dev_fail" => {
            let reason = arguments
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("requested failure");
            Ok(text_result(format!("dev_fail: {reason}"), true))
        }

        "dev_sleep" => {
            let ms = non_negative_int(arguments, "ms")?;
            thread::sleep(Duration::from_millis(ms));
            Ok(text_result(format!("slept {ms} ms"), false))
        }

        "dev_flood" => {
            let size = non_negative_int(arguments, "bytes")? as usize;
            if size > MAX_FLOOD_CHARS {
                return Err(invalid_params(&format!(
                    "bytes exceeds {MAX_FLOOD_CHARS} limit"
                )));
            }
            let marker = arguments
                .get("marker")
                .and_then(Value::as_str)
                .unwrap_or("");
            let body: String = marker
                .chars()
                .chain(std::iter::repeat('x'))
                .take(size)
                .collect();
            Ok(text_result(body, false))
        }

        "dev_bad_frame" => {
            emit_raw("{ this is not json");
            Ok(text_result("malformed frame emitted".to_string(), false))
        }

        "dev_crash" => {
            log("dev_crash: exiting process");
            let _ = io::stdout().flush();
            process::exit(1);
        }

        _ => Err((-32601, format!("unknown tool: {name}"))),
    }
}

fn handle_tool_call(id: &Value, params: &Value, work_dir: &Path) {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        reply_error(id, -32602, "invalid params: name must be a string");
        return;
    };
    if name.is_empty() {
        reply_error(id, -32602, "invalid params: name must be a string");
        return;
    }
    let arguments = match params.get("arguments") {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        Some(Value::Null) | None => json!({}),
        Some(_) => {
            reply_error(id, -32602, "invalid params: arguments must be an object");
            return;
        }
    };

    match run_tool(name, &arguments, work_dir) {
        Ok(result) => reply_result(id, result),
        Err((code, message)) => reply_error(id, code, &message),
    }
}

type WorkerList = Arc<Mutex<Vec<thread::JoinHandle<()>>>>;

fn spawn_worker(id: Value, params: Value, work_dir: PathBuf, workers: &WorkerList) {
    let handle = thread::spawn(move || {
        handle_tool_call(&id, &params, &work_dir);
    });
    if let Ok(mut guard) = workers.lock() {
        guard.push(handle);
    }
}

fn dispatch(frame: &Value, work_dir: &Path, workers: &WorkerList) {
    let Some(method) = frame.get("method").and_then(Value::as_str) else {
        return;
    };
    let params = match frame.get("params") {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        _ => json!({}),
    };
    let id = frame.get("id").cloned();

    // 通知：没有 id，不产生响应。
    let Some(id) = id else {
        if method == "notifications/cancelled" {
            let cancelled = params.get("requestId").cloned().unwrap_or(Value::Null);
            log(&format!("received notifications/cancelled for {cancelled}"));
        } else if method == "notifications/initialized" {
            log("client initialized");
        } else {
            log(&format!("ignored notification {method}"));
        }
        return;
    };

    match method {
        "initialize" => reply_result(
            &id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "dev-mcp", "version": "0.1.0" },
            }),
        ),
        "ping" => reply_result(&id, json!({})),
        "tools/list" => reply_result(&id, json!({ "tools": tools() })),
        "tools/call" => spawn_worker(id, params, work_dir.to_path_buf(), workers),
        other => reply_error(&id, -32601, &format!("method not found: {other}")),
    }
}

fn parse_work_dir() -> PathBuf {
    let mut args = env::args().skip(1);
    let mut work_dir = env::temp_dir().join("dev-mcp-work");
    while let Some(arg) = args.next() {
        if arg == "--work-dir" {
            if let Some(value) = args.next() {
                work_dir = PathBuf::from(value);
            }
        }
    }
    work_dir
}

fn main() {
    let work_dir = parse_work_dir();
    if let Err(error) = std::fs::create_dir_all(&work_dir) {
        log(&format!(
            "cannot create work dir {}: {error}",
            work_dir.display()
        ));
        process::exit(2);
    }

    log(&format!("started; work-dir={}", work_dir.display()));

    let workers: WorkerList = Arc::new(Mutex::new(Vec::new()));
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) => {
                log(&format!("stdin read failed: {error}"));
                break;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let frame = match serde_json::from_str::<Value>(trimmed) {
            Ok(frame) => frame,
            Err(_) => {
                log("dropped malformed frame");
                continue;
            }
        };
        if !frame.is_object() {
            log("dropped non-object frame");
            continue;
        }
        if frame.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            log("dropped frame without jsonrpc 2.0");
            continue;
        }
        dispatch(&frame, &work_dir, &workers);
    }

    log("stdin closed; draining in-flight calls");
    let pending = match workers.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => Vec::new(),
    };
    for handle in pending {
        let _ = handle.join();
    }
    log("bye");
}
