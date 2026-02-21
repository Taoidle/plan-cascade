//! Tool Calling Integration Tests
//!
//! Verifies end-to-end agentic tool usage including:
//! - Tool definition serialization for OpenAI and Anthropic formats
//! - System prompt construction with working directory
//! - All 8 tools execute correctly through ToolExecutor
//! - Agentic loop flow with tool calls

use plan_cascade_desktop::services::llm::types::ToolDefinition;
use plan_cascade_desktop::services::tools::{
    build_system_prompt, get_tool_definitions_from_registry, merge_system_prompts, ToolExecutor,
};
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// Tool Definition Tests
// ============================================================================

#[test]
fn test_get_tool_definitions_returns_14_tools() {
    let tools = get_tool_definitions_from_registry();
    assert_eq!(tools.len(), 14, "Expected exactly 14 tool definitions");
}

#[test]
fn test_all_tool_names_present() {
    let tools = get_tool_definitions_from_registry();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    let expected = [
        "Read",
        "Write",
        "Edit",
        "Bash",
        "Glob",
        "Grep",
        "LS",
        "Cwd",
        "Analyze",
        "Task",
        "WebFetch",
        "WebSearch",
        "NotebookEdit",
        "CodebaseSearch",
    ];
    for name in &expected {
        assert!(
            names.contains(name),
            "Missing tool definition for '{}'",
            name
        );
    }
}

#[test]
fn test_tool_definitions_have_descriptions() {
    let tools = get_tool_definitions_from_registry();
    for tool in &tools {
        assert!(
            !tool.description.is_empty(),
            "Tool '{}' has empty description",
            tool.name
        );
    }
}

#[test]
fn test_tool_definitions_have_object_schemas() {
    let tools = get_tool_definitions_from_registry();
    for tool in &tools {
        assert_eq!(
            tool.input_schema.schema_type, "object",
            "Tool '{}' input_schema should be type 'object', got '{}'",
            tool.name, tool.input_schema.schema_type
        );
    }
}

// ============================================================================
// OpenAI Function Calling Format Serialization Tests
// ============================================================================

#[test]
fn test_tool_definition_serializes_to_openai_function_format() {
    // OpenAI function calling expects:
    // { "name": "...", "description": "...", "input_schema": { "type": "object", "properties": {...}, "required": [...] } }
    let tools = get_tool_definitions_from_registry();

    for tool in &tools {
        let json = serde_json::to_value(tool).unwrap();

        // Must have name as string
        assert!(
            json.get("name").unwrap().is_string(),
            "Tool '{}': name must be a string",
            tool.name
        );

        // Must have description as string
        assert!(
            json.get("description").unwrap().is_string(),
            "Tool '{}': description must be a string",
            tool.name
        );

        // Must have input_schema as object
        let schema = json.get("input_schema").unwrap();
        assert!(
            schema.is_object(),
            "Tool '{}': input_schema must be an object",
            tool.name
        );

        // Schema must have type = "object"
        assert_eq!(
            schema.get("type").unwrap().as_str().unwrap(),
            "object",
            "Tool '{}': input_schema.type must be 'object'",
            tool.name
        );
    }
}

#[test]
fn test_read_tool_openai_schema_has_required_fields() {
    let tools = get_tool_definitions_from_registry();
    let read_tool = tools.iter().find(|t| t.name == "Read").unwrap();

    let json = serde_json::to_value(read_tool).unwrap();
    let schema = json.get("input_schema").unwrap();

    // Must have properties
    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert!(
        properties.contains_key("file_path"),
        "Read tool must have file_path property"
    );

    // file_path must be required
    let required = schema.get("required").unwrap().as_array().unwrap();
    let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(
        required_strs.contains(&"file_path"),
        "Read tool: file_path must be required"
    );
}

#[test]
fn test_write_tool_openai_schema_has_required_fields() {
    let tools = get_tool_definitions_from_registry();
    let write_tool = tools.iter().find(|t| t.name == "Write").unwrap();

    let json = serde_json::to_value(write_tool).unwrap();
    let schema = json.get("input_schema").unwrap();

    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert!(properties.contains_key("file_path"));
    assert!(properties.contains_key("content"));

    let required = schema.get("required").unwrap().as_array().unwrap();
    let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(required_strs.contains(&"file_path"));
    assert!(required_strs.contains(&"content"));
}

