//! Prompt Fallback Integration Tests
//!
//! Verifies the prompt-based tool calling fallback system:
//! - Tool call instruction generation
//! - Parsing tool calls from LLM text responses
//! - Extracting clean text from responses containing tool call blocks
//! - Tool result formatting for re-injection into conversation
//! - End-to-end fallback flow simulation

use plan_cascade_desktop::services::tools::{
    build_tool_call_instructions, extract_text_without_tool_calls, format_tool_result,
    get_tool_definitions_from_registry, parse_tool_calls, ParsedToolCall, ToolExecutor,
};

// ============================================================================
// Tool Call Instructions Generation Tests
// ============================================================================

#[test]
fn test_build_instructions_contains_format_markers() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    assert!(
        instructions.contains("```tool_call"),
        "Instructions must show the tool_call code fence format"
    );
    assert!(
        instructions.contains("```"),
        "Instructions must show closing fence"
    );
}

#[test]
fn test_build_instructions_contains_all_tool_names() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    for tool in &tools {
        assert!(
            instructions.contains(&format!("### {}", tool.name)),
            "Instructions must contain tool name header for '{}'",
            tool.name
        );
    }
}

#[test]
fn test_build_instructions_contains_parameter_info() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    // Read tool should show file_path as required
    assert!(instructions.contains("file_path"));
    assert!(instructions.contains("(required)"));
    assert!(instructions.contains("(optional)"));
}

#[test]
fn test_build_instructions_contains_examples() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    assert!(
        instructions.contains("Example Tool Calls"),
        "Instructions should contain examples"
    );
    // Should contain at least one example with JSON
    assert!(instructions.contains(r#""tool": "Read""#));
}

#[test]
fn test_build_instructions_contains_usage_guidance() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    assert!(instructions.contains("IMPORTANT"));
    assert!(instructions.contains("Available Tools"));
}

// ============================================================================
// Parsing Single Tool Call Tests
// ============================================================================

#[test]
fn test_parse_single_read_tool_call() {
    let text = r#"I'll read the configuration file.

```tool_call
{"tool": "Read", "arguments": {"file_path": "/project/config.toml"}}
```

Let me analyze the configuration."#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Read");
    assert_eq!(
        calls[0].arguments["file_path"].as_str(),
        Some("/project/config.toml")
    );
}

