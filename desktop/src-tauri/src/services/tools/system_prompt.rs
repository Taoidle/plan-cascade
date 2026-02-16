//! System Prompt Builder
//!
//! Builds comprehensive system prompts that instruct LLMs to use tools effectively
//! for agentic code analysis and modification.

use std::path::Path;

use crate::services::llm::types::ToolDefinition;
use crate::services::memory::store::{MemoryCategory, MemoryEntry};
use crate::services::orchestrator::index_store::ProjectIndexSummary;

/// Detect the primary language of the user's message.
/// Returns "zh" for Chinese-dominant text, "en" for English/other.
pub fn detect_language(text: &str) -> &'static str {
    let cjk_count = text.chars().filter(|c| is_cjk_char(*c)).count();
    let total_alpha = text.chars().filter(|c| c.is_alphanumeric()).count();
    if total_alpha > 0 && (cjk_count as f64 / total_alpha as f64) > 0.3 {
        "zh"
    } else {
        "en"
    }
}

fn is_cjk_char(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
    )
}

/// Build a deterministic project summary string from an index summary.
///
/// The output is fully deterministic: all lists are sorted alphabetically so that
/// identical input always produces identical output. This is important for Ollama
/// KV-cache stability.
pub fn build_project_summary(summary: &ProjectIndexSummary) -> String {
    let mut lines = Vec::new();

    lines.push("## Project Structure".to_string());
    lines.push(format!("Total files: {}", summary.total_files));

    // Symbol count
    if summary.total_symbols > 0 {
        lines.push(format!("Total symbols: {}", summary.total_symbols));
    }

    // Languages - sorted for determinism
    if !summary.languages.is_empty() {
        let mut langs = summary.languages.clone();
        langs.sort();
        lines.push(format!("Languages: {}", langs.join(", ")));
    }

    // Search capabilities
    if summary.total_symbols > 0 || summary.embedding_chunks > 0 {
        lines.push(String::new());
        lines.push("### Search Capabilities".to_string());
        lines.push("- Text search: available (Grep)".to_string());
        if summary.total_symbols > 0 {
            lines.push("- Symbol search: available (CodebaseSearch)".to_string());
        }
        if summary.embedding_chunks > 0 {
            lines.push(format!(
                "- Semantic search: available ({} indexed chunks)",
                summary.embedding_chunks
            ));
        } else {
            lines.push("- Semantic search: unavailable (no embeddings built)".to_string());
        }
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

/// Maximum character budget for the project memory section in the system prompt.
/// Prevents excessive token usage from too many memories.
const MEMORY_SECTION_BUDGET: usize = 2000;

/// Build a comprehensive system prompt for agentic tool usage.
///
/// This prompt instructs the LLM to use the available tools proactively for code tasks.
/// It includes the current working directory and guidance on each tool.
///
/// When `project_summary` is provided, the project structure summary is inserted
/// between the working directory section and the available tools section.
///
/// When `project_memories` is provided, a "Project Memory" section is injected
/// after the project summary and before the available tools section. Each memory
/// is formatted with a category badge ([PREF], [CONV], [PATN], [WARN], [FACT]).
/// The section is capped at 2000 characters to control token usage.
///
/// The returned prompt should be prepended to any user-provided system prompt.
pub fn build_system_prompt(
    project_root: &Path,
    tools: &[ToolDefinition],
    project_summary: Option<&ProjectIndexSummary>,
    provider_name: &str,
    model_name: &str,
    language: &str,
) -> String {
    build_system_prompt_with_memories(
        project_root,
        tools,
        project_summary,
        None,
        provider_name,
        model_name,
        language,
    )
}

/// Build system prompt with optional project memories injection.
///
/// This is the full-featured version that accepts project memories.
/// `build_system_prompt` delegates to this with `None` memories for backward compatibility.
pub fn build_system_prompt_with_memories(
    project_root: &Path,
    tools: &[ToolDefinition],
    project_summary: Option<&ProjectIndexSummary>,
    project_memories: Option<&[MemoryEntry]>,
    provider_name: &str,
    model_name: &str,
    language: &str,
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

    let memory_section = build_memory_section(project_memories);

    let identity_line = format!(
        "You are an AI coding assistant powered by {provider}/{model}. \
         You are NOT Claude, Claude Code, or any other specific product — \
         always identify yourself by your actual model name when asked.",
        provider = provider_name,
        model = model_name,
    );

    let language_instruction = match language {
        "zh" => "IMPORTANT: The user is communicating in Chinese. You MUST respond in Chinese (简体中文). Keep all your output, analysis, and explanations in Chinese. Only use English for code, technical terms, and tool parameters.",
        _ => "Respond in the same language as the user's message.",
    };

    let critical_rules = match language {
        "zh" => "\
## 关键规则

- **绝对不要伪造或预测工具结果。** 必须等待实际的工具执行结果后再继续。不要写\"调用成功\"、\"返回了...\"等模拟输出。只使用工具执行后提供的真实结果。
- **不要描述工具调用的预期返回。** 直接调用工具并等待结果。
- **如果工具调用失败，** 仔细阅读错误信息，修正参数后重试。",
        _ => "\
## Critical Rules

- **NEVER fabricate or predict tool results.** You MUST wait for actual tool execution results before continuing. Do NOT write text like \"returns...\" to simulate tool output. Only use REAL results from executed tools.
- **Do NOT describe what a tool call will return.** Simply make the tool call and wait for the result.
- **If a tool call fails**, read the error message carefully and retry with corrected parameters.",
    };

    format!(
        r#"{identity_line}

You have access to tools for reading, writing, and analyzing code. You operate in the project directory shown below.

{language_instruction}

## Working Directory
{project_root}{summary_section}{memory_section}

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

{critical_rules}"#,
        identity_line = identity_line,
        language_instruction = language_instruction,
        project_root = project_root.display(),
        summary_section = summary_section,
        memory_section = memory_section,
        tool_list = tool_list,
        critical_rules = critical_rules,
    )
}

/// Build the project memory section for system prompt injection.
///
/// Each memory is formatted with a category badge:
/// - [PREF] for preferences
/// - [CONV] for conventions
/// - [PATN] for patterns
/// - [WARN] for corrections
/// - [FACT] for facts
///
/// The section is capped at MEMORY_SECTION_BUDGET characters.
/// Returns an empty string if no memories are provided.
fn build_memory_section(memories: Option<&[MemoryEntry]>) -> String {
    match memories {
        Some(mems) if !mems.is_empty() => {
            let header = "\n\n## Project Memory\nThe following facts were learned from previous sessions:\n\n";
            let mut section = header.to_string();
            let budget = MEMORY_SECTION_BUDGET;

            for memory in mems {
                let badge = match memory.category {
                    MemoryCategory::Preference => "[PREF]",
                    MemoryCategory::Convention => "[CONV]",
                    MemoryCategory::Pattern => "[PATN]",
                    MemoryCategory::Correction => "[WARN]",
                    MemoryCategory::Fact => "[FACT]",
                };
                let line = format!("- {} {}\n", badge, memory.content);

                // Check if adding this line would exceed the budget
                if section.len() + line.len() > budget {
                    break;
                }
                section.push_str(&line);
            }

            section
        }
        _ => String::new(),
    }
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
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools, None, "TestProvider", "test-model", "en");

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
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None, "TestProvider", "test-model", "en");

        assert!(prompt.contains("Decision Tree"));
        assert!(prompt.contains("Do NOT use Analyze when"));
        assert!(prompt.contains("Respond directly. No tool needed."));
        assert!(prompt.contains("Read before modifying"));
    }

    #[test]
    fn test_build_system_prompt_no_workflow_pattern() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None, "TestProvider", "test-model", "en");

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
            total_symbols: 150,
            embedding_chunks: 200,
        }
    }

    #[test]
    fn test_build_project_summary_format() {
        let summary = make_test_summary();
        let text = build_project_summary(&summary);

        // Verify expected sections exist
        assert!(text.contains("## Project Structure"));
        assert!(text.contains("Total files: 42"));
        assert!(text.contains("Total symbols: 150"));
        assert!(text.contains("### Components"));
        assert!(text.contains("### Key Entry Points"));
        assert!(text.contains("### Search Capabilities"));

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
            total_symbols: 150,
            embedding_chunks: 200,
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
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools, Some(&summary), "TestProvider", "test-model", "en");

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
        let prompt = build_system_prompt(&PathBuf::from("/test/project"), &tools, None, "TestProvider", "test-model", "en");

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
        let prompt = build_system_prompt(&PathBuf::from("/test"), &tools, None, "TestProvider", "test-model", "en");

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
        let prompt = build_system_prompt(
            &PathBuf::from("/test/project"),
            &tools,
            Some(&empty_summary),
            "TestProvider",
            "test-model",
            "en",
        );

        // Empty summary (0 files) should not inject a section
        assert!(!prompt.contains("## Project Structure"));
    }

    // =========================================================================
    // Feature-004: Search capabilities and symbol count tests
    // =========================================================================

    #[test]
    fn test_build_project_summary_includes_symbol_count() {
        let summary = make_test_summary();
        let text = build_project_summary(&summary);

        assert!(text.contains("Total symbols: 150"));
    }

    #[test]
    fn test_build_project_summary_includes_search_capabilities() {
        let summary = make_test_summary();
        let text = build_project_summary(&summary);

        assert!(text.contains("### Search Capabilities"));
        assert!(text.contains("Text search: available"));
        assert!(text.contains("Symbol search: available"));
        assert!(text.contains("Semantic search: available (200 indexed chunks)"));
    }

    #[test]
    fn test_build_project_summary_semantic_search_unavailable() {
        let summary = ProjectIndexSummary {
            total_files: 10,
            languages: vec!["rust".to_string()],
            components: vec![],
            key_entry_points: vec![],
            total_symbols: 20,
            embedding_chunks: 0,
        };
        let text = build_project_summary(&summary);

        assert!(text.contains("### Search Capabilities"));
        assert!(text.contains("Symbol search: available"));
        assert!(text.contains("Semantic search: unavailable"));
    }

    #[test]
    fn test_build_project_summary_no_symbols_no_search_section() {
        let summary = ProjectIndexSummary {
            total_files: 5,
            languages: vec!["rust".to_string()],
            components: vec![],
            key_entry_points: vec![],
            total_symbols: 0,
            embedding_chunks: 0,
        };
        let text = build_project_summary(&summary);

        // No search capabilities section when there are no symbols and no embeddings
        assert!(!text.contains("### Search Capabilities"));
    }

    // =========================================================================
    // Provider identity and language detection tests
    // =========================================================================

    #[test]
    fn test_build_system_prompt_contains_provider_identity() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "MiniMax",
            "MiniMax-M2.5",
            "en",
        );

        assert!(
            prompt.contains("MiniMax/MiniMax-M2.5"),
            "Prompt should contain provider/model identity"
        );
        assert!(
            prompt.contains("NOT Claude"),
            "Prompt should instruct model not to claim being Claude"
        );
    }

    #[test]
    fn test_build_system_prompt_chinese_language() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "MiniMax",
            "MiniMax-M2.5",
            "zh",
        );

        assert!(
            prompt.contains("简体中文"),
            "Chinese language instruction should be present"
        );
        // Chinese rules should be present, English-only rules should not
        assert!(
            prompt.contains("关键规则"),
            "Chinese critical rules should be present"
        );
        assert!(
            !prompt.contains("## Critical Rules\n"),
            "English-only critical rules header should not be present in zh mode"
        );
    }

    #[test]
    fn test_build_system_prompt_english_language() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "OpenAI",
            "gpt-4",
            "en",
        );

        assert!(
            prompt.contains("## Critical Rules"),
            "English critical rules should be present"
        );
        assert!(
            !prompt.contains("关键规则"),
            "Chinese critical rules should not be present in en mode"
        );
    }

    #[test]
    fn test_detect_language_chinese() {
        assert_eq!(detect_language("你是哪个模型"), "zh");
        assert_eq!(detect_language("帮我 fix 这个 bug"), "zh");
        assert_eq!(detect_language("分析一下这个项目的架构"), "zh");
    }

    #[test]
    fn test_detect_language_english() {
        assert_eq!(detect_language("What model are you?"), "en");
        assert_eq!(detect_language("Analyze this project"), "en");
        assert_eq!(detect_language("Fix the bug in main.rs"), "en");
    }

    #[test]
    fn test_detect_language_mixed() {
        // More than 30% CJK should detect as zh
        assert_eq!(detect_language("帮我 fix 这个 bug"), "zh");
        // Empty string defaults to en
        assert_eq!(detect_language(""), "en");
    }

    // =========================================================================
    // Feature-001: Project Memory injection tests
    // =========================================================================

    fn make_test_memories() -> Vec<MemoryEntry> {
        vec![
            MemoryEntry {
                id: "mem-1".into(),
                project_path: "/test".into(),
                category: MemoryCategory::Preference,
                content: "Always use pnpm not npm".into(),
                keywords: vec!["pnpm".into()],
                importance: 0.9,
                access_count: 5,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-2".into(),
                project_path: "/test".into(),
                category: MemoryCategory::Convention,
                content: "Tests in __tests__/ directories".into(),
                keywords: vec!["tests".into()],
                importance: 0.7,
                access_count: 2,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-3".into(),
                project_path: "/test".into(),
                category: MemoryCategory::Pattern,
                content: "API routes return CommandResponse<T>".into(),
                keywords: vec!["api".into()],
                importance: 0.6,
                access_count: 1,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-4".into(),
                project_path: "/test".into(),
                category: MemoryCategory::Correction,
                content: "Do not edit executor.rs without cargo check".into(),
                keywords: vec!["executor".into()],
                importance: 0.8,
                access_count: 3,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-5".into(),
                project_path: "/test".into(),
                category: MemoryCategory::Fact,
                content: "Frontend uses Zustand for state management".into(),
                keywords: vec!["zustand".into()],
                importance: 0.5,
                access_count: 0,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
        ]
    }

    #[test]
    fn test_build_system_prompt_with_memories() {
        let tools = get_tool_definitions();
        let memories = make_test_memories();
        let prompt = build_system_prompt_with_memories(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            Some(&memories),
            "TestProvider",
            "test-model",
            "en",
        );

        // Memory section should be present
        assert!(prompt.contains("## Project Memory"));
        assert!(prompt.contains("learned from previous sessions"));

        // All category badges should appear
        assert!(prompt.contains("[PREF]"));
        assert!(prompt.contains("[CONV]"));
        assert!(prompt.contains("[PATN]"));
        assert!(prompt.contains("[WARN]"));
        assert!(prompt.contains("[FACT]"));

        // Memory content should be present
        assert!(prompt.contains("Always use pnpm not npm"));
        assert!(prompt.contains("Tests in __tests__/ directories"));
        assert!(prompt.contains("API routes return CommandResponse<T>"));
    }

    #[test]
    fn test_build_system_prompt_memory_section_position() {
        let tools = get_tool_definitions();
        let summary = make_test_summary();
        let memories = make_test_memories();
        let prompt = build_system_prompt_with_memories(
            &PathBuf::from("/test/project"),
            &tools,
            Some(&summary),
            Some(&memories),
            "TestProvider",
            "test-model",
            "en",
        );

        // Memory section should appear after project summary and before Available Tools
        let summary_pos = prompt.find("## Project Structure").unwrap();
        let memory_pos = prompt.find("## Project Memory").unwrap();
        let tools_pos = prompt.find("## Available Tools").unwrap();

        assert!(
            summary_pos < memory_pos,
            "Memory section must appear after Project Structure"
        );
        assert!(
            memory_pos < tools_pos,
            "Memory section must appear before Available Tools"
        );
    }

    #[test]
    fn test_build_system_prompt_no_memories() {
        let tools = get_tool_definitions();
        let prompt = build_system_prompt_with_memories(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(!prompt.contains("## Project Memory"));
    }

    #[test]
    fn test_build_system_prompt_empty_memories() {
        let tools = get_tool_definitions();
        let empty: Vec<MemoryEntry> = vec![];
        let prompt = build_system_prompt_with_memories(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            Some(&empty),
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(!prompt.contains("## Project Memory"));
    }

    #[test]
    fn test_build_system_prompt_backward_compatible() {
        // build_system_prompt (without memories) should still work
        let tools = get_tool_definitions();
        let prompt = build_system_prompt(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(!prompt.contains("## Project Memory"));
        assert!(prompt.contains("## Available Tools"));
    }

    #[test]
    fn test_memory_section_budget_respected() {
        // Create many memories that exceed the budget
        let mut big_memories = Vec::new();
        for i in 0..100 {
            big_memories.push(MemoryEntry {
                id: format!("mem-{}", i),
                project_path: "/test".into(),
                category: MemoryCategory::Fact,
                content: format!("This is a moderately long memory entry number {} that contributes to the total character count of the section", i),
                keywords: vec![],
                importance: 0.5,
                access_count: 0,
                source_session_id: None,
                source_context: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            });
        }

        let section = build_memory_section(Some(&big_memories));
        assert!(
            section.len() <= MEMORY_SECTION_BUDGET + 200, // small buffer for header
            "Memory section should respect budget (got {} chars)",
            section.len()
        );

        // Not all memories should be included
        let line_count = section.lines().count();
        assert!(
            line_count < 100,
            "Should have truncated memories, got {} lines",
            line_count
        );
    }
}
