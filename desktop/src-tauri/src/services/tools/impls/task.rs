//! Task Tool Implementation
//!
//! Spawns sub-agent tasks with independent context windows.
//! Includes prompt-hash deduplication cache.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use crate::services::llm::types::ParameterSchema;
use crate::services::tools::executor::ToolResult;
use crate::services::tools::trait_def::{Tool, ToolExecutionContext};

/// Task sub-agent tool — spawns sub-agents with independent context.
///
/// The actual sub-agent spawning is handled by ToolExecutor's execute_with_context
/// which has access to the TaskContext. This trait implementation provides the
/// tool definition and a fallback execute that returns an error (sub-agents cannot
/// spawn further sub-agents).
pub struct TaskTool {
    /// Task sub-agent deduplication cache.
    /// Keyed by hash of the prompt string. Only successful results are cached.
    task_dedup_cache: Mutex<HashMap<u64, String>>,
}

impl TaskTool {
    pub fn new() -> Self {
        Self {
            task_dedup_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Compute a hash of a prompt string
    pub fn hash_prompt(prompt: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        prompt.hash(&mut hasher);
        hasher.finish()
    }

    /// Clear the task deduplication cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.task_dedup_cache.lock() {
            cache.clear();
        }
    }

    /// Check the dedup cache for a prompt
    pub fn check_cache(&self, prompt: &str) -> Option<String> {
        let hash = Self::hash_prompt(prompt);
        self.task_dedup_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&hash).cloned())
    }

    /// Store a result in the dedup cache
    pub fn cache_result(&self, prompt: &str, result: &str) {
        let hash = Self::hash_prompt(prompt);
        if let Ok(mut cache) = self.task_dedup_cache.lock() {
            cache.insert(hash, result.to_string());
        }
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        "Launch a sub-agent with its own independent context window to handle complex tasks. The sub-agent has access to all basic tools (Read, Write, Edit, Bash, Glob, Grep, LS, Cwd) but cannot spawn further sub-agents. Only the final summary is returned to you. Use this for codebase exploration, deep analysis, or focused implementations that benefit from a fresh context."
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

    async fn execute(&self, _ctx: &ToolExecutionContext, _args: Value) -> ToolResult {
        // Task tool execution is handled by ToolExecutor's execute_with_context
        // which has access to the TaskContext/TaskSpawner. When called directly
        // through the trait (e.g., in sub-agents), it returns an error.
        ToolResult::err(
            "Task tool is not available at this depth. Sub-agents cannot spawn further sub-agents.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_task_tool_cache() {
        let tool = TaskTool::new();
        assert!(tool.check_cache("test").is_none());

        tool.cache_result("test", "result");
        assert_eq!(tool.check_cache("test"), Some("result".to_string()));

        tool.clear_cache();
        assert!(tool.check_cache("test").is_none());
    }
}
