//! System Prompt Builder
//!
//! Builds comprehensive system prompts that instruct LLMs to use tools effectively
//! for agentic code analysis and modification.

use std::path::Path;

use crate::services::llm::types::ToolDefinition;
use crate::services::memory::store::{MemoryCategory, MemoryEntry};
use crate::services::orchestrator::index_store::ProjectIndexSummary;

/// Detect the primary language of the user's message.
/// Returns one of: "zh", "ja", "ko", "en".
pub fn detect_language(text: &str) -> &'static str {
    let han_count = text.chars().filter(|c| is_han_char(*c)).count();
    let kana_count = text.chars().filter(|c| is_japanese_kana(*c)).count();
    let hangul_count = text.chars().filter(|c| is_hangul(*c)).count();
    let total_alnum = text.chars().filter(|c| c.is_alphanumeric()).count();

    // Hangul is unique to Korean.
    if hangul_count >= 2 && (hangul_count as f64 / total_alnum.max(1) as f64) >= 0.2 {
        return "ko";
    }

    // Hiragana/Katakana are unique to Japanese.
    if kana_count >= 1 {
        return "ja";
    }

    // Han ideographs without kana/hangul are treated as Chinese.
    if total_alnum > 0 && (han_count as f64 / total_alnum as f64) > 0.3 {
        "zh"
    } else {
        "en"
    }
}

fn is_han_char(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
    )
}

fn is_japanese_kana(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{309F}' // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
            | '\u{31F0}'..='\u{31FF}' // Katakana Phonetic Extensions
    )
}

