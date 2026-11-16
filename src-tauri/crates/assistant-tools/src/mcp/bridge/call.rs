use std::panic::AssertUnwindSafe;

use async_trait::async_trait;
use futures::FutureExt;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::result::{ToolErrorKind, ToolResult};

/// 一次 `tools/call` 的归一化结局。
#[derive(Debug, Clone, PartialEq)]
pub enum McpCallOutcome {
    /// server 正常应答（`isError` 由 MCP 侧声明）。
    Content { blocks: Vec<Value>, is_error: bool },
    /// server 不认识这个工具（`tools/list` 之后被下线）。
    ToolNotFound,
    /// 进程已退出 / 管道断开。
    ServerUnavailable,
    /// 客户端本地超时。`exec::invoke` 的统一 deadline 是另一层，两层都归 `Timeout`。
    Timeout,
    /// turn 取消（用户停止）：只中止本次请求，进程与工具保留（规格 §4.3）。
    Cancelled,
    /// 协议错误、非法帧等客户端内部失败。
    Internal { message: String },
}

/// `McpTool` 的传输抽象：C1 只定义契约，C2 用 stdio 实现。
#[async_trait]
pub trait McpCaller: Send + Sync {
    /// `tool` 是**原始** MCP 工具名（非规范化名）。
    ///
    /// `cancel` 来自 `ToolUseContext.cancel`；`None` 表示本次调用不响应取消。
    async fn call_tool(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: Option<&CancellationToken>,
    ) -> McpCallOutcome;
}

/// 调用边界：把传输实现的 panic 收敛成一次失败调用（规格 §4.4 硬约束三）。
///
/// 只在 unwind 构建下有效；release 是 `panic = "abort"`，那层防线靠
/// 「客户端解析零 panic」与「读取任务自带收敛」两条硬约束。
pub async fn guarded_call(
    caller: &dyn McpCaller,
    tool: &str,
    arguments: &Value,
    cancel: Option<&CancellationToken>,
) -> McpCallOutcome {
    match AssertUnwindSafe(caller.call_tool(tool, arguments, cancel))
        .catch_unwind()
        .await
    {
        Ok(outcome) => outcome,
        Err(_) => McpCallOutcome::Internal {
            message: "MCP client panicked while handling the call".to_string(),
        },
    }
}

pub(super) fn map_outcome(outcome: McpCallOutcome) -> ToolResult {
    match outcome {
        McpCallOutcome::Content { blocks, is_error } => map_content(blocks, is_error),
        McpCallOutcome::ToolNotFound => ToolResult::err("MCP tool is not registered on the server"),
        McpCallOutcome::ServerUnavailable => {
            ToolResult::err_with_kind(ToolErrorKind::Internal, "MCP server is unavailable")
        }
        McpCallOutcome::Timeout => {
            ToolResult::err_with_kind(ToolErrorKind::Timeout, "MCP call timed out")
        }
        McpCallOutcome::Cancelled => {
            ToolResult::err_with_kind(ToolErrorKind::Cancelled, "MCP call was cancelled")
        }
        McpCallOutcome::Internal { message } => {
            ToolResult::err_with_kind(ToolErrorKind::Internal, message)
        }
    }
}

/// 内容块 → `ToolResult`。
///
/// 只有文本块进入 `result_for_assistant`；图片 / 资源块原样留在 `data`，
/// 避免 base64 负载被当成模型文本回填。无文本块时给计数摘要，
/// 禁止回退到 `data` 序列化（`assistant_text` 的兜底路径会把整个负载写进模型通道）。
fn map_content(blocks: Vec<Value>, is_error: bool) -> ToolResult {
    let text = blocks
        .iter()
        .filter_map(text_block)
        .collect::<Vec<_>>()
        .join("\n");
    let data = Value::Array(blocks.clone());

    if is_error {
        let message = if text.is_empty() {
            "MCP tool returned an error".to_string()
        } else {
            text
        };
        return ToolResult {
            ok: false,
            data: Some(data),
            result_for_assistant: None,
            error: Some(message),
            error_kind: Some(ToolErrorKind::Business),
        };
    }

    let assistant = if text.is_empty() {
        format!(
            "MCP tool returned {} content block(s) without text",
            blocks.len()
        )
    } else {
        text
    };
    ToolResult {
        ok: true,
        data: Some(data),
        result_for_assistant: Some(assistant),
        error: None,
        error_kind: None,
    }
}

fn text_block(block: &Value) -> Option<&str> {
    if block.get("type").and_then(Value::as_str) == Some("text") {
        block.get("text").and_then(Value::as_str)
    } else {
        None
    }
}