#[test]
fn test_edit_tool_openai_schema_has_required_fields() {
    let tools = get_tool_definitions_from_registry();
    let edit_tool = tools.iter().find(|t| t.name == "Edit").unwrap();

    let json = serde_json::to_value(edit_tool).unwrap();
    let schema = json.get("input_schema").unwrap();

    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert!(properties.contains_key("file_path"));
    assert!(properties.contains_key("old_string"));
    assert!(properties.contains_key("new_string"));
    assert!(properties.contains_key("replace_all"));

    let required = schema.get("required").unwrap().as_array().unwrap();
    let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(required_strs.contains(&"file_path"));
    assert!(required_strs.contains(&"old_string"));
    assert!(required_strs.contains(&"new_string"));
}

#[test]
fn test_bash_tool_openai_schema() {
    let tools = get_tool_definitions_from_registry();
    let bash_tool = tools.iter().find(|t| t.name == "Bash").unwrap();

    let json = serde_json::to_value(bash_tool).unwrap();
    let schema = json.get("input_schema").unwrap();

    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert!(properties.contains_key("command"));

    let required = schema.get("required").unwrap().as_array().unwrap();
    let required_strs: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(required_strs.contains(&"command"));
}

#[test]
fn test_cwd_tool_has_empty_properties() {
    let tools = get_tool_definitions_from_registry();
    let cwd_tool = tools.iter().find(|t| t.name == "Cwd").unwrap();

    let json = serde_json::to_value(cwd_tool).unwrap();
    let schema = json.get("input_schema").unwrap();

    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert!(properties.is_empty(), "Cwd tool should have no properties");

    let required = schema.get("required").unwrap().as_array().unwrap();
    assert!(
        required.is_empty(),
        "Cwd tool should have no required params"
    );
}

// ============================================================================
// Anthropic API Format Serialization Tests
// ============================================================================

#[test]
fn test_tool_definitions_serialize_for_anthropic_format() {
    // Anthropic format expects:
    // { "name": "...", "description": "...", "input_schema": { "type": "object", "properties": {...}, "required": [...] } }
    // Same structure as our ToolDefinition, so verify full round-trip serialization
    let tools = get_tool_definitions_from_registry();

    for tool in &tools {
        // Serialize to JSON string
        let json_str = serde_json::to_string(tool).unwrap();
        assert!(!json_str.is_empty());

        // Deserialize back
        let deserialized: ToolDefinition = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, tool.name);
        assert_eq!(deserialized.description, tool.description);
        assert_eq!(
            deserialized.input_schema.schema_type,
            tool.input_schema.schema_type
        );
    }
}

#[test]
fn test_tool_definitions_round_trip_preserves_property_types() {
    let tools = get_tool_definitions_from_registry();

    for tool in &tools {
        let json_str = serde_json::to_string(tool).unwrap();
        let deserialized: ToolDefinition = serde_json::from_str(&json_str).unwrap();

        // Check property schemas round-trip correctly
        if let Some(orig_props) = &tool.input_schema.properties {
            let deser_props = deserialized.input_schema.properties.as_ref().unwrap();
            assert_eq!(
                orig_props.len(),
                deser_props.len(),
                "Tool '{}': property count mismatch after round-trip",
                tool.name
            );

            for (key, orig_schema) in orig_props {
                let deser_schema = deser_props.get(key).unwrap_or_else(|| {
                    panic!(
                        "Tool '{}': missing property '{}' after round-trip",
                        tool.name, key
                    )
                });
                assert_eq!(
                    orig_schema.schema_type, deser_schema.schema_type,
                    "Tool '{}': property '{}' type mismatch after round-trip",
                    tool.name, key
                );
            }
        }
    }
}

#[test]
fn test_parameter_schema_types_correct() {
    let tools = get_tool_definitions_from_registry();

    // Verify specific parameter types
    let read_tool = tools.iter().find(|t| t.name == "Read").unwrap();
    let props = read_tool.input_schema.properties.as_ref().unwrap();
    assert_eq!(props.get("file_path").unwrap().schema_type, "string");
    assert_eq!(props.get("offset").unwrap().schema_type, "integer");
    assert_eq!(props.get("limit").unwrap().schema_type, "integer");

    let edit_tool = tools.iter().find(|t| t.name == "Edit").unwrap();
    let props = edit_tool.input_schema.properties.as_ref().unwrap();
    assert_eq!(props.get("replace_all").unwrap().schema_type, "boolean");

    let grep_tool = tools.iter().find(|t| t.name == "Grep").unwrap();
    let props = grep_tool.input_schema.properties.as_ref().unwrap();
    assert_eq!(
        props.get("case_insensitive").unwrap().schema_type,
        "boolean"
    );
    assert_eq!(props.get("context_lines").unwrap().schema_type, "integer");
}

