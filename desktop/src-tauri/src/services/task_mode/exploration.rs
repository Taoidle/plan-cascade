//! Project Exploration for Task Mode PRD Generation
//!
//! Provides a project exploration phase between configuration/interview and PRD generation.
//! Gathers project context (tech stack, key files, components, patterns) to improve PRD quality.
//!
//! Two exploration levels:
//! - **Deterministic** (standard flow): Extracts project summary from IndexStore
//! - **LLM-assisted** (full flow): Runs a coordinator OrchestratorService that spawns
//!   parallel Task(explore) sub-agents to search the codebase

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::services::orchestrator::index_store::ProjectIndexSummary;
use crate::services::persona::{PersonaRegistry, PersonaRole};

// ============================================================================
// Types
// ============================================================================

/// Result of the project exploration phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorationResult {
    /// Detected technology stack
    pub tech_stack: TechStackSummary,
    /// Key files relevant to the task
    pub key_files: Vec<KeyFileEntry>,
    /// Discovered project components/modules
    pub components: Vec<ComponentSummary>,
    /// Detected code patterns and conventions
    pub patterns: Vec<String>,
    /// LLM-generated exploration summary (None for deterministic-only)
    pub llm_summary: Option<String>,
    /// Quality label for the summary completeness
    pub summary_quality: SummaryQuality,
    /// Source category for the summary text
    pub summary_source: SummarySource,
    /// Additional notes about summary generation/fallback behavior
    pub summary_notes: Option<String>,
    /// Total exploration duration in milliseconds
    pub duration_ms: u64,
    /// Whether LLM exploration was used (true for full flow)
    pub used_llm_exploration: bool,
}

/// Completeness quality of an exploration summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryQuality {
    Complete,
    Partial,
    Empty,
}

/// Source kind for exploration summary text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummarySource {
    Llm,
    FallbackSynthesized,
    DeterministicOnly,
}

/// Technology stack summary extracted from project analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechStackSummary {
    /// Programming languages detected
    pub languages: Vec<String>,
    /// Frameworks detected (e.g., React, Tauri, Express)
    pub frameworks: Vec<String>,
    /// Build tools detected (e.g., cargo, pnpm, webpack)
    pub build_tools: Vec<String>,
    /// Test frameworks detected (e.g., vitest, pytest, cargo test)
    pub test_frameworks: Vec<String>,
    /// Primary package manager (e.g., pnpm, npm, cargo)
    pub package_manager: Option<String>,
}

/// A key file entry discovered during exploration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyFileEntry {
    /// File path relative to project root
    pub path: String,
    /// File type classification
    pub file_type: String, // "config" | "entry_point" | "model" | "service" | "test"
    /// Why this file is relevant
    pub relevance: String,
}

/// A component/module discovered in the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentSummary {
    /// Component name
    pub name: String,
    /// Component root path
    pub path: String,
    /// Brief description
    pub description: String,
    /// Number of files in this component
    pub file_count: usize,
}

// ============================================================================
// Deterministic Exploration
// ============================================================================

/// Extract project context from IndexStore's project summary.
///
/// This is a fast, deterministic exploration that doesn't require LLM calls.
/// Used for both standard and full flow levels.
pub fn deterministic_explore(
    project_summary: &ProjectIndexSummary,
    project_root: &Path,
) -> ExplorationResult {
    let start = std::time::Instant::now();

    // Extract tech stack from languages
    let tech_stack = detect_tech_stack(&project_summary.languages, project_root);

    // Convert IndexStore components to our format
    let components: Vec<ComponentSummary> = project_summary
        .components
        .iter()
        .map(|c| ComponentSummary {
            name: c.name.clone(),
            path: c.name.clone(), // component name is typically the directory name
            description: format!("{} files", c.count),
            file_count: c.count,
        })
        .collect();

    // Identify key files from entry points
    let key_files: Vec<KeyFileEntry> = project_summary
        .key_entry_points
        .iter()
        .map(|path| {
            let file_type = classify_file(path);
            KeyFileEntry {
                path: path.clone(),
                file_type,
                relevance: "entry point".to_string(),
            }
        })
        .collect();

    // Detect patterns from project structure
    let patterns = detect_patterns(project_summary, project_root);

    let duration = start.elapsed();

    ExplorationResult {
        tech_stack,
        key_files,
        components,
        patterns,
        llm_summary: None,
        summary_quality: SummaryQuality::Empty,
        summary_source: SummarySource::DeterministicOnly,
        summary_notes: None,
        duration_ms: duration.as_millis() as u64,
        used_llm_exploration: false,
    }
}

