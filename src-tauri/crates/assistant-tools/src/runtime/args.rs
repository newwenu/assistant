//! 工具入参的类型归一化与宽松标量解析。
//!
//! 部分模型（线上案例为 `agnes-2.5-flash`）会把数字写成字符串：
//! `{"offset": "1", "limit": "500"}`。严格按 JSON 类型校验会把它判成业务失败，
//! 并行批次里连续两次就烧掉 `Validation` 的熔断预算，模型没有纠正机会。
//!
//! 这里提供两层：
//! - [`coerce_to_schema`]：Pipeline 在校验前按 `input_schema` 把标量归一化，
//!   只做「语义等价、类型偏差」的改写，`"abc"` 这类真错值原样保留；
//! - [`optional_u64`] / [`optional_bool`] 等：工具解析层同样宽松，
//!   覆盖不经 Pipeline 的调用方（测试、子代理直调）。

use serde_json::{Map, Number, Value};

/// 按 JSON Schema 的 `properties` / `items` 递归归一化标量类型。
///
/// 返回 `Some(归一化后的入参)` 表示发生了改写；无改写返回 `None`，
/// 调用方据此保持原始参数，避免无意义的克隆。
pub fn coerce_to_schema(schema: &Value, input: &Value) -> Option<Value> {
    let object = input.as_object()?;
    let properties = schema.get("properties").and_then(Value::as_object)?;
    let mut changed: Option<Map<String, Value>> = None;
    for (key, value) in object {
        let Some(property_schema) = properties.get(key) else {
            continue;
        };
        let Some(coerced) = coerce_value(property_schema, value) else {
            continue;
        };
        changed
            .get_or_insert_with(|| object.clone())
            .insert(key.clone(), coerced);
    }
    changed.map(Value::Object)
}

fn coerce_value(schema: &Value, value: &Value) -> Option<Value> {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        if let Some(object) = value.as_object() {
            let mut changed = false;
            let mut out = object.clone();
            for (key, sub) in object {
                let Some(property_schema) = properties.get(key) else {
                    continue;
                };
                if let Some(coerced) = coerce_value(property_schema, sub) {
                    out.insert(key.clone(), coerced);
                    changed = true;
                }
            }
            return changed.then_some(Value::Object(out));
        }
    }

    if let Some(items) = schema.get("items") {
        if let Some(array) = value.as_array() {
            let mut changed = false;
            let mut out = array.clone();
            for item in &mut out {
                if let Some(coerced) = coerce_value(items, item) {
                    *item = coerced;
                    changed = true;
                }
            }
            return changed.then_some(Value::Array(out));
        }
    }

    coerce_scalar(schema, value)
}

fn coerce_scalar(schema: &Value, value: &Value) -> Option<Value> {
    let types = declared_types(schema);
    if types.is_empty() {
        return None;
    }
    match value {
        Value::String(text) => coerce_string(&types, text),
        Value::Number(number) => coerce_integral_number(&types, number),
        _ => None,
    }
}

fn declared_types(schema: &Value) -> Vec<&str> {
    match schema.get("type") {
        Some(Value::String(kind)) => vec![kind.as_str()],
        Some(Value::Array(kinds)) => kinds.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn coerce_string(types: &[&str], text: &str) -> Option<Value> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if types.contains(&"integer") {
        if let Ok(value) = text.parse::<u64>() {
            return Some(Value::from(value));
        }
        if let Ok(value) = text.parse::<i64>() {
            return Some(Value::from(value));
        }
    }
    if types.contains(&"number") {
        if let Ok(value) = text.parse::<f64>() {
            if let Some(number) = Number::from_f64(value) {
                return Some(Value::Number(number));
            }
        }
    }
    if types.contains(&"boolean") {
        return match text.to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        };
    }
    None
}

/// 整值浮点（`100.0`）折成整数；真小数（`1.5`）不折，交给工具校验报错。
fn coerce_integral_number(types: &[&str], number: &Number) -> Option<Value> {
    if !types.contains(&"integer") || number.is_u64() || number.is_i64() {
        return None;
    }
    let value = number.as_f64()?;
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    if value >= 0.0 {
        if value <= u64::MAX as f64 {
            return Some(Value::from(value as u64));
        }
    } else if value >= i64::MIN as f64 {
        return Some(Value::from(value as i64));
    }
    None
}

