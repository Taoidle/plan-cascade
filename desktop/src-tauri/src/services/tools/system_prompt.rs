//! System Prompt Builder
//!
//! Builds comprehensive system prompts that instruct LLMs to use tools effectively
//! for agentic code analysis and modification.

use std::path::Path;

use crate::services::llm::types::ToolDefinition;

/// Build a comprehensive system prompt for agentic tool usage.
///
/// This prompt instructs the LLM to use the available tools proactively for code tasks.
/// It includes the current working directory and guidance on each tool.
///
/// The returned prompt should be prepended to any user-provided system prompt.
pub fn build_system_prompt(project_root: &Path, tools: &[ToolDefinition]) -> String {
    let tool_list = tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are an AI coding assistant with access to tools for reading, writing, and analyzing code. You operate in the project directory shown below.

## Working Directory
{project_root}

## Available Tools
{tool_list}

## Tool Usage Guidelines

1. **Always read before modifying**: Before editing or writing a file, use the Read tool to understand its current contents.
2. **Use LS to explore directories**: When you need to understand a project's structure, use LS to list directory contents before diving into files.
3. **Use Glob to find files**: When looking for specific files by pattern (e.g., all `.rs` files), use Glob instead of guessing paths.
4. **Use Grep to search code**: When looking for specific code patterns, function definitions, or references, use Grep with regex patterns.
5. **Use Cwd when unsure**: If you need to confirm the current working directory, use the Cwd tool.
6. **Prefer Edit over Write for existing files**: When modifying an existing file, use Edit with the exact string to replace rather than rewriting the entire file with Write.
7. **Use Bash for system commands**: For running tests, builds, git operations, or other system commands, use Bash.
8. **Relative paths resolve against the working directory**: You can use relative paths with Read, Write, Edit, Glob, and Grep — they resolve against the working directory shown above.
9. **Always use Task for multi-file exploration and analysis**: When a task requires reading more than 2-3 files, exploring directory structures, or analyzing code across multiple modules, you MUST delegate to a Task sub-agent. The sub-agent has its own context window and only returns a concise summary. This prevents your main context from filling up with raw file contents, which would cause execution to fail. Never read 5+ files directly — spawn a Task instead.
10. **Use WebFetch to read web pages**: Fetch documentation, API references, and other web content. HTML is automatically converted to markdown. Private/local URLs are blocked for security.
11. **Use WebSearch for current information**: Search the web for up-to-date information, documentation, and solutions. Results include titles, URLs, and snippets.
12. **Rich file format support**: Read can handle PDF, DOCX, XLSX, Jupyter notebooks (.ipynb), and images (returns metadata). Use the `pages` parameter for PDFs to read specific page ranges.
13. **Use NotebookEdit for Jupyter notebooks**: Edit .ipynb cells (replace, insert, delete) while preserving notebook structure and untouched cell outputs.
14. **Be context-aware to avoid token budget exhaustion**: Prefer targeted reads (specific line ranges) over reading entire large files. Use Grep to find relevant code sections before reading full files. When exploring unfamiliar codebases, always delegate bulk exploration to a Task sub-agent rather than reading files directly. Your context is limited — treat it as a precious resource.

## Workflow Pattern

For typical code tasks, follow this pattern:
1. **Explore**: Use LS and Glob to understand the project structure
2. **Read**: Use Read and Grep to understand existing code
3. **Plan**: Determine what changes are needed
4. **Implement**: Use Edit or Write to make changes
5. **Verify**: Use Read to confirm changes, Bash to run tests

When you need to use a tool, make a tool call. You can use multiple tools in sequence to accomplish complex tasks.

## Critical Rules

- **NEVER fabricate or predict tool results.** You MUST wait for actual tool execution results before continuing. Do NOT write text like "调用成功" or "returns..." to simulate tool output. Only use REAL results from executed tools.
- **Do NOT describe what a tool call will return.** Simply make the tool call and wait for the result.
- **If a tool call fails**, read the error message carefully and retry with corrected parameters."#,
        project_root = project_root.display(),
        tool_list = tool_list,
    )
}

/// Merge the tool system prompt with a user-provided system prompt.
///
/// The tool prompt is placed first, followed by the user prompt separated by a delimiter.
pub fn merge_system_prompts(tool_prompt: &str, user_prompt: Option<&str>) -> String {
    match user_prompt {
        Some(user) if !user.is_empty() => {
            format!("{}\n\n---\n\n{}", tool_prompt, user)
        }
        _ => tool_prompt.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tools::definitions::get_tool_definitions;
    use std::path::PathBuf;

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools);

        // Should contain working directory
        assert!(prompt.contains("/test/project"));

        // Should contain all tool names
        assert!(prompt.contains("**Read**"));
        assert!(prompt.contains("**Write**"));
        assert!(prompt.contains("**Edit**"));
        assert!(prompt.contains("**Bash**"));
        assert!(prompt.contains("**Glob**"));
        assert!(prompt.contains("**Grep**"));
        assert!(prompt.contains("**LS**"));
        assert!(prompt.contains("**Cwd**"));
        assert!(prompt.contains("**WebFetch**"));
        assert!(prompt.contains("**WebSearch**"));
        assert!(prompt.contains("**NotebookEdit**"));
    }

    #[test]
    fn test_build_system_prompt_contains_guidelines() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools);

        assert!(prompt.contains("Tool Usage Guidelines"));
        assert!(prompt.contains("Always read before modifying"));
        assert!(prompt.contains("Workflow Pattern"));
    }

    #[test]
    fn test_merge_system_prompts_with_user() {
        let tool_prompt = "Tool instructions here.";
        let user_prompt = Some("Custom instructions.");

        let merged = merge_system_prompts(tool_prompt, user_prompt);

        assert!(merged.starts_with("Tool instructions here."));
        assert!(merged.contains("---"));
        assert!(merged.ends_with("Custom instructions."));
    }

    #[test]
    fn test_merge_system_prompts_without_user() {
        let tool_prompt = "Tool instructions here.";

        let merged = merge_system_prompts(tool_prompt, None);
        assert_eq!(merged, "Tool instructions here.");

        let merged = merge_system_prompts(tool_prompt, Some(""));
        assert_eq!(merged, "Tool instructions here.");
    }
}
