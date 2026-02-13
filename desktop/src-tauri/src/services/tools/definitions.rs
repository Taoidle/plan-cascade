//! Tool Definitions
//!
//! Provides tool definitions for the executor.

use crate::services::llm::types::{ParameterSchema, ToolDefinition};
use std::collections::HashMap;

/// Get all available tool definitions (including Task tool for parent agents)
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_tool(),
        write_tool(),
        edit_tool(),
        bash_tool(),
        glob_tool(),
        grep_tool(),
        ls_tool(),
        cwd_tool(),
        analyze_tool(),
        task_tool(),
        web_fetch_tool(),
        web_search_tool(),
        notebook_edit_tool(),
        codebase_search_tool(),
    ]
}

/// Get basic tool definitions (without Task tool, for sub-agents to prevent recursion)
pub fn get_basic_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_tool(),
        write_tool(),
        edit_tool(),
        bash_tool(),
        glob_tool(),
        grep_tool(),
        ls_tool(),
        cwd_tool(),
        web_fetch_tool(),
        web_search_tool(),
        notebook_edit_tool(),
        codebase_search_tool(),
    ]
}

/// Read file tool definition
fn read_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "file_path".to_string(),
        ParameterSchema::string(Some("The absolute path to the file to read")),
    );
    properties.insert(
        "offset".to_string(),
        ParameterSchema::integer(Some("The line number to start reading from (1-indexed)")),
    );
    properties.insert(
        "limit".to_string(),
        ParameterSchema::integer(Some("Maximum number of lines to read")),
    );
    properties.insert(
        "pages".to_string(),
        ParameterSchema::string(Some("Page range for PDF files (e.g., '1-5', '3', '10-20'). Only for PDFs. Max 20 pages per request.")),
    );

    ToolDefinition {
        name: "Read".to_string(),
        description: "Read the contents of a file. Returns the file contents with line numbers. Supports optional offset and limit for reading specific portions of large files. Also reads PDF, DOCX, XLSX, Jupyter notebooks (.ipynb), and images (returns metadata).".to_string(),
        input_schema: ParameterSchema::object(
            Some("Read file parameters"),
            properties,
            vec!["file_path".to_string()],
        ),
    }
}

/// Write file tool definition
fn write_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "file_path".to_string(),
        ParameterSchema::string(Some("The absolute path to the file to write")),
    );
    properties.insert(
        "content".to_string(),
        ParameterSchema::string(Some("The content to write to the file")),
    );

    ToolDefinition {
        name: "Write".to_string(),
        description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Creates parent directories as needed.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Write file parameters"),
            properties,
            vec!["file_path".to_string(), "content".to_string()],
        ),
    }
}

/// Edit file tool definition
fn edit_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "file_path".to_string(),
        ParameterSchema::string(Some("The absolute path to the file to edit")),
    );
    properties.insert(
        "old_string".to_string(),
        ParameterSchema::string(Some("The exact string to replace")),
    );
    properties.insert(
        "new_string".to_string(),
        ParameterSchema::string(Some("The string to replace it with")),
    );
    properties.insert(
        "replace_all".to_string(),
        ParameterSchema::boolean(Some("Replace all occurrences (default: false)")),
    );

    ToolDefinition {
        name: "Edit".to_string(),
        description: "Perform string replacement in a file. The old_string must be unique in the file unless replace_all is true. Preserves file encoding and line endings.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Edit file parameters"),
            properties,
            vec!["file_path".to_string(), "old_string".to_string(), "new_string".to_string()],
        ),
    }
}

/// Bash command tool definition
fn bash_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "command".to_string(),
        ParameterSchema::string(Some("The command to execute")),
    );
    properties.insert(
        "timeout".to_string(),
        ParameterSchema::integer(Some(
            "Timeout in milliseconds (default: 120000, max: 600000)",
        )),
    );
    properties.insert(
        "working_dir".to_string(),
        ParameterSchema::string(Some("Working directory for the command")),
    );

    ToolDefinition {
        name: "Bash".to_string(),
        description: "Execute a shell command. Returns stdout and stderr. Has a configurable timeout. Some dangerous commands are blocked for safety.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Bash command parameters"),
            properties,
            vec!["command".to_string()],
        ),
    }
}

/// Glob pattern matching tool definition
fn glob_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "pattern".to_string(),
        ParameterSchema::string(Some(
            "The glob pattern to match (e.g., '**/*.rs', 'src/**/*.ts')",
        )),
    );
    properties.insert(
        "path".to_string(),
        ParameterSchema::string(Some(
            "The directory to search in (defaults to current working directory)",
        )),
    );

    ToolDefinition {
        name: "Glob".to_string(),
        description: "Find files matching a glob pattern. Returns a list of matching file paths sorted by modification time.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Glob parameters"),
            properties,
            vec!["pattern".to_string()],
        ),
    }
}

