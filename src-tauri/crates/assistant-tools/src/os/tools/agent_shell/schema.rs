//! 输入解析与校验（I56 §4.1 / §4.2）。
//!
//! 检查顺序：形状 → `writable_root` → `scratch_root` → `cwd` 解析 → 超时范围。
//! `writable_root` 从第 4 步提前到第 2 步：它是整个工具的前置条件（没有可写根就不存在
//! 可写边界），且 `cwd` 的相对路径解析本来就以它为基准——先判定它，失败原因才是
//! `sandbox_root_missing` 而不是含糊的「路径无法解析」。`scratch_root` 同口径
//! （I73 §5.4 P4.0）：临时区必须落在沙箱可写根内，缺根同样 fail-closed。
//!
//! 本模块**不做**命令内容匹配：命令是什么由沙箱边界决定，不由字符串白/黑名单决定。

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::context::ToolUseContext;
use crate::os::path;
use crate::r#trait::ValidationResult;

/// `command` 长度上限（字符）。超限直接 `invalid`，不截断——
/// 截断会让「看到的就是执行的」失效。
pub const MAX_COMMAND_CHARS: usize = 8192;

/// 缺省超时：长任务必须由模型显式请求（写进工具描述）。
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;
pub const MIN_TIMEOUT_MS: u64 = 1_000;
/// 上限 30 分钟：本仓冷 `cargo build` / 全量测试轻易越过 10 分钟。
pub const MAX_TIMEOUT_MS: u64 = 1_800_000;

/// `writable_root` 缺失的错误码（审批前 `blocked`）。
pub const SANDBOX_ROOT_MISSING: &str = "sandbox_root_missing";

/// 通过校验的输入。
#[derive(Debug, Clone)]
pub struct ShellInput {
    /// 原样字符串：不裁剪内部空白、不改写引号，审批键与执行体共用它。
    pub command: String,
    /// 已 canonicalize、且落在 `writable_root` 内。
    pub cwd: PathBuf,
    pub timeout_ms: u64,
    /// 会话可写根（canonicalize 后），由 `ToolUseContext` 派生，**不接受**模型输入。
    pub writable_root: PathBuf,
    /// 会话临时区域根（已含 `<session_hash>`，构造点已归一），直接取
    /// `ToolUseContext.tool_scratch_root`；`TEMP` / `TMP` / `TMPDIR` 与 `scratch://` 共用它。
    pub scratch_root: PathBuf,
}

impl ShellInput {
    /// 解析并校验；`validate_input` 与 `execute` / `permission_intents` 共用同一条路径，
    /// 保证「校验 A、执行 B」不可能发生。
    pub fn parse(input: &Value, context: &ToolUseContext) -> Result<Self, ShellInputError> {
        // 1. 形状：对象 + 无未知字段 + `command` 非空且不超长。
        let object = input
            .as_object()
            .ok_or_else(|| ShellInputError::invalid("input must be a JSON object"))?;
        for key in object.keys() {
            if !matches!(key.as_str(), "command" | "cwd" | "timeout_ms") {
                return Err(ShellInputError::invalid(format!(
                    "unknown field '{}'; accepted fields are command, cwd, timeout_ms",
                    key
                )));
            }
        }
        let raw_command = object
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| ShellInputError::invalid("missing or non-string 'command' field"))?;
        if raw_command.trim().is_empty() {
            return Err(ShellInputError::invalid("'command' must not be empty"));
        }
        if raw_command.chars().count() > MAX_COMMAND_CHARS {
            return Err(ShellInputError::invalid(format!(
                "'command' exceeds {} characters; split it into smaller commands",
                MAX_COMMAND_CHARS
            )));
        }

        // 2. `cwd` 解析：缺省取可写根；给出时按可写根解析并做边界检查。
        let writable_root = resolve_writable_root(context)?;
        // 2b. 临时区根：与可写根同口径 fail-closed（缺根即 `sandbox_root_missing`）。
        // 该字段已含会话段且在注入点归一过（§3.9），这里不再 canonicalize。
        let scratch_root = context.tool_scratch_root.clone().ok_or_else(|| {
            ShellInputError::blocked_with_code(
                "shell requires a session scratch root; this context has none",
                SANDBOX_ROOT_MISSING,
            )
        })?;
        let cwd = match object.get("cwd") {
            None | Some(Value::Null) => writable_root.clone(),
            Some(Value::String(raw)) => path::parse_ref(raw)
                .and_then(|reference| reference.resolve_working_directory(context))
                .map_err(|error| {
                    // 换路径重试同一意图不构成新意图：不可通过改写输入绕过。
                    ShellInputError::blocked(error)
                })?,
            Some(_) => return Err(ShellInputError::invalid("'cwd' must be a string")),
        };

        // 3. `timeout_ms` 范围。
        let timeout_ms = match object.get("timeout_ms") {
            None | Some(Value::Null) => DEFAULT_TIMEOUT_MS,
            Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
                ShellInputError::invalid("'timeout_ms' must be a non-negative integer")
            })?,
            Some(_) => return Err(ShellInputError::invalid("'timeout_ms' must be an integer")),
        };
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
            return Err(ShellInputError::invalid(format!(
                "'timeout_ms' must be between {} and {}",
                MIN_TIMEOUT_MS, MAX_TIMEOUT_MS
            )));
        }

        Ok(Self {
            command: raw_command.to_string(),
            cwd,
            timeout_ms,
            writable_root,
            scratch_root,
        })
    }

    /// 审批前的纯检查入口（`Tool::validate_input`）。
    pub fn validate(input: &Value, context: &ToolUseContext) -> ValidationResult {
        match Self::parse(input, context) {
            Ok(_) => ValidationResult::ok(),
            Err(error) => error.into_validation(),
        }
    }
}

/// 校验失败的两档：`Invalid` 可改写重试，`Blocked` 不可通过改写输入绕过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellInputError {
    Invalid(String),
    Blocked {
        message: String,
        code: Option<&'static str>,
    },
}

impl ShellInputError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    fn blocked(message: impl Into<String>) -> Self {
        Self::Blocked {
            message: message.into(),
            code: None,
        }
    }

    fn blocked_with_code(message: impl Into<String>, code: &'static str) -> Self {
        Self::Blocked {
            message: message.into(),
            code: Some(code),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Invalid(message) => message,
            Self::Blocked { message, .. } => message,
        }
    }

    /// 失败码：只有「不可绕过」那一档才带码（如 `sandbox_root_missing`）。
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Invalid(_) => None,
            Self::Blocked { code, .. } => *code,
        }
    }

    pub fn into_validation(self) -> ValidationResult {
        match self {
            Self::Invalid(message) => ValidationResult::invalid(message),
            Self::Blocked { message, code } => {
                let result = ValidationResult::blocked(message);
                match code {
                    Some(code) => result.with_error_code(code),
                    None => result,
                }
            }
        }
    }
}

impl std::fmt::Display for ShellInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// 第 4 步：`writable_root` 缺失即 `blocked` + `sandbox_root_missing`。
fn resolve_writable_root(context: &ToolUseContext) -> Result<PathBuf, ShellInputError> {
    let root = context
        .workspace_root
        .as_deref()
        .map(str::trim)
        .filter(|root| !root.is_empty())
        .ok_or_else(|| {
            ShellInputError::blocked_with_code(
                "shell requires a writable workspace root; this context has none",
                SANDBOX_ROOT_MISSING,
            )
        })?;
    path::canonicalize_lenient(Path::new(root))
        .map_err(|error| ShellInputError::blocked_with_code(error, SANDBOX_ROOT_MISSING))
}
