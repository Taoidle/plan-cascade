//! LLM-Powered PRD Generation
//!
//! Decomposes a task description into a structured PRD with stories using an LLM provider.
//! Implements retry-with-repair per ADR-F002: on JSON parse failure, retries once with
//! a repair prompt that includes the parse error and original response.

use std::sync::Arc;

use tracing::debug;

use crate::commands::task_mode::{ConversationTurnInput, TaskPrd, TaskStory};
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::services::task_mode::batch_executor::ExecutableStory;
use crate::services::task_mode::calculate_batches;

/// Default maximum parallel stories per batch for PRD generation.
const DEFAULT_MAX_PARALLEL: usize = 4;

/// Token budget reserved for system prompt + PRD request + response.
const PRD_RESERVED_TOKENS: usize = 4000;

/// Build the system prompt for PRD generation.
///
/// Instructs the LLM to decompose a task description into stories with structured JSON output.
pub fn build_prd_system_prompt() -> String {
    r#"You are a technical project planner. Your job is to decompose a task description into a set of implementation stories.

Each story must have the following fields:
- "id": A unique identifier (e.g., "story-001", "story-002")
- "title": A concise title for the story
- "description": A detailed description of what needs to be done
- "priority": One of "high", "medium", or "low"
- "dependencies": An array of story IDs that must be completed before this story can start (empty array if no dependencies)
- "acceptanceCriteria": An array of strings describing what must be true for this story to be considered complete

Rules:
1. Generate between 2 and 10 stories depending on task complexity.
2. Stories should be ordered from foundational to higher-level. Earlier stories should have fewer dependencies.
3. Dependencies must only reference other story IDs in your output. No circular dependencies.
4. Each story should be independently executable once its dependencies are met.
5. Acceptance criteria should be specific and testable.

Respond with ONLY a valid JSON array of story objects. No markdown fences, no explanatory text.

Example output:
[
  {
    "id": "story-001",
    "title": "Set up database schema",
    "description": "Create the initial database tables for users and sessions.",
    "priority": "high",
    "dependencies": [],
    "acceptanceCriteria": ["Users table exists with id, email, name columns", "Sessions table exists with foreign key to users"]
  },
  {
    "id": "story-002",
    "title": "Implement user registration API",
    "description": "Create REST endpoint for user registration with validation.",
    "priority": "high",
    "dependencies": ["story-001"],
    "acceptanceCriteria": ["POST /api/register accepts email and password", "Validation errors return 400 status"]
  }
]"#.to_string()
}

/// Build the user message for PRD generation from a task description.
pub fn build_prd_user_message(task_description: &str) -> String {
    format!(
        "Decompose the following task into implementation stories:\n\n{}",
        task_description
    )
}

/// Build a repair prompt when the initial LLM response fails to parse as JSON.
///
/// Per ADR-F002: includes the parse error and original response to help the LLM fix its output.
pub fn build_repair_prompt(original_response: &str, parse_error: &str) -> String {
    format!(
        "Your previous response could not be parsed as valid JSON.\n\n\
         Parse error: {}\n\n\
         Your previous response was:\n{}\n\n\
         Please respond with ONLY a valid JSON array of story objects. \
         No markdown fences, no explanatory text, no code blocks. \
         Just the raw JSON array starting with [ and ending with ].",
        parse_error, original_response
    )
}

/// Extract the text content from an LLM response.
///
/// Checks `content` first, then falls back to `thinking` (for reasoning models
/// like qwq-plus, deepseek-r1 that may place output in the thinking field).
fn extract_response_text(
    response: &crate::services::llm::types::LlmResponse,
) -> Result<String, String> {
    // Try content field first
    if let Some(ref text) = response.content {
        if !text.trim().is_empty() {
            return Ok(text.clone());
        }
    }
    // Fallback: reasoning models may place output in thinking field
    if let Some(ref thinking) = response.thinking {
        if !thinking.trim().is_empty() {
            debug!(
                thinking_len = thinking.len(),
                "prd_generator: content field empty, falling back to thinking field"
            );
            return Ok(thinking.clone());
        }
    }
    Err(format!(
        "LLM response contained no text content (model: {}, stop_reason: {:?})",
        response.model, response.stop_reason
    ))
}

/// Attempt to extract a JSON array from an LLM response string.
///
/// Handles common LLM quirks like wrapping JSON in markdown code fences.
fn extract_json_from_response(response_text: &str) -> String {
    let trimmed = response_text.trim();

    // Try to extract from markdown code fences (```json ... ``` or ``` ... ```)
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Skip optional language identifier (e.g., "json")
        let content_start = if let Some(nl) = after_fence.find('\n') {
            nl + 1
        } else {
            0
        };
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }

    // Try to find the first [ and last ] for a raw JSON array
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    // Return as-is
    trimmed.to_string()
}

