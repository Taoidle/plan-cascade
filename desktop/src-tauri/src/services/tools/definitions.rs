//! Tool Definitions
//!
//! Provides tool definitions from the trait-based `ToolRegistry`.
//!
//! The canonical source of tool definitions is the `Tool` trait implementations
//! in `impls/`. This module caches a global `ToolRegistry` via `OnceLock` and
//! exposes convenience functions for obtaining definition vectors.

use crate::services::llm::types::ToolDefinition;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::trait_def::ToolRegistry;

/// Lazily-initialized global tool registry.
///
/// `build_registry_static()` creates ~15 `Arc<dyn Tool>` objects. Caching the
/// result in a `OnceLock` avoids rebuilding on every call. `ToolRegistry` is
/// `Send + Sync` (all `Tool` impls are `Send + Sync`).
static REGISTRY: OnceLock<ToolRegistry> = OnceLock::new();

pub(crate) fn cached_registry() -> &'static ToolRegistry {
    REGISTRY.get_or_init(|| super::executor::ToolExecutor::build_registry_static())
}

/// Get all available tool definitions from the trait-based registry.
///
/// This is the recommended way to get tool definitions, as they are
/// auto-generated from the `Tool` trait implementations and always in sync.
/// Uses a cached `OnceLock<ToolRegistry>` to avoid rebuilding on every call.
pub fn get_tool_definitions_from_registry() -> Vec<ToolDefinition> {
    let mut defs = cached_registry().definitions();
    let mut seen: HashSet<String> = defs.iter().map(|d| d.name.clone()).collect();

    for def in super::runtime_tools::definitions() {
        if seen.insert(def.name.clone()) {
            defs.push(def);
        }
    }

    defs
}

/// Get basic tool definitions (without Task/Analyze) from the trait-based registry.
///
/// Used for sub-agents to prevent recursion.
/// Uses a cached `OnceLock<ToolRegistry>` to avoid rebuilding on every call.
pub fn get_basic_tool_definitions_from_registry() -> Vec<ToolDefinition> {
    get_tool_definitions_from_registry()
        .into_iter()
        .filter(|d| d.name != "Task" && d.name != "Analyze")
        .collect()
}

/// Get tool definitions filtered by sub-agent type.
///
/// Each `SubAgentType` has a specific set of allowed tools. This function
/// returns only the definitions for tools permitted by that type.
pub fn get_tool_definitions_for_subagent(
    subagent_type: super::task_spawner::SubAgentType,
) -> Vec<ToolDefinition> {
    let allowed = subagent_type.allowed_tools();
    let mut defs: Vec<ToolDefinition> = cached_registry()
        .definitions()
        .into_iter()
        .filter(|d| allowed.contains(&d.name.as_str()))
        .collect();

    // Dynamic runtime tools (for example MCP tools) are allowed for
    // general-purpose sub-agents so they can delegate to external systems.
    if matches!(
        subagent_type,
        super::task_spawner::SubAgentType::GeneralPurpose
    ) {
        let mut seen: HashSet<String> = defs.iter().map(|d| d.name.clone()).collect();
        for def in super::runtime_tools::definitions() {
            if seen.insert(def.name.clone()) {
                defs.push(def);
            }
        }
    }

    defs
}

/// Check if a tool is parallel-safe by name (via the cached registry).
pub fn is_tool_parallel_safe(name: &str) -> bool {
    if let Some(tool) = cached_registry().get(name) {
        return tool.is_parallel_safe();
    }
    super::runtime_tools::is_parallel_safe(name).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic_definitions_exclude_task_and_analyze() {
        let basic = get_basic_tool_definitions_from_registry();
        let names: Vec<&str> = basic.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"Task"), "Basic should not include Task");
        assert!(
            !names.contains(&"Analyze"),
            "Basic should not include Analyze"
        );
        assert!(names.contains(&"Read"), "Basic should include Read");
        assert!(
            names.contains(&"CodebaseSearch"),
            "Basic should include CodebaseSearch"
        );
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
        assert!(enum_vals.contains(&"hybrid".to_string()));
        assert!(enum_vals.contains(&"symbol".to_string()));
        assert!(enum_vals.contains(&"path".to_string()));
        assert!(enum_vals.contains(&"semantic".to_string()));
        assert!(!enum_vals.contains(&"all".to_string()));

        // scope default is "hybrid"
        let default_val = scope.default.as_ref().unwrap();
        assert_eq!(
            default_val,
            &serde_json::Value::String("hybrid".to_string())
        );

        // V2 params exist and are optional
        assert!(props.contains_key("project_path"));
        assert!(props.contains_key("workspace_root_id"));
        assert!(props.contains_key("limit"));
        assert!(props.contains_key("include_snippet"));
        assert!(props.contains_key("filters"));
        assert!(!required.contains(&"project_path".to_string()));
        assert!(!required.contains(&"workspace_root_id".to_string()));
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
        // Built-in definitions are cached and should always be present
        // across multiple invocations, even if runtime tools are added.
        let first = get_tool_definitions_from_registry();
        let second = get_tool_definitions_from_registry();
        let first_names: std::collections::HashSet<String> =
            first.iter().map(|d| d.name.clone()).collect();
        let second_names: std::collections::HashSet<String> =
            second.iter().map(|d| d.name.clone()).collect();
        assert!(first_names.contains("Read"));
        assert!(first_names.contains("Write"));
        assert!(second_names.is_superset(&first_names) || first_names.is_superset(&second_names));
    }
}
