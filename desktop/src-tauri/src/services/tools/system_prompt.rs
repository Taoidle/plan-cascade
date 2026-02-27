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

| User wants to know... | Use this tool |
|---|---|
| Understand a SPECIFIC directory/module/component | **Task** with subagent_type='explore' |
| "How does X work?" / "Explain module Y" | **Task** with subagent_type='explore' |
| "What does this project do?" / "Analyze this project" | **Task** with subagent_type='explore' |
| "Explain the architecture" / "How is this codebase structured?" | **Task** with subagent_type='explore' |
| Focused deep analysis of patterns/interactions | **Task** with subagent_type='plan' |

For project understanding, use `Task(subagent_type='explore')`. The system auto-escalates to a coordinator pattern for large projects — you do not need to choose between explore and general-purpose yourself.

For medium-sized projects, emit multiple parallel explore Tasks in one response, each targeting a different top-level directory or component area. Decompose based on the problem structure — the system automatically queues and limits concurrency.

**Aggregated file inventory for implementation** → Use **Analyze**:

| User wants to know... | Use this tool |
|---|---|
| Quick project context before making cross-module changes | **Analyze** (quick mode, default) |
| Comprehensive structural analysis with coverage gates | **Analyze** with mode='deep' (only when explicitly requested) |

**Key rule**: For simple single-answer questions, use one basic tool. For project understanding that requires reading many files, use Task. For aggregated file inventory before code changes, use Analyze.

### Step 3: The user wants to modify code or run commands

**First, assess the scope:**

- Single file, simple change → handle directly (Read → Edit / Write / Bash)
- 2-3 closely related files → handle directly, one at a time
- **Multiple independent files/modules/domains** → **decompose into parallel Tasks** (see "Autonomous Task Decomposition" below)

**Direct execution tools:**
- **Edit existing file**: Use Read first to see current contents, then Edit with exact string replacement.
- **Create new file**: Use Write.
- **Run tests, build, git, or shell commands**: Use Bash.
- **Edit Jupyter notebook cells**: Use NotebookEdit.

### Step 4: Web resources

- **Fetch a specific URL**: Use WebFetch.
- **Search for current information**: Use WebSearch.

## When to Use Task (Sub-Agent)

**Use Task** when the request requires reading and synthesizing information from multiple files. The sub-agent gets its own context window and can read many files without exhausting your main context.

**Sub-agent types** (via `subagent_type` parameter):
- `explore`: Codebase exploration — reads code, uses git for context (Read, Glob, Grep, LS, CodebaseSearch, Bash)
- `plan`: Architecture design and deep analysis — same tools as explore
- `general-purpose`: Coordinator with ALL tools including Task — can spawn further sub-agents
- `bash`: Shell command execution only (Bash + Cwd)

**IMPORTANT: Emit multiple Task calls in ONE response for parallel execution.**

**Examples of when to use explore:**
- "How does the auth module work?" → explore reads auth-related files
- "What's in src/services/?" → explore reads that directory
- "What does this project do?" → explore agent (auto-escalated for large projects)

**Examples of when to use general-purpose:**
- Complex multi-file implementation that needs write access
- Tasks requiring shell commands AND file editing in sequence

**Thoroughness hints** — include in the Task prompt to control exploration depth:
- "quick exploration" — brief overview (3-5 files)
- "medium exploration" — standard understanding (8-15 files, default)
- "very thorough exploration" — deep analysis (15-30 files, includes tests and config)

**Examples of when to use plan:**
- "How do components A and B interact?" → plan reads both components and traces connections
- "Analyze the error handling pattern" → plan reads error-related code across modules

## Autonomous Task Decomposition

When a user request touches **multiple independent files, modules, or domains**, you MUST proactively decompose it into parallel sub-tasks. Do NOT execute everything sequentially yourself — spawn parallel Task agents to maximize speed.

### When to Decompose

| Signal | Action |
|--------|--------|
| Request mentions multiple specific files/modules/areas | Decompose by file/module |
| Request involves both frontend AND backend changes | Decompose by technology boundary |
| Request says "all", "every", "each" for a set of items | Decompose by item or group |
| Request involves repetitive operations across many targets | Decompose by target group |
| Single file, simple change | Do NOT decompose — handle directly |

### How to Decompose — 4 Steps

**Step A: Analyze** — Identify the independent work units. Read relevant files or use CodebaseSearch/Grep to understand the scope.

**Step B: Plan & Announce** — Briefly tell the user what you're about to do: how many sub-tasks, what each one handles.

**Step C: Spawn Parallel Tasks** — Emit ALL Task calls in ONE response. Each Task gets a specific, self-contained prompt describing exactly what to do. Use `subagent_type='general-purpose'` for tasks that need to write code.

