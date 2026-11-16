use globset::GlobSet;
use grep_searcher::Encoding as SearcherEncoding;
use serde_json::Value;

use crate::os::agent_glob;
use crate::os::path;
use crate::os::text_encoding::parse_optional_encoding;

use super::{DEFAULT_CONTEXT, MAX_CONTEXT};

pub(super) struct GrepRequest {
    pub(super) pattern: String,
    pub(super) path: Option<String>,
    pub(super) include: Option<String>,
    pub(super) context: u64,
    pub(super) encoding: Option<SearcherEncoding>,
}

impl GrepRequest {
    pub(super) fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        // `pattern` 原样编译，不做 `trim`：前导 / 尾随空格是正则的一部分（搜缩进、搜行尾空格）。
        // `trim` 只用于拒绝「全空白」输入，不改变编译用的字符串（[grep §3]）。
        let pattern = object
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'pattern' field".to_string())?;
        if pattern.trim().is_empty() {
            return Err("'pattern' must not be blank".to_string());
        }
        let pattern = pattern.to_string();

        let path = path::parse_optional_path_arg(object, "path")?;

        let include = match object.get("include") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "'include' must be a non-empty string".to_string())?
                    .to_string(),
            ),
        };

        let context = match object.get("context") {
            None | Some(Value::Null) => DEFAULT_CONTEXT,
            Some(value) => {
                let value = value
                    .as_u64()
                    .ok_or_else(|| "'context' must be a non-negative integer".to_string())?;
                if value > MAX_CONTEXT {
                    return Err(format!(
                        "'context' must be between 0 and {} (got {})",
                        MAX_CONTEXT, value
                    ));
                }
                value
            }
        };

        // label 校验与 `read` / `write` 共用一套（别名表与错误文案不分叉）；
        // searcher 只接受自己的 `Encoding` 类型，用规范化名重建即可。
        let encoding = match parse_optional_encoding(object)? {
            None => None,
            Some(resolved) => Some(SearcherEncoding::new(resolved.name()).map_err(|error| {
                format!(
                    "cannot search with encoding '{}': {}",
                    resolved.name(),
                    error
                )
            })?),
        };

        Ok(Self {
            pattern,
            path,
            include,
            context,
            encoding,
        })
    }
}

pub(super) fn build_include(pattern: Option<&str>) -> Result<Option<GlobSet>, String> {
    match pattern {
        None => Ok(None),
        Some(pattern) => agent_glob::build_glob_set(&[pattern.to_string()])
            .map(Some)
            .map_err(|error| format!("invalid include glob '{}': {}", pattern, error)),
    }
}
