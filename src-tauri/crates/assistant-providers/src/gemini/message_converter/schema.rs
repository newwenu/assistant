use serde_json::Value;

pub(super) fn sanitize_schema(parameters: &Value) -> Value {
    strip_unsupported_schema_fields(parameters.clone())
}

fn strip_unsupported_schema_fields(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.remove("allOf");
            map.remove("anyOf");
            map.remove("oneOf");

            let mut sanitized = serde_json::Map::new();
            for (key, val) in map {
                if key == "properties" {
                    if let Value::Object(properties) = val {
                        sanitized.insert(
                            key,
                            Value::Object(
                                properties
                                    .into_iter()
                                    .map(|(name, schema)| {
                                        (name, strip_unsupported_schema_fields(schema))
                                    })
                                    .collect(),
                            ),
                        );
                    }
                    continue;
                }
                if key == "items" {
                    sanitized.insert(key, strip_unsupported_schema_fields(val));
                    continue;
                }
                sanitized.insert(key, val);
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(strip_unsupported_schema_fields)
                .collect(),
        ),
        other => other,
    }
}