/// Parse LLM response text into a Vec<TaskStory>.
///
/// Extracts JSON from the response (handling markdown fences) and deserializes it.
pub fn parse_stories_from_response(response_text: &str) -> Result<Vec<TaskStory>, String> {
    if response_text.trim().is_empty() {
        return Err("LLM returned empty response (no text content)".to_string());
    }

    let json_str = extract_json_from_response(response_text);

    if json_str.trim().is_empty() {
        return Err(format!(
            "Could not extract JSON array from LLM response (response starts with: {:?})",
            response_text.chars().take(100).collect::<String>()
        ));
    }

    // Normalize snake_case field names to camelCase for serde compatibility.
    // LLMs may return "acceptance_criteria" despite prompt requesting "acceptanceCriteria".
    let json_str = json_str.replace("\"acceptance_criteria\"", "\"acceptanceCriteria\"");

    let stories: Vec<TaskStory> = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "Failed to parse LLM response as JSON array of stories: {}. JSON starts with: {:?}",
            e,
            json_str.chars().take(200).collect::<String>()
        )
    })?;

    // Validate stories
    if stories.is_empty() {
        return Err("LLM returned an empty stories array".to_string());
    }

    for story in &stories {
        if story.id.is_empty() {
            return Err("Story has an empty id".to_string());
        }
        if story.title.is_empty() {
            return Err(format!("Story '{}' has an empty title", story.id));
        }
    }

    Ok(stories)
}

/// Build the complete TaskPrd from stories, calculating execution batches.
pub fn build_task_prd(task_description: &str, stories: Vec<TaskStory>) -> Result<TaskPrd, String> {
    // Convert to ExecutableStory for batch calculation
    let executable_stories: Vec<ExecutableStory> = stories
        .iter()
        .map(|s| ExecutableStory {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            dependencies: s.dependencies.clone(),
            acceptance_criteria: s.acceptance_criteria.clone(),
            agent: None,
        })
        .collect();

    let batches = calculate_batches(&executable_stories, DEFAULT_MAX_PARALLEL)
        .map_err(|e| format!("Failed to calculate execution batches: {}", e))?;

    Ok(TaskPrd {
        title: format!("PRD: {}", task_description),
        description: task_description.to_string(),
        stories,
        batches,
    })
}

/// Generate a TaskPrd by calling an LLM provider.
///
/// Implements retry-with-repair per ADR-F002:
/// 1. First attempt: send task description, parse response as JSON
/// 2. On parse failure: retry once with repair prompt including the error
/// 3. If still failing: return an error to the user
///
/// When `conversation_history` is non-empty, the LLM sees the full Chat
/// conversation before the PRD request — enabling cross-mode context awareness.
/// Smart compaction ensures the history fits within the provider's context window.
pub async fn generate_prd_with_llm(
    provider: Arc<dyn LlmProvider>,
    task_description: &str,
    conversation_history: &[ConversationTurnInput],
    max_context_tokens: usize,
) -> Result<TaskPrd, String> {
    let system_prompt = build_prd_system_prompt();
    let user_message = build_prd_user_message(task_description);

    // Build messages with compacted conversation history
    let mut messages =
        compact_conversation_history(&provider, conversation_history, max_context_tokens).await;
    messages.push(Message::user(&user_message));

    // First attempt
    let response = provider
        .send_message(
            messages.clone(),
            Some(system_prompt.clone()),
            vec![], // No tools needed for PRD generation
            LlmRequestOptions::default(),
        )
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;

    let response_text = extract_response_text(&response).map_err(|e| {
        debug!(
            content_preview = ?response.content.as_ref().map(|s| s.chars().take(200).collect::<String>()),
            thinking_preview = ?response.thinking.as_ref().map(|s| s.chars().take(200).collect::<String>()),
            "prd_generator: first attempt returned empty LLM response"
        );
        e
    })?;

    debug!(
        len = response_text.len(),
        preview = %response_text.chars().take(300).collect::<String>(),
        "prd_generator: first attempt response"
    );

    match parse_stories_from_response(&response_text) {
        Ok(stories) => return build_task_prd(task_description, stories),
        Err(first_error) => {
            // ADR-F002: Retry once with repair prompt
            // Reuse the already-compacted messages (don't re-compact)
            let repair_message = build_repair_prompt(&response_text, &first_error);
            let mut retry_messages = messages;
            retry_messages.push(Message::assistant(&response_text));
            retry_messages.push(Message::user(&repair_message));

            let retry_response = provider
                .send_message(
                    retry_messages,
                    Some(system_prompt),
                    vec![],
                    LlmRequestOptions::default(),
                )
                .await
                .map_err(|e| format!("LLM retry request failed: {}", e))?;

            let retry_text = extract_response_text(&retry_response)?;

            match parse_stories_from_response(&retry_text) {
                Ok(stories) => build_task_prd(task_description, stories),
                Err(second_error) => Err(format!(
                    "Failed to generate PRD after retry. \
                     First attempt error: {}. \
                     Retry error: {}",
                    first_error, second_error
                )),
            }
        }
    }
}

