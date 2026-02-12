//! System Prompt Builder
//!
//! Builds comprehensive system prompts that instruct LLMs to use tools effectively
//! for agentic code analysis and modification.

use std::path::Path;

use crate::services::llm::types::ToolDefinition;
use crate::services::orchestrator::index_store::ProjectIndexSummary;

/// Build a deterministic project summary string from an index summary.
///
/// The output is fully deterministic: all lists are sorted alphabetically so that
/// identical input always produces identical output. This is important for Ollama
/// KV-cache stability.
pub fn build_project_summary(summary: &ProjectIndexSummary) -> String {
    let mut lines = Vec::new();

    lines.push("## Project Structure".to_string());
    lines.push(format!("Total files: {}", summary.total_files));

    // Languages - sorted for determinism
    if !summary.languages.is_empty() {
        let mut langs = summary.languages.clone();
        langs.sort();
        lines.push(format!("Languages: {}", langs.join(", ")));
    }

    // Components - sorted alphabetically by name for determinism
    if !summary.components.is_empty() {
        lines.push(String::new());
        lines.push("### Components".to_string());
        let mut components = summary.components.clone();
        components.sort_by(|a, b| a.name.cmp(&b.name));
        for comp in &components {
            lines.push(format!("- {}: {} files", comp.name, comp.count));
        }
    }

    // Key entry points - sorted for determinism
    if !summary.key_entry_points.is_empty() {
        lines.push(String::new());
        lines.push("### Key Entry Points".to_string());
        let mut entry_points = summary.key_entry_points.clone();
        entry_points.sort();
        for ep in &entry_points {
            lines.push(format!("- {}", ep));
        }
    }

    lines.join("\n")
}