/// Detect technology stack from languages and project files.
fn detect_tech_stack(languages: &[String], project_root: &Path) -> TechStackSummary {
    let mut frameworks = Vec::new();
    let mut build_tools = Vec::new();
    let mut test_frameworks = Vec::new();
    let mut package_manager = None;

    // Detect from project files
    if project_root.join("package.json").exists() {
        if project_root.join("pnpm-lock.yaml").exists() {
            package_manager = Some("pnpm".to_string());
            build_tools.push("pnpm".to_string());
        } else if project_root.join("yarn.lock").exists() {
            package_manager = Some("yarn".to_string());
            build_tools.push("yarn".to_string());
        } else if project_root.join("package-lock.json").exists() {
            package_manager = Some("npm".to_string());
            build_tools.push("npm".to_string());
        }
    }

    if project_root.join("Cargo.toml").exists() {
        build_tools.push("cargo".to_string());
        if package_manager.is_none() {
            package_manager = Some("cargo".to_string());
        }
    }

    if project_root.join("pyproject.toml").exists() || project_root.join("setup.py").exists() {
        if project_root.join("uv.lock").exists() {
            package_manager = Some("uv".to_string());
            build_tools.push("uv".to_string());
        } else {
            build_tools.push("pip".to_string());
        }
    }

    // Detect frameworks from common files
    if project_root.join("vite.config.ts").exists() || project_root.join("vite.config.js").exists()
    {
        build_tools.push("vite".to_string());
    }
    if project_root.join("next.config.js").exists()
        || project_root.join("next.config.mjs").exists()
        || project_root.join("next.config.ts").exists()
    {
        frameworks.push("Next.js".to_string());
    }
    if project_root.join("nuxt.config.ts").exists() || project_root.join("nuxt.config.js").exists()
    {
        frameworks.push("Nuxt".to_string());
    }
    if project_root.join("tauri.conf.json").exists() || project_root.join("src-tauri").exists() {
        frameworks.push("Tauri".to_string());
    }

    // Detect test frameworks
    if project_root.join("vitest.config.ts").exists()
        || project_root.join("vitest.config.js").exists()
    {
        test_frameworks.push("vitest".to_string());
    }
    if project_root.join("jest.config.ts").exists() || project_root.join("jest.config.js").exists()
    {
        test_frameworks.push("jest".to_string());
    }
    if project_root.join("pytest.ini").exists() || project_root.join("pyproject.toml").exists() {
        // Check for pytest in languages context
        if languages.iter().any(|l| l.to_lowercase() == "python") {
            test_frameworks.push("pytest".to_string());
        }
    }

    // Detect React/Vue from language context
    if languages
        .iter()
        .any(|l| l.to_lowercase().contains("tsx") || l.to_lowercase().contains("jsx"))
    {
        if !frameworks.iter().any(|f| f == "Next.js") {
            frameworks.push("React".to_string());
        }
    }
    if languages.iter().any(|l| l.to_lowercase().contains("vue")) {
        if !frameworks.iter().any(|f| f == "Nuxt") {
            frameworks.push("Vue".to_string());
        }
    }

    TechStackSummary {
        languages: languages.to_vec(),
        frameworks,
        build_tools,
        test_frameworks,
        package_manager,
    }
}

/// Classify a file path into a type category.
fn classify_file(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains("test") || lower.contains("spec") {
        "test".to_string()
    } else if lower.contains("config")
        || lower.ends_with(".toml")
        || lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
    {
        "config".to_string()
    } else if lower.contains("main")
        || lower.contains("index")
        || lower.contains("app")
        || lower.contains("entry")
        || lower.contains("mod.rs")
        || lower.contains("lib.rs")
    {
        "entry_point".to_string()
    } else if lower.contains("model") || lower.contains("schema") || lower.contains("types") {
        "model".to_string()
    } else if lower.contains("service")
        || lower.contains("controller")
        || lower.contains("handler")
        || lower.contains("api")
    {
        "service".to_string()
    } else {
        "source".to_string()
    }
}