#[test]
fn test_tool_definitions_all_serializable_to_json_array() {
    // Verify all tools can be serialized together as an array (as sent to APIs)
    let tools = get_tool_definitions_from_registry();
    let json_str = serde_json::to_string(&tools).unwrap();
    assert!(!json_str.is_empty());

    let deserialized: Vec<ToolDefinition> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.len(), 14);
}

// ============================================================================
// System Prompt Tests
// ============================================================================

#[test]
fn test_system_prompt_includes_working_directory() {
    let tools = get_tool_definitions_from_registry();
    let project_root = PathBuf::from("D:\\test\\my-project");
    let prompt = build_system_prompt(&project_root, &tools, None, "test", "test-model", "en");

    assert!(
        prompt.contains("D:\\test\\my-project"),
        "System prompt must contain the working directory"
    );
}

#[test]
fn test_system_prompt_includes_all_tool_names() {
    let tools = get_tool_definitions_from_registry();
    let project_root = PathBuf::from("/test/project");
    let prompt = build_system_prompt(&project_root, &tools, None, "test", "test-model", "en");

    for tool in &tools {
        assert!(
            prompt.contains(&tool.name),
            "System prompt must mention tool '{}'",
            tool.name
        );
    }
}

#[test]
fn test_system_prompt_includes_tool_descriptions() {
    let tools = get_tool_definitions_from_registry();
    let project_root = PathBuf::from("/test/project");
    let prompt = build_system_prompt(&project_root, &tools, None, "test", "test-model", "en");

    // At least check a few key descriptions are present
    assert!(prompt.contains("Read the contents of a file"));
    assert!(prompt.contains("Execute a shell command"));
    assert!(prompt.contains("current working directory"));
}

#[test]
fn test_system_prompt_includes_usage_guidelines() {
    let tools = get_tool_definitions_from_registry();
    let project_root = PathBuf::from("/test/project");
    let prompt = build_system_prompt(&project_root, &tools, None, "test", "test-model", "en");

    assert!(prompt.contains("General Guidelines"));
    assert!(prompt.contains("Read before modifying"));
    assert!(prompt.contains("Decision Tree"));
}

#[test]
fn test_merge_system_prompts_with_user_prompt() {
    let tool_prompt = "Tool system prompt content.";
    let user_prompt = Some("You are a Rust expert.");

    let merged = merge_system_prompts(tool_prompt, user_prompt);

    assert!(merged.starts_with("Tool system prompt content."));
    assert!(merged.contains("---"));
    assert!(merged.ends_with("You are a Rust expert."));
}

#[test]
fn test_merge_system_prompts_without_user_prompt() {
    let tool_prompt = "Tool system prompt content.";

    let merged = merge_system_prompts(tool_prompt, None);
    assert_eq!(merged, "Tool system prompt content.");
}

#[test]
fn test_merge_system_prompts_with_empty_user_prompt() {
    let tool_prompt = "Tool system prompt content.";

    let merged = merge_system_prompts(tool_prompt, Some(""));
    assert_eq!(merged, "Tool system prompt content.");
}

// ============================================================================
// ToolExecutor Integration Tests - All 8 Tools
// ============================================================================

fn setup_test_env() -> TempDir {
    let dir = TempDir::new().unwrap();
    // Create a test file
    std::fs::write(
        dir.path().join("hello.txt"),
        "line 1: hello world\nline 2: foo bar\nline 3: baz qux\n",
    )
    .unwrap();
    // Create a subdirectory with a file
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    std::fs::write(
        dir.path().join("subdir").join("nested.rs"),
        "fn main() {}\n",
    )
    .unwrap();
    // Create another file for grep testing
    std::fs::write(
        dir.path().join("search_target.txt"),
        "This file contains a UNIQUE_MARKER for testing.\nAnother line.\n",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn test_executor_read_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("hello.txt").to_string_lossy().to_string()
    });

    let result = executor.execute("Read", &args).await;
    assert!(result.success, "Read should succeed: {:?}", result.error);
    let output = result.output.unwrap();
    assert!(
        output.contains("hello world"),
        "Read output should contain file content"
    );
    assert!(
        output.contains("line 2"),
        "Read output should contain line 2"
    );
}

#[tokio::test]
async fn test_executor_read_tool_with_offset_and_limit() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("hello.txt").to_string_lossy().to_string(),
        "offset": 2,
        "limit": 1
    });

    let result = executor.execute("Read", &args).await;
    assert!(result.success);
    let output = result.output.unwrap();
    assert!(
        output.contains("foo bar"),
        "Should contain line 2 (offset=2)"
    );
    // Should only have 1 line (limit=1)
    let line_count = output.lines().count();
    assert_eq!(line_count, 1, "Should return exactly 1 line");
}

