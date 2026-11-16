use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::context::ToolUseContext;
use crate::r#trait::{PermissionIntent, ToolTier};

/// 审批缓存键的作用域。键必须精确描述「用户批准了什么」，
/// 不允许前缀匹配：批准 `git status` 不等于批准 `git status --short`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ApprovalScope {
    Path {
        canonical: String,
    },
    Command {
        argv: Vec<String>,
        cwd: String,
        sandbox_tier: ToolTier,
    },
    Endpoint {
        host: String,
        method: String,
    },
    Opaque {
        fingerprint: String,
    },
    /// MCP 外部工具的精确键（I38e §3.5）：四个字段全等才命中。
    ///
    /// 不存在工具级宽授权：同 server 的其它工具、其它参数、其它配置都不命中。
    Mcp {
        server_id: String,
        tool: String,
        args_fingerprint: String,
        config_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalKey {
    pub tool: String,
    pub scope: ApprovalScope,
}

impl ApprovalKey {
    pub fn new(tool: impl Into<String>, scope: ApprovalScope) -> Self {
        Self {
            tool: tool.into(),
            scope,
        }
    }

    /// 生成单个 resource 的审批键。
    ///
    /// 返回 `None` 表示「空键」：无法归一（路径不可解析、命令分词失败、
    /// 资源为空）时调用方必须直接走审批，不得查询缓存。
    pub fn from_intent(
        tool: &str,
        intent: &PermissionIntent,
        resource: &str,
        input: &Value,
        context: &ToolUseContext,
        tier: ToolTier,
    ) -> Option<Self> {
        let scope = match intent.action.as_str() {
            "read" | "edit" | "write" | "delete" | "move" | "copy" => ApprovalScope::Path {
                canonical: canonical_path_for_key(resource)?,
            },
            "execute" | "run" | "shell" => ApprovalScope::Command {
                argv: split_argv(resource)?,
                cwd: command_cwd(input, context),
                sandbox_tier: tier,
            },
            "network" | "fetch" | "http" => {
                let (host, method) = parse_endpoint(resource, input)?;
                ApprovalScope::Endpoint { host, method }
            }
            // MCP 键必须精确到一次调用：server + 工具 + 规范化参数指纹 + 配置哈希
            // 全等才命中（I38e §3.5），不得退化为工具级或 server 级放行。
            "mcp" => {
                let (server_id, config_hash) = parse_mcp_resource(resource)?;
                ApprovalScope::Mcp {
                    tool: mcp_tool_segment(tool, &server_id)?,
                    args_fingerprint: canonical_json_sha256(input)?,
                    server_id,
                    config_hash,
                }
            }
            _ => ApprovalScope::Opaque {
                fingerprint: opaque_fingerprint(resource)?,
            },
        };
        Some(Self::new(tool, scope))
    }
}

/// 路径归一：存在的路径交给 `canonicalize`（消解 `..` 与符号链接），
/// 不存在的路径退化为「已存在祖先归一 + 原样尾部」。
///
/// 归一结果必须是合法 UTF-8：审批键是缓存的**匹配键**，用 `to_string_lossy`
/// 会让两条不同的非 UTF-8 路径塌缩成同一键，方向是放宽权限。
/// 无法无损转成字符串时返回 `None`，调用方据此走审批、不查缓存（fail-closed）。
fn canonical_path_for_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    let canonical = dunce::canonicalize(path)
        .or_else(|_| crate::os::path::canonicalize_lenient(path).map_err(std::io::Error::other))
        .ok()?;
    canonical.to_str().map(str::to_string)
}

/// 命令分词：按 shell 风格引号切分为完整 argv。
/// 未闭合引号返回 `None`，避免用半截命令生成可复用键。
fn split_argv(command: &str) -> Option<Vec<String>> {
    let mut argv: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut pending = false;

    for ch in command.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                pending = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                pending = true;
            }
            ch if ch.is_whitespace() && !in_single && !in_double => {
                if pending {
                    argv.push(std::mem::take(&mut current));
                    pending = false;
                }
            }
            ch => {
                current.push(ch);
                pending = true;
            }
        }
    }

    if in_single || in_double {
        return None;
    }
    if pending {
        argv.push(current);
    }
    if argv.is_empty() {
        None
    } else {
        Some(argv)
    }
}

fn command_cwd(input: &Value, context: &ToolUseContext) -> String {
    let raw = input
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| context.workspace_root.clone());
    match raw {
        Some(value) if !value.trim().is_empty() => canonical_path_for_key(&value).unwrap_or(value),
        _ => String::new(),
    }
}

fn parse_endpoint(resource: &str, input: &Value) -> Option<(String, String)> {
    let raw = resource.trim();
    if raw.is_empty() {
        return None;
    }
    let method = input
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let authority = raw
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(raw)
        .split('/')
        .next()
        .unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some((host, method))
    }
}

fn opaque_fingerprint(resource: &str) -> Option<String> {
    let trimmed = resource.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 解析 MCP intent 资源 `mcp:{server_id}:{config_hash}`。
///
/// 恰好两段；两段均非空且均不得含 `:`（否则无法与后续字段区分，返回空键）。
fn parse_mcp_resource(resource: &str) -> Option<(String, String)> {
    let rest = resource.trim().strip_prefix("mcp:")?;
    let (server_id, config_hash) = rest.split_once(':')?;
    if server_id.is_empty() || config_hash.is_empty() || config_hash.contains(':') {
        return None;
    }
    Some((server_id.to_string(), config_hash.to_string()))
}

/// 从模型可见全名 `mcp__{server_id}__{tool}` 反解第二段（规范化后的 MCP 工具名）。
///
/// 前缀不符返回 `None`（空键）：宁可直接审批，也不构造跨工具的宽键。
fn mcp_tool_segment(full_name: &str, server_id: &str) -> Option<String> {
    let tool = full_name.strip_prefix(&format!("mcp__{server_id}__"))?;
    if tool.is_empty() {
        None
    } else {
        Some(tool.to_string())
    }
}

/// 参数指纹：`SHA-256(canonical_json(arguments))`（I38e §3.5）。
///
/// 非对象参数返回 `None`：审批键只能描述「这一次调用」，不接受其它形状。
fn canonical_json_sha256(input: &Value) -> Option<String> {
    if !input.is_object() {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(canonical_json(input).as_bytes());
    Some(hex::encode(hasher.finalize()))
}

/// 规范 JSON：对象键递归排序、无多余空白、数组顺序保留。
///
/// 不依赖 `serde_json::Map` 的迭代顺序（该顺序受 feature 影响），
/// 保证同一参数在不同构建、不同进程下得到同一指纹。
fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    let encoded = serde_json::to_string(key).unwrap_or_default();
                    format!("{encoded}:{}", canonical_json(&map[key]))
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