/// Detect patterns from project structure.
fn detect_patterns(summary: &ProjectIndexSummary, project_root: &Path) -> Vec<String> {
    let mut patterns = Vec::new();

    if summary.total_files > 50 {
        patterns.push(format!(
            "Large project with {} files and {} symbols",
            summary.total_files, summary.total_symbols
        ));
    }

    if summary.components.len() > 5 {
        patterns.push(format!(
            "Modular architecture with {} components",
            summary.components.len()
        ));
    }

    if summary.embedding_chunks > 0 {
        patterns.push("Semantic search index available".to_string());
    }

    // Detect monorepo patterns
    if project_root.join("packages").is_dir()
        || project_root.join("crates").is_dir()
        || project_root.join("apps").is_dir()
    {
        patterns.push("Monorepo/workspace structure".to_string());
    }

    // Detect CI/CD
    if project_root.join(".github/workflows").is_dir() {
        patterns.push("GitHub Actions CI/CD".to_string());
    }

    patterns
}

// ============================================================================
// LLM Exploration (Coordinator Prompt)
// ============================================================================

/// Build the system prompt for the exploration coordinator.
///
/// The coordinator LLM will analyze the task description combined with deterministic
/// exploration results, then spawn parallel Task(explore) sub-agents to search the
/// codebase for task-relevant information.
pub fn build_coordinator_exploration_prompt(
    task_description: &str,
    deterministic_result: &ExplorationResult,
    locale: Option<&str>,
) -> String {
    let tech_stack_info = format!(
        "Languages: {}\nFrameworks: {}\nBuild tools: {}\nTest frameworks: {}\nPackage manager: {}",
        deterministic_result.tech_stack.languages.join(", "),
        deterministic_result.tech_stack.frameworks.join(", "),
        deterministic_result.tech_stack.build_tools.join(", "),
        deterministic_result.tech_stack.test_frameworks.join(", "),
        deterministic_result
            .tech_stack
            .package_manager
            .as_deref()
            .unwrap_or("unknown"),
    );

    let components_info: String = deterministic_result
        .components
        .iter()
        .map(|c| format!("  - {} ({} files)", c.name, c.file_count))
        .collect::<Vec<_>>()
        .join("\n");

    let key_files_info: String = deterministic_result
        .key_files
        .iter()
        .take(20) // limit to prevent prompt bloat
        .map(|f| format!("  - {} [{}]", f.path, f.file_type))
        .collect::<Vec<_>>()
        .join("\n");

    let persona = PersonaRegistry::get(PersonaRole::SeniorEngineer);

    let locale_instruction = match locale.unwrap_or("en") {
        "zh" => {
            "Language Requirement:\n- Write the final exploration summary in Simplified Chinese.\n- Keep code symbols, file paths, and identifiers in original form."
        }
        "ja" => {
            "Language Requirement:\n- Write the final exploration summary in Japanese.\n- Keep code symbols, file paths, and identifiers in original form."
        }
        _ => {
            "Language Requirement:\n- Write the final exploration summary in English.\n- Keep code symbols, file paths, and identifiers in original form."
        }
    };

    format!(
        r#"{persona_prompt}

---

Your job is to explore the codebase to gather context that will help generate a high-quality PRD (Product Requirements Document) for the user's task.

## Task Description
{task_description}

## {locale_instruction}

## Known Project Context
{tech_stack_info}

### Components
{components_info}

### Key Files
{key_files_info}

## Your Mission
1. Analyze the task description and the known project context above.
2. Identify 2-4 specific search dimensions relevant to this task. For example:
   - Search for existing code related to the task's domain (e.g., authentication modules, API endpoints)
   - Analyze project structure patterns (e.g., how services are organized, naming conventions)
   - Find relevant test patterns for the task domain
   - Identify dependencies and interfaces the task will need to interact with
3. Use the available tools (Read, Glob, Grep, CodebaseSearch) to explore each dimension.
4. After exploring, provide a structured summary in the following format:

## Exploration Summary
### Relevant Existing Code
- List key files and modules related to the task
- Note existing patterns and conventions

### Architecture Insights
- How the project is structured
- Key interfaces and dependencies

### Recommendations for PRD
- What the PRD should account for based on the codebase
- Potential risks or considerations
- Existing code that can be reused or extended

IMPORTANT:
- This is a READ-ONLY exploration. Do NOT modify any files.
- Focus on gathering context that will make the PRD more accurate and aligned with the existing codebase.
- Be thorough but efficient — explore the most relevant areas first."#,
        persona_prompt = persona.identity_prompt
    )
}