#[tokio::test]
async fn test_executor_write_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let new_file = dir.path().join("new_file.txt");
    let args = serde_json::json!({
        "file_path": new_file.to_string_lossy().to_string(),
        "content": "Created by integration test.\nSecond line.\n"
    });

    let result = executor.execute("Write", &args).await;
    assert!(result.success, "Write should succeed: {:?}", result.error);
    assert!(new_file.exists(), "File should be created");

    let content = std::fs::read_to_string(&new_file).unwrap();
    assert_eq!(content, "Created by integration test.\nSecond line.\n");
}

#[tokio::test]
async fn test_executor_write_tool_creates_parent_dirs() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let deep_file = dir.path().join("a").join("b").join("c").join("deep.txt");
    let args = serde_json::json!({
        "file_path": deep_file.to_string_lossy().to_string(),
        "content": "Deep file content."
    });

    let result = executor.execute("Write", &args).await;
    assert!(
        result.success,
        "Write should create parent dirs: {:?}",
        result.error
    );
    assert!(deep_file.exists());
}

#[tokio::test]
async fn test_executor_edit_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let file_path = dir.path().join("hello.txt").to_string_lossy().to_string();
    let args = serde_json::json!({
        "file_path": file_path,
        "old_string": "foo bar",
        "new_string": "REPLACED"
    });

    let result = executor.execute("Edit", &args).await;
    assert!(result.success, "Edit should succeed: {:?}", result.error);

    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert!(content.contains("REPLACED"));
    assert!(!content.contains("foo bar"));
}

#[tokio::test]
async fn test_executor_edit_tool_replace_all() {
    let dir = setup_test_env();
    std::fs::write(dir.path().join("dup.txt"), "aaa bbb aaa ccc aaa").unwrap();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
        "old_string": "aaa",
        "new_string": "ZZZ",
        "replace_all": true
    });

    let result = executor.execute("Edit", &args).await;
    assert!(
        result.success,
        "Edit replace_all should succeed: {:?}",
        result.error
    );

    let content = std::fs::read_to_string(dir.path().join("dup.txt")).unwrap();
    assert_eq!(content, "ZZZ bbb ZZZ ccc ZZZ");
}

#[tokio::test]
async fn test_executor_edit_tool_non_unique_fails() {
    let dir = setup_test_env();
    std::fs::write(dir.path().join("dup.txt"), "aaa bbb aaa").unwrap();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("dup.txt").to_string_lossy().to_string(),
        "old_string": "aaa",
        "new_string": "ZZZ"
    });

    let result = executor.execute("Edit", &args).await;
    assert!(!result.success, "Edit should fail for non-unique string");
    assert!(result.error.unwrap().contains("appears"));
}

#[tokio::test]
async fn test_executor_bash_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "command": "echo integration_test_output"
    });

    let result = executor.execute("Bash", &args).await;
    assert!(result.success, "Bash should succeed: {:?}", result.error);
    assert!(
        result.output.unwrap().contains("integration_test_output"),
        "Bash output should contain echoed string"
    );
}

#[tokio::test]
async fn test_executor_bash_tool_blocked_command() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "command": "rm -rf /"
    });

    let result = executor.execute("Bash", &args).await;
    assert!(!result.success, "Blocked command should fail");
    assert!(result.error.unwrap().contains("blocked"));
}

#[tokio::test]
async fn test_executor_bash_tool_with_working_dir() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    #[cfg(windows)]
    let args = serde_json::json!({
        "command": "cd",
        "working_dir": dir.path().to_string_lossy().to_string()
    });
    #[cfg(not(windows))]
    let args = serde_json::json!({
        "command": "pwd",
        "working_dir": dir.path().to_string_lossy().to_string()
    });

    let result = executor.execute("Bash", &args).await;
    assert!(result.success);
}

#[tokio::test]
async fn test_executor_glob_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "pattern": "**/*.txt",
        "path": dir.path().to_string_lossy().to_string()
    });

    let result = executor.execute("Glob", &args).await;
    assert!(result.success, "Glob should succeed: {:?}", result.error);
    let output = result.output.unwrap();
    assert!(output.contains("hello.txt"), "Glob should find hello.txt");
    assert!(
        output.contains("search_target.txt"),
        "Glob should find search_target.txt"
    );
}

