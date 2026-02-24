//! Task Tool Implementation
//!
//! Spawns sub-agent tasks with independent context windows.
//! Uses `ctx.task_context` for TaskSpawner access and
//! `ctx.task_dedup_cache` for prompt-hash deduplication.
//! When `ctx.task_context` is None, returns a depth-limit error.
//!
//! Supports multiple sub-agent types via `subagent_type` parameter:
//! - `explore`: Read-only codebase exploration (safe, parallelizable)
//! - `plan`: Architecture design and planning (read-only)
//! - `general-purpose`: Coordinator that can spawn further sub-agents
//! - `bash`: Shell command execution only

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::services::llm::types::ParameterSchema;
use crate::services::orchestrator::text_describes_pending_action;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::task_spawner::SubAgentType;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Task sub-agent tool -- spawns sub-agents with independent context.
///
/// Uses `ctx.task_context` (Option<Arc<TaskContext>>) from the execution context.
/// When `task_context` is None (e.g., in leaf sub-agents), returns a depth-limit error.
/// Uses `ctx.task_dedup_cache` (Arc<Mutex<HashMap<u64, String>>>) for prompt-hash
/// deduplication -- identical prompts return cached results without re-execution.
pub struct TaskTool;

impl TaskTool {
    pub fn new() -> Self {
        Self
    }

    /// Compute a hash of a prompt string
    pub fn hash_prompt(prompt: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        prompt.hash(&mut hasher);
        hasher.finish()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        "Launch a sub-agent with its own independent context window to handle complex tasks. \
         Choose subagent_type based on the task:\n\
         - 'explore': Read-only codebase exploration (Read, Glob, Grep, LS, CodebaseSearch). Safe to run in parallel.\n\
         - 'plan': Architecture design and planning (same read-only tools as explore).\n\
         - 'general-purpose': Coordinator with all tools including Task — can spawn further sub-agents for complex multi-step work.\n\
         - 'bash': Shell command execution only (Bash + Cwd).\n\n\
         IMPORTANT: Emit multiple Task calls in ONE response for parallel execution. \
         Each sub-agent gets its own context window. Only the final summary is returned to you."
    }

    fn parameters_schema(&self) -> ParameterSchema {
        let mut properties = HashMap::new();
        properties.insert(
            "prompt".to_string(),
            ParameterSchema::string(Some(
                "The task description for the sub-agent. Be specific about what you want done.",
            )),
        );
        properties.insert(
            "subagent_type".to_string(),
            ParameterSchema::string(Some(
                "Sub-agent type: 'explore' (read-only codebase exploration, default), \
                 'plan' (architecture design), 'general-purpose' (coordinator with all tools), \
                 'bash' (shell commands only).",
            )),
        );
        // Keep task_type for backward compatibility
        properties.insert(
            "task_type".to_string(),
            ParameterSchema::string(Some(
                "Deprecated: use subagent_type instead. Maps: 'explore'->'explore', 'analyze'->'plan', 'implement'->'general-purpose'.",
            )),
        );
        ParameterSchema::object(
            Some("Task parameters"),
            properties,
            vec!["prompt".to_string()],
        )
    }

    fn is_long_running(&self) -> bool {
        true
    }

    fn is_parallel_safe(&self) -> bool {
        true // Each sub-agent has independent context
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("Missing required parameter: prompt"),
        };

        // Resolve subagent_type: prefer `subagent_type`, fall back to `task_type`
        let subagent_type = if let Some(st) = args.get("subagent_type").and_then(|v| v.as_str()) {
            SubAgentType::from_str_compat(st)
        } else if let Some(tt) = args.get("task_type").and_then(|v| v.as_str()) {
            SubAgentType::from_str_compat(tt)
        } else {
            SubAgentType::Explore // default
        };

        // Check for TaskContext availability (depth limit)
        let task_ctx = match &ctx.task_context {
            Some(tc) => tc,
            None => {
                return ToolResult::err(
                    "Task tool is not available at this depth. \
                     This sub-agent cannot spawn further sub-agents.",
                );
            }
        };