/// Extract the exploration summary from the coordinator's output.
///
/// Looks for the structured summary section, falls back to the full output.
pub fn parse_coordinator_summary(coordinator_output: &str) -> Option<String> {
    if coordinator_output.trim().is_empty() {
        return None;
    }

    let markers = [
        "## Exploration Summary",
        "# Exploration Summary",
        "## AI Exploration Summary",
        "# AI Exploration Summary",
        "## 探索摘要",
        "## 探索总结",
        "# 探索摘要",
        "# 探索总结",
        "## 探索サマリー",
        "## 調査サマリー",
        "# 探索サマリー",
        "# 調査サマリー",
    ];

    for marker in markers {
        if let Some(idx) = coordinator_output.find(marker) {
            return Some(coordinator_output[idx..].trim().to_string());
        }
    }

    // Use the full output as summary
    Some(coordinator_output.trim().to_string())
}

/// Heuristic check for LLM summary quality.
///
/// Returns `true` when the summary looks incomplete (very short, no structure,
/// or likely an unfinished opening sentence).
pub fn is_summary_incomplete(summary: &str) -> bool {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return true;
    }

    let char_len = trimmed.chars().count();
    let non_empty_lines = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let has_sections = trimmed.contains("##") || trimmed.contains("###");
    let has_bullets = trimmed.contains("\n- ") || trimmed.contains("\n• ");
    let ends_abruptly = trimmed.ends_with(':') || trimmed.ends_with('：');

    if char_len < 140 || non_empty_lines <= 1 {
        return true;
    }

    if !has_sections && !has_bullets && char_len < 260 {
        return true;
    }

    ends_abruptly
}

