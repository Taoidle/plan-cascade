//! LLM-Assisted Git Operations
//!
//! Single-call LLM integration for:
//! - Generating commit messages from diffs
//! - Suggesting conflict resolution
//! - Code review from diffs
//! - Summarizing changes

use std::sync::Arc;

use crate::services::llm::types::{LlmRequestOptions, Message, ToolCallMode, FallbackToolFormatMode};
use crate::services::llm::LlmProvider;
use crate::utils::error::{AppError, AppResult};

/// LLM-assisted git operations.
///
/// Uses a single LLM call per operation (no agentic loops).
pub struct GitLlmAssist {
    provider: Arc<dyn LlmProvider>,
}

impl GitLlmAssist {
    /// Create a new GitLlmAssist with the given LLM provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Generate a commit message from a diff.
    ///
    /// The diff should be the output of `git diff --cached` (staged changes).
    pub async fn generate_commit_message(&self, diff: &str) -> AppResult<String> {
        let system = "You are a helpful assistant that generates concise, conventional commit messages. \
                       Follow the Conventional Commits format (type(scope): description). \
                       The type should be one of: feat, fix, refactor, docs, test, chore, style, perf, ci, build. \
                       Keep the first line under 72 characters. \
                       If the changes are complex, add a blank line followed by a body with bullet points. \
                       Only output the commit message, nothing else.".to_string();

        let user_content = format!(
            "Generate a commit message for the following diff:\n\n```diff\n{}\n```",
            truncate_for_llm(diff, 8000)
        );

        let response = self
            .provider
            .send_message(
                vec![Message::user(user_content)],
                Some(system),
                vec![], // No tools — pure text completion
                no_tools_options(),
            )
            .await
            .map_err(|e| AppError::command(format!("LLM error generating commit message: {}", e)))?;

        response
            .content
            .ok_or_else(|| AppError::command("LLM returned empty response for commit message".to_string()))
            .map(|s| s.trim().to_string())
    }

    /// Suggest a conflict resolution from conflict file content.
    ///
    /// Returns a suggested resolved version of the file.
    pub async fn suggest_conflict_resolution(&self, conflict_content: &str) -> AppResult<String> {
        let system = "You are a helpful assistant that resolves git merge conflicts. \
                       Analyze the conflict markers (<<<<<<, =======, >>>>>>>) and suggest a clean resolution. \
                       Keep all meaningful changes from both sides when possible. \
                       Output ONLY the resolved file content without any conflict markers. \
                       Do not add any explanation before or after the code.".to_string();

        let user_content = format!(
            "Resolve the conflicts in this file:\n\n```\n{}\n```",
            truncate_for_llm(conflict_content, 8000)
        );

        let response = self
            .provider
            .send_message(
                vec![Message::user(user_content)],
                Some(system),
                vec![],
                no_tools_options(),
            )
            .await
            .map_err(|e| AppError::command(format!("LLM error suggesting conflict resolution: {}", e)))?;

        response
            .content
            .ok_or_else(|| AppError::command("LLM returned empty response for conflict resolution".to_string()))
            .map(|s| strip_code_fences(&s))
    }

    /// Review a diff and provide code review feedback.
    pub async fn review_diff(&self, diff: &str) -> AppResult<String> {
        let system = "You are an experienced code reviewer. \
                       Review the following diff and provide constructive feedback. \
                       Focus on: bugs, security issues, performance problems, and code quality. \
                       Be concise and actionable. Use bullet points. \
                       If the code looks good, say so briefly.".to_string();

        let user_content = format!(
            "Review this diff:\n\n```diff\n{}\n```",
            truncate_for_llm(diff, 8000)
        );

        let response = self
            .provider
            .send_message(
                vec![Message::user(user_content)],
                Some(system),
                vec![],
                no_tools_options(),
            )
            .await
            .map_err(|e| AppError::command(format!("LLM error reviewing diff: {}", e)))?;

        response
            .content
            .ok_or_else(|| AppError::command("LLM returned empty response for diff review".to_string()))
            .map(|s| s.trim().to_string())
    }

