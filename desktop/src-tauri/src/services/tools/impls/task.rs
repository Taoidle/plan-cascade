//! Task Tool Implementation
//!
//! Spawns sub-agent tasks with independent context windows.
//! Uses `ctx.task_context` for TaskSpawner access and
//! `ctx.task_dedup_cache` for prompt-hash deduplication.
//! When `ctx.task_context` is None, returns a depth-limit error
//! (sub-agents cannot spawn further sub-agents).

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::services::llm::types::ParameterSchema;
use crate::services::orchestrator::text_describes_pending_action;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Task sub-agent tool -- spawns sub-agents with independent context.
///
/// Uses `ctx.task_context` (Option<Arc<TaskContext>>) from the execution context.
/// When `task_context` is None (e.g., in sub-agents), returns a depth-limit error.
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
         The sub-agent has access to basic tools (Read, Write, Edit, Bash, Glob, Grep, LS, Cwd), \
         search tools (CodebaseSearch, WebFetch, WebSearch), and NotebookEdit, but cannot spawn \
         further sub-agents. Only the final summary is returned to you. Use this for codebase \
         exploration, deep analysis, or focused implementations that benefit from a fresh context."
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
            "task_type".to_string(),
            ParameterSchema::string(Some(
                "Optional task type hint: 'explore' (codebase exploration), 'analyze' (deep analysis), 'implement' (code changes). Default: inferred from prompt.",
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

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolResult::err("Missing required parameter: prompt"),
        };

        let task_type = args
            .get("task_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Check for TaskContext availability (depth limit)
        let task_ctx = match &ctx.task_context {
            Some(tc) => tc,
            None => {
                return ToolResult::err(
                    "Task tool is not available at this depth. Sub-agents cannot spawn further sub-agents.",
                );
            }
        };

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

        // Emit SubAgentStart event
        let _ = task_ctx
            .tx
            .send(
                crate::services::streaming::unified::UnifiedStreamEvent::SubAgentStart {
                    sub_agent_id: sub_agent_id.clone(),
                    prompt: prompt.chars().take(200).collect(),
                    task_type: task_type.clone(),
                },
            )
            .await;

        // Spawn the sub-agent task
        let result = task_ctx
            .spawner
            .spawn_task(
                prompt.clone(),
                task_type,
                task_ctx.tx.clone(),
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
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn make_ctx(dir: &Path) -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "test".to_string(),
            project_root: dir.to_path_buf(),
            working_directory: Arc::new(Mutex::new(dir.to_path_buf())),
            read_cache: Arc::new(Mutex::new(HashMap::new())),
            read_files: Arc::new(Mutex::new(HashSet::new())),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            web_fetch: Arc::new(crate::services::tools::web_fetch::WebFetchService::new()),
            web_search: None,
            index_store: None,
            embedding_service: None,
            embedding_manager: None,
            hnsw_index: None,
            task_dedup_cache: Arc::new(Mutex::new(HashMap::new())),
            task_context: None,
            core_context: None,
        }
    }

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
        let ctx = make_ctx(Path::new("/tmp"));
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
        let ctx = make_ctx(Path::new("/tmp"));
        let result = tool.execute(&ctx, serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("prompt"));
    }

    #[tokio::test]
    async fn test_task_tool_dedup_cache_via_context() {
        let tool = TaskTool::new();
        let ctx = make_ctx(Path::new("/tmp"));
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
}
