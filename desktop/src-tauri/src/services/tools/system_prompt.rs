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

## How to Choose the Right Tool — Decision Tree

Follow this decision tree to select the correct tool. Start from the top.

### Step 1: Does the user's message need a tool at all?

- Greeting, chitchat, or general knowledge question → **Respond directly. No tool needed.**
  - Examples: "hello", "what is Rust?", "explain async/await"
- Question about the project, files, or code → Go to Step 2.
- Request to modify code or run commands → Go to Step 3.

### Step 2: The user asks about the project or code — which tool?

**Simple, single-answer questions** → Use one basic tool:

| User wants to know... | Use this tool | Do NOT use |
|---|---|---|
| "What directory is this?" / "Where am I?" | **Cwd** | ~~Analyze~~ ~~Task~~ |
| "List files in src/" / "What's in this folder?" | **LS** | ~~Analyze~~ ~~Task~~ |
| "Find all .rs files" / "Where is config.toml?" | **Glob** | ~~Analyze~~ ~~Task~~ |
| "Find where function X is defined" / "Search for error handling" | **Grep** | ~~Analyze~~ ~~Task~~ |
| "Show me the contents of main.rs" | **Read** | ~~Analyze~~ ~~Task~~ |

**Project-level understanding** (requires reading multiple files) → Use **Task** sub-agent:

| User wants to know... | Use this tool | Why |
|---|---|---|
| "What does this project do?" / "Analyze this project" | **Task** with task_type='explore' | Sub-agent reads README, manifests, key source files, and synthesizes a comprehensive answer |
| "Explain the architecture" / "How is this codebase structured?" | **Task** with task_type='explore' | Sub-agent explores directory structure, reads key modules, and maps the architecture |
| "Analyze module X in depth" / "How do components A and B interact?" | **Task** with task_type='analyze' | Sub-agent does focused deep reading across multiple related files |

**Aggregated file inventory for implementation** → Use **Analyze**:

| User wants to know... | Use this tool |
|---|---|
| Quick project context before making cross-module changes | **Analyze** (quick mode, default) |
| Comprehensive structural analysis with coverage gates | **Analyze** with mode='deep' (only when explicitly requested) |

**Key rule**: For simple single-answer questions, use one basic tool. For project understanding that requires reading many files, use Task. For aggregated file inventory before code changes, use Analyze.

### Step 3: The user wants to modify code or run commands

- **Edit existing file**: Use Read first to see current contents, then Edit with exact string replacement.
- **Create new file**: Use Write.
- **Run tests, build, git, or shell commands**: Use Bash.
- **Edit Jupyter notebook cells**: Use NotebookEdit.
- **Complex multi-file implementation**: Use **Task** with task_type='implement' to delegate to a sub-agent with fresh context.

### Step 4: Web resources

- **Fetch a specific URL**: Use WebFetch.
- **Search for current information**: Use WebSearch.

## When to Use Task (Sub-Agent)

**Use Task** when the request requires reading and synthesizing information from multiple files. The sub-agent gets its own context window and can read many files without exhausting your main context.

- task_type='explore': For codebase exploration, project understanding, architecture questions
- task_type='analyze': For deep analysis of specific modules or cross-component interactions
- task_type='implement': For focused code changes that benefit from a fresh context

**Examples of when to use Task:**
- "What does this project do?" → Task(explore): reads README, config files, key source files
- "Analyze this project" → Task(explore): explores directory tree, reads multiple modules
- "How does the auth system work?" → Task(analyze): reads auth-related files across modules
- "Implement feature X in module Y" → Task(implement): reads context, makes changes

## When to Use Analyze (and When NOT To)

**Use Analyze** (defaults to quick mode) when you need a structured file inventory brief before making cross-module code changes — it returns relevant files, components, and test coverage in a compact format.

**Use Analyze with mode='deep'** only for comprehensive structural analysis with coverage gates and multi-phase pipeline.

**Do NOT use Analyze when:**
- The user asks "what directory is this?" → Use **Cwd**
- The user asks "list files" → Use **LS**
- The user asks "find X" → Use **Glob** or **Grep**
- The user asks "read this file" → Use **Read**
- The user asks "what does this project do?" or "analyze this project" → Use **Task**(explore)
- The user asks a general knowledge question → Respond directly
- The user greets you → Respond directly

## General Guidelines

- **Read before modifying**: Always Read a file before using Edit or Write on it.
- **Prefer Edit over Write** for existing files: use exact string replacement instead of rewriting.
- **Relative paths** resolve against the working directory shown above.
- **Be token-efficient**: Prefer targeted reads (specific line ranges) over reading entire large files. Use Grep to locate relevant sections first.
- **Rich format support**: Read handles PDF, DOCX, XLSX, Jupyter notebooks, and images.

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
        assert!(prompt.contains("**Analyze**"));
        assert!(prompt.contains("**WebFetch**"));
        assert!(prompt.contains("**WebSearch**"));
        assert!(prompt.contains("**NotebookEdit**"));
    }

    #[test]
    fn test_build_system_prompt_contains_decision_tree() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools);

        assert!(prompt.contains("Decision Tree"));
        assert!(prompt.contains("Do NOT use Analyze when"));
        assert!(prompt.contains("Respond directly. No tool needed."));
        assert!(prompt.contains("Read before modifying"));
    }

    #[test]
    fn test_build_system_prompt_no_workflow_pattern() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools);

        // The old "Workflow Pattern" section should be gone
        assert!(!prompt.contains("Workflow Pattern"));
        assert!(!prompt.contains("For typical code tasks, follow this pattern"));
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