/// Grep content search tool definition
fn grep_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "pattern".to_string(),
        ParameterSchema::string(Some("Regular expression pattern to search for")),
    );
    properties.insert(
        "path".to_string(),
        ParameterSchema::string(Some("File or directory to search in")),
    );
    properties.insert(
        "glob".to_string(),
        ParameterSchema::string(Some("Glob pattern to filter files (e.g., '*.rs')")),
    );
    properties.insert(
        "case_insensitive".to_string(),
        ParameterSchema::boolean(Some("Case insensitive search")),
    );
    properties.insert(
        "context_lines".to_string(),
        ParameterSchema::integer(Some("Number of context lines before and after matches")),
    );
    properties.insert(
        "output_mode".to_string(),
        ParameterSchema::string(Some(
            "Output mode: 'files_with_matches' (file paths only), 'content' (matching lines), 'count' (match counts). Default: 'files_with_matches'",
        )),
    );
    properties.insert(
        "head_limit".to_string(),
        ParameterSchema::integer(Some("Limit output to first N results (0 = unlimited)")),
    );

    ToolDefinition {
        name: "Grep".to_string(),
        description: "Search for content matching a regex pattern in files. Respects .gitignore and skips binary/hidden files. Returns matching lines with file paths and line numbers.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Grep parameters"),
            properties,
            vec!["pattern".to_string()],
        ),
    }
}

/// List directory tool definition
fn ls_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "path".to_string(),
        ParameterSchema::string(Some(
            "The directory path to list. Absolute or relative to working directory.",
        )),
    );
    properties.insert(
        "show_hidden".to_string(),
        ParameterSchema::boolean(Some("Show hidden files (starting with '.'), default false")),
    );

    ToolDefinition {
        name: "LS".to_string(),
        description: "List files and directories at the given path. Returns a formatted listing with type indicators (DIR/FILE), file sizes, and names.".to_string(),
        input_schema: ParameterSchema::object(
            Some("List directory parameters"),
            properties,
            vec!["path".to_string()],
        ),
    }
}

/// Task sub-agent tool definition
fn task_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "prompt".to_string(),
        ParameterSchema::string(Some(
            "The task description for the sub-agent. Be specific about what you want done.",
        )),
    );
    properties.insert(
        "task_type".to_string(),
        ParameterSchema::string(Some(
            "Optional task type hint: 'explore' (codebase exploration), 'analyze' (deep analysis), 'implement' (code changes). Default: inferred from prompt.",
        )),
    );

    ToolDefinition {
        name: "Task".to_string(),
        description: "Launch a sub-agent with its own independent context window to handle complex tasks. The sub-agent has access to all basic tools (Read, Write, Edit, Bash, Glob, Grep, LS, Cwd) but cannot spawn further sub-agents. Only the final summary is returned to you. Use this for codebase exploration, deep analysis, or focused implementations that benefit from a fresh context.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Task parameters"),
            properties,
            vec!["prompt".to_string()],
        ),
    }
}

/// Analyze tool definition
fn analyze_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        ParameterSchema::string(Some(
            "What to analyze. Use concise objective language (e.g., 'analyze architecture and test strategy').",
        )),
    );
    properties.insert(
        "mode".to_string(),
        ParameterSchema::string(Some(
            "Analysis mode: 'quick' (default — lightweight file inventory brief), 'deep' (full multi-phase analysis pipeline, use only when explicitly needed), or 'local' (focused on specific paths).",
        )),
    );
    properties.insert(
        "path_hint".to_string(),
        ParameterSchema::string(Some(
            "Optional path/file hint to focus the analysis scope (e.g., 'src/plan_cascade/core').",
        )),
    );

    ToolDefinition {
        name: "Analyze".to_string(),
        description: "Gather project context for informed decisions. Defaults to quick mode: returns a concise project brief from file inventory (relevant files, components, test coverage). Use mode='deep' ONLY when the user explicitly requests comprehensive architectural analysis, cross-module dependency tracing, or full codebase review. Do NOT use this tool for simple questions — use Cwd, LS, Read, Glob, or Grep instead.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Analyze parameters"),
            properties,
            vec!["query".to_string()],
        ),
    }
}

/// Current working directory tool definition
fn cwd_tool() -> ToolDefinition {
    ToolDefinition {
        name: "Cwd".to_string(),
        description: "Get the current working directory (project root). Returns the absolute path of the current working directory.".to_string(),
        input_schema: ParameterSchema::object(
            Some("No parameters required"),
            HashMap::new(),
            vec![],
        ),
    }
}

