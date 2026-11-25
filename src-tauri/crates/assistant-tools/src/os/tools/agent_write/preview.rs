use crate::context::ToolUseContext;
use crate::r#trait::ApprovalPreview;

use super::lines::{count_lines, count_lines_in_file};
use super::WriteRequest;

/// 审批预演：只取 `metadata` 与流式行计数，不把内容读进内存。
///
/// 结构化字段齐备时 `summary` 置空串（I72 §5.3）：`exists` / `entries` / `bytes`
/// 已完整表达「新建 / 覆盖 / 追加」与规模，散文只是同一内容的重复。
/// `entries` 是行数、`bytes` 是字节数，两者量纲由卡片按字段名表达。
pub(super) fn build_preview(
    request: &WriteRequest,
    context: &ToolUseContext,
) -> Option<ApprovalPreview> {
    let resolved = request.path.resolve_write(context).ok()?;
    let existing = std::fs::metadata(&resolved)
        .ok()
        .filter(|metadata| metadata.is_file());

    Some(match existing {
        Some(metadata) => ApprovalPreview {
            summary: String::new(),
            truncated: false,
            estimated: false,
            entries: count_lines_in_file(&resolved).map(|lines| lines as u64),
            bytes: Some(metadata.len()),
            target_kind: Some("file".to_string()),
            exists: Some(true),
            symlink: None,
            verb_code: None,
            shell: None,
        },
        None => ApprovalPreview {
            summary: String::new(),
            truncated: false,
            estimated: false,
            entries: Some(count_lines(request.content.as_bytes()) as u64),
            bytes: Some(request.content.len() as u64),
            target_kind: Some("file".to_string()),
            exists: Some(false),
            symlink: None,
            verb_code: None,
            shell: None,
        },
    })
}