/// Build a deterministic fallback exploration summary when LLM output is missing
/// or appears incomplete.
pub fn synthesize_summary_from_deterministic(
    task_description: &str,
    deterministic_result: &ExplorationResult,
    locale: Option<&str>,
) -> Option<String> {
    let locale_tag = locale.unwrap_or("en").to_lowercase();

    let key_files = deterministic_result
        .key_files
        .iter()
        .take(8)
        .map(|f| format!("- `{}` [{}]: {}", f.path, f.file_type, f.relevance))
        .collect::<Vec<_>>();
    let components = deterministic_result
        .components
        .iter()
        .take(8)
        .map(|c| format!("- {} ({} files): {}", c.name, c.file_count, c.description))
        .collect::<Vec<_>>();
    let patterns = deterministic_result
        .patterns
        .iter()
        .take(8)
        .map(|p| format!("- {}", p))
        .collect::<Vec<_>>();

    let languages = if deterministic_result.tech_stack.languages.is_empty() {
        "unknown".to_string()
    } else {
        deterministic_result.tech_stack.languages.join(", ")
    };
    let frameworks = if deterministic_result.tech_stack.frameworks.is_empty() {
        "none".to_string()
    } else {
        deterministic_result.tech_stack.frameworks.join(", ")
    };
    let package_manager = deterministic_result
        .tech_stack
        .package_manager
        .as_deref()
        .unwrap_or("unknown");

    let summary = if locale_tag.starts_with("zh") {
        format!(
            "## 探索摘要\n\
             ### 相关现有代码\n\
             任务目标：{task}\n\
             {key_files}\n\n\
             ### 架构洞察\n\
             - 语言栈：{languages}\n\
             - 框架：{frameworks}\n\
             - 组件结构：\n\
             {components}\n\
             - 识别到的模式：\n\
             {patterns}\n\n\
             ### 对 PRD 的建议\n\
             - 对齐现有组件边界与命名约定，避免重复抽象。\n\
             - 明确依赖与接口改动范围，优先复用已有模块。\n\
             - 在验收标准中补充测试与错误处理要求。\n\
             - 包管理/构建基线：{package_manager}",
            task = task_description,
            key_files = if key_files.is_empty() {
                "- 暂未识别到关键文件".to_string()
            } else {
                key_files.join("\n")
            },
            components = if components.is_empty() {
                "- 暂未识别到组件".to_string()
            } else {
                components.join("\n")
            },
            patterns = if patterns.is_empty() {
                "- 暂未识别到明显模式".to_string()
            } else {
                patterns.join("\n")
            },
        )
    } else if locale_tag.starts_with("ja") {
        format!(
            "## 探索サマリー\n\
             ### 関連する既存コード\n\
             タスク目的: {task}\n\
             {key_files}\n\n\
             ### アーキテクチャ洞察\n\
             - 言語: {languages}\n\
             - フレームワーク: {frameworks}\n\
             - コンポーネント構造:\n\
             {components}\n\
             - 検出されたパターン:\n\
             {patterns}\n\n\
             ### PRDへの提案\n\
             - 既存コンポーネント境界と命名規約に合わせる。\n\
             - 依存関係とインターフェース変更範囲を明確化する。\n\
             - 受け入れ基準にテストとエラーハンドリングを含める。\n\
             - パッケージ管理/ビルド基盤: {package_manager}",
            task = task_description,
            key_files = if key_files.is_empty() {
                "- 主要ファイルは未検出".to_string()
            } else {
                key_files.join("\n")
            },
            components = if components.is_empty() {
                "- コンポーネントは未検出".to_string()
            } else {
                components.join("\n")
            },
            patterns = if patterns.is_empty() {
                "- 明確なパターンは未検出".to_string()
            } else {
                patterns.join("\n")
            },
        )
    } else {
        format!(
            "## Exploration Summary\n\
             ### Relevant Existing Code\n\
             Task target: {task}\n\
             {key_files}\n\n\
             ### Architecture Insights\n\
             - Languages: {languages}\n\
             - Frameworks: {frameworks}\n\
             - Component structure:\n\
             {components}\n\
             - Detected patterns:\n\
             {patterns}\n\n\
             ### Recommendations for PRD\n\
             - Align stories to existing component boundaries and naming conventions.\n\
             - Call out dependency/interface changes explicitly in story scope.\n\
             - Add testing and error-handling requirements to acceptance criteria.\n\
             - Package/build baseline: {package_manager}",
            task = task_description,
            key_files = if key_files.is_empty() {
                "- No key files identified yet".to_string()
            } else {
                key_files.join("\n")
            },
            components = if components.is_empty() {
                "- No components identified yet".to_string()
            } else {
                components.join("\n")
            },
            patterns = if patterns.is_empty() {
                "- No obvious patterns identified yet".to_string()
            } else {
                patterns.join("\n")
            },
        )
    };

    Some(summary)
}

// ============================================================================
// Context Formatting for PRD Injection
// ============================================================================

