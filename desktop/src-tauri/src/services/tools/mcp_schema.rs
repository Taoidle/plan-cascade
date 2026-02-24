//! MCP Schema Sanitization
//!
//! Cleans MCP tool JSON schemas for LLM compatibility.
//! Many MCP servers return complex JSON Schema features that LLMs
//! cannot process. This module simplifies schemas to the subset
//! that LLM providers understand.

use serde_json::Value;

/// Remove unsupported JSON Schema features for LLM compatibility.
///
/// Performs the following transformations:
/// - Removes `$schema`, `$ref`, `$id`, `$defs`, `definitions`
/// - Flattens `allOf` with a single entry
/// - Simplifies `anyOf`/`oneOf` to description-based representation
/// - Removes `$comment`, `examples`, `readOnly`, `writeOnly`
/// - Recursively sanitizes nested schemas in `properties` and `items`
pub fn sanitize_schema(schema: &mut Value) {
    if let Some(obj) = schema.as_object_mut() {
        // Remove unsupported top-level keys
        let keys_to_remove = [
            "$schema",
            "$ref",
            "$id",
            "$defs",
            "definitions",
            "$comment",
            "examples",
            "readOnly",
            "writeOnly",
            "deprecated",
            "contentMediaType",
            "contentEncoding",
            "if",
            "then",
            "else",
        ];
        for key in &keys_to_remove {
            obj.remove(*key);
        }

        // Flatten allOf with single entry
        if let Some(all_of) = obj.remove("allOf") {
            if let Some(arr) = all_of.as_array() {
                if arr.len() == 1 {
                    // Merge the single schema into the parent
                    if let Some(inner_obj) = arr[0].as_object() {
                        for (k, v) in inner_obj {
                            if !obj.contains_key(k) {
                                obj.insert(k.clone(), v.clone());
                            }
                        }
                    }
                } else if !arr.is_empty() {
                    // Multiple allOf entries: merge all properties together
                    let mut merged_properties = serde_json::Map::new();
                    let mut merged_required: Vec<String> = Vec::new();
                    let mut merged_description_parts: Vec<String> = Vec::new();

                    for item in arr {
                        if let Some(item_obj) = item.as_object() {
                            if let Some(props) =
                                item_obj.get("properties").and_then(|v| v.as_object())
                            {
                                for (k, v) in props {
                                    merged_properties.insert(k.clone(), v.clone());
                                }
                            }
                            if let Some(req) = item_obj.get("required").and_then(|v| v.as_array()) {
                                for r in req {
                                    if let Some(s) = r.as_str() {
                                        if !merged_required.contains(&s.to_string()) {
                                            merged_required.push(s.to_string());
                                        }
                                    }
                                }
                            }
                            if let Some(desc) = item_obj.get("description").and_then(|v| v.as_str())
                            {
                                merged_description_parts.push(desc.to_string());
                            }
                        }
                    }

                    if !merged_properties.is_empty() {
                        obj.entry("properties")
                            .or_insert_with(|| Value::Object(serde_json::Map::new()));
                        if let Some(existing) =
                            obj.get_mut("properties").and_then(|v| v.as_object_mut())
                        {
                            for (k, v) in merged_properties {
                                existing.insert(k, v);
                            }
                        }
                    }
                    if !merged_required.is_empty() {
                        let existing_req = obj
                            .get("required")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let mut all_req: Vec<Value> = existing_req;
                        for r in merged_required {
                            let val = Value::String(r);
                            if !all_req.contains(&val) {
                                all_req.push(val);
                            }
                        }
                        obj.insert("required".to_string(), Value::Array(all_req));
                    }
                    if !merged_description_parts.is_empty() && !obj.contains_key("description") {
                        obj.insert(
                            "description".to_string(),
                            Value::String(merged_description_parts.join(". ")),
                        );
                    }
                }
            }
        }

        // Simplify anyOf/oneOf: if all entries are types, create a description
        for keyword in &["anyOf", "oneOf"] {
            if let Some(variant) = obj.remove(*keyword) {
                if let Some(arr) = variant.as_array() {
                    if arr.len() == 1 {
                        // Single variant: merge into parent
                        if let Some(inner_obj) = arr[0].as_object() {
                            for (k, v) in inner_obj {
                                if !obj.contains_key(k) {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    } else {
                        // Multiple variants: collect type info as description
                        let types: Vec<String> = arr
                            .iter()
                            .filter_map(|item| {
                                item.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        item.get("description")
                                            .and_then(|d| d.as_str())
                                            .map(|s| s.to_string())
                                    })
                            })
                            .collect();

                        if !types.is_empty() {
                            let type_desc = format!("One of: {}", types.join(", "));
                            if !obj.contains_key("description") {
                                obj.insert("description".to_string(), Value::String(type_desc));
                            }
                        }

                        // Default to string type if no type is set
                        if !obj.contains_key("type") {
                            obj.insert("type".to_string(), Value::String("string".to_string()));
                        }
                    }
                }
            }
        }

        // Recursively sanitize properties
        if let Some(properties) = obj.get_mut("properties") {
            if let Some(props_obj) = properties.as_object_mut() {
                for (_key, prop_schema) in props_obj.iter_mut() {
                    sanitize_schema(prop_schema);
                }
            }
        }

        // Recursively sanitize items (for array types)
        if let Some(items) = obj.get_mut("items") {
            sanitize_schema(items);
        }

        // Recursively sanitize additionalProperties if it's a schema
        if let Some(additional) = obj.get_mut("additionalProperties") {
            if additional.is_object() {
                sanitize_schema(additional);
            }
        }
    }
}

/// Convert a sanitized JSON Schema Value to our ParameterSchema type.
///
/// This is used by McpToolAdapter to convert MCP tool schemas
/// into the format expected by the Tool trait.
pub fn json_schema_to_parameter_schema(
    schema: &Value,
) -> crate::services::llm::types::ParameterSchema {
    use crate::services::llm::types::ParameterSchema;
    use std::collections::HashMap;

    let schema_type = schema
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("object")
        .to_string();

    let description = schema
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let properties = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), json_schema_to_parameter_schema(v)))
                .collect::<HashMap<String, ParameterSchema>>()
        });

    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });

    let items = schema
        .get("items")
        .map(|v| Box::new(json_schema_to_parameter_schema(v)));

    let enum_values = schema.get("enum").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>()
    });

    let default = schema.get("default").cloned();

    ParameterSchema {
        schema_type,
        description,
        properties,
        required,
        items,
        enum_values,
        default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_remove_dollar_schema() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("$schema").is_none());
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["name"]["type"] == "string");
    }

    #[test]
    fn test_remove_ref_id_defs() {
        let mut schema = json!({
            "$ref": "#/definitions/Foo",
            "$id": "http://example.com/schema",
            "$defs": {"Foo": {"type": "string"}},
            "definitions": {"Foo": {"type": "string"}},
            "type": "object"
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("$ref").is_none());
        assert!(schema.get("$id").is_none());
        assert!(schema.get("$defs").is_none());
        assert!(schema.get("definitions").is_none());
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_remove_comment_examples() {
        let mut schema = json!({
            "type": "string",
            "$comment": "This is a comment",
            "examples": ["foo", "bar"],
            "readOnly": true,
            "writeOnly": false
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("$comment").is_none());
        assert!(schema.get("examples").is_none());
        assert!(schema.get("readOnly").is_none());
        assert!(schema.get("writeOnly").is_none());
        assert_eq!(schema["type"], "string");
    }

    #[test]
    fn test_flatten_allof_single() {
        let mut schema = json!({
            "allOf": [
                {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "age": {"type": "integer"}
                    },
                    "required": ["name"]
                }
            ]
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("allOf").is_none());
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["age"]["type"], "integer");
        assert_eq!(schema["required"], json!(["name"]));
    }

    #[test]
    fn test_flatten_allof_multiple() {
        let mut schema = json!({
            "type": "object",
            "allOf": [
                {
                    "properties": {
                        "name": {"type": "string"}
                    },
                    "required": ["name"]
                },
                {
                    "properties": {
                        "age": {"type": "integer"}
                    },
                    "required": ["age"]
                }
            ]
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("allOf").is_none());
        // Properties should be merged
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["age"]["type"], "integer");
        // Required should be merged
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("name")));
        assert!(required.contains(&json!("age")));
    }

    #[test]
    fn test_simplify_anyof_single() {
        let mut schema = json!({
            "anyOf": [{"type": "string", "description": "A string value"}]
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("anyOf").is_none());
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "A string value");
    }

    #[test]
    fn test_simplify_anyof_multiple() {
        let mut schema = json!({
            "anyOf": [
                {"type": "string"},
                {"type": "integer"},
                {"type": "null"}
            ]
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("anyOf").is_none());
        // Should have a description listing the types
        let desc = schema["description"].as_str().unwrap();
        assert!(desc.contains("string"), "Description: {}", desc);
        assert!(desc.contains("integer"), "Description: {}", desc);
        assert!(desc.contains("null"), "Description: {}", desc);
    }

    #[test]
    fn test_simplify_oneof() {
        let mut schema = json!({
            "oneOf": [
                {"type": "string"},
                {"type": "number"}
            ]
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("oneOf").is_none());
        let desc = schema["description"].as_str().unwrap();
        assert!(desc.contains("string"));
        assert!(desc.contains("number"));
    }

    #[test]
    fn test_recursive_property_sanitization() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "properties": {
                        "nested": {
                            "$ref": "#/definitions/Nested",
                            "type": "string"
                        }
                    }
                }
            }
        });
        sanitize_schema(&mut schema);
        assert!(schema["properties"]["config"].get("$schema").is_none());
        assert!(schema["properties"]["config"]["properties"]["nested"]
            .get("$ref")
            .is_none());
    }

    #[test]
    fn test_recursive_items_sanitization() {
        let mut schema = json!({
            "type": "array",
            "items": {
                "$comment": "should be removed",
                "type": "string",
                "examples": ["foo"]
            }
        });
        sanitize_schema(&mut schema);
        assert!(schema["items"].get("$comment").is_none());
        assert!(schema["items"].get("examples").is_none());
        assert_eq!(schema["items"]["type"], "string");
    }

    #[test]
    fn test_remove_conditional_keywords() {
        let mut schema = json!({
            "type": "object",
            "if": {"properties": {"type": {"const": "a"}}},
            "then": {"required": ["a_field"]},
            "else": {"required": ["b_field"]}
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("if").is_none());
        assert!(schema.get("then").is_none());
        assert!(schema.get("else").is_none());
    }

    #[test]
    fn test_sanitize_preserves_basic_schema() {
        let mut schema = json!({
            "type": "object",
            "description": "A simple tool",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                },
                "content": {
                    "type": "string",
                    "description": "File content"
                }
            },
            "required": ["path", "content"]
        });
        let original = schema.clone();
        sanitize_schema(&mut schema);
        // Should be unchanged since there's nothing to sanitize
        assert_eq!(schema, original);
    }

    #[test]
    fn test_sanitize_empty_schema() {
        let mut schema = json!({});
        sanitize_schema(&mut schema);
        assert_eq!(schema, json!({}));
    }

    #[test]
    fn test_sanitize_non_object() {
        // If schema is not an object, it should be left unchanged
        let mut schema = json!("string");
        sanitize_schema(&mut schema);
        assert_eq!(schema, json!("string"));
    }

    #[test]
    fn test_json_schema_to_parameter_schema_simple() {
        let schema = json!({
            "type": "object",
            "description": "Test schema",
            "properties": {
                "name": {"type": "string", "description": "The name"},
                "count": {"type": "integer", "description": "A count"}
            },
            "required": ["name"]
        });

        let param_schema = json_schema_to_parameter_schema(&schema);
        assert_eq!(param_schema.schema_type, "object");
        assert_eq!(param_schema.description, Some("Test schema".to_string()));
        assert!(param_schema.properties.is_some());
        let props = param_schema.properties.unwrap();
        assert_eq!(props["name"].schema_type, "string");
        assert_eq!(props["count"].schema_type, "integer");
        assert_eq!(param_schema.required, Some(vec!["name".to_string()]));
    }

    #[test]
    fn test_json_schema_to_parameter_schema_array() {
        let schema = json!({
            "type": "array",
            "description": "A list of strings",
            "items": {"type": "string"}
        });

        let param_schema = json_schema_to_parameter_schema(&schema);
        assert_eq!(param_schema.schema_type, "array");
        assert!(param_schema.items.is_some());
        assert_eq!(param_schema.items.unwrap().schema_type, "string");
    }

    #[test]
    fn test_json_schema_to_parameter_schema_with_enum() {
        let schema = json!({
            "type": "string",
            "enum": ["read", "write", "execute"]
        });

        let param_schema = json_schema_to_parameter_schema(&schema);
        assert_eq!(param_schema.schema_type, "string");
        assert_eq!(
            param_schema.enum_values,
            Some(vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string()
            ])
        );
    }

    #[test]
    fn test_json_schema_to_parameter_schema_defaults() {
        let schema = json!({
            "type": "boolean",
            "default": false
        });

        let param_schema = json_schema_to_parameter_schema(&schema);
        assert_eq!(param_schema.schema_type, "boolean");
        assert_eq!(param_schema.default, Some(json!(false)));
    }

    #[test]
    fn test_complex_real_world_schema_sanitization() {
        // Simulate a complex schema from a real MCP server
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "$id": "https://example.com/tool-schema",
            "type": "object",
            "description": "Search for files",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query",
                    "$comment": "Supports regex"
                },
                "path": {
                    "anyOf": [
                        {"type": "string"},
                        {"type": "null"}
                    ],
                    "description": "Optional path filter"
                },
                "options": {
                    "allOf": [
                        {
                            "properties": {
                                "case_sensitive": {"type": "boolean"}
                            }
                        },
                        {
                            "properties": {
                                "max_results": {"type": "integer"}
                            }
                        }
                    ],
                    "type": "object"
                }
            },
            "required": ["query"],
            "$defs": {
                "SearchOption": {"type": "object"}
            }
        });

        sanitize_schema(&mut schema);

        // Top-level junk removed
        assert!(schema.get("$schema").is_none());
        assert!(schema.get("$id").is_none());
        assert!(schema.get("$defs").is_none());

        // Basic structure preserved
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "Search for files");
        assert_eq!(schema["required"], json!(["query"]));

        // query $comment removed
        assert!(schema["properties"]["query"].get("$comment").is_none());
        assert_eq!(schema["properties"]["query"]["type"], "string");

        // path anyOf simplified (keeps existing description)
        assert!(schema["properties"]["path"].get("anyOf").is_none());
        assert_eq!(
            schema["properties"]["path"]["description"],
            "Optional path filter"
        );

        // options allOf flattened
        assert!(schema["properties"]["options"].get("allOf").is_none());
        assert_eq!(
            schema["properties"]["options"]["properties"]["case_sensitive"]["type"],
            "boolean"
        );
        assert_eq!(
            schema["properties"]["options"]["properties"]["max_results"]["type"],
            "integer"
        );
    }

    #[test]
    fn test_additional_properties_sanitization() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": {
                "$ref": "#/definitions/Value",
                "type": "string",
                "$comment": "dynamic values"
            }
        });
        sanitize_schema(&mut schema);
        assert!(schema["additionalProperties"].get("$ref").is_none());
        assert!(schema["additionalProperties"].get("$comment").is_none());
        assert_eq!(schema["additionalProperties"]["type"], "string");
    }
}
