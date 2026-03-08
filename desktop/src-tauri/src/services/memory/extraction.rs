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

use crate::services::memory::store::{
    build_session_project_path, MemoryCategory, MemoryEntry, NewMemoryEntry, GLOBAL_PROJECT_PATH,
};
use crate::services::memory::query_v2::MemoryScopeV2;
use crate::utils::error::{AppError, AppResult};

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
    let trimmed = user_message.trim();
    if trimmed.is_empty() {
        return None;
    }

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
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, pattern) {
            let query = trim_command_payload(rest).to_string();
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
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, pattern) {
            let query = trim_command_payload(rest).to_string();
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
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, pattern) {
            let content = trim_command_payload(rest).to_string();
            if !content.is_empty() {
                return Some(MemoryCommand::Remember { content });
            }
        }
    }

    None
}

fn strip_prefix_case_insensitive<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        text.get(prefix.len()..)
    } else {
        None
    }
}

fn trim_command_payload(text: &str) -> &str {
    text.trim()
        .trim_end_matches(|c: char| matches!(c, '.' | '?' | '!' | '。' | '？' | '！'))
        .trim()
}

/// LLM-driven memory extractor
pub struct MemoryExtractor;

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedMemoryCandidate {
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub source_session_id: Option<String>,
    pub source_context: String,
    pub suggested_scope: Option<MemoryScopeV2>,
    pub evidence_snippets: Vec<String>,
    pub confidence: f32,
}

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
    "suggested_scope": "project|global|session|null",
    "evidence_snippets": ["short supporting quote"],
    "confidence": 0.0-1.0
  }}
]

Rules:
- "preference": user explicitly stated preferences (e.g., "use pnpm not npm")
- "convention": project-specific conventions discovered (e.g., "tests in __tests__/ directories")
- "pattern": recurring code/architecture patterns (e.g., "all API routes return CommandResponse<T>")
- "correction": mistakes to avoid (e.g., "editing executor.rs requires cargo check due to type complexity")
- "fact": general project facts (e.g., "frontend uses Zustand for state management")
- importance: 0.9+ for explicit user instructions, 0.5-0.8 for discovered patterns, 0.3-0.5 for general facts
- suggested_scope: "global" for cross-project user-level preferences (communication language, coding style, tool preferences, personal habits); "project" for project-specific info; "session" for details useful only in this specific session; use null when unsure
- Preferences about the user themselves (not the project) should be "global"
- evidence_snippets: include 1-3 short factual snippets from the conversation that justify the memory
- confidence: 0.9+ only when the conversation is explicit, 0.5-0.8 for inferred but well-supported facts
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
    /// Returns a parse error when the top-level payload is not valid JSON array.
    pub fn parse_extraction_candidates(
        response: &str,
        session_id: Option<&str>,
    ) -> AppResult<Vec<ExtractedMemoryCandidate>> {
        // Strip markdown code blocks if present
        let json_str = response
            .trim()
            .strip_prefix("```json")
            .or_else(|| response.trim().strip_prefix("```"))
            .unwrap_or(response.trim());
        let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

        // Parse JSON array
        let items: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
            AppError::parse(format!("Failed to parse memory extraction JSON: {}", e))
        })?;

        let entries = items
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
                let evidence_snippets: Vec<String> = item
                    .get("evidence_snippets")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                            .filter(|s| !s.is_empty())
                            .take(3)
                            .collect()
                    })
                    .unwrap_or_default();
                let confidence = item
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5) as f32;

                let category = MemoryCategory::from_str(category_str).ok()?;

                if content.is_empty() {
                    return None;
                }

                let suggested_scope = item
                    .get("suggested_scope")
                    .or_else(|| item.get("scope"))
                    .and_then(|v| v.as_str())
                    .and_then(MemoryScopeV2::from_str);

                Some(ExtractedMemoryCandidate {
                    category,
                    content,
                    keywords,
                    importance: importance.clamp(0.0, 1.0),
                    source_session_id: session_id.map(|s| s.to_string()),
                    source_context: "llm_extract:auto_v3".to_string(),
                    suggested_scope,
                    evidence_snippets,
                    confidence: confidence.clamp(0.0, 1.0),
                })
            })
            .collect();

        Ok(entries)
    }

    pub fn parse_extraction_response(
        response: &str,
        project_path: &str,
        session_id: Option<&str>,
    ) -> AppResult<Vec<NewMemoryEntry>> {
        let candidates = Self::parse_extraction_candidates(response, session_id)?;
        Ok(candidates
            .into_iter()
            .map(|candidate| {
                let effective_project_path = match candidate.suggested_scope.unwrap_or(MemoryScopeV2::Project) {
                    MemoryScopeV2::Global => GLOBAL_PROJECT_PATH.to_string(),
                    MemoryScopeV2::Session => session_id
                        .and_then(build_session_project_path)
                        .unwrap_or_else(|| project_path.to_string()),
                    MemoryScopeV2::Project => project_path.to_string(),
                };

                NewMemoryEntry {
                    project_path: effective_project_path,
                    category: candidate.category,
                    content: candidate.content,
                    keywords: candidate.keywords,
                    importance: candidate.importance,
                    source_session_id: candidate.source_session_id,
                    source_context: Some(candidate.source_context),
                }
            })
            .collect())
    }
}