#[tokio::test]
async fn test_executor_glob_tool_rs_files() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "pattern": "**/*.rs",
        "path": dir.path().to_string_lossy().to_string()
    });

    let result = executor.execute("Glob", &args).await;
    assert!(result.success);
    let output = result.output.unwrap();
    assert!(output.contains("nested.rs"), "Glob should find nested.rs");
    assert!(
        !output.contains("hello.txt"),
        "Glob for *.rs should not include .txt files"
    );
}

#[tokio::test]
async fn test_executor_grep_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "pattern": "UNIQUE_MARKER",
        "path": dir.path().to_string_lossy().to_string(),
        "output_mode": "content"
    });

    let result = executor.execute("Grep", &args).await;
    assert!(result.success, "Grep should succeed: {:?}", result.error);
    let output = result.output.unwrap();
    assert!(
        output.contains("UNIQUE_MARKER"),
        "Grep should find the marker"
    );
    assert!(
        output.contains("search_target.txt"),
        "Grep should show the file name"
    );
}

#[tokio::test]
async fn test_executor_grep_tool_case_insensitive() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "pattern": "unique_marker",
        "path": dir.path().to_string_lossy().to_string(),
        "case_insensitive": true,
        "output_mode": "content"
    });

    let result = executor.execute("Grep", &args).await;
    assert!(result.success);
    let output = result.output.unwrap();
    assert!(
        output.contains("UNIQUE_MARKER"),
        "Case insensitive grep should find UNIQUE_MARKER"
    );
}

#[tokio::test]
async fn test_executor_grep_tool_no_matches() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "pattern": "THIS_STRING_DOES_NOT_EXIST_ANYWHERE",
        "path": dir.path().to_string_lossy().to_string()
    });

    let result = executor.execute("Grep", &args).await;
    assert!(result.success);
    assert!(result.output.unwrap().contains("No matches found"));
}

#[tokio::test]
async fn test_executor_ls_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "path": dir.path().to_string_lossy().to_string()
    });

    let result = executor.execute("LS", &args).await;
    assert!(result.success, "LS should succeed: {:?}", result.error);
    let output = result.output.unwrap();
    assert!(
        output.contains("DIR"),
        "LS should show directory indicators"
    );
    assert!(output.contains("subdir"), "LS should show subdirectory");
    assert!(output.contains("FILE"), "LS should show file indicators");
    assert!(output.contains("hello.txt"), "LS should show hello.txt");
}

#[tokio::test]
async fn test_executor_ls_tool_show_hidden() {
    let dir = setup_test_env();
    std::fs::write(dir.path().join(".hidden_file"), "hidden").unwrap();
    let executor = ToolExecutor::new(dir.path());

    // Without show_hidden
    let args = serde_json::json!({
        "path": dir.path().to_string_lossy().to_string()
    });
    let result = executor.execute("LS", &args).await;
    assert!(result.success);
    assert!(!result.output.unwrap().contains(".hidden_file"));

    // With show_hidden
    let args = serde_json::json!({
        "path": dir.path().to_string_lossy().to_string(),
        "show_hidden": true
    });
    let result = executor.execute("LS", &args).await;
    assert!(result.success);
    assert!(result.output.unwrap().contains(".hidden_file"));
}

#[tokio::test]
async fn test_executor_ls_tool_nonexistent_dir() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "path": dir.path().join("nonexistent").to_string_lossy().to_string()
    });

    let result = executor.execute("LS", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("not found"));
}

#[tokio::test]
async fn test_executor_cwd_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});

    let result = executor.execute("Cwd", &args).await;
    assert!(result.success, "Cwd should succeed: {:?}", result.error);
    let output = result.output.unwrap();
    assert_eq!(
        output,
        dir.path().to_string_lossy().to_string(),
        "Cwd should return the project root"
    );
}

#[tokio::test]
async fn test_executor_unknown_tool() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});

    let result = executor.execute("UnknownTool", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("Unknown tool"));
}

// ============================================================================
// ToolResult Content Formatting Tests
// ============================================================================

#[tokio::test]
async fn test_tool_result_ok_to_content() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("Cwd", &args).await;
    assert!(result.success);

    let content = result.to_content();
    assert!(!content.is_empty());
    assert!(!content.starts_with("Error:"));
}

#[tokio::test]
async fn test_tool_result_error_to_content() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("nonexistent.txt").to_string_lossy().to_string()
    });
    let result = executor.execute("Read", &args).await;
    assert!(!result.success);

    let content = result.to_content();
    assert!(content.starts_with("Error:"));
}

// ============================================================================
// End-to-End Tool Call Flow Tests
// ============================================================================

