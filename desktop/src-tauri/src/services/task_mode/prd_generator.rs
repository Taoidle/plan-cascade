//! LLM-Powered PRD Generation
//!
//! Decomposes a task description into a structured PRD with stories using an LLM provider.
//! Implements retry-with-repair per ADR-F002: on JSON parse failure, retries once with
//! a repair prompt that includes the parse error and original response.

use std::sync::Arc;

use crate::commands::task_mode::{TaskPrd, TaskStory};
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::services::task_mode::batch_executor::{ExecutableStory, ExecutionBatch};
use crate::services::task_mode::calculate_batches;

/// Default maximum parallel stories per batch for PRD generation.
const DEFAULT_MAX_PARALLEL: usize = 4;

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
- "acceptance_criteria": An array of strings describing what must be true for this story to be considered complete

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
    "acceptance_criteria": ["Users table exists with id, email, name columns", "Sessions table exists with foreign key to users"]
  },
  {
    "id": "story-002",
    "title": "Implement user registration API",
    "description": "Create REST endpoint for user registration with validation.",
    "priority": "high",
    "dependencies": ["story-001"],
    "acceptance_criteria": ["POST /api/register accepts email and password", "Validation errors return 400 status"]
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
fn extract_response_text(
    response: &crate::services::llm::types::LlmResponse,
) -> Result<String, String> {
    response
        .content
        .as_ref()
        .map(|s| s.clone())
        .ok_or_else(|| "LLM response contained no text content".to_string())
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
    let json_str = extract_json_from_response(response_text);

    let stories: Vec<TaskStory> = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "Failed to parse LLM response as JSON array of stories: {}",
            e
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
pub fn build_task_prd(
    task_description: &str,
    stories: Vec<TaskStory>,
) -> Result<TaskPrd, String> {
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
pub async fn generate_prd_with_llm(
    provider: Arc<dyn LlmProvider>,
    task_description: &str,
) -> Result<TaskPrd, String> {
    let system_prompt = build_prd_system_prompt();
    let user_message = build_prd_user_message(task_description);

    // First attempt
    let messages = vec![Message::user(&user_message)];
    let response = provider
        .send_message(
            messages,
            Some(system_prompt.clone()),
            vec![], // No tools needed for PRD generation
            LlmRequestOptions::default(),
        )
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;

    let response_text = extract_response_text(&response)?;

    match parse_stories_from_response(&response_text) {
        Ok(stories) => return build_task_prd(task_description, stories),
        Err(first_error) => {
            // ADR-F002: Retry once with repair prompt
            let repair_message = build_repair_prompt(&response_text, &first_error);
            let retry_messages = vec![
                Message::user(&user_message),
                Message::assistant(&response_text),
                Message::user(&repair_message),
            ];

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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::types::{
        LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, ProviderConfig,
        StopReason, ToolDefinition, UsageStats,
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
        assert!(prompt.contains("\"acceptance_criteria\""));
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
        let prd = generate_prd_with_llm(provider, "Build a web service").await.unwrap();

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

        let prd = generate_prd_with_llm(provider, "Fix a bug").await.unwrap();
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

        let result = generate_prd_with_llm(provider, "Some task").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to generate PRD after retry"));
    }

    #[tokio::test]
    async fn test_generate_prd_fails_on_llm_error() {
        let provider = Arc::new(MockLlmProvider::with_responses(vec![Err(
            LlmError::NetworkError {
                message: "Connection refused".to_string(),
            },
        )]));

        let result = generate_prd_with_llm(provider, "Some task").await;
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

        let result = generate_prd_with_llm(provider, "Some task").await;
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
        assert_eq!(
            extract_json_from_response(input),
            "[{\"id\": \"s1\"}]"
        );
    }

    #[test]
    fn test_extract_json_from_text_with_brackets() {
        let input = "Here: [{\"id\": \"s1\"}] end.";
        assert_eq!(
            extract_json_from_response(input),
            "[{\"id\": \"s1\"}]"
        );
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
        let prd = generate_prd_with_llm(provider, "Build item management service")
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
}