/// Run LLM-driven session memory extraction (summarize-if-needed + structured extract).
///
/// Returns parsed `NewMemoryEntry` rows ready for upsert.
pub async fn run_session_extraction(
    provider: &dyn crate::services::llm::provider::LlmProvider,
    project_path: &str,
    task_description: &str,
    files_read: &[String],
    key_findings: &[String],
    conversation_content: &str,
    session_id: Option<&str>,
    existing_memories: &[MemoryEntry],
) -> AppResult<Vec<NewMemoryEntry>> {
    use crate::services::llm::types::{LlmRequestOptions, Message};
    use std::time::Duration;

    const LLM_TIMEOUT_SECS: u64 = 30;

    let effective_summary = if conversation_content.len() > MemoryExtractor::SUMMARIZE_THRESHOLD {
        let summarize_prompt =
            MemoryExtractor::build_summarization_prompt(task_description, conversation_content);
        let summarize_call = provider.send_message(
            vec![Message::user(summarize_prompt)],
            None,
            vec![],
            LlmRequestOptions {
                temperature_override: Some(0.2),
                ..Default::default()
            },
        );
        match tokio::time::timeout(Duration::from_secs(LLM_TIMEOUT_SECS), summarize_call).await {
            Ok(Ok(resp)) => resp
                .content
                .unwrap_or_else(|| conversation_content.to_string()),
            Ok(Err(_)) | Err(_) => conversation_content.to_string(),
        }
    } else {
        conversation_content.to_string()
    };

    let extraction_prompt = MemoryExtractor::build_extraction_prompt(
        task_description,
        files_read,
        key_findings,
        &effective_summary,
        existing_memories,
    );
    let extract_call = provider.send_message(
        vec![Message::user(extraction_prompt)],
        None,
        vec![],
        LlmRequestOptions {
            temperature_override: Some(0.2),
            ..Default::default()
        },
    );
    let extraction_response =
        match tokio::time::timeout(Duration::from_secs(LLM_TIMEOUT_SECS), extract_call).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(AppError::command(format!(
                    "memory extraction llm call failed: {}",
                    e
                )))
            }
            Err(_) => return Err(AppError::command("memory extraction llm call timed out")),
        };

    let response_text = extraction_response
        .content
        .ok_or_else(|| AppError::command("memory extraction llm returned empty content"))?;

    MemoryExtractor::parse_extraction_response(&response_text, project_path, session_id)
}

