use serde_json::Value;

pub(super) fn data_str(data: Option<&Value>, key: &str) -> Option<String> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn data_u64(data: Option<&Value>, key: &str) -> Option<u64> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

pub(super) fn data_bool(data: Option<&Value>, key: &str) -> Option<bool> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_bool)
}

pub(super) fn args_str(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}