**Step D: Synthesize & Verify** — After all sub-agents complete, summarize the results and run verification (e.g., `Bash` to run tests, build, or lint).

### Decomposition Example

User: "Remove all failing tests from the project"

Your response should be:

1. First, run the test suite to identify failing tests: `Bash(cargo test 2>&1 | grep FAILED)`
2. Analyze the failures — group them by module/domain
3. Announce: "I found 20 failing tests across 4 areas. Launching 4 parallel agents..."
4. Spawn in ONE response:
   - `Task(prompt='Remove these failing tests from src/services/auth/: [list]...', subagent_type='general-purpose')`
   - `Task(prompt='Remove these failing tests from src/services/api/: [list]...', subagent_type='general-purpose')`
   - `Task(prompt='Remove these failing tests from src/components/: [list]...', subagent_type='general-purpose')`
   - `Task(prompt='Remove these failing tests from src/utils/: [list]...', subagent_type='general-purpose')`
5. After completion, verify: `Bash(cargo test)`

### More Examples

**"Add error handling to all API endpoints"** →
- Identify all endpoint files
- Group by domain (auth, users, orders, etc.)
- Spawn one Task per domain group

**"Refactor all components to use the new design system"** →
- List all components that need changes
- Group into 3-5 batches by directory
- Spawn one Task per batch

**"Fix the import paths after the directory restructure"** →
- Find all files with broken imports
- Group by top-level module
- Spawn one Task per module

### Rules