/// Run LLM-driven session memory extraction and keep the richer candidate payload
/// for downstream routing and review.
pub async fn run_session_extraction_candidates(
    provider: &dyn crate::services::llm::provider::LlmProvider,
    task_description: &str,
    files_read: &[String],
    key_findings: &[String],
    conversation_content: &str,
    session_id: Option<&str>,
    existing_memories: &[MemoryEntry],
) -> AppResult<Vec<ExtractedMemoryCandidate>> {
    use crate::services::llm::types::{LlmRequestOptions, Message};
    use std::time::Duration;

    const LLM_TIMEOUT_SECS: u64 = 30;

    let effective_summary = if conversation_content.len() > MemoryExtractor::SUMMARIZE_THRESHOLD {
        let summarize_prompt =
            MemoryExtractor::build_summarization_prompt(task_description, conversation_content);
        let summarize_call = provider.send_message(
            vec![Message::user(summarize_prompt)],
            None,
            vec![],
            LlmRequestOptions {
                temperature_override: Some(0.2),
                ..Default::default()
            },
        );
        match tokio::time::timeout(Duration::from_secs(LLM_TIMEOUT_SECS), summarize_call).await {
            Ok(Ok(resp)) => resp
                .content
                .unwrap_or_else(|| conversation_content.to_string()),
            Ok(Err(_)) | Err(_) => conversation_content.to_string(),
        }
    } else {
        conversation_content.to_string()
    };

    let extraction_prompt = MemoryExtractor::build_extraction_prompt(
        task_description,
        files_read,
        key_findings,
        &effective_summary,
        existing_memories,
    );
    let extract_call = provider.send_message(
        vec![Message::user(extraction_prompt)],
        None,
        vec![],
        LlmRequestOptions {
            temperature_override: Some(0.2),
            ..Default::default()
        },
    );
    let extraction_response =
        match tokio::time::timeout(Duration::from_secs(LLM_TIMEOUT_SECS), extract_call).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(AppError::command(format!(
                    "memory extraction llm call failed: {}",
                    e
                )))
            }
            Err(_) => return Err(AppError::command("memory extraction llm call timed out")),
        };

    let response_text = extraction_response
        .content
        .ok_or_else(|| AppError::command("memory extraction llm returned empty content"))?;

    MemoryExtractor::parse_extraction_candidates(&response_text, session_id)
}

