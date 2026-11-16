use serde_json::Value;

/// 取必填路径参数：缺失、非字符串、空串、首尾空白都拒绝。
///
/// 不做 `trim()` 静默修正：`" a.txt"` 与 `"a.txt"` 是两个不同路径，静默改写会让
/// 删除 / 覆盖落到调用方没有指定的目标上。
pub fn parse_path_arg(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or non-string '{}' field", key))?;
    validate_path_text(value, key)?;
    Ok(value.to_string())
}

/// 取可选路径参数：缺失或 `null` 返回 `None`，其余按 [`parse_path_arg`] 校验。
pub fn parse_optional_path_arg(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => parse_path_arg(object, key).map(Some),
    }
}

fn validate_path_text(value: &str, key: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("missing or empty '{}' field", key));
    }
    if value != value.trim() {
        return Err(format!(
            "'{}' must not have leading or trailing whitespace",
            key
        ));
    }
    Ok(())
}
