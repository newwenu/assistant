use serde_json::Value;

use crate::result::{ToolErrorKind, ToolResult};
use crate::storage::PersistedRef;

use super::anchor::build_anchor;
use super::error_kind::error_kind_from_text;
use super::head::build_head;
use super::truncation::{parse_persisted_tag, parse_truncation};
use super::types::{PersistedInfo, ToolResultCompact};

/// 由结果事实渲染信封。`persisted` 必须来自存储后端返回的引用，
/// 不得由文本解析产生（门禁 8）。
pub fn build_compact(
    tool: &str,
    arguments: &Value,
    result: &ToolResult,
    persisted: Option<&PersistedRef>,
) -> ToolResultCompact {
    let text = result.assistant_text();
    let data = result.data.as_ref();
    ToolResultCompact {
        head: build_head(tool, arguments, data, result),
        truncated: parse_truncation(&text),
        persisted: persisted.map(|reference| PersistedInfo {
            // 卡片展示引用而非本机绝对路径（I73 §3.6 R3）。
            path: reference.model_ref(),
            bytes: Some(reference.bytes as u64),
        }),
        error_kind: if result.ok {
            None
        } else {
            Some(result.error_kind.unwrap_or(ToolErrorKind::Business))
        },
        anchor: build_anchor(tool, arguments, data),
    }
}

/// 从历史行重建信封：结果文本 + 参数 + 状态。`error_kind` 只在文本可判定时给出。
///
/// 这条路径是读取回退（§3.4 第三行），允许解析 `<persisted-output>` 标签；
/// 标签本身由 `PersistedRef` 渲染，round-trip 保证一致。
pub fn rebuild_compact(
    tool: &str,
    arguments: &Value,
    result_text: Option<&str>,
    is_error: bool,
) -> ToolResultCompact {
    let text = result_text.unwrap_or_default();
    let result = if is_error {
        ToolResult {
            ok: false,
            data: None,
            result_for_assistant: None,
            error: Some(text.to_string()),
            error_kind: error_kind_from_text(text),
            persisted: None,
        }
    } else {
        ToolResult {
            ok: true,
            data: None,
            result_for_assistant: Some(text.to_string()),
            error: None,
            error_kind: None,
            persisted: None,
        }
    };
    ToolResultCompact {
        head: build_head(tool, arguments, None, &result),
        truncated: parse_truncation(text),
        persisted: parse_persisted_tag(text),
        error_kind: result.error_kind,
        anchor: None,
    }
}
