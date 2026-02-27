//! Memory Extraction and Explicit Commands
//!
//! LLM-driven memory extraction from session data and explicit memory command
//! detection (remember/forget/query patterns).
//!
//! ## Explicit Memory Commands
//!
//! Detect user intent patterns and bypass LLM extraction:
//! - Create: "remember that...", "always use...", "never do...", "note that..."
//! - Delete: "forget that...", "stop remembering...", "delete memory about..."
//! - Query:  "what do you remember about...", "what are my preferences..."

use crate::services::memory::store::{MemoryCategory, MemoryEntry, NewMemoryEntry, GLOBAL_PROJECT_PATH};

/// Explicit memory commands detected from user messages
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryCommand {
    Remember { content: String },
    Forget { query: String },
    Query { query: String },
}

/// Detect explicit memory commands in user messages.
///
/// Patterns detected:
/// - Create: "remember that...", "always use...", "never do...", "note that...",
///           "keep in mind that...", "don't forget that..."
/// - Delete: "forget that...", "stop remembering...", "delete memory about...",
///           "remove the memory about..."
/// - Query:  "what do you remember about...", "what are my preferences...",
///           "do you remember...", "what do you know about..."
///
/// These bypass LLM extraction and directly create/modify memory entries
/// with importance = 0.95 (explicit user instruction).
///
/// Returns None if no memory command detected.
pub fn detect_memory_command(user_message: &str) -> Option<MemoryCommand> {
    let lower = user_message.trim().to_lowercase();

    // --- Forget patterns (check first to avoid matching "remember" in "stop remembering") ---
    let forget_patterns = [
        "forget that ",
        "forget about ",
        "stop remembering ",
        "delete memory about ",
        "delete the memory about ",
        "remove memory about ",
        "remove the memory about ",
    ];

    for pattern in &forget_patterns {
        if let Some(rest) = lower.strip_prefix(pattern) {
            let query = rest.trim().trim_end_matches('.').to_string();
            if !query.is_empty() {
                return Some(MemoryCommand::Forget { query });
            }
        }
    }

    // --- Query patterns ---
    let query_patterns = [
        "what do you remember about ",
        "what do you remember ",
        "what are my preferences",
        "what do you know about ",
        "do you remember ",
        "show me what you remember",
        "list my memories",
        "what memories do you have",
    ];

    for pattern in &query_patterns {
        if lower.starts_with(pattern) {
            let rest = &lower[pattern.len()..];
            let query = rest
                .trim()
                .trim_end_matches('?')
                .trim_end_matches('.')
                .to_string();
            // For patterns like "what are my preferences" that may have no additional text
            return Some(MemoryCommand::Query {
                query: if query.is_empty() {
                    pattern.trim().to_string()
                } else {
                    query
                },
            });
        }
    }

    // --- Remember patterns ---
    let remember_patterns = [
        "remember that ",
        "remember: ",
        "always use ",
        "always ",
        "never do ",
        "never use ",
        "never ",
        "note that ",
        "note: ",
        "keep in mind that ",
        "keep in mind: ",
        "don't forget that ",
        "don't forget: ",
    ];

    for pattern in &remember_patterns {
        if let Some(rest) = lower.strip_prefix(pattern) {
            let content = rest.trim().trim_end_matches('.').to_string();
            if !content.is_empty() {
                return Some(MemoryCommand::Remember { content });
            }
        }
    }

    None
}

/// LLM-driven memory extractor
pub struct MemoryExtractor;

impl MemoryExtractor {
    /// Character threshold above which a conversation should be summarized
    /// before memory extraction. ~6000 chars ≈ 2000 tokens.
    pub const SUMMARIZE_THRESHOLD: usize = 6000;