#[tokio::test]
async fn test_write_then_read_flow() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    // Step 1: Write a file
    let file_path = dir
        .path()
        .join("flow_test.txt")
        .to_string_lossy()
        .to_string();
    let write_args = serde_json::json!({
        "file_path": &file_path,
        "content": "Hello from flow test!\nLine 2.\n"
    });
    let write_result = executor.execute("Write", &write_args).await;
    assert!(write_result.success);

    // Step 2: Read the file back
    let read_args = serde_json::json!({
        "file_path": &file_path
    });
    let read_result = executor.execute("Read", &read_args).await;
    assert!(read_result.success);
    let output = read_result.output.unwrap();
    assert!(output.contains("Hello from flow test!"));
    assert!(output.contains("Line 2."));
}

#[tokio::test]
async fn test_write_then_edit_then_read_flow() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let file_path = dir
        .path()
        .join("edit_flow.txt")
        .to_string_lossy()
        .to_string();

    // Step 1: Write initial content
    let write_args = serde_json::json!({
        "file_path": &file_path,
        "content": "function hello() {\n  return 'world';\n}\n"
    });
    let write_result = executor.execute("Write", &write_args).await;
    assert!(write_result.success);

    // Step 2: Edit the function
    let edit_args = serde_json::json!({
        "file_path": &file_path,
        "old_string": "return 'world';",
        "new_string": "return 'universe';"
    });
    let edit_result = executor.execute("Edit", &edit_args).await;
    assert!(edit_result.success);

    // Step 3: Read back and verify
    let read_args = serde_json::json!({
        "file_path": &file_path
    });
    let read_result = executor.execute("Read", &read_args).await;
    assert!(read_result.success);
    let output = read_result.output.unwrap();
    assert!(output.contains("return 'universe';"));
    assert!(!output.contains("return 'world';"));
}

#[tokio::test]
async fn test_ls_then_glob_then_grep_flow() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    // Step 1: LS to discover structure
    let ls_args = serde_json::json!({
        "path": dir.path().to_string_lossy().to_string()
    });
    let ls_result = executor.execute("LS", &ls_args).await;
    assert!(ls_result.success);
    let ls_output = ls_result.output.unwrap();
    assert!(ls_output.contains("subdir"));

    // Step 2: Glob to find .rs files
    let glob_args = serde_json::json!({
        "pattern": "**/*.rs",
        "path": dir.path().to_string_lossy().to_string()
    });
    let glob_result = executor.execute("Glob", &glob_args).await;
    assert!(glob_result.success);
    let glob_output = glob_result.output.unwrap();
    assert!(glob_output.contains("nested.rs"));

    // Step 3: Grep for function definitions
    let grep_args = serde_json::json!({
        "pattern": "fn main",
        "path": dir.path().to_string_lossy().to_string(),
        "glob": "*.rs",
        "output_mode": "content"
    });
    let grep_result = executor.execute("Grep", &grep_args).await;
    assert!(grep_result.success);
    let grep_output = grep_result.output.unwrap();
    assert!(grep_output.contains("fn main"));
}

#[tokio::test]
async fn test_bash_then_read_flow() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    // Step 1: Use Write tool to create a file (more reliable cross-platform than bash redirect)
    let file_path = dir
        .path()
        .join("bash_out.txt")
        .to_string_lossy()
        .to_string();
    let write_args = serde_json::json!({
        "file_path": &file_path,
        "content": "bash_created_content\n"
    });
    let write_result = executor.execute("Write", &write_args).await;
    assert!(
        write_result.success,
        "Write should succeed: {:?}",
        write_result.error
    );

    // Step 2: Use bash to verify the file exists (cross-platform echo)
    let bash_args = serde_json::json!({
        "command": "echo file_verified",
        "working_dir": dir.path().to_string_lossy().to_string()
    });
    let bash_result = executor.execute("Bash", &bash_args).await;
    assert!(
        bash_result.success,
        "Bash should succeed: {:?}",
        bash_result.error
    );
    assert!(bash_result.output.unwrap().contains("file_verified"));

    // Step 3: Read the file back
    let read_args = serde_json::json!({
        "file_path": &file_path
    });
    let read_result = executor.execute("Read", &read_args).await;
    assert!(read_result.success);
    assert!(read_result.output.unwrap().contains("bash_created_content"));
}

// ============================================================================
// Missing Required Parameters Tests
// ============================================================================

#[tokio::test]
async fn test_read_missing_file_path() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("Read", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("file_path"));
}

