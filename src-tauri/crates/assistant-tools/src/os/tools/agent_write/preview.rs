use crate::context::ToolUseContext;
use crate::os::output::human_bytes;
use crate::os::path;
use crate::r#trait::ApprovalPreview;

use super::lines::{count_lines, count_lines_in_file};
use super::WriteRequest;

/// 审批预演：只取 `metadata` 与流式行计数，不把内容读进内存。
pub(super) fn build_preview(
    request: &WriteRequest,
    context: &ToolUseContext,
) -> Option<ApprovalPreview> {
    let resolved = path::resolve(&request.path, context).ok()?;
    let existing = std::fs::metadata(&resolved)
        .ok()
        .filter(|metadata| metadata.is_file());

    Some(match existing {
        Some(metadata) => {
            let lines = count_lines_in_file(&resolved)?;
            let verb = if request.append {
                "追加到"
            } else {
                "覆盖"
            };
            ApprovalPreview {
                summary: format!(
                    "{} 1 个现有文件（当前 {} 行 / {}）",
                    verb,
                    lines,
                    human_bytes(metadata.len() as usize)
                ),
                truncated: false,
                estimated: false,
            }
        }
        None => ApprovalPreview {
            summary: format!(
                "新建文件（{} 行 / {}）",
                count_lines(request.content.as_bytes()),
                human_bytes(request.content.len())
            ),
            truncated: false,
            estimated: false,
        },
    })
}
