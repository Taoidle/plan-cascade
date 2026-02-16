//! CodebaseSearch Tool Implementation
//!
//! Searches the project's indexed codebase for symbols, files, or semantic similarity.
//! Uses the pre-built SQLite index for fast lookups.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// CodebaseSearch tool — searches the project index for symbols, files, and semantic matches.
///
/// The actual index queries are handled by ToolExecutor which owns IndexStore and
/// EmbeddingService. This trait implementation provides the tool definition.
/// The ToolExecutor intercepts "CodebaseSearch" calls and delegates to its internal
/// execute_codebase_search method.
pub struct CodebaseSearchTool;

impl CodebaseSearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CodebaseSearchTool {
    fn name(&self) -> &str {
        "CodebaseSearch"
    }

    fn description(&self) -> &str {
        "Search the project's indexed codebase for symbols, files, or semantic similarity. Uses the pre-built SQLite index for fast lookups without scanning the filesystem. The 'semantic' scope performs vector similarity search over code chunks. Preferred over Grep/Glob for initial code exploration when the index is available."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            ParameterSchema::string(Some(
                "Search pattern — symbol name, file path fragment, or keyword to search for",
            )),
        );

        let mut scope_schema = ParameterSchema::string(Some(
            "Search scope: 'symbols' (search symbol names), 'files' (search file paths/components), 'semantic' (vector similarity search over code chunks), 'all' (merge symbols + files). Default: 'all'",
        ));
        scope_schema.enum_values = Some(vec![
            "files".to_string(),
            "symbols".to_string(),
            "semantic".to_string(),
            "all".to_string(),
        ]);
        scope_schema.default = Some(serde_json::Value::String("all".to_string()));
        properties.insert("scope".to_string(), scope_schema);

        properties.insert(
            "component".to_string(),
            ParameterSchema::string(Some(
                "Optional component name to narrow results (e.g., 'desktop-rust', 'desktop-web')",
            )),
        );

        ParameterSchema::object(
            Some("CodebaseSearch parameters"),
            properties,
            vec!["query".to_string()],
        )
    }

    async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
        // CodebaseSearch execution requires IndexStore and EmbeddingService
        // which are managed by ToolExecutor. When called through the registry
        // without these services, ToolExecutor intercepts the call.
        ToolResult::ok(
            "Codebase index not available. The project has not been indexed yet. \
             Use Grep for content search or Glob/LS for file discovery instead.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codebase_search_tool_name() {
        let tool = CodebaseSearchTool::new();
        assert_eq!(tool.name(), "CodebaseSearch");
    }

    #[test]
    fn test_codebase_search_tool_schema() {
        let tool = CodebaseSearchTool::new();
        let schema = tool.parameters_schema();
        let props = schema.properties.as_ref().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("scope"));
        assert!(props.contains_key("component"));

        let scope = props.get("scope").unwrap();
        let enum_vals = scope.enum_values.as_ref().unwrap();
        assert!(enum_vals.contains(&"files".to_string()));
        assert!(enum_vals.contains(&"symbols".to_string()));
        assert!(enum_vals.contains(&"semantic".to_string()));
        assert!(enum_vals.contains(&"all".to_string()));
    }

    #[test]
    fn test_codebase_search_tool_not_long_running() {
        let tool = CodebaseSearchTool::new();
        assert!(!tool.is_long_running());
    }
}