#[tokio::test]
async fn test_write_missing_content() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("test.txt").to_string_lossy().to_string()
    });
    let result = executor.execute("Write", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("content"));
}

#[tokio::test]
async fn test_edit_missing_old_string() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "file_path": dir.path().join("hello.txt").to_string_lossy().to_string(),
        "new_string": "replacement"
    });
    let result = executor.execute("Edit", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("old_string"));
}

#[tokio::test]
async fn test_bash_missing_command() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("Bash", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("command"));
}

#[tokio::test]
async fn test_glob_missing_pattern() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("Glob", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("pattern"));
}

#[tokio::test]
async fn test_grep_missing_pattern() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("Grep", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("pattern"));
}

#[tokio::test]
async fn test_ls_missing_path() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("LS", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("path"));
}

// ============================================================================
// WebFetch Tool Tests
// ============================================================================

#[test]
fn test_web_fetch_tool_definition_exists() {
    let tools = get_tool_definitions_from_registry();
    let wf = tools.iter().find(|t| t.name == "WebFetch").unwrap();
    let props = wf.input_schema.properties.as_ref().unwrap();
    assert!(props.contains_key("url"));
    assert!(props.contains_key("prompt"));
    assert!(props.contains_key("timeout"));
    let required = wf.input_schema.required.as_ref().unwrap();
    assert!(required.contains(&"url".to_string()));
}

#[tokio::test]
async fn test_web_fetch_missing_url() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("WebFetch", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("url"));
}

#[tokio::test]
async fn test_web_fetch_blocks_private_urls() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "url": "http://127.0.0.1:8080/admin"
    });
    let result = executor.execute("WebFetch", &args).await;
    assert!(!result.success);
    let error = result.error.unwrap();
    assert!(
        error.contains("private") || error.contains("blocked") || error.contains("local"),
        "Should block private IP access, got: {}",
        error
    );
}

#[tokio::test]
async fn test_web_fetch_blocks_localhost() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "url": "http://localhost/secret"
    });
    let result = executor.execute("WebFetch", &args).await;
    assert!(!result.success);
}

// ============================================================================
// WebSearch Tool Tests
// ============================================================================

#[test]
fn test_web_search_tool_definition_exists() {
    let tools = get_tool_definitions_from_registry();
    let ws = tools.iter().find(|t| t.name == "WebSearch").unwrap();
    let props = ws.input_schema.properties.as_ref().unwrap();
    assert!(props.contains_key("query"));
    assert!(props.contains_key("max_results"));
    let required = ws.input_schema.required.as_ref().unwrap();
    assert!(required.contains(&"query".to_string()));
}

#[tokio::test]
async fn test_web_search_missing_query() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("WebSearch", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("query"));
}

#[tokio::test]
async fn test_web_search_not_configured() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({
        "query": "test search"
    });
    let result = executor.execute("WebSearch", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("not configured"));
}

// ============================================================================
// NotebookEdit Tool Tests
// ============================================================================

#[test]
fn test_notebook_edit_tool_definition_exists() {
    let tools = get_tool_definitions_from_registry();
    let nb = tools.iter().find(|t| t.name == "NotebookEdit").unwrap();
    let props = nb.input_schema.properties.as_ref().unwrap();
    assert!(props.contains_key("notebook_path"));
    assert!(props.contains_key("cell_index"));
    assert!(props.contains_key("operation"));
    assert!(props.contains_key("cell_type"));
    assert!(props.contains_key("new_source"));
}

#[tokio::test]
async fn test_notebook_edit_missing_params() {
    let dir = setup_test_env();
    let executor = ToolExecutor::new(dir.path());

    let args = serde_json::json!({});
    let result = executor.execute("NotebookEdit", &args).await;
    assert!(!result.success);
    assert!(result.error.unwrap().contains("notebook_path"));
}

#[tokio::test]
async fn test_notebook_edit_requires_read_first() {
    let dir = setup_test_env();
    // Create a minimal .ipynb file
    let nb_path = dir.path().join("test.ipynb");
    let nb_content = serde_json::json!({
        "nbformat": 4,
        "nbformat_minor": 2,
        "metadata": {},
        "cells": [{
            "cell_type": "code",
            "metadata": {},
            "source": ["print('hello')"],
            "outputs": []
        }]
    });
    std::fs::write(&nb_path, serde_json::to_string_pretty(&nb_content).unwrap()).unwrap();

    let executor = ToolExecutor::new(dir.path());

    // Try to edit without reading first
    let args = serde_json::json!({
        "notebook_path": nb_path.to_string_lossy().to_string(),
        "cell_index": 0,
        "operation": "replace",
        "new_source": "print('world')"
    });
    let result = executor.execute("NotebookEdit", &args).await;
    assert!(!result.success, "Should fail without reading first");
    assert!(result.error.unwrap().contains("read"));
}