fn is_hangul(c: char) -> bool {
    matches!(
        c,
        '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
            | '\u{1100}'..='\u{11FF}' // Hangul Jamo
            | '\u{3130}'..='\u{318F}' // Hangul Compatibility Jamo
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

fn build_operating_contract(language: &str) -> String {
    match language {
        "zh" => r#"
## 基础操作契约

### 身份
- 你是当前项目的 AI 软件工程助手。
- 你的职责是理解代码库、定位实现、修改代码、制定计划、执行任务，并基于真实工具结果回答问题。
- 对代码库的结论优先使用工具和文件证据，不要凭空假设仓库内容。

### 非协商规则
- 不要伪造工具结果。
- 不要声称读取过某个文件，除非你真的读取了它。
- 不要声称搜索到了某个结果，除非搜索工具真的返回了它。
- 不要声称测试通过，除非测试真的通过。
- 不要声称修改成功，除非修改真的完成并已落盘。
- 不要声称记忆已经保存，除非系统实际完成了记忆抽取/持久化。

### 证据分层
按以下顺序组织事实：
1. 当前会话中的真实工具结果
2. 注入的 Project Memory
3. 当前对话上下文
4. 明确标注为推断的结论

### 输出规则
- 事实与推断要分开。
- 能引用文件路径、符号名、模块名时，就不要用模糊措辞。
- 不要用空泛安慰或营销式语气。
- 优先简洁、直接、技术化。

### 完成标准
只有以下情况之一满足时，任务才算完成：
- 用户问题已基于足够证据回答
- 请求的修改已实际完成并尽可能验证
- 存在真实阻塞，且阻塞已明确说明
"#
        .to_string(),
        _ => r#"
## Operating Contract

### Identity
- You are the AI software engineering assistant for the current project.
- Your job is to understand the codebase, locate implementations, modify code, plan work, execute tasks, and answer using real tool evidence.
- For repository-specific claims, prefer tool-backed evidence over intuition.

### Non-Negotiable Rules
- Never fabricate tool results.
- Never claim you read a file unless you actually read it.
- Never claim a search found something unless a search actually found it.
- Never claim tests passed unless they passed.
- Never claim a change was applied unless it was actually written.
- Never claim memory was saved unless the system actually completed extraction/persistence.

### Evidence Hierarchy
Use this order of trust:
1. Real tool results from the current session
2. Injected Project Memory
3. Current conversation context
4. Explicitly labeled inference

### Output Rules
- Separate verified facts from inference.
- Prefer exact file paths, symbols, and modules over vague wording.
- Avoid hype, filler, and reassurance-heavy language.
- Be concise, direct, and technical.

### Completion Criteria
A task is complete only when one of these is true:
- the user's question has been answered with sufficient evidence
- the requested change has been made and verified as far as possible
- a real blocker prevents progress and it has been clearly stated
"#
        .to_string(),
    }
}

fn build_memory_policy_addendum(language: &str, has_project_memories: bool) -> String {
    match language {
        "zh" => {
            let availability = if has_project_memories {
                "当前会话已注入 Project Memory。可以把其中内容视为跨会话持久记忆，但如果它与新鲜代码证据冲突，必须优先相信最新代码证据，并明确指出冲突。"
            } else {
                "当前会话没有注入 Project Memory。不要假装记得过去的会话，也不要说“我之前记得/我一直知道”，除非当前上下文真的提供了这些内容。"
            };
            format!(
                r#"
## 记忆策略

- 记忆系统是自动抽取、自动注入的，不要求你显式调用某个“记忆工具”才算有记忆。
- 只有当 Project Memory 段真实出现在系统提示中时，你才可以把它当作跨会话持久记忆。
- 不要把“当前会话里刚刚看到的内容”误说成“已经长期记住的内容”。
- 不要虚构记忆状态，不要假装记忆持久化已经成功。
- 如果记忆与仓库当前证据冲突，优先使用当前仓库证据。

{availability}
"#
            )
        }
        _ => {
            let availability = if has_project_memories {
                "Project Memory is injected for this session. Treat it as persistent cross-session memory, but prefer fresh repository evidence if they conflict, and call out the conflict explicitly."
            } else {
                "No Project Memory is injected for this session. Do not pretend to remember prior sessions or claim persistent memory that is not actually present."
            };
            format!(
                r#"
## Memory Policy

- The memory system is automatic: extraction and prompt injection are system-managed.
- You may treat memory as cross-session persistent memory only when a Project Memory section is actually present.
- Do not confuse current-session context with persistent memory.
- Do not fabricate memory state or imply memory persistence succeeded if it did not.
- If memory conflicts with fresh repository evidence, prefer the fresh repository evidence.

{availability}
"#
            )
        }
    }
}

pub fn build_mode_addendum(mode_key: &str, language: &str) -> String {
    match (mode_key, language) {
        ("chat", "zh") => {
            r#"
## 模式附加段：Chat

- 这是交互式代码助手模式。
- 优先快速、准确地回答问题或完成局部修改。
- 如果问题需要多文件理解，先搜索和收窄范围，再给出结论或执行修改。
- 不要把简单问题过度升级成复杂工作流。
"#
        }
        ("plan", "zh") => {
            r#"
## 模式附加段：Plan

- 这是规划与分解模式。
- 重点是任务拆解、依赖关系、风险、验收标准和执行顺序。
- 对代码证据仍要真实引用，但输出优先结构化、可执行、可验证。
- 不要把计划模式退化成泛泛分析。
"#
        }
        ("task", "zh") => {
            r#"
## 模式附加段：Task

- 这是任务执行模式。
- 重点是围绕当前任务形成正确的实现、验证和交付链路。
- 在探索和实现时，先用 CodebaseSearch 缩小范围，再读取和修改目标文件。
- 不要跳过验证，也不要把中间推断当成最终结论。
"#
        }
        ("analysis", "zh") => {
            r#"
## 模式附加段：Analysis

- 这是分析阶段。
- 目标是用尽可能高密度的证据生成结构化分析，而不是直接修改代码。
- 关注覆盖率、代表性文件、关键实现链路和架构事实。
"#
        }
        ("subagent_explore", "zh") => {
            r#"
## 模式附加段：Sub-Agent Explore

- 你是被委派出来做代码探索的子代理。
- 对实现定位、架构理解、符号追踪这类问题，先用 CodebaseSearch，再用 Read 读取缩小后的目标文件。
- 只有在任务明确要求正则、精确字符串、日志全文或已知字面量时才优先使用 Grep。
- 只返回与你被分配的子任务直接相关的高信号结论。
"#
        }
        ("subagent_plan", "zh") => {
            r#"
## 模式附加段：Sub-Agent Plan

- 你是被委派出来做深度分析/规划的子代理。
- 重点是依赖关系、交互边界、风险点和实施顺序，而不是泛泛复述代码。
- 先用 CodebaseSearch 缩小分析范围，再读取代表性文件。
- 保持范围收敛，不要扩散成无边界探索。
"#
        }
        ("subagent_general", "zh") => {
            r#"
## 模式附加段：Sub-Agent General

- 你是被委派出来执行具体子任务的通用子代理。
- 先定位目标，再执行修改或命令；不要无边界地重新探索整个项目。
- 如果子任务涉及代码理解，先用 CodebaseSearch 收窄范围。
- 汇报时突出结果、验证和阻塞，不要复述噪声。
"#
        }
        ("subagent_bash", "zh") => {
            r#"
## 模式附加段：Sub-Agent Bash

- 你是被委派出来执行命令的 Bash 子代理。
- 只报告真实命令输出、退出码和必要结论。
- 不要伪造命令结果，也不要把未执行的命令描述成已完成。
"#
        }
        ("composer_llm", "zh") => {
            r#"
## 模式附加段：Agent Composer LLM Step

- 你正在 Agent Composer 的 LLM 步骤中工作。
- 目标是基于输入和共享状态产出明确、结构化、可供后续节点消费的结果。
- 不要假设外部副作用已经发生，除非工具结果明确表明它发生了。
"#
        }
        ("composer_loop", "zh") => {
            r#"
## 模式附加段：Agent Composer Loop Step

- 你正在 Agent Composer 的循环步骤中工作。
- 每一轮都要基于共享状态和前一轮真实结果推进，不要原地重复。
- 如果没有新的证据或状态变化，就应收敛而不是空转。
"#
        }
        ("subagent", "zh") => {
            r#"
## 模式附加段：Sub-Agent

- 你是主代理分派出来的子代理。
- 只处理分配给你的子任务，不要偏离范围。
- 先收集证据，再返回高信号结论，不要复述噪声。
"#
        }
        ("chat", _) => {
            r#"
## Mode Addendum: Chat

- This is interactive assistant mode.
- Prefer fast, accurate answers and focused edits.
- If the question requires multi-file understanding, narrow the target area before concluding or editing.
- Do not escalate simple requests into unnecessary workflows.
"#
        }
        ("plan", _) => {
            r#"
## Mode Addendum: Plan

- This is planning and decomposition mode.
- Prioritize task breakdown, dependencies, risks, acceptance criteria, and execution order.
- Ground planning claims in real code evidence when relevant.
- Do not degrade planning into vague commentary.
"#
        }
        ("task", _) => {
            r#"
## Mode Addendum: Task

- This is execution-oriented task mode.
- Focus on the implementation, verification, and delivery chain for the current task.
- Use CodebaseSearch first to narrow the search space before reading and editing code.
- Do not present intermediate guesses as final conclusions.
"#
        }
        ("analysis", _) => {
            r#"
## Mode Addendum: Analysis

- This is analysis mode.
- The goal is to produce structured analysis grounded in strong evidence, not to jump directly into code changes.
- Focus on coverage, representative files, key implementation chains, and architecture facts.
"#
        }
        ("subagent_explore", _) => {
            r#"
## Mode Addendum: Sub-Agent Explore

- You are a delegated exploration sub-agent.
- For implementation tracing, architecture understanding, and symbol discovery, use CodebaseSearch first, then Read narrowed files.
- Use Grep first only for regex, exact-string, log-text, or literal matching tasks.
- Return only the findings that directly answer the assigned sub-task.
"#
        }
        ("subagent_plan", _) => {
            r#"
## Mode Addendum: Sub-Agent Plan

- You are a delegated deep-analysis/planning sub-agent.
- Focus on dependencies, interaction boundaries, risks, and execution order rather than generic summaries.
- Use CodebaseSearch to narrow the analysis scope before reading files.
- Stay tightly scoped to the assigned analytical question.
"#
        }
        ("subagent_general", _) => {
            r#"
## Mode Addendum: Sub-Agent General

- You are a delegated general-purpose execution sub-agent.
- Narrow the target area before editing or executing commands; do not re-explore the whole project without need.
- If the task involves code understanding, use CodebaseSearch first to narrow the search space.
- Report outcomes, verification, and blockers with minimal noise.
"#
        }
        ("subagent_bash", _) => {
            r#"
## Mode Addendum: Sub-Agent Bash

- You are a delegated bash execution sub-agent.
- Report only real command output, exit codes, and necessary conclusions.
- Do not fabricate command success or describe unexecuted commands as completed.
"#
        }
        ("composer_llm", _) => {
            r#"
## Mode Addendum: Agent Composer LLM Step

- You are operating inside an Agent Composer LLM step.
- Produce explicit, structured output that downstream nodes can consume.
- Do not imply external side effects occurred unless real tool evidence confirms them.
"#
        }
        ("composer_loop", _) => {
            r#"
## Mode Addendum: Agent Composer Loop Step

- You are operating inside an Agent Composer loop step.
- Each round must advance based on shared state and real prior results.
- If there is no new evidence or state change, converge instead of repeating yourself.
"#
        }
        ("subagent", _) => {
            r#"
## Mode Addendum: Sub-Agent

- You are a delegated sub-agent.
- Stay tightly scoped to the assigned sub-task.
- Gather evidence first, then return high-signal findings without noise.
"#
        }
        _ => "",
    }
    .to_string()
}

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
    let operating_contract = build_operating_contract(language);
    let memory_policy = build_memory_policy_addendum(
        language,
        project_memories.map(|mems| !mems.is_empty()).unwrap_or(false),
    );

    let identity_line = format!(
        "You are an AI coding assistant powered by {provider}/{model}. \
         You are NOT Claude, Claude Code, or any other specific product — \
         always identify yourself by your actual model name when asked.",
        provider = provider_name,
        model = model_name,
    );

    let language_instruction = match language {
        "zh" => "IMPORTANT: The user is communicating in Chinese. You MUST respond in Chinese (简体中文). Keep all your output, analysis, and explanations in Chinese. Only use English for code, technical terms, and tool parameters.",
        "ja" => "IMPORTANT: The user is communicating in Japanese. You MUST respond in Japanese. Keep code symbols, identifiers, and file paths in original form.",
        "ko" => "IMPORTANT: The user is communicating in Korean. You MUST respond in Korean. Keep code symbols, identifiers, and file paths in original form.",
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

{operating_contract}

{memory_policy}

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
| "Find where function X is defined" / "What files are in component Y?" | **CodebaseSearch** | ~~Analyze~~ ~~Task~~ |
| "Search for error handling" / "Find string in files" | **Grep** | ~~Analyze~~ ~~Task~~ |
| "Show me the contents of main.rs" | **Read** | ~~Analyze~~ ~~Task~~ |

> **Tip — CodebaseSearch vs Grep**: For code discovery, architecture tracing, symbol lookup, and implementation search, use **CodebaseSearch first**. Use **Grep** only for regex, exact-string, or full-text fallback, or when CodebaseSearch reports the index is unavailable.

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
- `explore`: Codebase exploration — prefer CodebaseSearch first, then Read after narrowing; use Grep only for exact/full-text fallback (CodebaseSearch, Read, Grep, LS, Glob, Bash)
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

**Step A: Analyze** — Identify the independent work units. Use CodebaseSearch first to locate relevant code, then Read narrowed files. Use Grep only for regex/exact/full-text fallback.

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
        operating_contract = operating_contract,
        memory_policy = memory_policy,
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
            let header = "\n\n## Project Memory\nThe following persistent facts were learned from previous sessions and injected automatically. Treat them as cross-session memory only for this session, and prefer fresh repository evidence if there is a conflict.\n\n";
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
/// Lists available knowledge collections so the AI knows what can be searched.
/// Tool priority instructions are handled separately by `build_tool_priority_section()`.
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
            section.push_str("以下知识集合可通过 SearchKnowledge 工具搜索：\n");
            for col in collections {
                section.push_str(&format!(
                    "- {} ({} documents, {} chunks)\n",
                    col.name, col.document_count, col.chunk_count,
                ));
            }
        }
        _ => {
            section.push_str("\n\n## Knowledge Base\n");
            section.push_str(
                "The following knowledge collections are available via the SearchKnowledge tool:\n",
            );
            for col in collections {
                section.push_str(&format!(
                    "- {} ({} documents, {} chunks)\n",
                    col.name, col.document_count, col.chunk_count,
                ));
            }
        }
    }

    section
}