    /// Build a focused summarization prompt for long conversations.
    ///
    /// Instead of a generic summary, this prompt guides the LLM to extract
    /// specific categories of information that are most valuable for
    /// cross-session memory persistence.
    pub fn build_summarization_prompt(
        task_description: &str,
        conversation_content: &str,
    ) -> String {
        format!(
            r#"You are a conversation analyst. Your job is to distill a long development session into a focused summary that captures information worth remembering across sessions.

## Original Task
{task}

## Full Conversation
{conversation}

---

Produce a focused summary (under 2000 characters) that prioritizes the following categories. Only include categories where the conversation provides clear evidence:

1. **User Preferences**: Communication language (e.g., prefers Chinese/Japanese/English), coding style preferences, tool preferences (e.g., "use pnpm not npm"), formatting preferences
2. **Project Tech Stack**: Programming languages, frameworks, key libraries, build tools, package managers discovered or confirmed
3. **Architecture & Patterns**: Key architectural decisions, design patterns in use, module organization, API conventions
4. **Workflow Conventions**: Testing approach, branch naming, commit style, CI/CD patterns, directory structure conventions
5. **Corrections & Pitfalls**: Mistakes encountered, things that don't work, approaches that were abandoned and why
6. **Key Decisions Made**: Important choices during the session, trade-offs considered, rationale for decisions

Format as a structured list with category headers. Omit categories with no relevant information. Be concise and factual — no filler text."#,
            task = task_description,
            conversation = conversation_content,
        )
    }

    /// Build the extraction prompt for the LLM.
    ///
    /// This prompt instructs the LLM to analyze session data and extract
    /// structured memories for cross-session persistence.
    pub fn build_extraction_prompt(
        task_description: &str,
        files_read: &[String],
        key_findings: &[String],
        conversation_summary: &str,
        existing_memories: &[MemoryEntry],
    ) -> String {
        let files_section = if files_read.is_empty() {
            "(none)".to_string()
        } else {
            files_read
                .iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let findings_section = if key_findings.is_empty() {
            "(none)".to_string()
        } else {
            key_findings
                .iter()
                .map(|f| format!("- {}", f))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let existing_section = if existing_memories.is_empty() {
            "(none)".to_string()
        } else {
            existing_memories
                .iter()
                .map(|m| format!("- [{}] {}", m.category, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"You are a memory extraction system. Analyze the following session data and extract
facts worth remembering for future sessions with this project.

## Session Task
{task}

## Files Read
{files}

## Key Findings
{findings}

## Conversation Summary
{summary}

## Already Known (DO NOT duplicate these)
{existing}

---

Extract NEW facts in the following JSON format. Only extract information that is:
1. Stable (unlikely to change frequently)
2. Useful for future sessions
3. Not already known

Return a JSON array:
[
  {{
    "category": "preference|convention|pattern|correction|fact",
    "content": "concise factual statement",
    "keywords": ["keyword1", "keyword2"],
    "importance": 0.0-1.0,
    "scope": "project|global"
  }}
]

Rules:
- "preference": user explicitly stated preferences (e.g., "use pnpm not npm")
- "convention": project-specific conventions discovered (e.g., "tests in __tests__/ directories")
- "pattern": recurring code/architecture patterns (e.g., "all API routes return CommandResponse<T>")
- "correction": mistakes to avoid (e.g., "editing executor.rs requires cargo check due to type complexity")
- "fact": general project facts (e.g., "frontend uses Zustand for state management")
- importance: 0.9+ for explicit user instructions, 0.5-0.8 for discovered patterns, 0.3-0.5 for general facts
- scope: "global" for cross-project user-level preferences (communication language, coding style, tool preferences, personal habits); "project" for project-specific info (default)
- Preferences about the user themselves (not the project) should be "global"
- Return empty array [] if nothing worth extracting"#,
            task = task_description,
            files = files_section,
            findings = findings_section,
            summary = conversation_summary,
            existing = existing_section,
        )
    }

    /// Parse extraction results from LLM response.
    ///
    /// Expects a JSON array of objects with category, content, keywords, importance.
    /// Tolerant of markdown code blocks wrapping the JSON.
    pub fn parse_extraction_response(
        response: &str,
        project_path: &str,
        session_id: Option<&str>,
    ) -> Vec<NewMemoryEntry> {
        // Strip markdown code blocks if present
        let json_str = response
            .trim()
            .strip_prefix("```json")
            .or_else(|| response.trim().strip_prefix("```"))
            .unwrap_or(response.trim());
        let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

        // Parse JSON array
        let items: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        items
            .iter()
            .filter_map(|item| {
                let category_str = item.get("category")?.as_str()?;
                let content = item.get("content")?.as_str()?.to_string();
                let importance = item
                    .get("importance")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5) as f32;
                let keywords: Vec<String> = item
                    .get("keywords")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let category = MemoryCategory::from_str(category_str).ok()?;

                if content.is_empty() {
                    return None;
                }

                // Route to global or project scope based on LLM output
                let scope = item.get("scope").and_then(|v| v.as_str()).unwrap_or("project");
                let effective_project_path = if scope == "global" {
                    GLOBAL_PROJECT_PATH.to_string()
                } else {
                    project_path.to_string()
                };

                Some(NewMemoryEntry {
                    project_path: effective_project_path,
                    category,
                    content,
                    keywords,
                    importance: importance.clamp(0.0, 1.0),
                    source_session_id: session_id.map(|s| s.to_string()),
                    source_context: None,
                })
            })
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // detect_memory_command tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_remember_that() {
        let cmd = detect_memory_command("Remember that this project uses pnpm");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "this project uses pnpm".into()
            })
        );
    }