#[test]
fn test_parse_write_tool_call() {
    let text = r#"```tool_call
{"tool": "Write", "arguments": {"file_path": "src/main.rs", "content": "fn main() {\n    println!(\"hello\");\n}\n"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Write");
    assert_eq!(
        calls[0].arguments["file_path"].as_str(),
        Some("src/main.rs")
    );
    assert!(calls[0].arguments["content"]
        .as_str()
        .unwrap()
        .contains("fn main()"));
}

#[test]
fn test_parse_edit_tool_call() {
    let text = r#"```tool_call
{"tool": "Edit", "arguments": {"file_path": "lib.rs", "old_string": "TODO", "new_string": "DONE", "replace_all": true}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Edit");
    assert_eq!(calls[0].arguments["old_string"].as_str(), Some("TODO"));
    assert_eq!(calls[0].arguments["new_string"].as_str(), Some("DONE"));
    assert_eq!(calls[0].arguments["replace_all"].as_bool(), Some(true));
}

#[test]
fn test_parse_bash_tool_call() {
    let text = r#"Let me run the tests.

```tool_call
{"tool": "Bash", "arguments": {"command": "cargo test --release"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Bash");
    assert_eq!(
        calls[0].arguments["command"].as_str(),
        Some("cargo test --release")
    );
}

#[test]
fn test_parse_cwd_tool_call_no_arguments() {
    let text = r#"```tool_call
{"tool": "Cwd", "arguments": {}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Cwd");
    assert!(calls[0].arguments.is_object());
}

#[test]
fn test_parse_cwd_tool_call_missing_arguments_field() {
    // When "arguments" key is omitted entirely
    let text = r#"```tool_call
{"tool": "Cwd"}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Cwd");
    // Should default to empty object
    assert!(calls[0].arguments.is_object());
}

#[test]
fn test_parse_ls_tool_call() {
    let text = r#"```tool_call
{"tool": "LS", "arguments": {"path": ".", "show_hidden": true}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "LS");
    assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    assert_eq!(calls[0].arguments["show_hidden"].as_bool(), Some(true));
}

#[test]
fn test_parse_glob_tool_call() {
    let text = r#"```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs", "path": "src"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Glob");
    assert_eq!(calls[0].arguments["pattern"].as_str(), Some("**/*.rs"));
}

#[test]
fn test_parse_grep_tool_call() {
    let text = r#"```tool_call
{"tool": "Grep", "arguments": {"pattern": "fn\\s+main", "path": "src", "case_insensitive": false, "context_lines": 2}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Grep");
    assert_eq!(calls[0].arguments["context_lines"].as_u64(), Some(2));
}

// ============================================================================
// Parsing Multiple Tool Calls Tests
// ============================================================================

#[test]
fn test_parse_two_tool_calls() {
    let text = r#"First, let me list the directory.

```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```

And also check the project structure:

```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs"}}
```

I'll analyze the results next."#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].tool_name, "LS");
    assert_eq!(calls[1].tool_name, "Glob");
}

#[test]
fn test_parse_three_tool_calls() {
    let text = r#"```tool_call
{"tool": "Cwd", "arguments": {}}
```

```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```

```tool_call
{"tool": "Read", "arguments": {"file_path": "README.md"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].tool_name, "Cwd");
    assert_eq!(calls[1].tool_name, "LS");
    assert_eq!(calls[2].tool_name, "Read");
}

// ============================================================================
// Parsing Edge Cases Tests
// ============================================================================

#[test]
fn test_parse_no_tool_calls_in_text() {
    let text = "This is a regular text response with no tool calls. Just discussing code.";
    let calls = parse_tool_calls(text);
    assert!(calls.is_empty());
}

#[test]
fn test_parse_regular_code_block_ignored() {
    let text = r#"Here's some code:

```rust
fn main() {
    println!("Hello, world!");
}
```

Nothing else."#;

    let calls = parse_tool_calls(text);
    assert!(calls.is_empty());
}

#[test]
fn test_parse_invalid_json_skipped() {
    let text = r#"```tool_call
{this is not valid json}
```"#;

    let calls = parse_tool_calls(text);
    assert!(
        calls.is_empty(),
        "Invalid JSON should be skipped gracefully"
    );
}

#[test]
fn test_parse_missing_tool_field_skipped() {
    let text = r#"```tool_call
{"arguments": {"path": "."}}
```"#;

    let calls = parse_tool_calls(text);
    assert!(calls.is_empty(), "Missing 'tool' field should be skipped");
}

#[test]
fn test_parse_unclosed_tool_call_block() {
    let text = r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "test.txt"}}
"#;

    let calls = parse_tool_calls(text);
    assert!(
        calls.is_empty(),
        "Unclosed tool_call block should not produce a result"
    );
}

#[test]
fn test_parse_tool_call_with_whitespace() {
    let text = "```tool_call\n  \n  {\"tool\": \"Cwd\", \"arguments\": {}}  \n  \n```";

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Cwd");
}

#[test]
fn test_parse_valid_and_invalid_mixed() {
    let text = r#"```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```

```tool_call
{invalid json}
```

```tool_call
{"tool": "Read", "arguments": {"file_path": "test.txt"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 2, "Should parse 2 valid calls, skip 1 invalid");
    assert_eq!(calls[0].tool_name, "LS");
    assert_eq!(calls[1].tool_name, "Read");
}

// ============================================================================
// Extract Text Without Tool Calls Tests
// ============================================================================

#[test]
fn test_extract_text_removes_tool_call_blocks() {
    let text = r#"Let me read the file.

```tool_call
{"tool": "Read", "arguments": {"file_path": "test.rs"}}
```

I'll analyze the contents."#;

    let clean = extract_text_without_tool_calls(text);
    assert!(clean.contains("Let me read the file."));
    assert!(clean.contains("I'll analyze the contents."));
    assert!(!clean.contains("tool_call"));
    assert!(!clean.contains("file_path"));
}

#[test]
fn test_extract_text_from_multiple_tool_calls() {
    let text = r#"Step 1:

```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```

Step 2:

```tool_call
{"tool": "Read", "arguments": {"file_path": "main.rs"}}
```

Done."#;

    let clean = extract_text_without_tool_calls(text);
    assert!(clean.contains("Step 1:"));
    assert!(clean.contains("Step 2:"));
    assert!(clean.contains("Done."));
    assert!(!clean.contains("tool_call"));
}

#[test]
fn test_extract_text_no_tool_calls() {
    let text = "Just regular text with no tool calls at all.";
    let clean = extract_text_without_tool_calls(text);
    assert_eq!(clean, text);
}

#[test]
fn test_extract_text_only_tool_calls() {
    let text = r#"```tool_call
{"tool": "Cwd", "arguments": {}}
```"#;

    let clean = extract_text_without_tool_calls(text);
    assert!(
        clean.is_empty() || clean.trim().is_empty(),
        "Text with only tool calls should produce empty/blank result, got: '{}'",
        clean
    );
}

// ============================================================================
// Tool Result Formatting Tests
// ============================================================================

#[test]
fn test_format_tool_result_success() {
    let result = format_tool_result("Read", "call_001", "file contents here", false);
    assert!(result.contains("[Tool Result: Read (id: call_001)]"));
    assert!(result.contains("file contents here"));
    assert!(!result.contains("Error:"));
}

#[test]
fn test_format_tool_result_error() {
    let result = format_tool_result("Read", "call_002", "File not found", true);
    assert!(result.contains("[Tool Result: Read (id: call_002)]"));
    assert!(result.contains("Error: File not found"));
}

#[test]
fn test_format_tool_result_for_all_tools() {
    let tool_names = ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "LS", "Cwd"];

    for name in &tool_names {
        let result = format_tool_result(name, "id_1", "output", false);
        assert!(
            result.contains(&format!("Tool Result: {}", name)),
            "Format should include tool name '{}'",
            name
        );
    }
}

// ============================================================================
// ParsedToolCall Structure Tests
// ============================================================================

#[test]
fn test_parsed_tool_call_has_raw_text() {
    let text = r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "test.txt"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);
    assert!(
        !calls[0].raw_text.is_empty(),
        "ParsedToolCall should preserve raw text"
    );
    assert!(calls[0].raw_text.contains("tool_call"));
}

#[test]
fn test_parsed_tool_call_serialization() {
    let text = r#"```tool_call
{"tool": "Bash", "arguments": {"command": "ls -la"}}
```"#;

    let calls = parse_tool_calls(text);
    assert_eq!(calls.len(), 1);

    // Should be serializable
    let json = serde_json::to_string(&calls[0]).unwrap();
    assert!(!json.is_empty());

    // Should be deserializable
    let deserialized: ParsedToolCall = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tool_name, "Bash");
    assert_eq!(deserialized.arguments["command"].as_str(), Some("ls -la"));
}

// ============================================================================
// End-to-End Fallback Flow Simulation Tests
// ============================================================================

#[tokio::test]
async fn test_fallback_parse_and_execute_flow() {
    // Simulate the complete prompt fallback flow:
    // 1. LLM outputs text with tool_call blocks
    // 2. We parse the tool calls
    // 3. We execute them
    // 4. We format results for re-injection

    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.txt"), "Hello World\n").unwrap();
    let executor = ToolExecutor::new(dir.path());

    // Simulate LLM response with a tool call
    let llm_response = format!(
        r#"Let me read the file to understand the code.

```tool_call
{{"tool": "Read", "arguments": {{"file_path": "{}"}}}}
```

I'll analyze the results."#,
        dir.path()
            .join("test.txt")
            .to_string_lossy()
            .replace('\\', "\\\\")
    );

    // Step 1: Parse tool calls
    let calls = parse_tool_calls(&llm_response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "Read");

    // Step 2: Execute the tool
    let result = executor
        .execute(&calls[0].tool_name, &calls[0].arguments)
        .await;
    assert!(result.success);
    assert!(result.output.as_ref().unwrap().contains("Hello World"));

    // Step 3: Format result for re-injection
    let formatted = format_tool_result(
        &calls[0].tool_name,
        "fallback_1",
        &result.to_content(),
        !result.success,
    );
    assert!(formatted.contains("Tool Result: Read"));
    assert!(formatted.contains("Hello World"));

    // Step 4: Extract clean text
    let clean_text = extract_text_without_tool_calls(&llm_response);
    assert!(clean_text.contains("Let me read the file"));
    assert!(clean_text.contains("I'll analyze the results."));
    assert!(!clean_text.contains("tool_call"));
}

#[tokio::test]
async fn test_fallback_multiple_tools_flow() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("readme.md"), "# My Project\n").unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src").join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(dir.path());

    // Simulate LLM outputting multiple tool calls
    let llm_response = format!(
        r#"Let me explore the project.

```tool_call
{{"tool": "LS", "arguments": {{"path": "{path}"}}}}
```

```tool_call
{{"tool": "Glob", "arguments": {{"pattern": "**/*.rs", "path": "{path}"}}}}
```

I'll look at the results."#,
        path = dir.path().to_string_lossy().replace('\\', "\\\\")
    );

    // Parse
    let calls = parse_tool_calls(&llm_response);
    assert_eq!(calls.len(), 2);

    // Execute all tool calls and collect formatted results
    let mut tool_results = Vec::new();
    for (i, call) in calls.iter().enumerate() {
        let result = executor.execute(&call.tool_name, &call.arguments).await;
        assert!(
            result.success,
            "Tool '{}' should succeed: {:?}",
            call.tool_name, result.error
        );

        let formatted = format_tool_result(
            &call.tool_name,
            &format!("fallback_{}", i + 1),
            &result.to_content(),
            !result.success,
        );
        tool_results.push(formatted);
    }

    // Combine results as they would be injected into conversation
    let combined = tool_results.join("\n\n");
    assert!(combined.contains("Tool Result: LS"));
    assert!(combined.contains("Tool Result: Glob"));
    assert!(combined.contains("src"));
}

#[tokio::test]
async fn test_fallback_write_then_verify_flow() {
    let dir = tempfile::TempDir::new().unwrap();
    let executor = ToolExecutor::new(dir.path());

    let file_path = dir
        .path()
        .join("created.txt")
        .to_string_lossy()
        .replace('\\', "\\\\");

    // Simulate LLM writing a file
    let write_response = format!(
        r#"I'll create the file now.

```tool_call
{{"tool": "Write", "arguments": {{"file_path": "{}", "content": "Created by fallback test.\n"}}}}
```"#,
        file_path
    );

    let calls = parse_tool_calls(&write_response);
    assert_eq!(calls.len(), 1);

    let result = executor
        .execute(&calls[0].tool_name, &calls[0].arguments)
        .await;
    assert!(result.success, "Write failed: {:?}", result.error);

    // Simulate LLM then reading it back
    let read_response = format!(
        r#"```tool_call
{{"tool": "Read", "arguments": {{"file_path": "{}"}}}}
```"#,
        file_path
    );

    let read_calls = parse_tool_calls(&read_response);
    let read_result = executor
        .execute(&read_calls[0].tool_name, &read_calls[0].arguments)
        .await;
    assert!(read_result.success);
    assert!(read_result
        .output
        .unwrap()
        .contains("Created by fallback test."));
}

// ============================================================================
// Instructions Integration with Tool Definitions Tests
// ============================================================================

#[test]
fn test_instructions_describe_all_parameters() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    // Verify key parameters are documented in the instructions
    assert!(instructions.contains("file_path"));
    assert!(instructions.contains("command"));
    assert!(instructions.contains("pattern"));
    assert!(instructions.contains("old_string"));
    assert!(instructions.contains("new_string"));
    assert!(instructions.contains("content"));
    assert!(instructions.contains("show_hidden"));
}

#[test]
fn test_instructions_mark_required_vs_optional() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    // Should contain both markers
    let required_count = instructions.matches("(required)").count();
    let optional_count = instructions.matches("(optional)").count();

    assert!(required_count > 0, "Should have required parameters");
    assert!(optional_count > 0, "Should have optional parameters");
}

#[test]
fn test_instructions_include_parameter_types() {
    let tools = get_tool_definitions_from_registry();
    let instructions = build_tool_call_instructions(&tools);

    // Check parameter types are present
    assert!(instructions.contains("string"));
    assert!(instructions.contains("boolean"));
    assert!(instructions.contains("integer"));
}