/// Execute explicit memory commands (`remember` / `forget` / `query`) before
/// the normal agent loop consumes the user message.
///
/// Returns a context string that should be injected into the current turn when
/// a command is detected, otherwise `None`.
pub async fn execute_explicit_memory_command(
    store: &crate::services::memory::store::ProjectMemoryStore,
    project_path: &str,
    session_id: Option<&str>,
    user_message: &str,
) -> Option<String> {
    use crate::services::memory::query_policy_v2::{memory_query_tuning_v2, MemoryQueryPresetV2};
    use crate::services::memory::query_v2::{
        query_memory_entries_v2 as query_memory_entries_unified_v2,
        review_memory_candidates_v2 as review_memory_candidates_unified_v2, MemoryReviewDecisionV2,
        MemoryScopeV2, MemoryStatusV2, UnifiedMemoryQueryRequestV2,
    };
    use crate::services::memory::retrieval::{extract_query_keywords, MemorySearchIntent};
    use crate::services::tools::system_prompt::build_memory_section;

    let command = detect_memory_command(user_message)?;

    let mut scopes = vec![MemoryScopeV2::Project, MemoryScopeV2::Global];
    if session_id.is_some() {
        scopes.push(MemoryScopeV2::Session);
    }
    let search_tuning = memory_query_tuning_v2(MemoryQueryPresetV2::CommandSearch);

    match command {
        MemoryCommand::Remember { content } => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return Some(
                    "[memory-command] Remember command ignored because content was empty."
                        .to_string(),
                );
            }
            let _ = store.add_memory(NewMemoryEntry {
                project_path: project_path.to_string(),
                category: MemoryCategory::Fact,
                content: trimmed.to_string(),
                keywords: extract_query_keywords(trimmed),
                importance: 0.95,
                source_session_id: session_id.map(|value| value.to_string()),
                source_context: Some("explicit_command:remember".to_string()),
            });
            Some(format!(
                "[memory-command] Saved explicit memory (importance=0.95): {}.\nPlease acknowledge this memory update before continuing.",
                trimmed
            ))
        }
        MemoryCommand::Forget { query } => {
            let trimmed = query.trim();
            let request = UnifiedMemoryQueryRequestV2 {
                project_path: project_path.to_string(),
                query: trimmed.to_string(),
                scopes,
                categories: vec![],
                include_ids: vec![],
                exclude_ids: vec![],
                session_id: session_id.map(|value| value.to_string()),
                top_k_total: search_tuning.top_k_total.max(20),
                min_importance: 0.0,
                per_scope_budget: search_tuning.per_scope_budget.max(24),
                intent: MemorySearchIntent::Default,
                enable_semantic: true,
                enable_lexical: true,
                statuses: vec![MemoryStatusV2::Active],
            };
            let matched = query_memory_entries_unified_v2(store, &request)
                .await
                .ok()
                .map(|rows| {
                    rows.results
                        .into_iter()
                        .map(|row| row.entry.id)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if matched.is_empty() {
                return Some(format!(
                    "[memory-command] No active memories matched forget query '{}'.",
                    trimmed
                ));
            }
            let archived = review_memory_candidates_unified_v2(
                store,
                &matched,
                MemoryReviewDecisionV2::Archive,
            )
            .ok()
            .map(|summary| summary.updated)
            .unwrap_or(matched.len());
            Some(format!(
                "[memory-command] Archived {} memory entries for forget query '{}'.",
                archived, trimmed
            ))
        }
        MemoryCommand::Query { query } => {
            let trimmed = query.trim();
            let request = UnifiedMemoryQueryRequestV2 {
                project_path: project_path.to_string(),
                query: trimmed.to_string(),
                scopes,
                categories: vec![],
                include_ids: vec![],
                exclude_ids: vec![],
                session_id: session_id.map(|value| value.to_string()),
                top_k_total: search_tuning.top_k_total.max(20),
                min_importance: 0.0,
                per_scope_budget: search_tuning.per_scope_budget.max(24),
                intent: MemorySearchIntent::Default,
                enable_semantic: true,
                enable_lexical: true,
                statuses: vec![MemoryStatusV2::Active],
            };
            let entries = query_memory_entries_unified_v2(store, &request)
                .await
                .ok()
                .map(|rows| {
                    rows.results
                        .into_iter()
                        .map(|row| row.entry)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if entries.is_empty() {
                return Some(
                    "[memory-command] No matching active memories found. Respond accordingly."
                        .to_string(),
                );
            }
            let memory_block = build_memory_section(Some(&entries));
            Some(format!(
                "[memory-command] User asked for memory recall. Use the following authoritative memory context to answer directly.\n\n{}",
                memory_block
            ))
        }
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
                content: "the API uses REST not GraphQL".into()
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
                query: "I said to use pnpm".into()
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

    #[test]
    fn test_detect_preserves_case_for_payload() {
        let cmd = detect_memory_command("Remember that I prefer CamelCase identifiers.");
        assert_eq!(
            cmd,
            Some(MemoryCommand::Remember {
                content: "I prefer CamelCase identifiers".into()
            })
        );
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
            scope: Some("project".into()),
            session_id: None,
            category: MemoryCategory::Fact,
            content: "This is a Tauri app".into(),
            keywords: vec![],
            importance: 0.5,
            access_count: 0,
            source_session_id: None,
            source_context: None,
            status: Some("active".into()),
            risk_tier: Some("high".into()),
            conflict_flag: Some(false),
            trace_id: None,
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
        )
        .unwrap();

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

        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, MemoryCategory::Convention);
    }

    #[test]
    fn test_parse_extraction_response_empty_array() {
        let response = "[]";
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_invalid_json() {
        let response = "not valid json";
        let result = MemoryExtractor::parse_extraction_response(response, "/test", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_extraction_response_invalid_category() {
        let response = r#"[{"category": "invalid", "content": "test", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_empty_content_skipped() {
        let response = r#"[{"category": "fact", "content": "", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();
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
        )
        .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].project_path, "__global__");
        assert_eq!(entries[1].project_path, "/test/project");
    }

    #[test]
    fn test_parse_extraction_response_default_scope_is_project() {
        let response = r#"[{"category": "fact", "content": "test fact", "importance": 0.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].project_path, "/test");
    }

    #[test]
    fn test_parse_extraction_response_session_scope() {
        let response = r#"[{"category":"fact","content":"Temporary session note","importance":0.6,"scope":"session"}]"#;
        let entries = MemoryExtractor::parse_extraction_response(
            response,
            "/test/project",
            Some("standalone:abc-1"),
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].project_path, "__session__:abc-1");
    }

    #[test]
    fn test_parse_extraction_response_session_scope_without_session_id_falls_back_to_project() {
        let response = r#"[{"category":"fact","content":"Temporary session note","importance":0.6,"scope":"session"}]"#;
        let entries =
            MemoryExtractor::parse_extraction_response(response, "/test/project", None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].project_path, "/test/project");
    }

    #[test]
    fn test_parse_extraction_response_clamps_importance() {
        let response = r#"[{"category": "fact", "content": "test", "importance": 1.5}]"#;
        let entries = MemoryExtractor::parse_extraction_response(response, "/test", None).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].importance <= 1.0);
    }
}
