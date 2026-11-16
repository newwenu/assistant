use std::collections::HashMap;

use sha2::{Digest, Sha256};

use super::defs::McpToolDef;

/// 模型可见名的固定前缀（规格 §3.1）。
pub const TOOL_PREFIX: &str = "mcp";
const NAME_SEPARATOR: &str = "__";
const SHORT_HASH_LEN: usize = 8;

/// 一个 MCP 工具的三层名字。
///
/// `full` 是模型可见名（稳定标识，展示名不参与）；`original` 是发给 server 的原始名；
/// `display` 只给 UI 用，可随 server 展示名变化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolName {
    pub full: String,
    pub server_id: String,
    pub tool: String,
    pub original: String,
    pub display: String,
}

/// 段规范化：非 `[a-zA-Z0-9_-]` 替换为 `_`；空段回退 `_`（规格 §3.1）。
pub fn normalize_segment(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if replaced.is_empty() {
        "_".to_string()
    } else {
        replaced
    }
}

/// 为一次 `tools/list` 快照生成工具名。
///
/// 顺序与 `defs` 一致；同一 server 内规范化后重名的，追加原始名的短哈希
/// （8 位 hex，`_` 分隔），避免两个工具共用同一个模型可见名。
pub fn build_tool_names(
    server_id: &str,
    server_display_name: &str,
    defs: &[McpToolDef],
) -> Vec<McpToolName> {
    let normalized_server = normalize_segment(server_id);
    let display_server = if server_display_name.trim().is_empty() {
        server_id
    } else {
        server_display_name.trim()
    };

    let normalized: Vec<String> = defs
        .iter()
        .map(|def| normalize_segment(&def.name))
        .collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in &normalized {
        *counts.entry(name.clone()).or_default() += 1;
    }

    defs.iter()
        .zip(&normalized)
        .map(|(def, base)| {
            let tool = if counts.get(base).copied().unwrap_or(0) > 1 {
                format!("{}_{}", base, short_hash(&def.name))
            } else {
                base.clone()
            };
            McpToolName {
                full: format!(
                    "{TOOL_PREFIX}{NAME_SEPARATOR}{normalized_server}{NAME_SEPARATOR}{tool}"
                ),
                server_id: normalized_server.clone(),
                tool,
                original: def.name.clone(),
                display: format!("{display_server} / {}", def.name),
            }
        })
        .collect()
}

pub(super) fn short_hash(raw: &str) -> String {
    let digest = hex::encode(Sha256::digest(raw.as_bytes()));
    digest[..SHORT_HASH_LEN].to_string()
}
