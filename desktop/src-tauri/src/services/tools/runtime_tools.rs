//! Runtime Tool Snapshot
//!
//! Stores dynamically registered tools (for example MCP tools) in a
//! process-wide snapshot so orchestrator tool definition discovery and
//! tool execution can see the same runtime tool set.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::services::llm::types::ToolDefinition;

use super::trait_def::{Tool, ToolRegistry};

#[derive(Default)]
struct RuntimeToolsStore {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
    metadata: HashMap<String, RuntimeToolMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeToolMetadata {
    #[serde(default)]
    pub source: String,
    pub capability_class: Option<String>,
    #[serde(default)]
    pub debug_categories: Vec<String>,
    #[serde(default)]
    pub environment_allowlist: Vec<String>,
    pub write_behavior: Option<String>,
    pub approval_required: Option<bool>,
}

static RUNTIME_TOOLS: OnceLock<RwLock<RuntimeToolsStore>> = OnceLock::new();

fn store() -> &'static RwLock<RuntimeToolsStore> {
    RUNTIME_TOOLS.get_or_init(|| RwLock::new(RuntimeToolsStore::default()))
}

/// Replace the runtime snapshot from a source registry.
///
/// This is called by MCP connect/disconnect flows after mutating their
/// runtime registry so all orchestrator paths observe the same tool set.
pub fn replace_from_registry(registry: &ToolRegistry) {
    replace_from_registry_with_metadata(registry, HashMap::new());
}

/// Replace the runtime snapshot from a source registry and an optional metadata map.
pub fn replace_from_registry_with_metadata(
    registry: &ToolRegistry,
    metadata: HashMap<String, RuntimeToolMetadata>,
) {
    let mut tools = HashMap::new();
    let mut order = Vec::new();

    for name in registry.names() {
        if let Some(tool) = registry.get(&name) {
            order.push(name.clone());
            tools.insert(name, tool);
        }
    }

    if let Ok(mut guard) = store().write() {
        guard.tools = tools;
        guard.order = order;
        guard.metadata = metadata;
    }
}

/// Clear all runtime tools.
pub fn clear() {
    if let Ok(mut guard) = store().write() {
        guard.tools.clear();
        guard.order.clear();
        guard.metadata.clear();
    }
}

/// Get runtime tool definitions in insertion order.
pub fn definitions() -> Vec<ToolDefinition> {
    if let Ok(guard) = store().read() {
        return guard
            .order
            .iter()
            .filter_map(|name| guard.tools.get(name))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.parameters_schema(),
            })
            .collect();
    }
    Vec::new()
}

/// Get runtime tool names in insertion order.
pub fn names() -> Vec<String> {
    if let Ok(guard) = store().read() {
        return guard.order.clone();
    }
    Vec::new()
}

/// Get a runtime tool by name.
pub fn get(name: &str) -> Option<Arc<dyn Tool>> {
    if let Ok(guard) = store().read() {
        return guard.tools.get(name).cloned();
    }
    None
}

/// Get runtime tool metadata by name.
pub fn metadata_for(name: &str) -> Option<RuntimeToolMetadata> {
    if let Ok(guard) = store().read() {
        return guard.metadata.get(name).cloned();
    }
    None
}

/// Check runtime tool parallel safety.
pub fn is_parallel_safe(name: &str) -> Option<bool> {
    get(name).map(|tool| tool.is_parallel_safe())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tools::impls::ReadTool;

    #[test]
    fn test_replace_and_lookup_runtime_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ReadTool::new()));
        replace_from_registry(&registry);

        let defs = definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "Read");
        assert!(get("Read").is_some());
    }

    #[test]
    fn test_clear_runtime_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ReadTool::new()));
        replace_from_registry(&registry);
        clear();
        assert!(definitions().is_empty());
        assert!(names().is_empty());
    }
}