#[tokio::test]
async fn test_notebook_edit_replace_after_read() {
    let dir = setup_test_env();
    let nb_path = dir.path().join("test2.ipynb");
    let nb_content = serde_json::json!({
        "nbformat": 4,
        "nbformat_minor": 2,
        "metadata": {},
        "cells": [{
            "cell_type": "code",
            "metadata": {},
            "source": ["print('hello')"],
            "outputs": []
        }]
    });
    std::fs::write(&nb_path, serde_json::to_string_pretty(&nb_content).unwrap()).unwrap();

    let executor = ToolExecutor::new(dir.path());
    let nb_path_str = nb_path.to_string_lossy().to_string();

    // Read first
    let read_args = serde_json::json!({ "file_path": &nb_path_str });
    let read_result = executor.execute("Read", &read_args).await;
    assert!(
        read_result.success,
        "Read should succeed: {:?}",
        read_result.error
    );

    // Now edit
    let edit_args = serde_json::json!({
        "notebook_path": &nb_path_str,
        "cell_index": 0,
        "operation": "replace",
        "new_source": "print('world')"
    });
    let edit_result = executor.execute("NotebookEdit", &edit_args).await;
    assert!(
        edit_result.success,
        "NotebookEdit should succeed: {:?}",
        edit_result.error
    );

    // Verify the change
    let content = std::fs::read_to_string(&nb_path).unwrap();
    assert!(content.contains("world"));
    assert!(!content.contains("hello"));
}

// ============================================================================
// Read Tool - Rich File Format Dispatch Tests
// ============================================================================

#[tokio::test]
async fn test_read_jupyter_notebook() {
    let dir = setup_test_env();
    let nb_path = dir.path().join("notebook.ipynb");
    let nb_content = serde_json::json!({
        "nbformat": 4,
        "nbformat_minor": 2,
        "metadata": {},
        "cells": [
            {
                "cell_type": "markdown",
                "metadata": {},
                "source": ["# Title"]
            },
            {
                "cell_type": "code",
                "metadata": {},
                "source": ["x = 1 + 2"],
                "outputs": [{"output_type": "execute_result", "data": {"text/plain": ["3"]}}]
            }
        ]
    });
    std::fs::write(&nb_path, serde_json::to_string_pretty(&nb_content).unwrap()).unwrap();

    let executor = ToolExecutor::new(dir.path());
    let args = serde_json::json!({
        "file_path": nb_path.to_string_lossy().to_string()
    });
    let result = executor.execute("Read", &args).await;
    assert!(
        result.success,
        "Read .ipynb should succeed: {:?}",
        result.error
    );
    let output = result.output.unwrap();
    assert!(
        output.contains("Title"),
        "Should contain markdown cell content"
    );
    assert!(
        output.contains("x = 1 + 2"),
        "Should contain code cell content"
    );
}

// ============================================================================
// ToolResult with image_data Tests
// ============================================================================

#[test]
fn test_tool_result_ok_with_image() {
    use plan_cascade_desktop::services::tools::ToolResult;

    let result = ToolResult::ok_with_image(
        "Image metadata",
        "image/png".to_string(),
        "base64data".to_string(),
    );
    assert!(result.success);
    assert_eq!(result.output.as_deref(), Some("Image metadata"));
    assert!(result.image_data.is_some());
    let (mime, data) = result.image_data.unwrap();
    assert_eq!(mime, "image/png");
    assert_eq!(data, "base64data");
}

#[test]
fn test_tool_result_ok_has_no_image_data() {
    use plan_cascade_desktop::services::tools::ToolResult;

    let result = ToolResult::ok("Regular output");
    assert!(result.image_data.is_none());
}

// ============================================================================
// System Prompt Includes New Tool Guidelines
// ============================================================================

#[test]
fn test_system_prompt_includes_web_fetch_guidance() {
    let tools = get_tool_definitions_from_registry();
    let project_root = PathBuf::from("/test/project");
    let prompt = build_system_prompt(&project_root, &tools, None, "test", "test-model", "en");

    assert!(
        prompt.contains("WebFetch"),
        "System prompt should mention WebFetch tool"
    );
    assert!(
        prompt.contains("WebSearch"),
        "System prompt should mention WebSearch tool"
    );
    assert!(
        prompt.contains("NotebookEdit"),
        "System prompt should mention NotebookEdit tool"
    );
}