// ============================================================================
// Conversation History Compaction
// ============================================================================

/// Rough token estimate: ~4 chars per token.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Estimate total tokens for a set of conversation turns.
fn estimate_history_tokens(history: &[ConversationTurnInput]) -> usize {
    history
        .iter()
        .map(|t| estimate_tokens(&t.user) + estimate_tokens(&t.assistant))
        .sum()
}

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

/// Smart compaction of conversation history into LLM messages.
///
/// Three-tier strategy:
/// - Tier 1: History fits in budget → pass all turns verbatim
/// - Tier 2: Over budget → LLM-summarize older turns, keep recent turns verbatim
/// - Tier 3: LLM summary fails → prefix-stable sliding window (head 2 + tail N)
async fn compact_conversation_history(
    provider: &Arc<dyn LlmProvider>,
    history: &[ConversationTurnInput],
    max_context_tokens: usize,
) -> Vec<Message> {
    if history.is_empty() {
        return vec![];
    }

    let budget = max_context_tokens.saturating_sub(PRD_RESERVED_TOKENS);
    let total_tokens = estimate_history_tokens(history);

    // Tier 1: fits in budget — full pass-through
    if total_tokens <= budget {
        return history
            .iter()
            .flat_map(|t| vec![Message::user(&t.user), Message::assistant(&t.assistant)])
            .collect();
    }

    // Tier 2: over budget — LLM-summarize older turns, keep recent turns verbatim
    // Allocate ~40% of budget to recent turns
    let recent_budget = budget * 2 / 5;
    let mut recent_start = history.len();
    let mut recent_tokens = 0;
    for i in (0..history.len()).rev() {
        let turn_tokens =
            estimate_tokens(&history[i].user) + estimate_tokens(&history[i].assistant);
        if recent_tokens + turn_tokens > recent_budget {
            break;
        }
        recent_tokens += turn_tokens;
        recent_start = i;
    }
    // Ensure we summarize at least the first turn
    let recent_start = recent_start.max(1);

    let older_turns = &history[..recent_start];
    let recent_turns = &history[recent_start..];

    // Build summary prompt (inspired by agentic_loop compact_messages)
    let conversation_text: String = older_turns
        .iter()
        .map(|t| {
            format!(
                "User: {}\nAssistant: {}",
                truncate_str(&t.user, 500),
                truncate_str(&t.assistant, 500)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let summary_prompt = format!(
        "Summarize the following conversation concisely. Preserve:\n\
         - Key technical decisions and architectural choices\n\
         - Specific requirements, constraints, and preferences mentioned\n\
         - Code patterns, file structures, and technology choices discussed\n\
         - Any unresolved questions or open items\n\n\
         Conversation ({} turns):\n{}\n\n\
         Provide a structured summary that captures the essential context.",
        older_turns.len(),
        conversation_text
    );

    let summary_messages = vec![Message::user(&summary_prompt)];
    match provider
        .send_message(summary_messages, None, vec![], LlmRequestOptions::default())
        .await
    {
        Ok(response) => {
            let summary = response
                .content
                .unwrap_or_else(|| "Previous conversation context was summarized.".to_string());

            // Build: [summary as user+assistant pair] + [recent turns verbatim]
            let mut messages = vec![
                Message::user("[Prior conversation context]"),
                Message::assistant(&summary),
            ];
            for turn in recent_turns {
                messages.push(Message::user(&turn.user));
                messages.push(Message::assistant(&turn.assistant));
            }
            messages
        }
        Err(_) => {
            // Tier 3: LLM summary failed — prefix-stable sliding window
            // Keep head 2 turns + tail N turns (recent_turns)
            let mut messages: Vec<Message> = Vec::new();
            // Head: first 2 turns
            for turn in history.iter().take(2) {
                messages.push(Message::user(&turn.user));
                messages.push(Message::assistant(&turn.assistant));
            }
            // Tail: recent turns
            for turn in recent_turns {
                messages.push(Message::user(&turn.user));
                messages.push(Message::assistant(&turn.assistant));
            }
            messages
        }
    }
}

/// Create an LLM provider from a ProviderConfig.
///
/// Factory function that maps ProviderType to the concrete provider implementation.
pub fn create_provider(
    config: crate::services::llm::types::ProviderConfig,
) -> Arc<dyn LlmProvider> {
    use crate::services::llm::*;

    match config.provider {
        ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config)),
        ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config)),
        ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config)),
        ProviderType::Glm => Arc::new(GlmProvider::new(config)),
        ProviderType::Qwen => Arc::new(QwenProvider::new(config)),
        ProviderType::Minimax => Arc::new(MinimaxProvider::new(config)),
        ProviderType::Ollama => Arc::new(OllamaProvider::new(config)),
    }
}