/// Format exploration result as a context block for PRD generation.
///
/// Produces a `[PROJECT CONTEXT]` text block that is injected into the LLM conversation
/// before the PRD generation request, giving the LLM awareness of the actual codebase.
pub fn format_exploration_context(result: &ExplorationResult) -> String {
    let mut parts = Vec::new();

    parts.push("[PROJECT CONTEXT]".to_string());
    parts.push(String::new());

    // Tech stack
    parts.push("## Technology Stack".to_string());
    if !result.tech_stack.languages.is_empty() {
        parts.push(format!(
            "Languages: {}",
            result.tech_stack.languages.join(", ")
        ));
    }
    if !result.tech_stack.frameworks.is_empty() {
        parts.push(format!(
            "Frameworks: {}",
            result.tech_stack.frameworks.join(", ")
        ));
    }
    if !result.tech_stack.build_tools.is_empty() {
        parts.push(format!(
            "Build tools: {}",
            result.tech_stack.build_tools.join(", ")
        ));
    }
    if !result.tech_stack.test_frameworks.is_empty() {
        parts.push(format!(
            "Test frameworks: {}",
            result.tech_stack.test_frameworks.join(", ")
        ));
    }
    if let Some(ref pm) = result.tech_stack.package_manager {
        parts.push(format!("Package manager: {}", pm));
    }
    parts.push(String::new());

    // Components
    if !result.components.is_empty() {
        parts.push("## Project Components".to_string());
        for comp in &result.components {
            parts.push(format!(
                "- **{}** ({}): {} files",
                comp.name, comp.path, comp.file_count
            ));
        }
        parts.push(String::new());
    }

    // Key files
    if !result.key_files.is_empty() {
        parts.push("## Key Files".to_string());
        for file in &result.key_files {
            parts.push(format!(
                "- `{}` [{}]: {}",
                file.path, file.file_type, file.relevance
            ));
        }
        parts.push(String::new());
    }

    // Patterns
    if !result.patterns.is_empty() {
        parts.push("## Detected Patterns".to_string());
        for pattern in &result.patterns {
            parts.push(format!("- {}", pattern));
        }
        parts.push(String::new());
    }

    // LLM summary
    if let Some(ref summary) = result.llm_summary {
        parts.push("## AI Exploration Summary".to_string());
        parts.push(summary.clone());
        parts.push(format!(
            "Summary quality: {:?}, source: {:?}",
            result.summary_quality, result.summary_source
        ));
        if let Some(notes) = &result.summary_notes {
            parts.push(format!("Summary notes: {}", notes));
        }
        parts.push(String::new());
    }

    parts.push("[/PROJECT CONTEXT]".to_string());

    parts.join("\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::orchestrator::index_store::{
        ComponentSummary as IndexComponentSummary, ProjectIndexSummary,
    };

    fn sample_summary() -> ProjectIndexSummary {
        ProjectIndexSummary {
            total_files: 120,
            languages: vec![
                "Rust".to_string(),
                "TypeScript".to_string(),
                "TSX".to_string(),
            ],
            components: vec![
                IndexComponentSummary {
                    name: "src-tauri/src/commands".to_string(),
                    count: 15,
                },
                IndexComponentSummary {
                    name: "src-tauri/src/services".to_string(),
                    count: 30,
                },
                IndexComponentSummary {
                    name: "src/components".to_string(),
                    count: 25,
                },
                IndexComponentSummary {
                    name: "src/store".to_string(),
                    count: 10,
                },
            ],
            key_entry_points: vec![
                "src-tauri/src/main.rs".to_string(),
                "src/main.tsx".to_string(),
                "package.json".to_string(),
            ],
            total_symbols: 500,
            embedding_chunks: 200,
        }
    }

    #[test]
    fn test_deterministic_explore_basic() {
        let summary = sample_summary();
        let temp_dir = std::env::temp_dir();
        let result = deterministic_explore(&summary, &temp_dir);

        assert_eq!(result.tech_stack.languages.len(), 3);
        assert!(!result.components.is_empty());
        assert!(!result.key_files.is_empty());
        assert!(!result.used_llm_exploration);
        assert!(result.llm_summary.is_none());
    }

    #[test]
    fn test_format_exploration_context_contains_markers() {
        let summary = sample_summary();
        let temp_dir = std::env::temp_dir();
        let result = deterministic_explore(&summary, &temp_dir);
        let context = format_exploration_context(&result);

        assert!(context.starts_with("[PROJECT CONTEXT]"));
        assert!(context.contains("[/PROJECT CONTEXT]"));
        assert!(context.contains("## Technology Stack"));
        assert!(context.contains("Rust"));
    }

    #[test]
    fn test_format_exploration_context_with_llm_summary() {
        let mut result = ExplorationResult {
            tech_stack: TechStackSummary {
                languages: vec!["Rust".to_string()],
                frameworks: vec![],
                build_tools: vec!["cargo".to_string()],
                test_frameworks: vec![],
                package_manager: Some("cargo".to_string()),
            },
            key_files: vec![],
            components: vec![],
            patterns: vec![],
            llm_summary: Some("The project uses a service-oriented architecture.".to_string()),
            summary_quality: SummaryQuality::Complete,
            summary_source: SummarySource::Llm,
            summary_notes: None,
            duration_ms: 100,
            used_llm_exploration: true,
        };

        let context = format_exploration_context(&result);
        assert!(context.contains("## AI Exploration Summary"));
        assert!(context.contains("service-oriented architecture"));

        // Without LLM summary
        result.llm_summary = None;
        let context2 = format_exploration_context(&result);
        assert!(!context2.contains("## AI Exploration Summary"));
    }

    #[test]
    fn test_classify_file() {
        assert_eq!(classify_file("src/main.rs"), "entry_point");
        assert_eq!(classify_file("tests/test_prd.rs"), "test");
        assert_eq!(classify_file("Cargo.toml"), "config");
        assert_eq!(classify_file("src/services/auth.rs"), "service");
        assert_eq!(classify_file("src/models/user.rs"), "model");
        assert_eq!(classify_file("src/utils/helpers.rs"), "source");
    }

    #[test]
    fn test_parse_coordinator_summary() {
        // With summary section
        let output = "Some preamble\n\n## Exploration Summary\n\nThis project...";
        let summary = parse_coordinator_summary(output);
        assert!(summary.is_some());
        assert!(summary.unwrap().starts_with("## Exploration Summary"));

        // Without summary section
        let output2 = "Just some exploration notes about the project.";
        let summary2 = parse_coordinator_summary(output2);
        assert!(summary2.is_some());
        assert_eq!(summary2.unwrap(), output2);

        // Empty
        assert!(parse_coordinator_summary("").is_none());
        assert!(parse_coordinator_summary("   ").is_none());
    }

    #[test]
    fn test_is_summary_incomplete() {
        assert!(is_summary_incomplete("Let me explore this project:"));
        assert!(!is_summary_incomplete(
            "## Exploration Summary\n### Relevant Existing Code\n- src/main.rs\n### Architecture Insights\n- modular structure\n### Recommendations for PRD\n- add tests"
        ));
    }

    #[test]
    fn test_synthesize_summary_from_deterministic() {
        let result = ExplorationResult {
            tech_stack: TechStackSummary {
                languages: vec!["Rust".to_string()],
                frameworks: vec!["Tauri".to_string()],
                build_tools: vec!["cargo".to_string()],
                test_frameworks: vec![],
                package_manager: Some("cargo".to_string()),
            },
            key_files: vec![KeyFileEntry {
                path: "src/main.rs".to_string(),
                file_type: "entry_point".to_string(),
                relevance: "entry point".to_string(),
            }],
            components: vec![ComponentSummary {
                name: "services".to_string(),
                path: "src/services".to_string(),
                description: "business logic".to_string(),
                file_count: 12,
            }],
            patterns: vec!["Modular architecture".to_string()],
            llm_summary: None,
            summary_quality: SummaryQuality::Empty,
            summary_source: SummarySource::DeterministicOnly,
            summary_notes: None,
            duration_ms: 0,
            used_llm_exploration: false,
        };

        let synthesized = synthesize_summary_from_deterministic("Add auth", &result, Some("en"));
        assert!(synthesized.is_some());
        let text = synthesized.unwrap();
        assert!(text.contains("Exploration Summary"));
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("Rust"));
    }

    #[test]
    fn test_build_coordinator_prompt_contains_task() {
        let result = ExplorationResult {
            tech_stack: TechStackSummary {
                languages: vec!["Rust".to_string()],
                frameworks: vec!["Tauri".to_string()],
                build_tools: vec!["cargo".to_string()],
                test_frameworks: vec![],
                package_manager: Some("cargo".to_string()),
            },
            key_files: vec![KeyFileEntry {
                path: "src/main.rs".to_string(),
                file_type: "entry_point".to_string(),
                relevance: "main entry".to_string(),
            }],
            components: vec![ComponentSummary {
                name: "services".to_string(),
                path: "src/services".to_string(),
                description: "Business logic".to_string(),
                file_count: 20,
            }],
            patterns: vec![],
            llm_summary: None,
            summary_quality: SummaryQuality::Empty,
            summary_source: SummarySource::DeterministicOnly,
            summary_notes: None,
            duration_ms: 0,
            used_llm_exploration: false,
        };

        let prompt = build_coordinator_exploration_prompt("Add user authentication", &result, Some("en"));
        assert!(prompt.contains("Add user authentication"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("Tauri"));
        assert!(prompt.contains("services"));
        assert!(prompt.contains("READ-ONLY"));
    }
}