/// Build a unified tool priority section for the system prompt.
///
/// Generates priority instructions based on which high-value tools are available:
/// - Knowledge + Code project: `SearchKnowledge → CodebaseSearch → others`
/// - Knowledge only:           `SearchKnowledge → others`
/// - Code project only:        `CodebaseSearch → others`
/// - Neither:                  returns empty string (no priority needed)
///
/// This is injected into both main agent and sub-agent system prompts by the
/// orchestrator, which knows whether index_store and knowledge_pipeline are set.
pub fn build_tool_priority_section(
    has_knowledge: bool,
    has_codebase_search: bool,
    codebase_index_ready: bool,
    semantic_search_ready: bool,
    language: &str,
) -> String {
    if !has_knowledge && !has_codebase_search {
        return String::new();
    }

    let mut section = String::new();

    match language {
        "zh" => {
            section.push_str("\n\n## 工具选择指南（重要）\n\n");
            section.push_str("根据你要完成的**任务场景**选择最合适的工具：\n\n");

            // Scenario 1: domain knowledge lookup
            if has_knowledge {
                section.push_str(
                    "### 查找文档/规范/设计/标准\n\
                     → **SearchKnowledge**\n\
                     当你需要查阅项目文档、设计规范、API 参考、编码标准、业务规则等领域知识时，\
                     **首先使用 SearchKnowledge**。知识库基于语义搜索，能理解查询意图。\n\n",
                );
            }

            // Scenario 2: code understanding
            if has_codebase_search {
                if codebase_index_ready {
                    section.push_str(
                        "### 代码索引状态\n\
                         - Codebase index: **ready**\n\
                         - Semantic search: "
                    );
                    section.push_str(if semantic_search_ready {
                        "**available**\n\n"
                    } else {
                        "**unavailable**\n\n"
                    });
                    section.push_str(
                        "### 理解代码架构 / 定位功能实现\n\
                         → **必须先使用 CodebaseSearch**\n\
                         当你不确定代码在哪里、需要理解「某个功能是怎么实现的」、或探索不熟悉的代码模块时，\
                         先用 CodebaseSearch，不要先用 Grep/Read/Glob 逐文件扫描。\n\
                         - `scope=\"hybrid\"` — 综合搜索（默认）\n\
                         - `scope=\"symbol\"` — 按名称查找函数/类/结构体定义\n\
                         - `scope=\"path\"` — 按路径/文件名查找文件\n"
                    );
                    if semantic_search_ready {
                        section.push_str(
                            "                         - `scope=\"semantic\"` — 用自然语言描述搜索概念\n\n"
                        );
                    } else {
                        section.push_str("\n");
                    }
                } else {
                    section.push_str(
                        "### 代码索引状态\n\
                         - Codebase index: **unavailable**\n\
                         - Semantic search: **unavailable**\n\n\
                         当前项目索引不可用。不要尝试 CodebaseSearch；直接使用 Grep / Read / Glob / LS 完成代码探索。\n\n",
                    );
                }
            }

            // Scenario 3: precise matching
            section.push_str(
                "### 搜索精确标识符 / 字符串 / 正则匹配\n\
                 → **Grep**\n\
                 当你已知确切的函数名、变量名、错误信息、字符串常量、或需要正则表达式匹配时，\
                 使用 Grep。它逐文件扫描，精确匹配，适合查找所有引用点。\n\n",
            );

            // Scenario 4: reading files
            section.push_str(
                "### 查看文件内容\n\
                 → **Read**\n\
                 当你已知文件路径，需要查看具体代码或文档内容时使用。\
                 支持 PDF、DOCX、图片、Jupyter Notebook 等多格式。\n\n",
            );

            // Scenario 5: finding files by pattern
            section.push_str(
                "### 按路径/名称模式查找文件\n\
                 → **Glob**\n\
                 当你需要按文件名模式搜索（如 `**/*.rs`、`**/test_*.py`）时使用。\
                 仅匹配路径，不搜索内容。\n\n",
            );

            // Scenario 6: browsing directory structure
            section.push_str(
                "### 浏览目录结构\n\
                 → **LS**\n\
                 当你需要查看某个目录下的文件和子目录列表时使用。\n\n",
            );

            // Scenario 7: web research
            section.push_str(
                "### 查找外部技术方案 / 第三方文档\n\
                 → **WebSearch** + **WebFetch**\n\
                 需要搜索互联网信息时用 WebSearch；需要获取特定 URL 内容时用 WebFetch。\n\n",
            );

            // Key principles
            section.push_str("### 关键原则\n\n");

            if has_knowledge && has_codebase_search {
                section.push_str(
                    "- 涉及**领域知识**（文档/规范/标准/设计）→ 优先 SearchKnowledge\n\
                     - 涉及**代码搜索**（架构/实现/符号）→ 先用 CodebaseSearch；只有查不到或索引不可用时才用 Grep\n\
                     - SearchKnowledge 和 CodebaseSearch 可以**并行调用**以加速信息收集\n\
                     - 只有在 CodebaseSearch 结果不足，或需要 regex / 精确字符串 / 日志全文匹配时，才使用 Grep\n\
                     - Read 只用于在 CodebaseSearch 缩小范围后读取目标文件\n\
                     - 即使你认为自己已经知道答案，也应先搜索知识库以获取最新信息\n",
                );
            } else if has_knowledge {
                section.push_str(
                    "- 涉及**领域知识**（文档/规范/标准/设计）→ 优先 SearchKnowledge\n\
                     - 即使你认为自己已经知道答案，也应先搜索知识库以获取最新信息\n\
                     - 知识库搜索不到时再用其他工具定位信息\n",
                );
            } else {
                // has_codebase_search only
                section.push_str(
                    "- 涉及**代码搜索**（架构/实现/符号）→ 先用 CodebaseSearch\n\
                     - 不要在索引可用时用 Grep 逐文件扫描代码实现\n\
                     - 只有在 CodebaseSearch 不满足需求，或任务明确要求 regex / 全文匹配时，才使用 Grep\n\
                     - Read 只用于在 CodebaseSearch 缩小范围后读取目标文件\n",
                );
            }
        }
        _ => {
            section.push_str("\n\n## Tool Selection Guide (IMPORTANT)\n\n");
            section
                .push_str("Choose the most appropriate tool based on your **task scenario**:\n\n");

            // Scenario 1: domain knowledge lookup
            if has_knowledge {
                section.push_str(
                    "### Looking up documentation / specs / design / standards\n\
                     → **SearchKnowledge**\n\
                     When you need project docs, design specs, API references, coding standards, \
                     or business rules, **use SearchKnowledge first**. \
                     It uses semantic search and understands query intent.\n\n",
                );
            }

            // Scenario 2: code understanding
            if has_codebase_search {
                if codebase_index_ready {
                    section.push_str("### Codebase index status\n");
                    section.push_str("- Codebase index: **ready**\n");
                    section.push_str(if semantic_search_ready {
                        "- Semantic search: **available**\n\n"
                    } else {
                        "- Semantic search: **unavailable**\n\n"
                    });
                    section.push_str(
                        "### Understanding code architecture / locating implementations\n\
                         → **MUST use CodebaseSearch first**\n\
                         When you need to locate code, understand how a feature is implemented, \
                         trace symbols, or explore unfamiliar modules, use CodebaseSearch before \
                         Grep, Read, or Glob.\n\
                         - `scope=\"hybrid\"` — comprehensive search (default)\n\
                         - `scope=\"symbol\"` — find function/class/struct definitions by name\n\
                         - `scope=\"path\"` — find files by path/name patterns\n"
                    );
                    if semantic_search_ready {
                        section.push_str(
                            "                         - `scope=\"semantic\"` — search concepts using natural language\n\n"
                        );
                    } else {
                        section.push_str("\n");
                    }
                } else {
                    section.push_str(
                        "### Codebase index status\n\
                         - Codebase index: **unavailable**\n\
                         - Semantic search: **unavailable**\n\n\
                         The current project index is unavailable. Do not try CodebaseSearch for this request. \
                         Use Grep / Read / Glob / LS directly.\n\n",
                    );
                }
            }

            // Scenario 3: precise matching
            section.push_str(
                "### Searching for exact identifiers / strings / regex patterns\n\
                 → **Grep**\n\
                 When you know the exact function name, variable, error message, string constant, \
                 or need regex matching, use Grep. It scans files for precise matches \
                 and is ideal for finding all reference points.\n\n",
            );

            // Scenario 4: reading files
            section.push_str(
                "### Viewing file contents\n\
                 → **Read**\n\
                 When you know the file path and need to view specific code or document content. \
                 Supports PDF, DOCX, images, Jupyter Notebooks, and more.\n\n",
            );

            // Scenario 5: finding files by pattern
            section.push_str(
                "### Finding files by path/name pattern\n\
                 → **Glob**\n\
                 When you need to search by filename pattern (e.g., `**/*.rs`, `**/test_*.py`). \
                 Matches paths only, does not search file contents.\n\n",
            );

            // Scenario 6: browsing directory structure
            section.push_str(
                "### Browsing directory structure\n\
                 → **LS**\n\
                 When you need to list files and subdirectories in a folder.\n\n",
            );

            // Scenario 7: web research
            section.push_str(
                "### Looking up external solutions / third-party docs\n\
                 → **WebSearch** + **WebFetch**\n\
                 Use WebSearch for internet queries; WebFetch to retrieve specific URL content.\n\n",
            );

            // Key principles
            section.push_str("### Key Principles\n\n");

            if has_knowledge && has_codebase_search {
                section.push_str(
                    "- **Domain knowledge** (docs/specs/standards/design) → prefer SearchKnowledge\n\
                     - **Code search** (architecture/implementation/symbols) → use CodebaseSearch first; use Grep only if the index is unavailable or insufficient\n\
                     - SearchKnowledge and CodebaseSearch can be called **in parallel** to speed up gathering\n\
                     - Use Grep only for regex / exact-string / full-text matching\n\
                     - Use Read only after CodebaseSearch narrows the target files\n\
                     - Even if you think you know the answer, search the knowledge base for the latest info\n",
                );
            } else if has_knowledge {
                section.push_str(
                    "- **Domain knowledge** (docs/specs/standards/design) → prefer SearchKnowledge\n\
                     - Even if you think you know the answer, search the knowledge base for the latest info\n\
                     - Fall back to other tools only when knowledge search is insufficient\n",
                );
            } else {
                // has_codebase_search only
                section.push_str(
                    "- **Code search** (architecture/implementation/symbols) → use CodebaseSearch first\n\
                     - Do NOT use Grep to scan files when CodebaseSearch can answer the question\n\
                     - Use Grep only for regex / exact-string / full-text matching, or when CodebaseSearch is insufficient\n\
                     - Use Read only after CodebaseSearch narrows the target files\n",
                );
            }
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

/// Build a scenario-based tool selection guide for sub-agents.
///
/// Sub-agents don't receive the full tool selection guide from the main agent's
/// system prompt. This function provides a condensed scenario-based version
/// so sub-agents choose the right tool for each task type.
///
/// The `task_type` parameter differentiates guidance:
/// - `Some("explore")`: LS and CodebaseSearch are equally recommended (broad exploration)
/// - Other values / `None`: CodebaseSearch is preferred for code understanding tasks
pub fn build_sub_agent_tool_guidance(
    has_index: bool,
    index_ready: bool,
    has_semantic: bool,
    task_type: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push("## Tool Selection Guide / 工具选择指南".to_string());
    lines.push(String::new());
    if !has_index || !index_ready {
        lines.push("Codebase index status: unavailable".to_string());
        lines.push("代码索引状态：不可用".to_string());
        lines.push(
            "Use Grep for exact/full-text search, Read for file contents, and LS/Glob for structure discovery."
                .to_string(),
        );
        lines.push("此时直接使用 Grep、Read、LS、Glob，不要尝试 CodebaseSearch。".to_string());
        return lines.join("\n");
    }

    lines.push("Codebase index status: ready".to_string());
    lines.push("代码索引状态：已就绪".to_string());
    lines.push(if has_semantic {
        "Semantic search: available / 语义搜索：可用".to_string()
    } else {
        "Semantic search: unavailable / 语义搜索：不可用".to_string()
    });
    lines.push(String::new());
    lines.push(
        "A pre-built codebase index is available. For code understanding tasks, use CodebaseSearch before Grep/Read."
            .to_string(),
    );
    lines.push("已有预构建的代码索引。涉及代码理解时，先用 CodebaseSearch，再用 Grep/Read。".to_string());
    lines.push(String::new());

    match task_type {
        Some("explore") => {
            lines.push(
                "- **Locate code / implementations first** → **CodebaseSearch** (scope=\"hybrid\") / 先定位实现 → CodebaseSearch"
                    .to_string(),
            );
            lines.push("- **Browse directory structure** → **LS** / 浏览目录结构 → LS".to_string());
            lines.push(
                "- **Find symbols / locate files** → **CodebaseSearch** (scope=\"symbol\" or \"path\") / 查找符号、定位文件 → CodebaseSearch"
                    .to_string(),
            );
            if has_semantic {
                lines.push(
                    "- **Search by concept / natural language** → **CodebaseSearch** (scope=\"semantic\") / 自然语言搜索 → CodebaseSearch(semantic)"
                        .to_string(),
                );
            }
            lines.push(
                "- **Read specific files after narrowing the target set** → **Read** / 缩小范围后读取文件 → Read"
                    .to_string(),
            );
            lines.push(
                "- **Regex / exact string / full-text search only** → **Grep** / 仅在正则/精确字符串/全文匹配时使用 → Grep".to_string(),
            );
            lines.push(String::new());
            lines.push(
                "**Hard rule / 强规则**: Do not start with Grep for code discovery when CodebaseSearch is available."
                    .to_string(),
            );
            lines.push(
                "**Query tips / 查询技巧**: Use short, focused queries (1-2 keywords). 使用简短关键词（1-2个词）。"
                    .to_string(),
            );
        }
        _ => {
            lines.push(
                "**Analyze priority order**: Use CodebaseSearch first. Read comes after narrowing files. Grep is fallback only."
                    .to_string(),
            );
            lines.push(
                "分析任务优先顺序：先用 CodebaseSearch。Read 用于缩小范围后读取，Grep 仅作回退。".to_string(),
            );
            lines.push(String::new());
            lines.push(
                "- **Understand architecture / locate implementations** → **CodebaseSearch** (scope=\"hybrid\")"
                    .to_string(),
            );
            lines.push(
                "  理解架构、定位功能实现 → CodebaseSearch（综合搜索，比 Grep 更快更准确）"
                    .to_string(),
            );
            lines.push(
                "- **Find function/class/struct definitions** → **CodebaseSearch** (scope=\"symbol\")"
                    .to_string(),
            );
            lines.push("  按名称查找函数/类/结构体定义 → CodebaseSearch(symbol)".to_string());
            if has_semantic {
                lines.push(
                    "- **Conceptual / natural language search** → **CodebaseSearch** (scope=\"semantic\")"
                        .to_string(),
                );
                lines.push("  自然语言语义搜索 → CodebaseSearch(semantic)".to_string());
            }
            lines.push(
                "- **View file contents** → **Read** (only after locating files via CodebaseSearch)"
                    .to_string(),
            );
            lines.push(
                "  查看文件内容 → Read（先用 CodebaseSearch 定位，再用 Read 读取）".to_string(),
            );
            lines.push(
                "- **Exact identifier / regex / full-text search** → **Grep** (only when CodebaseSearch is insufficient)"
                    .to_string(),
            );
            lines.push("  精确标识符/正则搜索 → Grep（CodebaseSearch 不满足时使用）".to_string());
            lines.push("- **Find files by pattern** → **Glob** (e.g., `**/*.rs`)".to_string());
            lines.push("  按模式查找文件 → Glob".to_string());
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
            prompt.contains("CodebaseSearch first"),
            "System prompt should explicitly require CodebaseSearch-first behavior"
        );
        assert!(
            prompt.contains("Use **Grep** only for regex, exact-string, or full-text fallback"),
            "System prompt should constrain Grep to fallback usage"
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
    fn test_build_system_prompt_japanese_language() {
        let tools = get_tool_definitions_from_registry();
        let prompt = build_system_prompt(
            &PathBuf::from("/test"),
            &tools,
            None,
            "OpenAI",
            "gpt-4.1",
            "ja",
        );

        assert!(
            prompt.contains("respond in Japanese") || prompt.contains("respond in japanese"),
            "Japanese language instruction should be present"
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
        // More than 30% Han should detect as zh.
        assert_eq!(detect_language("帮我 fix 这个 bug"), "zh");
        // Empty string defaults to en
        assert_eq!(detect_language(""), "en");
    }

    #[test]
    fn test_detect_language_japanese() {
        assert_eq!(detect_language("このプロジェクトを分析してください"), "ja");
    }

    #[test]
    fn test_detect_language_korean() {
        assert_eq!(detect_language("이 프로젝트 구조를 분석해줘"), "ko");
    }

    // =========================================================================
    // Feature-001: Project Memory injection tests
    // =========================================================================

    fn make_test_memories() -> Vec<MemoryEntry> {
        vec![
            MemoryEntry {
                id: "mem-1".into(),
                project_path: "/test".into(),
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Preference,
                content: "Always use pnpm not npm".into(),
                keywords: vec!["pnpm".into()],
                importance: 0.9,
                access_count: 5,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-2".into(),
                project_path: "/test".into(),
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Convention,
                content: "Tests in __tests__/ directories".into(),
                keywords: vec!["tests".into()],
                importance: 0.7,
                access_count: 2,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-3".into(),
                project_path: "/test".into(),
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Pattern,
                content: "API routes return CommandResponse<T>".into(),
                keywords: vec!["api".into()],
                importance: 0.6,
                access_count: 1,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-4".into(),
                project_path: "/test".into(),
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Correction,
                content: "Do not edit executor.rs without cargo check".into(),
                keywords: vec!["executor".into()],
                importance: 0.8,
                access_count: 3,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
                created_at: String::new(),
                updated_at: String::new(),
                last_accessed_at: String::new(),
            },
            MemoryEntry {
                id: "mem-5".into(),
                project_path: "/test".into(),
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Fact,
                content: "Frontend uses Zustand for state management".into(),
                keywords: vec!["zustand".into()],
                importance: 0.5,
                access_count: 0,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
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

    #[test]
    fn test_build_system_prompt_includes_operating_contract_and_memory_policy() {
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

        assert!(prompt.contains("## Operating Contract"));
        assert!(prompt.contains("## Memory Policy"));
        assert!(prompt.contains("Never fabricate tool results"));
        assert!(prompt.contains("No Project Memory is injected for this session"));
    }

    #[test]
    fn test_build_system_prompt_memory_policy_mentions_injected_memories_when_present() {
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

        assert!(prompt.contains(
            "Project Memory is injected for this session. Treat it as persistent cross-session memory"
        ));
    }

    #[test]
    fn test_build_mode_addendum_covers_task_and_subagent_explore() {
        let task = build_mode_addendum("task", "en");
        let explore = build_mode_addendum("subagent_explore", "en");

        assert!(task.contains("Mode Addendum: Task"));
        assert!(task.contains("Use CodebaseSearch first"));
        assert!(explore.contains("Mode Addendum: Sub-Agent Explore"));
        assert!(explore.contains("use CodebaseSearch first"));
    }

    // =========================================================================
    // Sub-agent tool guidance tests
    // =========================================================================

    #[test]
    fn test_build_sub_agent_tool_guidance_with_index() {
        let guidance = build_sub_agent_tool_guidance(true, true, false, None);

        assert!(
            guidance.contains("CodebaseSearch"),
            "Should mention CodebaseSearch when index is available"
        );
        assert!(
            guidance.contains("scope=\"hybrid\""),
            "Should recommend scope=hybrid"
        );
        assert!(guidance.contains("Grep"), "Should mention Grep as fallback");
        assert!(
            !guidance.contains("scope=\"all\""),
            "Should not mention invalid all scope"
        );
        assert!(
            !guidance.contains("scope=\"files\""),
            "Should not mention invalid files scope"
        );
        // No semantic search
        assert!(
            !guidance.contains("semantic"),
            "Should not mention semantic when no embeddings"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_with_semantic() {
        let guidance = build_sub_agent_tool_guidance(true, true, true, None);

        assert!(
            guidance.contains("scope=\"semantic\""),
            "Should mention semantic scope when embeddings available"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_without_index() {
        let guidance = build_sub_agent_tool_guidance(false, false, false, None);

        assert!(
            guidance.contains("unavailable"),
            "Should explain fallback when no index is available"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_without_index_but_semantic() {
        // Edge case: semantic flag without index — should still fall back cleanly
        let guidance = build_sub_agent_tool_guidance(false, false, true, None);

        assert!(
            guidance.contains("Codebase index status: unavailable"),
            "Should explain fallback when no index, even with semantic flag"
        );
    }

    #[test]
    fn test_build_sub_agent_tool_guidance_explore_allows_ls() {
        let guidance = build_sub_agent_tool_guidance(true, true, false, Some("explore"));

        assert!(
            guidance.contains("LS"),
            "Explore guidance should mention LS"
        );
        assert!(
            guidance.contains("Do not start with Grep"),
            "Explore guidance should explicitly prohibit Grep-first exploration"
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
        let guidance = build_sub_agent_tool_guidance(true, true, false, Some("analyze"));

        assert!(
            guidance.contains("priority order"),
            "Analyze guidance should maintain priority order"
        );
        assert!(
            guidance.contains("Use CodebaseSearch first"),
            "Analyze guidance should require CodebaseSearch before fallback tools"
        );
    }

    #[test]
    fn test_build_tool_priority_section_codebase_ready_is_strict() {
        let section = build_tool_priority_section(false, true, true, false, "en");
        assert!(section.contains("Codebase index: **ready**"));
        assert!(section.contains("MUST use CodebaseSearch first"));
        assert!(section.contains("Do NOT use Grep to scan files"));
    }

    #[test]
    fn test_build_tool_priority_section_codebase_unavailable_falls_back() {
        let section = build_tool_priority_section(false, true, false, false, "en");
        assert!(section.contains("Codebase index: **unavailable**"));
        assert!(section.contains("Do not try CodebaseSearch"));
        assert!(section.contains("Use Grep / Read / Glob / LS directly"));
    }

    #[test]
    fn test_build_system_prompt_decision_tree_prefers_codebase_search_over_grep() {
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
            prompt.contains("| \"Find where function X is defined\" / \"What files are in component Y?\" | **CodebaseSearch** |"),
            "Decision table should use CodebaseSearch directly for code location questions"
        );
        assert!(
            !prompt.contains("CodebaseSearch** (preferred) or **Grep"),
            "Decision table should not keep the old ambiguous Grep-first wording"
        );
        assert!(
            prompt.contains("Use **Grep** only for regex, exact-string, or full-text fallback"),
            "System prompt should constrain Grep to fallback usage"
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
                scope: Some("project".into()),
                session_id: None,
                category: MemoryCategory::Fact,
                content: format!("This is a moderately long memory entry number {} that contributes to the total character count of the section", i),
                keywords: vec![],
                importance: 0.5,
                access_count: 0,
                source_session_id: None,
                source_context: None,
                status: Some("active".into()),
                risk_tier: Some("high".into()),
                conflict_flag: Some(false),
                trace_id: None,
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