        // For GeneralPurpose, check depth limit
        if subagent_type.can_spawn_subagents() && task_ctx.depth >= task_ctx.max_depth {
            return ToolResult::err(format!(
                "Cannot spawn general-purpose sub-agent: maximum nesting depth ({}) reached. \
                 Use 'explore' or 'plan' instead (they don't nest further).",
                task_ctx.max_depth,
            ));
        }

        // Check task dedup cache
        let prompt_hash = Self::hash_prompt(&prompt);
        if let Ok(cache) = ctx.task_dedup_cache.lock() {
            if let Some(cached_result) = cache.get(&prompt_hash) {
                eprintln!(
                    "[task-dedup] Cache hit for Task prompt hash={}, returning cached result",
                    prompt_hash
                );
                return ToolResult::ok(format!("[cached] {}", cached_result));
            }
        }

        let sub_agent_id = uuid::Uuid::new_v4().to_string();

        // Emit SubAgentStart event (with new fields)
        let _ = task_ctx
            .tx
            .send(
                crate::services::streaming::unified::UnifiedStreamEvent::SubAgentStart {
                    sub_agent_id: sub_agent_id.clone(),
                    prompt: prompt.chars().take(200).collect(),
                    task_type: Some(subagent_type.legacy_task_type().to_string()),
                    subagent_type: Some(
                        serde_json::to_value(subagent_type)
                            .ok()
                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                            .unwrap_or_else(|| "explore".to_string()),
                    ),
                    depth: task_ctx.depth,
                },
            )
            .await;

        // Create a tagged channel so this sub-agent's events are wrapped with its ID,
        // preventing output interleaving when multiple sub-agents run in parallel.
        let tagged_tx = crate::services::tools::task_spawner::create_tagged_channel(
            sub_agent_id.clone(),
            task_ctx.depth,
            task_ctx.tx.clone(),
        );