/// WebFetch tool definition
fn web_fetch_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "url".to_string(),
        ParameterSchema::string(Some(
            "The URL to fetch content from. HTTP URLs are auto-upgraded to HTTPS.",
        )),
    );
    properties.insert(
        "prompt".to_string(),
        ParameterSchema::string(Some("Description of what to extract from the page (included as context, not processed locally)")),
    );
    properties.insert(
        "timeout".to_string(),
        ParameterSchema::integer(Some("Timeout in seconds (default: 30, max: 60)")),
    );

    ToolDefinition {
        name: "WebFetch".to_string(),
        description: "Fetch a web page and convert it to markdown. Supports HTML pages, documentation, and other web content. Private/local URLs are blocked for security. Results are cached for 15 minutes.".to_string(),
        input_schema: ParameterSchema::object(
            Some("WebFetch parameters"),
            properties,
            vec!["url".to_string()],
        ),
    }
}

/// WebSearch tool definition
fn web_search_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        ParameterSchema::string(Some("The search query")),
    );
    properties.insert(
        "max_results".to_string(),
        ParameterSchema::integer(Some("Maximum number of results (default: 5, max: 10)")),
    );

    ToolDefinition {
        name: "WebSearch".to_string(),
        description: "Search the web for current information. Returns titles, URLs, and snippets. Supports Tavily, Brave Search, and DuckDuckGo providers (configured in settings).".to_string(),
        input_schema: ParameterSchema::object(
            Some("WebSearch parameters"),
            properties,
            vec!["query".to_string()],
        ),
    }
}

/// CodebaseSearch tool definition
pub fn codebase_search_tool() -> ToolDefinition {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        ParameterSchema::string(Some(
            "Search pattern — symbol name, file path fragment, or keyword to search for",
        )),
    );

    // scope with enum constraint and default
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

    ToolDefinition {
        name: "CodebaseSearch".to_string(),
        description: "Search the project's indexed codebase for symbols, files, or semantic similarity. Uses the pre-built SQLite index for fast lookups without scanning the filesystem. The 'semantic' scope performs vector similarity search over code chunks. Preferred over Grep/Glob for initial code exploration when the index is available.".to_string(),
        input_schema: ParameterSchema::object(
            Some("CodebaseSearch parameters"),
            properties,
            vec!["query".to_string()],
        ),
    }
}

/// NotebookEdit tool definition
fn notebook_edit_tool() -> ToolDefinition {
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
        ParameterSchema::string(Some(
            "Cell type: 'code' or 'markdown' (required for insert)",
        )),
    );
    properties.insert(
        "new_source".to_string(),
        ParameterSchema::string(Some("New cell content (required for replace and insert)")),
    );

    ToolDefinition {
        name: "NotebookEdit".to_string(),
        description: "Edit a Jupyter notebook (.ipynb) cell. Supports replacing cell content, inserting new cells, and deleting cells. Preserves notebook metadata and untouched cell outputs.".to_string(),
        input_schema: ParameterSchema::object(
            Some("NotebookEdit parameters"),
            properties,
            vec!["notebook_path".to_string(), "cell_index".to_string(), "operation".to_string()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 14);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Read"));
        assert!(names.contains(&"Write"));
        assert!(names.contains(&"Edit"));
        assert!(names.contains(&"Bash"));
        assert!(names.contains(&"Glob"));
        assert!(names.contains(&"Grep"));
        assert!(names.contains(&"LS"));
        assert!(names.contains(&"Cwd"));
        assert!(names.contains(&"Analyze"));
        assert!(names.contains(&"Task"));
        assert!(names.contains(&"CodebaseSearch"));
    }

    #[test]
    fn test_get_basic_tool_definitions() {
        let tools = get_basic_tool_definitions();
        assert_eq!(tools.len(), 12);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"Task"));
        assert!(names.contains(&"CodebaseSearch"));
    }

    #[test]
    fn test_tool_serialization() {
        let tools = get_tool_definitions();
        for tool in tools {
            let json = serde_json::to_string(&tool).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_analyze_tool_describes_quick_and_deep_modes() {
        let tools = get_tool_definitions();
        let analyze = tools.iter().find(|t| t.name == "Analyze").unwrap();

        // Description should mention quick mode (default) and deep mode
        assert!(
            analyze.description.contains("quick mode"),
            "Analyze description should mention quick mode"
        );
        assert!(
            analyze.description.contains("deep"),
            "Analyze description should mention deep mode"
        );
        assert!(
            analyze
                .description
                .contains("Do NOT use this tool for simple questions"),
            "Analyze description should discourage use for simple questions"
        );

        // Mode parameter should describe quick and deep
        let mode_schema = analyze
            .input_schema
            .properties
            .as_ref()
            .unwrap()
            .get("mode")
            .unwrap();
        let mode_desc = mode_schema.description.as_deref().unwrap_or("");
        assert!(
            mode_desc.contains("quick") && mode_desc.contains("deep"),
            "Mode parameter should describe quick and deep modes"
        );
    }

    #[test]
    fn test_codebase_search_tool_schema() {
        let tool = codebase_search_tool();
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
}
