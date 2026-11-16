use crate::types::ToolSpec;
use serde_json::{json, Value};

use super::schema::sanitize_schema;

pub(crate) fn convert_tools(tool_defs: &[ToolSpec]) -> Option<Vec<Value>> {
    if tool_defs.is_empty() {
        return None;
    }

    let declarations: Vec<Value> = tool_defs
        .iter()
        .map(|tool| {
            let schema = sanitize_schema(&tool.input_schema);
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": schema,
            })
        })
        .collect();

    if declarations.is_empty() {
        None
    } else {
        Some(vec![json!({
            "functionDeclarations": declarations,
        })])
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn convert_tools_uses_function_declarations_with_sanitized_schema() {
        let tools = vec![ToolSpec {
            name: "search".to_string(),
            description: "search the web".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "q": { "type": "string" } },
                "allOf": [{ "type": "object" }]
            }),
        }];
        let converted = convert_tools(&tools).expect("non-empty tools");
        assert_eq!(converted.len(), 1);
        let declarations = &converted[0]["functionDeclarations"];
        assert_eq!(declarations[0]["name"], "search");
        assert_eq!(declarations[0]["description"], "search the web");
        assert_eq!(
            declarations[0]["parameters"]["properties"]["q"]["type"],
            "string"
        );
        assert!(declarations[0]["parameters"].get("allOf").is_none());
    }

    #[test]
    fn convert_tools_returns_none_for_empty_input() {
        assert!(convert_tools(&[]).is_none());
    }
}