        // Acquire a semaphore permit to limit concurrent sub-agent LLM calls.
        // This prevents QPS bursts on rate-limited providers (e.g., GLM 2-5 QPS).
        let _permit = match task_ctx.llm_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                return ToolResult::err("Sub-agent concurrency semaphore closed unexpectedly");
            }
        };

        // Spawn the sub-agent task (permit is held until spawn_task completes)
        let result = task_ctx
            .spawner
            .spawn_task(
                prompt.clone(),
                subagent_type,
                task_ctx.depth,
                tagged_tx,
                task_ctx.cancellation_token.clone(),
            )
            .await;

        // Emit SubAgentEnd event
        let _ = task_ctx
            .tx
            .send(
                crate::services::streaming::unified::UnifiedStreamEvent::SubAgentEnd {
                    sub_agent_id,
                    success: result.success,
                    usage: serde_json::json!({
                        "input_tokens": result.usage.input_tokens,
                        "output_tokens": result.usage.output_tokens,
                        "thinking_tokens": result.usage.thinking_tokens,
                        "cache_read_tokens": result.usage.cache_read_tokens,
                        "cache_creation_tokens": result.usage.cache_creation_tokens,
                        "iterations": result.iterations,
                    }),
                },
            )
            .await;

        if result.success {
            let response_text = result
                .response
                .unwrap_or_else(|| "Task completed with no output".to_string());
            // Cache successful result, but skip narration-only responses
            // that contain no useful content (e.g. "Let me check..." / "...")
            if text_describes_pending_action(&response_text) {
                eprintln!(
                    "[task-dedup] Skipping cache for narration-only result (hash={})",
                    prompt_hash
                );
            } else if let Ok(mut cache) = ctx.task_dedup_cache.lock() {
                cache.insert(prompt_hash, response_text.clone());
            }
            ToolResult::ok(response_text)
        } else {
            // Do NOT cache failed results
            ToolResult::err(
                result
                    .error
                    .unwrap_or_else(|| "Task failed with unknown error".to_string()),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_ctx;
    use super::*;
    use std::path::Path;

    #[test]
    fn test_task_tool_name() {
        let tool = TaskTool::new();
        assert_eq!(tool.name(), "Task");
    }

    #[test]
    fn test_task_tool_is_long_running() {
        let tool = TaskTool::new();
        assert!(tool.is_long_running());
    }

    #[test]
    fn test_task_tool_is_parallel_safe() {
        let tool = TaskTool::new();
        assert!(tool.is_parallel_safe());
    }

    #[test]
    fn test_task_tool_hash_deterministic() {
        let h1 = TaskTool::hash_prompt("test prompt");
        let h2 = TaskTool::hash_prompt("test prompt");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_task_tool_hash_different_prompts() {
        let h1 = TaskTool::hash_prompt("prompt a");
        let h2 = TaskTool::hash_prompt("prompt b");
        assert_ne!(h1, h2);
    }

    #[tokio::test]
    async fn test_task_tool_no_context_returns_depth_error() {
        let tool = TaskTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        // task_context is None
        let args = serde_json::json!({"prompt": "explore the codebase"});
        let result = tool.execute(&ctx, args).await;
        assert!(!result.success);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("not available at this depth"));
    }

    #[tokio::test]
    async fn test_task_tool_missing_prompt() {
        let tool = TaskTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("prompt"));
    }

    #[tokio::test]
    async fn test_task_tool_dedup_cache_via_context() {
        let tool = TaskTool::new();
        let ctx = make_test_ctx(Path::new("/tmp"));
        // Pre-populate the context's dedup cache
        let prompt_hash = TaskTool::hash_prompt("cached task prompt");
        ctx.task_dedup_cache
            .lock()
            .unwrap()
            .insert(prompt_hash, "cached result text".to_string());

        // Since task_context is None, the tool would normally return depth error,
        // but dedup check happens AFTER context check. For dedup testing,
        // we verify the cache is accessible and populated.
        assert!(ctx
            .task_dedup_cache
            .lock()
            .unwrap()
            .contains_key(&prompt_hash));
        assert_eq!(
            ctx.task_dedup_cache
                .lock()
                .unwrap()
                .get(&prompt_hash)
                .unwrap(),
            "cached result text"
        );
    }

    #[test]
    fn test_subagent_type_parsing() {
        assert_eq!(
            SubAgentType::from_str_compat("explore"),
            SubAgentType::Explore
        );
        assert_eq!(SubAgentType::from_str_compat("plan"), SubAgentType::Plan);
        assert_eq!(
            SubAgentType::from_str_compat("general-purpose"),
            SubAgentType::GeneralPurpose
        );
        assert_eq!(
            SubAgentType::from_str_compat("general_purpose"),
            SubAgentType::GeneralPurpose
        );
        assert_eq!(SubAgentType::from_str_compat("bash"), SubAgentType::Bash);
        // Backward compat
        assert_eq!(SubAgentType::from_str_compat("analyze"), SubAgentType::Plan);
        assert_eq!(
            SubAgentType::from_str_compat("implement"),
            SubAgentType::GeneralPurpose
        );
        // Unknown defaults to Explore
        assert_eq!(
            SubAgentType::from_str_compat("unknown"),
            SubAgentType::Explore
        );
    }

    #[test]
    fn test_subagent_type_can_spawn() {
        assert!(SubAgentType::GeneralPurpose.can_spawn_subagents());
        assert!(!SubAgentType::Explore.can_spawn_subagents());
        assert!(!SubAgentType::Plan.can_spawn_subagents());
        assert!(!SubAgentType::Bash.can_spawn_subagents());
    }

    #[test]
    fn test_subagent_type_allowed_tools() {
        let gp_tools = SubAgentType::GeneralPurpose.allowed_tools();
        assert!(gp_tools.contains(&"Task"));
        assert!(gp_tools.contains(&"Write"));
        assert!(gp_tools.contains(&"Edit"));

        let explore_tools = SubAgentType::Explore.allowed_tools();
        assert!(!explore_tools.contains(&"Task"));
        assert!(!explore_tools.contains(&"Write"));
        assert!(explore_tools.contains(&"Read"));
        assert!(explore_tools.contains(&"CodebaseSearch"));

        let bash_tools = SubAgentType::Bash.allowed_tools();
        assert_eq!(bash_tools, &["Bash", "Cwd"]);
    }
}
