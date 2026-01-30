//! Tool Definitions
//!
//! Provides tool definitions for the executor.

use std::collections::HashMap;
use crate::services::llm::types::{ParameterSchema, ToolDefinition};

/// Get all available tool definitions
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_tool(),
        write_tool(),
        edit_tool(),
        bash_tool(),
        glob_tool(),
        grep_tool(),
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

    ToolDefinition {
        name: "Read".to_string(),
        description: "Read the contents of a file. Returns the file contents with line numbers. Supports optional offset and limit for reading specific portions of large files.".to_string(),
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
        ParameterSchema::integer(Some("Timeout in milliseconds (default: 120000, max: 600000)")),
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
        ParameterSchema::string(Some("The glob pattern to match (e.g., '**/*.rs', 'src/**/*.ts')")),
    );
    properties.insert(
        "path".to_string(),
        ParameterSchema::string(Some("The directory to search in (defaults to current working directory)")),
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

    ToolDefinition {
        name: "Grep".to_string(),
        description: "Search for content matching a regex pattern in files. Returns matching lines with file paths and line numbers.".to_string(),
        input_schema: ParameterSchema::object(
            Some("Grep parameters"),
            properties,
            vec!["pattern".to_string()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 6);

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Read"));
        assert!(names.contains(&"Write"));
        assert!(names.contains(&"Edit"));
        assert!(names.contains(&"Bash"));
        assert!(names.contains(&"Glob"));
        assert!(names.contains(&"Grep"));
    }

    #[test]
    fn test_tool_serialization() {
        let tools = get_tool_definitions();
        for tool in tools {
            let json = serde_json::to_string(&tool).unwrap();
            assert!(!json.is_empty());
        }
    }
}
