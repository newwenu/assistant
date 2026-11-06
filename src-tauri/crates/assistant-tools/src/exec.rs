use std::time::Duration;

use serde_json::Value;

use crate::context::ToolUseContext;
use crate::r#trait::Tool;
use crate::result::{ToolErrorKind, ToolResult};

pub const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// 工具执行边界的唯一入口。边界只返回 `ToolResult`，永不返回 `Err`：
/// 工具的 `Err(String)` 在此归一化为 `ToolResult::err`，对上游不可见。
///
/// 错误前缀取自 `tool.id()`（内部唯一标识），工具实现自身声明身份。
///
/// 超时的能力边界：`tokio::time::timeout` 只在 `await` 点抢占。
/// - 异步且 `await` 点密集的工具：超时直接生效。
/// - 同步阻塞工具（`std::fs` / `std::process::Command`）：必须自行用 `spawn_blocking` 包裹；
///   超时只保证调用方及时返回，被阻塞的线程仍会跑完，无法中断。
/// - 子进程类工具（`shell_exec`）：不得依赖本函数的超时，必须做进程级超时 + kill。
pub async fn invoke(
    tool: &dyn Tool,
    input: &Value,
    context: &ToolUseContext,
    timeout: Duration,
) -> ToolResult {
    if !input.is_object() {
        return ToolResult::err("input must be a JSON object");
    }

    let id = tool.id();
    match tokio::time::timeout(timeout, tool.execute(input, context)).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => ToolResult::from_exec_error(id, error),
        Err(_) => ToolResult::from_exec_error_kind(
            id,
            ToolErrorKind::Timeout,
            format!("timed out after {:?}", timeout),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#trait::ToolTier;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    enum StubBehavior {
        Ok,
        Fail(&'static str),
        Slow(Duration),
        Blocking(Duration, Arc<AtomicBool>),
    }

    struct StubTool {
        behavior: StubBehavior,
    }

    #[async_trait]
    impl Tool for StubTool {
        fn id(&self) -> &str {
            "stub"
        }

        async fn description(&self) -> Result<String, String> {
            Ok("stub".to_string())
        }

        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }

        fn tier(&self) -> ToolTier {
            ToolTier::Os
        }

        async fn execute(
            &self,
            _input: &Value,
            _context: &ToolUseContext,
        ) -> Result<ToolResult, String> {
            match &self.behavior {
                StubBehavior::Ok => Ok(ToolResult::ok(json!({ "value": 1 }))),
                StubBehavior::Fail(message) => Err(message.to_string()),
                StubBehavior::Slow(delay) => {
                    tokio::time::sleep(*delay).await;
                    Ok(ToolResult::ok(json!({ "slow": true })))
                }
                StubBehavior::Blocking(delay, finished) => {
                    let delay = *delay;
                    let finished = Arc::clone(finished);
                    tokio::task::spawn_blocking(move || {
                        std::thread::sleep(delay);
                        finished.store(true, Ordering::SeqCst);
                    })
                    .await
                    .expect("blocking task should not panic");
                    Ok(ToolResult::ok(json!({ "blocking": true })))
                }
            }
        }
    }

    fn context() -> ToolUseContext {
        ToolUseContext::for_toolbox(None)
    }

    fn stub(behavior: StubBehavior) -> StubTool {
        StubTool { behavior }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_non_object_input() {
        let tool = stub(StubBehavior::Ok);
        let result = invoke(
            &tool,
            &json!("not an object"),
            &context(),
            DEFAULT_TOOL_TIMEOUT,
        )
        .await;

        assert!(!result.ok);
        assert!(result.data.is_none());
        assert_eq!(result.error.as_deref(), Some("input must be a JSON object"));
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn passes_through_ok_result() {
        let tool = stub(StubBehavior::Ok);
        let result = invoke(&tool, &json!({}), &context(), DEFAULT_TOOL_TIMEOUT).await;

        assert!(result.ok);
        assert_eq!(result.data, Some(json!({ "value": 1 })));
        assert!(result.error.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn normalizes_exec_error_with_tool_prefix() {
        let tool = stub(StubBehavior::Fail("boom"));
        let result = invoke(&tool, &json!({}), &context(), Duration::from_secs(5)).await;

        assert!(!result.ok);
        assert!(result.data.is_none());
        assert_eq!(result.error.as_deref(), Some("tool 'stub': boom"));
        assert_eq!(result.error_kind, Some(ToolErrorKind::Internal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn times_out_slow_async_tool() {
        let tool = stub(StubBehavior::Slow(Duration::from_millis(300)));
        let result = invoke(&tool, &json!({}), &context(), Duration::from_millis(50)).await;

        assert!(!result.ok);
        let error = result.error.unwrap_or_default();
        assert!(
            error.contains("timed out after"),
            "unexpected timeout error: {}",
            error
        );
        assert_eq!(result.error_kind, Some(ToolErrorKind::Timeout));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn timeout_returns_before_blocking_work_finishes() {
        let finished = Arc::new(AtomicBool::new(false));
        let tool = stub(StubBehavior::Blocking(
            Duration::from_millis(400),
            Arc::clone(&finished),
        ));

        let started = std::time::Instant::now();
        let result = invoke(&tool, &json!({}), &context(), Duration::from_millis(50)).await;
        let elapsed = started.elapsed();

        assert!(!result.ok);
        assert!(
            elapsed < Duration::from_millis(250),
            "invoke should return early, elapsed {:?}",
            elapsed
        );
        assert!(
            !finished.load(Ordering::SeqCst),
            "blocking work must still be running after timeout"
        );

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            finished.load(Ordering::SeqCst),
            "blocking work should finish on its own"
        );
    }
}