    #[test]
    fn test_detect_always_use() {
        let cmd = detect_memory_command("Always use tabs instead of spaces");
        // "always use " is matched first, yielding "tabs instead of spaces"
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "tabs instead of spaces".into()
            })
        );
    }

    #[test]
    fn test_detect_never_do() {
        let cmd = detect_memory_command("Never do force pushes to main");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "force pushes to main".into()
            })
        );
    }

    #[test]
    fn test_detect_note_that() {
        let cmd = detect_memory_command("Note that the API uses REST not GraphQL.");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "the api uses rest not graphql".into()
            })
        );
    }

    #[test]
    fn test_detect_keep_in_mind() {
        let cmd = detect_memory_command("Keep in mind that tests are in __tests__/");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "tests are in __tests__/".into()
            })
        );
    }

    #[test]
    fn test_detect_forget_that() {
        let cmd = detect_memory_command("Forget that I said to use pnpm");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Forget {
                query: "i said to use pnpm".into()
            })
        );
    }

    #[test]
    fn test_detect_stop_remembering() {
        let cmd = detect_memory_command("Stop remembering the tabs preference");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Forget {
                query: "the tabs preference".into()
            })
        );
    }

    #[test]
    fn test_detect_delete_memory() {
        let cmd = detect_memory_command("Delete memory about npm preferences");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Forget {
                query: "npm preferences".into()
            })
        );
    }

    #[test]
    fn test_detect_what_do_you_remember() {
        let cmd = detect_memory_command("What do you remember about this project?");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Query {
                query: "this project".into()
            })
        );
    }

    #[test]
    fn test_detect_do_you_remember() {
        let cmd = detect_memory_command("Do you remember my preferences?");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Query {
                query: "my preferences".into()
            })
        );
    }

    #[test]
    fn test_detect_what_are_my_preferences() {
        let cmd = detect_memory_command("What are my preferences?");
        assert!(matches!(cmd, Some(MemoryCommand::Query { .. })));
    }

    #[test]
    fn test_detect_no_command() {
        assert!(detect_memory_command("How do I fix this bug?").is_none());
        assert!(detect_memory_command("Explain the architecture").is_none());
        assert!(detect_memory_command("Write a test for the parser").is_none());
        assert!(detect_memory_command("").is_none());
        assert!(detect_memory_command("hello world").is_none());
    }

    #[test]
    fn test_detect_case_insensitive() {
        let cmd = detect_memory_command("REMEMBER THAT pnpm is preferred");
        assert!(matches!(cmd, Some(MemoryCommand::Remember { .. })));

        let cmd = detect_memory_command("FORGET THAT npm preference");
        assert!(matches!(cmd, Some(MemoryCommand::Forget { .. })));
    }

    #[test]
    fn test_detect_empty_content_returns_none() {
        assert!(detect_memory_command("Remember that ").is_none());
        assert!(detect_memory_command("Forget that ").is_none());
    }

    // -----------------------------------------------------------------------
    // MemoryExtractor tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_extraction_prompt_contains_sections() {
        let prompt = MemoryExtractor::build_extraction_prompt(
            "Fix the login bug",
            &["src/auth.rs".into(), "src/main.rs".into()],
            &["Uses JWT tokens".into()],
            "Found and fixed auth issue",
            &[],
        );

        assert!(prompt.contains("Fix the login bug"));
        assert!(prompt.contains("src/auth.rs"));
        assert!(prompt.contains("Uses JWT tokens"));
        assert!(prompt.contains("Found and fixed auth issue"));
        assert!(prompt.contains("Already Known"));
    }

    #[test]
    fn test_build_extraction_prompt_empty_inputs() {
        let prompt = MemoryExtractor::build_extraction_prompt("", &[], &[], "", &[]);

        assert!(prompt.contains("(none)"));
    }

    #[test]
    fn test_build_extraction_prompt_with_existing_memories() {
        let existing = vec![MemoryEntry {
            id: "test-1".into(),
            project_path: "/test".into(),
            category: MemoryCategory::Fact,
            content: "This is a Tauri app".into(),
            keywords: vec![],
            importance: 0.5,
            access_count: 0,
            source_session_id: None,
            source_context: None,
            created_at: "".into(),
            updated_at: "".into(),
            last_accessed_at: "".into(),
        }];

        let prompt = MemoryExtractor::build_extraction_prompt(
            "Explore project",
            &[],
            &[],
            "Explored architecture",
            &existing,
        );

        assert!(prompt.contains("This is a Tauri app"));
        assert!(prompt.contains("[fact]"));
    }

    #[test]
    fn test_parse_extraction_response_valid() {
        let response = r#"[
            {
                "category": "preference",
                "content": "Always use pnpm not npm",
                "keywords": ["pnpm", "npm"],
                "importance": 0.9
            },
            {
                "category": "fact",
                "content": "Uses Tauri 2 with React 18",
                "keywords": ["tauri", "react"],
                "importance": 0.5
            }
        ]"#;

        let entries = MemoryExtractor::parse_extraction_response(
            response,
            "/test/project",
            Some("session-1"),
        );

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].category, MemoryCategory::Preference);
        assert_eq!(entries[0].content, "Always use pnpm not npm");
        assert_eq!(entries[0].keywords, vec!["pnpm", "npm"]);
        assert_eq!(entries[0].importance, 0.9);
        assert_eq!(entries[0].project_path, "/test/project");
        assert_eq!(entries[0].source_session_id, Some("session-1".into()));

        assert_eq!(entries[1].category, MemoryCategory::Fact);
    }

    #[test]
    fn test_parse_extraction_response_markdown_wrapped() {
        let response = "```json\n[\n  {\n    \"category\": \"convention\",\n    \"content\": \"Tests in __tests__\",\n    \"keywords\": [\"tests\"],\n    \"importance\": 0.6\n  }\n]\n```";

        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, MemoryCategory::Convention);
    }

    #[test]
    fn test_parse_extraction_response_empty_array() {
        let response = "[]";
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_invalid_json() {
        let response = "not valid json";
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_invalid_category() {
        let response = r#"[{"category": "invalid", "content": "test", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_empty_content_skipped() {
        let response = r#"[{"category": "fact", "content": "", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_global_scope() {
        let response = r#"[
            {
                "category": "preference",
                "content": "User prefers Chinese for communication",
                "keywords": ["language", "chinese"],
                "importance": 0.9,
                "scope": "global"
            },
            {
                "category": "fact",
                "content": "Project uses Tauri 2",
                "keywords": ["tauri"],
                "importance": 0.5,
                "scope": "project"
            }
        ]"#;

        let entries = MemoryExtractor::parse_extraction_response(
            response,
            "/test/project",
            Some("session-1"),
        );

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].project_path, "__global__");
        assert_eq!(entries[1].project_path, "/test/project");
    }

    #[test]
    fn test_parse_extraction_response_default_scope_is_project() {
        let response = r#"[{"category": "fact", "content": "test fact", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].project_path, "/test");
    }

    #[test]
    fn test_parse_extraction_response_clamps_importance() {
        let response = r#"[{"category": "fact", "content": "test", "importance": 1.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].importance <= 1.0);
    }
}