// ============================================================================
// Compiled Spec → TaskPrd Conversion
// ============================================================================

/// Convert a compiled spec PRD JSON (from the interview pipeline) to a TaskPrd.
///
/// The compiled spec's `prd_json` contains stories in snake_case format.
/// This function converts them to camelCase `TaskPrd` format and calculates
/// execution batches.
///
/// Expected input structure:
/// ```json
/// {
///   "title": "...",
///   "description": "...",
///   "stories": [
///     {
///       "id": "story-001",
///       "title": "...",
///       "description": "...",
///       "priority": "high",
///       "dependencies": [],
///       "acceptanceCriteria": ["..."]
///     }
///   ]
/// }
/// ```
pub fn convert_compiled_prd_to_task_prd(spec_value: serde_json::Value) -> Result<TaskPrd, String> {
    // Extract prd_json field if present, otherwise treat the value itself as PRD
    let prd_value = spec_value.get("prd_json").cloned().unwrap_or(spec_value);

    // Extract title and description
    let title = prd_value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Compiled PRD")
        .to_string();
    let description = prd_value
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Extract stories
    let stories_array = prd_value
        .get("stories")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing or invalid 'stories' array in compiled spec".to_string())?;

    if stories_array.is_empty() {
        return Err("Compiled spec contains no stories".to_string());
    }

    let mut stories = Vec::new();
    for (i, story_val) in stories_array.iter().enumerate() {
        let id = story_val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("story-{:03}", i + 1))
            .to_string();
        let story_title = story_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled Story")
            .to_string();
        let story_desc = story_val
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let priority = story_val
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string();
        let dependencies: Vec<String> = story_val
            .get("dependencies")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        // Support both snake_case and camelCase field names
        let acceptance_criteria: Vec<String> = story_val
            .get("acceptanceCriteria")
            .or_else(|| story_val.get("acceptanceCriteria"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        stories.push(TaskStory {
            id,
            title: story_title,
            description: story_desc,
            priority,
            dependencies,
            acceptance_criteria,
        });
    }

    // Calculate batches from dependencies
    let executable: Vec<ExecutableStory> = stories
        .iter()
        .map(|s| ExecutableStory {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            dependencies: s.dependencies.clone(),
            acceptance_criteria: s.acceptance_criteria.clone(),
            agent: None,
        })
        .collect();

    let batches = calculate_batches(&executable, DEFAULT_MAX_PARALLEL)
        .map_err(|e| format!("Batch calculation failed: {}", e))?;

    Ok(TaskPrd {
        title,
        description,
        stories,
        batches,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::types::{
        LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, MessageContent,
        ProviderConfig, StopReason, ToolDefinition, UsageStats,
    };
    use crate::services::streaming::UnifiedStreamEvent;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // ========================================================================
    // Mock LLM Provider
    // ========================================================================

    /// A mock LLM provider that returns predefined responses for testing.
    struct MockLlmProvider {
        /// Responses to return in sequence; each call pops the first response.
        responses: std::sync::Mutex<Vec<LlmResult<LlmResponse>>>,
    }

    impl MockLlmProvider {
        fn new(responses: Vec<LlmResult<LlmResponse>>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }

        fn with_text_response(text: &str) -> Self {
            Self::new(vec![Ok(LlmResponse {
                content: Some(text.to_string()),
                thinking: None,
                tool_calls: vec![],
                stop_reason: StopReason::EndTurn,
                usage: UsageStats::default(),
                model: "mock-model".to_string(),
            })])
        }

        fn with_responses(responses: Vec<LlmResult<LlmResponse>>) -> Self {
            Self::new(responses)
        }
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn model(&self) -> &str {
            "mock-model"
        }

        fn supports_thinking(&self) -> bool {
            false
        }

        fn supports_tools(&self) -> bool {
            false
        }

        fn config(&self) -> &ProviderConfig {
            // We won't actually use this in tests
            unimplemented!("MockLlmProvider::config() not used in tests")
        }

        async fn send_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Err(LlmError::Other {
                    message: "No more mock responses available".to_string(),
                })
            } else {
                responses.remove(0)
            }
        }

        async fn stream_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _tx: mpsc::Sender<UnifiedStreamEvent>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            unimplemented!("MockLlmProvider does not support streaming")
        }

        async fn health_check(&self) -> LlmResult<()> {
            Ok(())
        }
    }

    // ========================================================================
    // Prompt Tests
    // ========================================================================

    #[test]
    fn test_system_prompt_contains_required_fields() {
        let prompt = build_prd_system_prompt();
        assert!(prompt.contains("\"id\""));
        assert!(prompt.contains("\"title\""));
        assert!(prompt.contains("\"description\""));
        assert!(prompt.contains("\"priority\""));
        assert!(prompt.contains("\"dependencies\""));
        assert!(prompt.contains("\"acceptanceCriteria\""));
    }

    #[test]
    fn test_user_message_includes_description() {
        let msg = build_prd_user_message("Build a REST API");
        assert!(msg.contains("Build a REST API"));
    }

    #[test]
    fn test_repair_prompt_includes_error_and_response() {
        let repair = build_repair_prompt("bad json {", "Expected array");
        assert!(repair.contains("Expected array"));
        assert!(repair.contains("bad json {"));
    }

    // ========================================================================
    // JSON Parsing Tests (RED -> GREEN)
    // ========================================================================

    #[test]
    fn test_parse_valid_json_response() {
        let json = r#"[
            {
                "id": "story-001",
                "title": "Setup project",
                "description": "Initialize project structure",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["Project compiles", "CI passes"]
            },
            {
                "id": "story-002",
                "title": "Add auth",
                "description": "Implement authentication",
                "priority": "medium",
                "dependencies": ["story-001"],
                "acceptanceCriteria": ["Login works", "Logout works"]
            }
        ]"#;

        let stories = parse_stories_from_response(json).unwrap();
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].id, "story-001");
        assert_eq!(stories[0].title, "Setup project");
        assert_eq!(stories[0].priority, "high");
        assert!(stories[0].dependencies.is_empty());
        assert_eq!(stories[0].acceptance_criteria.len(), 2);
        assert_eq!(stories[1].id, "story-002");
        assert_eq!(stories[1].dependencies, vec!["story-001"]);
    }

    #[test]
    fn test_parse_json_with_snake_case_fields() {
        // TaskStory uses serde(rename_all = "camelCase") so it accepts camelCase.
        // Test that the standard camelCase form works.
        let json = r#"[
            {
                "id": "story-001",
                "title": "Test",
                "description": "Desc",
                "priority": "low",
                "dependencies": [],
                "acceptanceCriteria": ["Works"]
            }
        ]"#;

        let stories = parse_stories_from_response(json).unwrap();
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].priority, "low");
    }

    #[test]
    fn test_parse_json_from_markdown_fenced_response() {
        let response = r#"Here are the stories:

```json
[
    {
        "id": "story-001",
        "title": "Setup",
        "description": "Init",
        "priority": "high",
        "dependencies": [],
        "acceptanceCriteria": ["Done"]
    }
]
```"#;

        let stories = parse_stories_from_response(response).unwrap();
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].id, "story-001");
    }

    #[test]
    fn test_parse_json_with_surrounding_text() {
        let response = r#"Sure, here are the decomposed stories:
[
    {
        "id": "story-001",
        "title": "First",
        "description": "First story",
        "priority": "high",
        "dependencies": [],
        "acceptanceCriteria": ["Criterion 1"]
    }
]
Hope this helps!"#;

        let stories = parse_stories_from_response(response).unwrap();
        assert_eq!(stories.len(), 1);
    }

    #[test]
    fn test_parse_empty_array_returns_error() {
        let json = "[]";
        let result = parse_stories_from_response(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        let json = "this is not json at all";
        let result = parse_stories_from_response(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_story_with_empty_id_returns_error() {
        let json = r#"[
            {
                "id": "",
                "title": "Test",
                "description": "Desc",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["Done"]
            }
        ]"#;
        let result = parse_stories_from_response(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty id"));
    }

    #[test]
    fn test_parse_story_with_empty_title_returns_error() {
        let json = r#"[
            {
                "id": "story-001",
                "title": "",
                "description": "Desc",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["Done"]
            }
        ]"#;
        let result = parse_stories_from_response(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty title"));
    }

    // ========================================================================
    // TaskPrd Building Tests
    // ========================================================================

    #[test]
    fn test_build_task_prd_with_stories() {
        let stories = vec![
            TaskStory {
                id: "story-001".to_string(),
                title: "Setup".to_string(),
                description: "Init project".to_string(),
                priority: "high".to_string(),
                dependencies: vec![],
                acceptance_criteria: vec!["Compiles".to_string()],
            },
            TaskStory {
                id: "story-002".to_string(),
                title: "Feature".to_string(),
                description: "Add feature".to_string(),
                priority: "medium".to_string(),
                dependencies: vec!["story-001".to_string()],
                acceptance_criteria: vec!["Feature works".to_string()],
            },
        ];

        let prd = build_task_prd("Build something", stories).unwrap();
        assert_eq!(prd.title, "PRD: Build something");
        assert_eq!(prd.description, "Build something");
        assert_eq!(prd.stories.len(), 2);
        // Batches should be calculated: story-001 in batch 0, story-002 in batch 1
        assert_eq!(prd.batches.len(), 2);
        assert!(prd.batches[0].story_ids.contains(&"story-001".to_string()));
        assert!(prd.batches[1].story_ids.contains(&"story-002".to_string()));
    }

    #[test]
    fn test_build_task_prd_with_circular_deps_returns_error() {
        let stories = vec![
            TaskStory {
                id: "story-001".to_string(),
                title: "A".to_string(),
                description: "".to_string(),
                priority: "high".to_string(),
                dependencies: vec!["story-002".to_string()],
                acceptance_criteria: vec![],
            },
            TaskStory {
                id: "story-002".to_string(),
                title: "B".to_string(),
                description: "".to_string(),
                priority: "high".to_string(),
                dependencies: vec!["story-001".to_string()],
                acceptance_criteria: vec![],
            },
        ];

        let result = build_task_prd("Circular", stories);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    #[test]
    fn test_build_task_prd_parallel_stories_in_same_batch() {
        let stories = vec![
            TaskStory {
                id: "story-001".to_string(),
                title: "A".to_string(),
                description: "".to_string(),
                priority: "high".to_string(),
                dependencies: vec![],
                acceptance_criteria: vec![],
            },
            TaskStory {
                id: "story-002".to_string(),
                title: "B".to_string(),
                description: "".to_string(),
                priority: "high".to_string(),
                dependencies: vec![],
                acceptance_criteria: vec![],
            },
            TaskStory {
                id: "story-003".to_string(),
                title: "C".to_string(),
                description: "".to_string(),
                priority: "medium".to_string(),
                dependencies: vec![],
                acceptance_criteria: vec![],
            },
        ];

        let prd = build_task_prd("Parallel tasks", stories).unwrap();
        // All 3 stories have no deps, should be in 1 batch (max_parallel=4)
        assert_eq!(prd.batches.len(), 1);
        assert_eq!(prd.batches[0].story_ids.len(), 3);
    }

    // ========================================================================
    // LLM Integration Tests (with mock)
    // ========================================================================

    #[tokio::test]
    async fn test_generate_prd_with_valid_response() {
        let mock_response = r#"[
            {
                "id": "story-001",
                "title": "Setup database",
                "description": "Create database schema",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["Schema created", "Migrations run"]
            },
            {
                "id": "story-002",
                "title": "Implement API",
                "description": "Build REST endpoints",
                "priority": "high",
                "dependencies": ["story-001"],
                "acceptanceCriteria": ["Endpoints respond", "Auth works"]
            }
        ]"#;

        let provider = Arc::new(MockLlmProvider::with_text_response(mock_response));
        let prd = generate_prd_with_llm(provider, "Build a web service", &[], 200_000)
            .await
            .unwrap();

        assert_eq!(prd.stories.len(), 2);
        assert_eq!(prd.stories[0].id, "story-001");
        assert_eq!(prd.stories[1].id, "story-002");
        assert_eq!(prd.batches.len(), 2); // Linear dependency chain
        assert_eq!(prd.title, "PRD: Build a web service");
    }

    #[tokio::test]
    async fn test_generate_prd_retry_on_invalid_first_response() {
        // First response: invalid JSON
        let first_response = Ok(LlmResponse {
            content: Some("Sure! Here are some stories for you...".to_string()),
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "mock".to_string(),
        });

        // Second response (after repair prompt): valid JSON
        let second_response = Ok(LlmResponse {
            content: Some(
                r#"[
                    {
                        "id": "story-001",
                        "title": "Only story",
                        "description": "The only story",
                        "priority": "high",
                        "dependencies": [],
                        "acceptanceCriteria": ["Done"]
                    }
                ]"#
                .to_string(),
            ),
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "mock".to_string(),
        });

        let provider = Arc::new(MockLlmProvider::with_responses(vec![
            first_response,
            second_response,
        ]));

        let prd = generate_prd_with_llm(provider, "Fix a bug", &[], 200_000)
            .await
            .unwrap();
        assert_eq!(prd.stories.len(), 1);
        assert_eq!(prd.stories[0].id, "story-001");
    }

    #[tokio::test]
    async fn test_generate_prd_fails_after_two_invalid_responses() {
        // Both responses are invalid
        let bad1 = Ok(LlmResponse {
            content: Some("not json".to_string()),
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "mock".to_string(),
        });

        let bad2 = Ok(LlmResponse {
            content: Some("still not json".to_string()),
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "mock".to_string(),
        });

        let provider = Arc::new(MockLlmProvider::with_responses(vec![bad1, bad2]));

        let result = generate_prd_with_llm(provider, "Some task", &[], 200_000).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Failed to generate PRD after retry"));
    }

    #[tokio::test]
    async fn test_generate_prd_fails_on_llm_error() {
        let provider = Arc::new(MockLlmProvider::with_responses(vec![Err(
            LlmError::NetworkError {
                message: "Connection refused".to_string(),
            },
        )]));

        let result = generate_prd_with_llm(provider, "Some task", &[], 200_000).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("LLM request failed"));
    }

    #[tokio::test]
    async fn test_generate_prd_fails_on_empty_content() {
        let provider = Arc::new(MockLlmProvider::with_responses(vec![Ok(LlmResponse {
            content: None,
            thinking: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
            usage: UsageStats::default(),
            model: "mock".to_string(),
        })]));

        let result = generate_prd_with_llm(provider, "Some task", &[], 200_000).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no text content"));
    }

    // ========================================================================
    // JSON Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_json_from_clean_array() {
        let input = r#"[{"id": "s1"}]"#;
        assert_eq!(extract_json_from_response(input), r#"[{"id": "s1"}]"#);
    }

    #[test]
    fn test_extract_json_from_markdown_fences() {
        let input = "```json\n[{\"id\": \"s1\"}]\n```";
        assert_eq!(extract_json_from_response(input), "[{\"id\": \"s1\"}]");
    }

    #[test]
    fn test_extract_json_from_text_with_brackets() {
        let input = "Here: [{\"id\": \"s1\"}] end.";
        assert_eq!(extract_json_from_response(input), "[{\"id\": \"s1\"}]");
    }

    // ========================================================================
    // Provider Factory Test
    // ========================================================================

    #[test]
    fn test_create_provider_anthropic() {
        let config = ProviderConfig {
            provider: crate::services::llm::types::ProviderType::Anthropic,
            api_key: Some("test-key".to_string()),
            model: "claude-3-5-sonnet-20241022".to_string(),
            ..Default::default()
        };
        let provider = create_provider(config);
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_create_provider_openai() {
        let config = ProviderConfig {
            provider: crate::services::llm::types::ProviderType::OpenAI,
            api_key: Some("test-key".to_string()),
            model: "gpt-4".to_string(),
            ..Default::default()
        };
        let provider = create_provider(config);
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_provider_ollama() {
        let config = ProviderConfig {
            provider: crate::services::llm::types::ProviderType::Ollama,
            model: "llama3.2".to_string(),
            ..Default::default()
        };
        let provider = create_provider(config);
        assert_eq!(provider.name(), "ollama");
    }

    // ========================================================================
    // Full Round-Trip Test
    // ========================================================================

    #[tokio::test]
    async fn test_full_prd_generation_roundtrip() {
        // This is the acceptance test: mock LLM produces a valid JSON response
        // and the full pipeline produces a correct TaskPrd with batches.
        let mock_json = r#"[
            {
                "id": "story-001",
                "title": "Create data model",
                "description": "Define the core data structures",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["Models defined", "Serialization works"]
            },
            {
                "id": "story-002",
                "title": "Implement storage layer",
                "description": "Build persistence with SQLite",
                "priority": "high",
                "dependencies": ["story-001"],
                "acceptanceCriteria": ["CRUD operations work", "Error handling"]
            },
            {
                "id": "story-003",
                "title": "Add API endpoints",
                "description": "REST API over the storage layer",
                "priority": "medium",
                "dependencies": ["story-002"],
                "acceptanceCriteria": ["GET /items returns list", "POST /items creates item"]
            },
            {
                "id": "story-004",
                "title": "Add tests",
                "description": "Unit and integration tests",
                "priority": "medium",
                "dependencies": ["story-001"],
                "acceptanceCriteria": ["80% coverage", "All tests pass"]
            }
        ]"#;

        let provider = Arc::new(MockLlmProvider::with_text_response(mock_json));
        let prd = generate_prd_with_llm(provider, "Build item management service", &[], 200_000)
            .await
            .unwrap();

        // Verify stories
        assert_eq!(prd.stories.len(), 4);

        // Verify batches respect dependencies
        // story-001: no deps -> batch 0
        // story-002: depends on story-001 -> batch 1
        // story-004: depends on story-001 -> batch 1
        // story-003: depends on story-002 -> batch 2
        assert!(prd.batches.len() >= 2);

        // Find which batch contains story-001
        let batch_0_ids: Vec<&str> = prd.batches[0]
            .story_ids
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert!(batch_0_ids.contains(&"story-001"));

        // story-003 should be in a later batch than story-002
        let story_002_batch = prd
            .batches
            .iter()
            .position(|b| b.story_ids.contains(&"story-002".to_string()))
            .unwrap();
        let story_003_batch = prd
            .batches
            .iter()
            .position(|b| b.story_ids.contains(&"story-003".to_string()))
            .unwrap();
        assert!(story_003_batch > story_002_batch);

        // Verify PRD metadata
        assert!(prd.title.contains("item management service"));
        assert!(!prd.description.is_empty());
    }

    // ========================================================================
    // Conversation History Compaction Tests
    // ========================================================================

    #[tokio::test]
    async fn test_compact_empty_history() {
        let provider: Arc<dyn LlmProvider> =
            Arc::new(MockLlmProvider::with_text_response("unused"));
        let messages = compact_conversation_history(&provider, &[], 200_000).await;
        assert!(messages.is_empty());
    }

    /// Helper to extract text from the first content block of a Message.
    fn msg_text(msg: &Message) -> &str {
        match &msg.content[0] {
            MessageContent::Text { text } => text.as_str(),
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_compact_history_within_budget() {
        let provider: Arc<dyn LlmProvider> =
            Arc::new(MockLlmProvider::with_text_response("unused"));
        let history = vec![
            ConversationTurnInput {
                user: "Hello".to_string(),
                assistant: "Hi there!".to_string(),
            },
            ConversationTurnInput {
                user: "How are you?".to_string(),
                assistant: "I'm fine.".to_string(),
            },
        ];

        let messages = compact_conversation_history(&provider, &history, 200_000).await;
        // Should have 4 messages: 2 turns * 2 messages each
        assert_eq!(messages.len(), 4);
        assert_eq!(msg_text(&messages[0]), "Hello");
        assert_eq!(msg_text(&messages[1]), "Hi there!");
        assert_eq!(msg_text(&messages[2]), "How are you?");
        assert_eq!(msg_text(&messages[3]), "I'm fine.");
    }

    #[tokio::test]
    async fn test_compact_history_over_budget_triggers_summary() {
        // Provider returns a summary when called
        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_text_response(
            "Summary: User discussed building a REST API with OAuth.",
        ));

        // Create history that exceeds a very small budget
        let history = vec![
            ConversationTurnInput {
                user: "I want to build a REST API with OAuth authentication".to_string(),
                assistant: "That's a great project! Let me help you plan it out. We'll need models, routes, middleware...".to_string(),
            },
            ConversationTurnInput {
                user: "Use Express.js with TypeScript".to_string(),
                assistant: "Perfect choice. I'll set up Express with TypeScript, including proper type definitions.".to_string(),
            },
            ConversationTurnInput {
                user: "Add rate limiting too".to_string(),
                assistant: "I'll add rate limiting using express-rate-limit.".to_string(),
            },
        ];

        // Very small budget forces compaction
        let messages =
            compact_conversation_history(&provider, &history, PRD_RESERVED_TOKENS + 100).await;

        // Should have summary pair + at least one recent turn
        assert!(messages.len() >= 4); // summary pair (2) + at least 1 recent turn (2)
                                      // First message should be the context marker
        assert_eq!(msg_text(&messages[0]), "[Prior conversation context]");
    }

    #[tokio::test]
    async fn test_generate_prd_with_conversation_history() {
        // The PRD response uses the 2nd mock response (1st is consumed by summary if needed)
        let mock_json = r#"[
            {
                "id": "story-001",
                "title": "Setup OAuth",
                "description": "Configure OAuth with Google",
                "priority": "high",
                "dependencies": [],
                "acceptanceCriteria": ["OAuth flow works"]
            }
        ]"#;

        let provider = Arc::new(MockLlmProvider::with_text_response(mock_json));
        let history = vec![ConversationTurnInput {
            user: "I want OAuth with Google".to_string(),
            assistant: "Sure, I'll help with Google OAuth.".to_string(),
        }];

        let prd = generate_prd_with_llm(provider, "Implement OAuth", &history, 200_000)
            .await
            .unwrap();

        assert_eq!(prd.stories.len(), 1);
        assert_eq!(prd.stories[0].title, "Setup OAuth");
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("1234"), 1);
        assert_eq!(estimate_tokens("12345678"), 2);
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }
}
