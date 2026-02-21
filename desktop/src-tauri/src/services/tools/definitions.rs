//! Tool Definitions
//!
//! Provides tool definitions from the trait-based `ToolRegistry`.
//!
//! The canonical source of tool definitions is the `Tool` trait implementations
//! in `impls/`. This module caches a global `ToolRegistry` via `OnceLock` and
//! exposes convenience functions for obtaining definition vectors.

use crate::services::llm::types::ToolDefinition;
use std::sync::OnceLock;

use super::trait_def::ToolRegistry;

/// Lazily-initialized global tool registry.
///
/// `build_registry_static()` creates ~15 `Arc<dyn Tool>` objects. Caching the
/// result in a `OnceLock` avoids rebuilding on every call. `ToolRegistry` is
/// `Send + Sync` (all `Tool` impls are `Send + Sync`).
static REGISTRY: OnceLock<ToolRegistry> = OnceLock::new();

fn cached_registry() -> &'static ToolRegistry {
    REGISTRY.get_or_init(|| super::executor::ToolExecutor::build_registry_static())
}

/// Get all available tool definitions from the trait-based registry.
///
/// This is the recommended way to get tool definitions, as they are
/// auto-generated from the `Tool` trait implementations and always in sync.
/// Uses a cached `OnceLock<ToolRegistry>` to avoid rebuilding on every call.
pub fn get_tool_definitions_from_registry() -> Vec<ToolDefinition> {
    cached_registry().definitions()
}

/// Get basic tool definitions (without Task/Analyze) from the trait-based registry.
///
/// Used for sub-agents to prevent recursion.
/// Uses a cached `OnceLock<ToolRegistry>` to avoid rebuilding on every call.
pub fn get_basic_tool_definitions_from_registry() -> Vec<ToolDefinition> {
    cached_registry()
        .definitions()
        .into_iter()
        .filter(|d| d.name != "Task" && d.name != "Analyze")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic_definitions_exclude_task_and_analyze() {
        let basic = get_basic_tool_definitions_from_registry();
        let names: Vec<&str> = basic.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"Task"), "Basic should not include Task");
        assert!(!names.contains(&"Analyze"), "Basic should not include Analyze");
        assert!(names.contains(&"Read"), "Basic should include Read");
        assert!(names.contains(&"CodebaseSearch"), "Basic should include CodebaseSearch");
    }

    #[test]
    fn test_registry_definitions_serializable() {
        let tools = get_tool_definitions_from_registry();
        for tool in tools {
            let json = serde_json::to_string(&tool).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_codebase_search_tool_schema() {
        let tools = get_tool_definitions_from_registry();
        let tool = tools.iter().find(|t| t.name == "CodebaseSearch").unwrap();
        assert_eq!(tool.name, "CodebaseSearch");

        // Description should indicate index-based search
        assert!(
            tool.description.contains("index"),
            "Description should mention index"
        );

        let props = tool.input_schema.properties.as_ref().unwrap();

        // query is required
        let required = tool.input_schema.required.as_ref().unwrap();
        assert!(required.contains(&"query".to_string()));

        // query param exists
        assert!(props.contains_key("query"));

        // scope param with enum values
        let scope = props.get("scope").unwrap();
        let enum_vals = scope.enum_values.as_ref().unwrap();
        assert!(enum_vals.contains(&"files".to_string()));
        assert!(enum_vals.contains(&"symbols".to_string()));
        assert!(enum_vals.contains(&"semantic".to_string()));
        assert!(enum_vals.contains(&"all".to_string()));

        // scope default is "all"
        let default_val = scope.default.as_ref().unwrap();
        assert_eq!(default_val, &serde_json::Value::String("all".to_string()));

        // component param exists and is optional
        assert!(props.contains_key("component"));
        assert!(!required.contains(&"component".to_string()));
    }

    #[test]
    fn test_registry_basic_excludes_both_task_and_analyze() {
        let basic = get_basic_tool_definitions_from_registry();
        let names: Vec<&str> = basic.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"Task"), "Registry basic must exclude Task");
        assert!(
            !names.contains(&"Analyze"),
            "Registry basic must exclude Analyze"
        );
    }

    #[test]
    fn test_cached_registry_returns_consistent_results() {
        // Verify that the OnceLock-cached registry returns the same results
        // across multiple invocations.
        let first = get_tool_definitions_from_registry();
        let second = get_tool_definitions_from_registry();
        assert_eq!(first.len(), second.len());

        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.description, b.description);
        }
    }
}
