//! NotebookEdit Tool Implementation
//!
//! Edits Jupyter notebook (.ipynb) cells: replace, insert, delete.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

use super::read::validate_path;

/// NotebookEdit tool — edits Jupyter notebook cells.
pub struct NotebookEditTool;

impl NotebookEditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    fn description(&self) -> &str {
        "Edit a Jupyter notebook (.ipynb) cell. Supports replacing cell content, inserting new cells, and deleting cells. Preserves notebook metadata and untouched cell outputs."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "notebook_path".to_string(),
            ParameterSchema::string(Some("Absolute path to the .ipynb file")),
        );
        properties.insert(
            "cell_index".to_string(),
            ParameterSchema::integer(Some("0-based cell index")),
        );
        properties.insert(
            "operation".to_string(),
            ParameterSchema::string(Some("Operation: 'replace', 'insert', or 'delete'")),
        );
        properties.insert(
            "cell_type".to_string(),
            ParameterSchema::string(Some("Cell type: 'code' or 'markdown' (required for insert)")),
        );
        properties.insert(
            "new_source".to_string(),
            ParameterSchema::string(Some("New cell content (required for replace and insert)")),
        );
        ParameterSchema::object(
            Some("NotebookEdit parameters"),
            properties,
            vec![
                "notebook_path".to_string(),
                "cell_index".to_string(),
                "operation".to_string(),
            ],
        )
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let notebook_path = match args.get("notebook_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: notebook_path"),
        };

        let cell_index = match args.get("cell_index").and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => return ToolResult::err("Missing required parameter: cell_index"),
        };

        let operation = match args.get("operation").and_then(|v| v.as_str()) {
            Some(o) => o,
            None => return ToolResult::err("Missing required parameter: operation"),
        };

        let cell_type = args.get("cell_type").and_then(|v| v.as_str());
        let new_source = args.get("new_source").and_then(|v| v.as_str());

        let path = match validate_path(notebook_path, &ctx.working_directory_snapshot(), &ctx.project_root) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(e),
        };

        // Enforce read-before-write for existing notebooks
        if path.exists() {
            if let Ok(read_files) = ctx.read_files.lock() {
                if !read_files.contains(&path) {
                    return ToolResult::err(
                        "You must read a notebook before editing it. Use the Read tool first.",
                    );
                }
            }
        }

        match crate::services::tools::notebook_edit::edit_notebook(
            &path, cell_index, operation, cell_type, new_source,
        ) {
            Ok(msg) => ToolResult::ok(msg),
            Err(e) => ToolResult::err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notebook_edit_tool_name() {
        let tool = NotebookEditTool::new();
        assert_eq!(tool.name(), "NotebookEdit");
    }

    #[test]
    fn test_notebook_edit_tool_not_long_running() {
        let tool = NotebookEditTool::new();
        assert!(!tool.is_long_running());
    }
}