/// 取可选非负整数：缺失或 `null` 返回 `None`，数字字符串同样接受。
pub fn optional_u64(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => parse_u64(value)
            .map(Some)
            .ok_or_else(|| format!("'{}' must be a non-negative integer", key)),
    }
}

/// 取必填非负整数。
pub fn required_u64(object: &Map<String, Value>, key: &str) -> Result<u64, String> {
    optional_u64(object, key)?.ok_or_else(|| format!("missing or non-integer '{}' field", key))
}

/// 取可选布尔：缺失或 `null` 返回 `None`，`"true"` / `"false"` 同样接受。
pub fn optional_bool(object: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(format!("'{}' must be a boolean", key)),
        },
        Some(_) => Err(format!("'{}' must be a boolean", key)),
    }
}

/// 宽松解析非负整数：数字、整值浮点、可解析的数字字符串。
pub fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                return Some(value);
            }
            let float = number.as_f64()?;
            if float.is_finite() && float.fract() == 0.0 && float >= 0.0 {
                return Some(float as u64);
            }
            None
        }
        Value::String(text) => {
            let text = text.trim();
            if let Ok(value) = text.parse::<u64>() {
                return Some(value);
            }
            text.parse::<i64>()
                .ok()
                .filter(|value| *value >= 0)
                .map(|value| value as u64)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerce_string_numbers_into_integers() {
        let schema = json!({
            "type": "object",
            "properties": {
                "offset": { "type": "integer" },
                "limit": { "type": "integer" }
            }
        });
        let input = json!({ "offset": "1", "limit": "500" });
        let coerced = coerce_to_schema(&schema, &input).expect("must coerce");
        assert_eq!(coerced, json!({ "offset": 1, "limit": 500 }));
    }

    #[test]
    fn leaves_non_numeric_strings_alone() {
        let schema = json!({
            "type": "object",
            "properties": { "offset": { "type": "integer" } }
        });
        let input = json!({ "offset": "abc" });
        assert!(coerce_to_schema(&schema, &input).is_none());
    }

    #[test]
    fn coerces_booleans_and_whole_floats() {
        let schema = json!({
            "type": "object",
            "properties": {
                "all": { "type": "boolean" },
                "depth": { "type": "integer" }
            }
        });
        let input = json!({ "all": "true", "depth": 3.0 });
        let coerced = coerce_to_schema(&schema, &input).expect("must coerce");
        assert_eq!(coerced, json!({ "all": true, "depth": 3 }));
    }

    #[test]
    fn keeps_fractional_numbers_for_validation() {
        let schema = json!({
            "type": "object",
            "properties": { "offset": { "type": "integer" } }
        });
        let input = json!({ "offset": 1.5 });
        assert!(coerce_to_schema(&schema, &input).is_none());
    }

    #[test]
    fn coerces_nested_objects_and_arrays() {
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "count": { "type": "integer" } }
                    }
                }
            }
        });
        let input = json!({ "items": [{ "count": "2" }] });
        let coerced = coerce_to_schema(&schema, &input).expect("must coerce");
        assert_eq!(coerced, json!({ "items": [{ "count": 2 }] }));
    }

    #[test]
    fn lenient_scalar_parsers_accept_strings() {
        let object = json!({ "offset": "12", "flag": "false" });
        let object = object.as_object().expect("object");
        assert_eq!(optional_u64(object, "offset"), Ok(Some(12)));
        assert_eq!(optional_bool(object, "flag"), Ok(Some(false)));
    }

    #[test]
    fn lenient_scalar_parsers_reject_real_errors() {
        let object = json!({ "offset": -1, "flag": "yes" });
        let object = object.as_object().expect("object");
        assert_eq!(
            optional_u64(object, "offset"),
            Err("'offset' must be a non-negative integer".to_string())
        );
        assert_eq!(
            optional_bool(object, "flag"),
            Err("'flag' must be a boolean".to_string())
        );
    }
}