- Always tell the user your decomposition plan BEFORE spawning
- Give each Task a SPECIFIC, COMPLETE prompt — the sub-agent has no access to your conversation history
- Include file paths, exact requirements, and acceptance criteria in each Task prompt
- After all Tasks complete, ALWAYS run a verification step (tests, build, lint)
- If a sub-agent fails, analyze the error and retry that specific sub-task

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
pub fn build_memory_section(memories: Option<&[MemoryEntry]>) -> String {
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

/// Summary of a knowledge collection for system prompt awareness injection.
pub struct KnowledgeCollectionSummary {
    /// Human-readable collection name.
    pub name: String,
    /// Number of documents in the collection.
    pub document_count: usize,
    /// Number of indexed chunks in the collection.
    pub chunk_count: usize,
}

/// Build a lightweight knowledge awareness section for the system prompt.
///
/// This tells the AI which knowledge collections are available without
/// injecting any actual content. The AI uses the SearchKnowledge tool
/// to query on demand.
///
/// Returns an empty string if no collections are provided.
pub fn build_knowledge_awareness_section(
    collections: &[KnowledgeCollectionSummary],
    language: &str,
) -> String {
    if collections.is_empty() {
        return String::new();
    }

    let mut section = String::new();

    match language {
        "zh" => {
            section.push_str("\n\n## 知识库\n");
            section.push_str("你可以使用 SearchKnowledge 工具搜索以下知识集合：\n");
            for col in collections {
                section.push_str(&format!(
                    "- {} ({} documents, {} chunks)\n",
                    col.name, col.document_count, col.chunk_count,
                ));
            }
            section.push_str(
                "\n当你需要参考文档、规范、或项目相关知识时，请主动使用 SearchKnowledge 工具查询。",
            );
        }
        _ => {
            section.push_str("\n\n## Knowledge Base\n");
            section.push_str(
                "You can search the following knowledge collections using the SearchKnowledge tool:\n",
            );
            for col in collections {
                section.push_str(&format!(
                    "- {} ({} documents, {} chunks)\n",
                    col.name, col.document_count, col.chunk_count,
                ));
            }
            section.push_str(
                "\nWhen you need reference documentation, standards, or project-specific knowledge, \
                 proactively use the SearchKnowledge tool to query.",
            );
        }
    }

    section
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

/// Build a tool selection guidance section for sub-agents.
///
/// Sub-agents don't receive the full Decision Tree from the main agent's
/// system prompt. This function provides a condensed but directive version
/// that instructs the LLM to use CodebaseSearch as the primary exploration
/// tool when the index is available.
///
/// The `task_type` parameter differentiates guidance:
/// - `Some("explore")`: LS and CodebaseSearch are equally recommended (broad exploration)
/// - Other values / `None`: CodebaseSearch is preferred for initial file discovery
pub fn build_sub_agent_tool_guidance(
    has_index: bool,
    has_semantic: bool,
    task_type: Option<&str>,
) -> String {
    if !has_index {
        // No index — CodebaseSearch won't work, so no special guidance needed.
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("## Tool Selection Rules / 工具选择规则".to_string());
    lines.push(String::new());

    match task_type {
        Some("explore") => {
            // Exploration tasks: tools recommended based on goal, not rigid priority
            lines.push(
                "A pre-built codebase index is available. Choose tools based on your goal:"
                    .to_string(),
            );
            lines.push("已有预构建的代码索引。请根据目标选择工具：".to_string());
            lines.push(String::new());
            lines.push(
                "- **LS** — Understand directory structure, list files in a folder.".to_string(),
            );
            lines.push("  使用 LS 了解目录结构、列出文件夹内容。".to_string());
            lines.push(
                "- **CodebaseSearch** (scope=\"all\") — Find symbols, locate files by keyword."
                    .to_string(),
            );
            lines.push("  使用 CodebaseSearch 查找符号、按关键词定位文件。".to_string());

            if has_semantic {
                lines.push(
                    "- **CodebaseSearch** (scope=\"semantic\") — Natural-language conceptual queries."
                        .to_string(),
                );
                lines.push("  使用 scope=\"semantic\" 进行自然语言语义搜索。".to_string());
            }

            lines.push("- **Read** — Read specific files after discovery.".to_string());
            lines.push("  使用 Read 读取发现的具体文件。".to_string());
            lines.push(
                "- **Grep** — Full-text regex search when CodebaseSearch doesn't cover it."
                    .to_string(),
            );
            lines.push("  使用 Grep 进行 CodebaseSearch 无法覆盖的全文正则搜索。".to_string());
            lines.push(String::new());
            lines.push(
                "**Query tips**: Use short, focused queries (1-2 keywords). Make separate calls for different concepts."
                    .to_string(),
            );
            lines.push(
                "**查询技巧**：使用简短关键词（每次1-2个词），不同概念分开查询。".to_string(),
            );
        }
        _ => {
            // Analyze/implement/main agent: CodebaseSearch preferred
            lines.push(
                "A pre-built codebase index is available. You MUST follow this priority order:"
                    .to_string(),
            );
            lines.push("已有预构建的代码索引，你必须按以下优先级选择工具：".to_string());
            lines.push(String::new());
            lines.push(
                "1. **CodebaseSearch** (scope=\"all\") — Use first for finding symbols, \
                 locating files, and understanding project structure. It is faster and more accurate \
                 than scanning files manually."
                    .to_string(),
            );
            lines.push(
                "   优先使用 CodebaseSearch（scope=\"all\"）查找符号、定位文件和理解项目结构。"
                    .to_string(),
            );
            lines.push(
                "2. **CodebaseSearch** (scope=\"symbols\") — Use to find specific function, class, \
                 or struct definitions by name."
                    .to_string(),
            );
            lines.push("   使用 scope=\"symbols\" 按名称查找函数、类或结构体定义。".to_string());

            if has_semantic {
                lines.push(
                    "3. **CodebaseSearch** (scope=\"semantic\") — Use for natural-language conceptual \
                     queries when you need semantic matches."
                        .to_string(),
                );
                lines.push("   使用 scope=\"semantic\" 进行自然语言语义搜索。".to_string());
            }

            lines.push(String::new());
            lines.push(
                "Use **Read** after CodebaseSearch to read specific files you discovered."
                    .to_string(),
            );
            lines.push("使用 Read 读取通过 CodebaseSearch 发现的具体文件。".to_string());
            lines.push(
                "Use **Grep** ONLY for full-text regex search or when CodebaseSearch reports index unavailable."
                    .to_string(),
            );
            lines.push(
                "仅在需要正则全文搜索或 CodebaseSearch 报告索引不可用时使用 Grep。".to_string(),
            );
            lines.push(
                "Prefer CodebaseSearch over LS/Glob for initial file discovery — the index is faster and more comprehensive."
                    .to_string(),
            );
            lines.push(
                "建议优先使用 CodebaseSearch 而非 LS/Glob 进行初始文件发现——索引更快更全面。"
                    .to_string(),
            );
        }
    }

    lines.join("\n")
}

/// Build a skills injection section for the system prompt.
///
/// Accepts matched skills and formats them into a section that can be appended
/// to the system prompt. Returns empty string if no skills are provided.
///
/// This function is designed to be used alongside `build_system_prompt` without
/// modifying its signature, keeping backward compatibility.
pub fn build_skills_section(
    matched_skills: &[crate::services::skills::model::SkillMatch],
) -> String {
    if matched_skills.is_empty() {
        return String::new();
    }

    let mut section = String::new();
    section.push_str("\n\n## Framework-Specific Best Practices\n\n");
    section.push_str("The following guidelines apply based on detected frameworks:\n");

    for (i, skill_match) in matched_skills.iter().enumerate() {
        let source_label = match &skill_match.skill.source {
            crate::services::skills::model::SkillSource::Builtin => "builtin".to_string(),
            crate::services::skills::model::SkillSource::External { source_name } => {
                format!("{} (external)", source_name)
            }
            crate::services::skills::model::SkillSource::User => "user".to_string(),
            crate::services::skills::model::SkillSource::ProjectLocal => {
                "project-local".to_string()
            }
            crate::services::skills::model::SkillSource::Generated => "auto-generated".to_string(),
        };

        section.push_str(&format!("\n### {}\n", skill_match.skill.name));
        section.push_str(&format!(
            "*Source: {} | Priority: {}*\n\n",
            source_label, skill_match.skill.priority
        ));
        section.push_str(&skill_match.skill.description);

        if i < matched_skills.len() - 1 {
            section.push_str("\n\n---\n");
        } else {
            section.push('\n');
        }
    }

    section
}

/// Build a plugin instructions section for system prompt injection.
pub fn build_plugin_instructions_section(instructions: &str) -> String {
    if instructions.is_empty() {
        return String::new();
    }
    format!("\n\n## Plugin Instructions\n\n{}", instructions)
}

/// Build a plugin skills section for system prompt injection.
pub fn build_plugin_skills_section(
    plugin_skills: &[crate::services::plugins::models::PluginSkill],
) -> String {
    if plugin_skills.is_empty() {
        return String::new();
    }
    let mut section = String::from("\n\n## Plugin Skills\n\n");
    section.push_str("The following skills are provided by enabled plugins:\n");
    for (i, skill) in plugin_skills.iter().enumerate() {
        section.push_str(&format!("\n### {}\n", skill.name));
        section.push_str(&format!("*{}*\n\n", skill.description));
        section.push_str(&skill.body);
        if !skill.allowed_tools.is_empty() {
            section.push_str(&format!(
                "\n\n**Allowed tools**: {}\n",
                skill.allowed_tools.join(", ")
            ));
        }
        if i < plugin_skills.len() - 1 {
            section.push_str("\n\n---\n");
        } else {
            section.push('\n');
        }
    }
    section
}

/// Build a plugin commands section for system prompt injection.
///
/// Each command is formatted as `### /<name>` with description and body.
/// The LLM should follow the command body when the user invokes `/<command-name>`.
pub fn build_plugin_commands_section(
    plugin_commands: &[crate::services::plugins::models::PluginCommand],
) -> String {
    if plugin_commands.is_empty() {
        return String::new();
    }
    let mut section = String::from("\n\n## Plugin Commands\n\n");
    section.push_str(
        "The following commands are provided by enabled plugins. When the user invokes a command \
         (e.g. `/<command-name>`), follow the instructions in the command body.\n",
    );
    for (i, cmd) in plugin_commands.iter().enumerate() {
        section.push_str(&format!("\n### /{}\n", cmd.name));
        if !cmd.description.is_empty() {
            section.push_str(&format!("*{}*\n\n", cmd.description));
        }
        section.push_str(&cmd.body);
        if i < plugin_commands.len() - 1 {
            section.push_str("\n\n---\n");
        } else {
            section.push('\n');
        }
    }
    section
}

/// Build a full system prompt with optional skills injection.
///
/// This is a convenience wrapper that combines `build_system_prompt` with
/// `build_skills_section` for callers that have matched skills available.
pub fn build_system_prompt_with_skills(
    project_root: &Path,
    tools: &[ToolDefinition],
    project_summary: Option<&ProjectIndexSummary>,
    matched_skills: Option<&[crate::services::skills::model::SkillMatch]>,
    provider_name: &str,
    model_name: &str,
    language: &str,
) -> String {
    let base_prompt = build_system_prompt(
        project_root,
        tools,
        project_summary,
        provider_name,
        model_name,
        language,
    );

    let skills_section = matched_skills
        .map(|skills| build_skills_section(skills))
        .unwrap_or_default();

    if skills_section.is_empty() {
        base_prompt
    } else {
        format!("{}{}", base_prompt, skills_section)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::orchestrator::index_store::ComponentSummary;
    use crate::services::tools::definitions::get_tool_definitions_from_registry;
    use std::path::PathBuf;

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

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
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(prompt.contains("Decision Tree"));
        assert!(prompt.contains("Do NOT use Analyze when"));
        assert!(prompt.contains("Respond directly. No tool needed."));
        assert!(prompt.contains("Read before modifying"));
    }

    #[test]
    fn test_build_system_prompt_no_workflow_pattern() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

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
        let tools = get_tool_definitions_from_registry();
        let summary = make_test_summary();
        let prompt = build_system_prompt(
            &PathBuf::from("/test/project"),
            &tools,
            Some(&summary),
            "TestProvider",
            "test-model",
            "en",
        );

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
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test/project"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

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
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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
        let tools = get_tool_definitions_from_registry();
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

    // =========================================================================
    // Sub-agent tool guidance tests
    // =========================================================================

    #[test]
    fn test_build_sub_agent_tool_guidance_with_index() {
        let guidance = build_sub_agent_tool_guidance(true, false, None);

        assert!(
            guidance.contains("CodebaseSearch"),
            "Should mention CodebaseSearch when index is available"
        );
        assert!(
            guidance.contains("scope=\"all\""),
            "Should recommend scope=all"
        );
        assert!(guidance.contains("Grep"), "Should mention Grep as fallback");
        // No semantic search
        assert!(
            !guidance.contains("semantic"),
            "Should not mention semantic when no embeddings"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_with_semantic() {
        let guidance = build_sub_agent_tool_guidance(true, true, None);

        assert!(
            guidance.contains("scope=\"semantic\""),
            "Should mention semantic scope when embeddings available"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_without_index() {
        let guidance = build_sub_agent_tool_guidance(false, false, None);

        assert!(
            guidance.is_empty(),
            "Should return empty string when no index is available"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_without_index_but_semantic() {
        // Edge case: semantic without index — should still be empty
        let guidance = build_sub_agent_tool_guidance(false, true, None);

        assert!(
            guidance.is_empty(),
            "Should return empty string when no index, even with semantic flag"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_explore_allows_ls() {
        let guidance = build_sub_agent_tool_guidance(true, false, Some("explore"));

        assert!(
            guidance.contains("LS"),
            "Explore guidance should mention LS"
        );
        assert!(
            !guidance.contains("ALWAYS use FIRST"),
            "Explore guidance should not have absolute CodebaseSearch-first directive"
        );
        assert!(
            !guidance.contains("Do NOT start"),
            "Explore guidance should not prohibit starting with LS"
        );
        assert!(
            guidance.contains("Query tips"),
            "Explore guidance should include query tips"
        );
        assert!(
            guidance.contains("CodebaseSearch"),
            "Explore guidance should still mention CodebaseSearch"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_analyze_keeps_priority() {
        let guidance = build_sub_agent_tool_guidance(true, false, Some("analyze"));

        assert!(
            guidance.contains("priority order"),
            "Analyze guidance should maintain priority order"
        );
        assert!(
            guidance.contains("Prefer CodebaseSearch"),
            "Analyze guidance should prefer CodebaseSearch over LS/Glob"
        );
        assert!(
            !guidance.contains("Do NOT start"),
            "Analyze guidance should use softer language"
        );
    }

    #[test]
    fn test_system_prompt_simplified_routing_no_project_size_table() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        // Old project-size routing table should be gone
        assert!(
            !prompt.contains("Small (≤50 files"),
            "Old small/medium/large routing table should be removed"
        );
        assert!(
            !prompt.contains("Medium (51-200 files"),
            "Old medium row should be removed"
        );
        assert!(
            !prompt.contains("Large (>200 files"),
            "Old large row should be removed"
        );

        // Should contain the simplified auto-escalation note
        assert!(
            prompt.contains("auto-escalates"),
            "Should mention auto-escalation for large projects"
        );
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

    // =========================================================================
    // Autonomous Task Decomposition section tests
    // =========================================================================

    #[test]
    fn test_system_prompt_contains_autonomous_decomposition() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(
            prompt.contains("Autonomous Task Decomposition"),
            "System prompt should contain the Autonomous Task Decomposition section"
        );
        assert!(
            prompt.contains("When to Decompose"),
            "Should contain decomposition trigger table"
        );
        assert!(
            prompt.contains("How to Decompose"),
            "Should contain the 4-step decomposition strategy"
        );
        assert!(prompt.contains("Step A: Analyze"), "Should contain Step A");
        assert!(
            prompt.contains("Step D: Synthesize"),
            "Should contain Step D"
        );
        assert!(
            prompt.contains("Decomposition Example"),
            "Should contain a concrete decomposition example"
        );
    }

    #[test]
    fn test_system_prompt_step3_has_scope_assessment() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "TestProvider",
            "test-model",
            "en",
        );

        assert!(
            prompt.contains("assess the scope"),
            "Step 3 should start with scope assessment"
        );
        assert!(
            prompt.contains("decompose into parallel Tasks"),
            "Step 3 should mention decomposition for multi-file changes"
        );
    }
}
