use crate::types::ToolSpec;
use serde_json::{json, Value};

pub(crate) fn convert_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema
            })
        })
        .collect()
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn convert_tools_uses_input_schema_without_parameters_wrapper() {
        let tools = vec![ToolSpec {
            name: "search".to_string(),
            description: "search the web".to_string(),
            input_schema: json!({ "type": "object", "properties": { "q": { "type": "string" } } }),
        }];
        let converted = convert_tools(&tools);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["name"], json!("search"));
        assert_eq!(converted[0]["description"], json!("search the web"));
        assert_eq!(
            converted[0]["input_schema"]["properties"]["q"]["type"],
            json!("string")
        );
        assert!(converted[0].get("parameters").is_none());
        assert!(converted[0].get("type").is_none());
    }
}