/// Build a comprehensive system prompt for agentic tool usage.
///
/// This prompt instructs the LLM to use the available tools proactively for code tasks.
/// It includes the current working directory and guidance on each tool.
///
/// When `project_summary` is provided, the project structure summary is inserted
/// between the working directory section and the available tools section.
///
/// The returned prompt should be prepended to any user-provided system prompt.
pub fn build_system_prompt(
    project_root: &Path,
    tools: &[ToolDefinition],
    project_summary: Option<&ProjectIndexSummary>,
) -> String {
    let tool_list = tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    let summary_section = match project_summary {
        Some(summary) if summary.total_files > 0 => {
            format!("\n\n{}", build_project_summary(summary))
        }
        _ => String::new(),
    };

    format!(
        r#"You are an AI coding assistant with access to tools for reading, writing, and analyzing code. You operate in the project directory shown below.

## Working Directory
{project_root}{summary_section}

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
| "Find where function X is defined" / "What files are in component Y?" | **CodebaseSearch** (preferred) or **Grep** | ~~Analyze~~ ~~Task~~ |
| "Search for error handling" / "Find string in files" | **Grep** | ~~Analyze~~ ~~Task~~ |
| "Show me the contents of main.rs" | **Read** | ~~Analyze~~ ~~Task~~ |

> **Tip — CodebaseSearch vs Grep**: Use **CodebaseSearch** first when exploring the codebase (finding symbols, locating files by component, understanding project structure). It queries the pre-built index and is faster than scanning files. Use **Grep** when you need full-text content search with regex, or when CodebaseSearch reports the index is unavailable.

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

## Critical Rules / 关键规则

- **NEVER fabricate or predict tool results.** You MUST wait for actual tool execution results before continuing. Do NOT write text like "调用成功" or "returns..." to simulate tool output. Only use REAL results from executed tools.
- **绝对不要伪造或预测工具结果。** 必须等待实际的工具执行结果后再继续。不要写"调用成功"、"返回了..."等模拟输出。只使用工具执行后提供的真实结果。
- **Do NOT describe what a tool call will return.** Simply make the tool call and wait for the result.
- **不要描述工具调用的预期返回。** 直接调用工具并等待结果。
- **If a tool call fails**, read the error message carefully and retry with corrected parameters.
- **如果工具调用失败，** 仔细阅读错误信息，修正参数后重试。"#,
        project_root = project_root.display(),
        summary_section = summary_section,
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
    use crate::services::orchestrator::index_store::ComponentSummary;
    use crate::services::tools::definitions::get_tool_definitions;
    use std::path::PathBuf;

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools, None);

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
        assert!(prompt.contains("**CodebaseSearch**"));
    }

    #[test]
    fn test_build_system_prompt_contains_decision_tree() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None);

        assert!(prompt.contains("Decision Tree"));
        assert!(prompt.contains("Do NOT use Analyze when"));
        assert!(prompt.contains("Respond directly. No tool needed."));
        assert!(prompt.contains("Read before modifying"));
    }

    #[test]
    fn test_build_system_prompt_no_workflow_pattern() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None);

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

    // =========================================================================
    // Story-008: Project summary injection tests
    // =========================================================================

    fn make_test_summary() -> ProjectIndexSummary {
        ProjectIndexSummary {
            total_files: 42,
            languages: vec![
                "typescript".to_string(),
                "rust".to_string(),
                "python".to_string(),
            ],
            components: vec![
                ComponentSummary {
                    name: "desktop-web".to_string(),
                    count: 20,
                },
                ComponentSummary {
                    name: "desktop-rust".to_string(),
                    count: 15,
                },
                ComponentSummary {
                    name: "api-server".to_string(),
                    count: 7,
                },
            ],
            key_entry_points: vec![
                "src/main.rs".to_string(),
                "src/app.tsx".to_string(),
                "src/index.ts".to_string(),
            ],
        }
    }

    #[test]
    fn test_build_project_summary_format() {
        let summary = make_test_summary();
        let text = build_project_summary(&summary);

        // Verify expected sections exist
        assert!(text.contains("## Project Structure"));
        assert!(text.contains("Total files: 42"));
        assert!(text.contains("### Components"));
        assert!(text.contains("### Key Entry Points"));

        // Verify languages are present
        assert!(text.contains("Languages:"));
        assert!(text.contains("rust"));
        assert!(text.contains("typescript"));
        assert!(text.contains("python"));

        // Verify components are present with counts
        assert!(text.contains("desktop-rust: 15 files"));
        assert!(text.contains("desktop-web: 20 files"));
        assert!(text.contains("api-server: 7 files"));

        // Verify entry points are present
        assert!(text.contains("- src/main.rs"));
        assert!(text.contains("- src/app.tsx"));
        assert!(text.contains("- src/index.ts"));
    }

    #[test]
    fn test_build_project_summary_deterministic() {
        let summary = make_test_summary();

        let text1 = build_project_summary(&summary);
        let text2 = build_project_summary(&summary);

        // Same input MUST produce identical output
        assert_eq!(text1, text2, "Project summary must be deterministic");

        // Also verify with a differently-ordered input
        let summary_reordered = ProjectIndexSummary {
            total_files: 42,
            // Languages in different order
            languages: vec![
                "python".to_string(),
                "rust".to_string(),
                "typescript".to_string(),
            ],
            // Components in different order
            components: vec![
                ComponentSummary {
                    name: "api-server".to_string(),
                    count: 7,
                },
                ComponentSummary {
                    name: "desktop-web".to_string(),
                    count: 20,
                },
                ComponentSummary {
                    name: "desktop-rust".to_string(),
                    count: 15,
                },
            ],
            // Entry points in different order
            key_entry_points: vec![
                "src/index.ts".to_string(),
                "src/main.rs".to_string(),
                "src/app.tsx".to_string(),
            ],
        };

        let text_reordered = build_project_summary(&summary_reordered);
        assert_eq!(
            text1, text_reordered,
            "Same data in different order must produce identical output"
        );
    }

    #[test]
    fn test_build_project_summary_empty() {
        let summary = ProjectIndexSummary::default();
        let text = build_project_summary(&summary);

        assert!(text.contains("## Project Structure"));
        assert!(text.contains("Total files: 0"));
        // Should not contain component or entry point sections when empty
        assert!(!text.contains("### Components"));
        assert!(!text.contains("### Key Entry Points"));
    }

    #[test]
    fn test_build_system_prompt_with_summary() {
        let tools = get_tool_definitions();
        let summary = make_test_summary();
        let prompt =
            build_system_prompt(&PathBuf::from("/test/project"), &tools, Some(&summary));

        // Summary should be present
        assert!(prompt.contains("## Project Structure"));
        assert!(prompt.contains("Total files: 42"));
        assert!(prompt.contains("### Components"));

        // Summary should appear between Working Directory and Available Tools
        let wd_pos = prompt.find("## Working Directory").unwrap();
        let summary_pos = prompt.find("## Project Structure").unwrap();
        let tools_pos = prompt.find("## Available Tools").unwrap();

        assert!(
            wd_pos < summary_pos,
            "Summary must appear after Working Directory"
        );
        assert!(
            summary_pos < tools_pos,
            "Summary must appear before Available Tools"
        );
    }

    #[test]
    fn test_build_system_prompt_without_summary() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools, None);

        // Should not crash and should not contain summary section
        assert!(!prompt.contains("## Project Structure"));
        assert!(!prompt.contains("### Components"));
        assert!(!prompt.contains("### Key Entry Points"));

        // Should still contain normal sections
        assert!(prompt.contains("## Working Directory"));
        assert!(prompt.contains("## Available Tools"));
    }

    #[test]
    fn test_system_prompt_recommends_codebase_search_for_exploration() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None);

        assert!(
            prompt.contains("CodebaseSearch"),
            "System prompt should mention CodebaseSearch"
        );
        assert!(
            prompt.contains("CodebaseSearch vs Grep"),
            "System prompt should explain when to prefer CodebaseSearch over Grep"
        );
        assert!(
            prompt.contains("pre-built index"),
            "System prompt should mention the pre-built index advantage"
        );
    }

    #[test]
    fn test_build_system_prompt_with_empty_summary_no_injection() {
        let tools = get_tool_definitions();
        let empty_summary = ProjectIndexSummary::default();
        let prompt =
            build_system_prompt(&PathBuf::from("/test/project"), &tools, Some(&empty_summary));

        // Empty summary (0 files) should not inject a section
        assert!(!prompt.contains("## Project Structure"));
    }
}
