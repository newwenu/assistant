use serde_json::Value;

use super::types::{DocAnchor, LineRange};
use super::{args_str, data_str, data_u64};

/// 锚点只在结果时刻取（门禁 11）；`edit` 的行区间来自实际写入，不由 `args` 估算。
pub(super) fn build_anchor(
    tool: &str,
    arguments: &Value,
    data: Option<&Value>,
) -> Option<DocAnchor> {
    let path = data_str(data, "path").or_else(|| args_str(arguments, "path"))?;
    match tool {
        "edit" => {
            let start = data_u64(data, "startLine")?;
            let end = data_u64(data, "newEndLine")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange { start, end }],
            })
        }
        "write" => {
            let lines = data_u64(data, "lines")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange {
                    start: 1,
                    end: lines.max(1),
                }],
            })
        }
        "read" => {
            let start = data_u64(data, "startLine")?;
            let end = data_u64(data, "endLine")?;
            Some(DocAnchor {
                path,
                ranges: vec![LineRange { start, end }],
            })
        }
        _ => None,
    }
}