    /// Summarize recent changes from commit messages and optionally diffs.
    pub async fn summarize_changes(
        &self,
        commit_messages: &[String],
        diff_summary: Option<&str>,
    ) -> AppResult<String> {
        let system = "You are a helpful assistant that writes clear, concise summaries of code changes. \
                       Summarize what changed, why, and any notable impacts. \
                       Use bullet points. Keep it brief (3-5 bullet points for most cases).".to_string();

        let mut user_content = String::from("Summarize these changes:\n\nCommit messages:\n");
        for msg in commit_messages {
            user_content.push_str(&format!("- {}\n", msg));
        }

        if let Some(diff) = diff_summary {
            user_content.push_str(&format!(
                "\nDiff summary:\n```\n{}\n```",
                truncate_for_llm(diff, 4000)
            ));
        }

        let response = self
            .provider
            .send_message(
                vec![Message::user(user_content)],
                Some(system),
                vec![],
                no_tools_options(),
            )
            .await
            .map_err(|e| AppError::command(format!("LLM error summarizing changes: {}", e)))?;

        response
            .content
            .ok_or_else(|| AppError::command("LLM returned empty response for change summary".to_string()))
            .map(|s| s.trim().to_string())
    }
}

/// Create LlmRequestOptions with no tools and low temperature for deterministic output.
fn no_tools_options() -> LlmRequestOptions {
    LlmRequestOptions {
        tool_call_mode: ToolCallMode::None,
        fallback_tool_format_mode: FallbackToolFormatMode::Off,
        temperature_override: Some(0.3),
        reasoning_effort_override: None,
        analysis_phase: None,
    }
}

/// Truncate text to a maximum character count for LLM context.
fn truncate_for_llm(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        text
    } else {
        // Find a safe boundary (don't cut in middle of multi-byte char)
        let mut end = max_chars;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    }
}

/// Strip markdown code fences from LLM response if present.
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip optional language tag on first line
        let rest = if let Some(newline_pos) = rest.find('\n') {
            &rest[newline_pos + 1..]
        } else {
            rest
        };
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }
    trimmed.to_string()
}

impl std::fmt::Debug for GitLlmAssist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitLlmAssist")
            .field("provider", &self.provider.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_for_llm_short() {
        let text = "short text";
        assert_eq!(truncate_for_llm(text, 100), "short text");
    }

    #[test]
    fn test_truncate_for_llm_exact() {
        let text = "12345";
        assert_eq!(truncate_for_llm(text, 5), "12345");
    }

    #[test]
    fn test_truncate_for_llm_truncated() {
        let text = "this is a longer text that should be truncated";
        let result = truncate_for_llm(text, 10);
        assert_eq!(result.len(), 10);
        assert_eq!(result, "this is a ");
    }

    #[test]
    fn test_truncate_for_llm_multibyte() {
        let text = "hello\u{1F600}world"; // emoji is multi-byte
        let result = truncate_for_llm(text, 6);
        // Should not cut in the middle of the emoji
        assert!(result.len() <= 6);
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn test_strip_code_fences_no_fences() {
        assert_eq!(strip_code_fences("hello world"), "hello world");
    }

    #[test]
    fn test_strip_code_fences_with_fences() {
        assert_eq!(strip_code_fences("```\ncode here\n```"), "code here");
    }

    #[test]
    fn test_strip_code_fences_with_language() {
        assert_eq!(
            strip_code_fences("```rust\nfn main() {}\n```"),
            "fn main() {}"
        );
    }

    #[test]
    fn test_strip_code_fences_with_whitespace() {
        assert_eq!(
            strip_code_fences("  ```\n  code  \n```  "),
            "code"
        );
    }

    #[test]
    fn test_no_tools_options() {
        let opts = no_tools_options();
        assert_eq!(opts.tool_call_mode, ToolCallMode::None);
        assert_eq!(opts.fallback_tool_format_mode, FallbackToolFormatMode::Off);
        assert_eq!(opts.temperature_override, Some(0.3));
    }

    #[test]
    fn test_strip_code_fences_incomplete() {
        // Only opening fence, no closing
        let input = "```\nsome code";
        assert_eq!(strip_code_fences(input), "```\nsome code");
    }
}
