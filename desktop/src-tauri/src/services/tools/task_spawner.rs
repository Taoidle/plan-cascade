//! Task Spawner
//!
//! Defines the trait and types for spawning sub-agent tasks with independent context windows.

use crate::services::llm::types::UsageStats;
use crate::services::streaming::unified::UnifiedStreamEvent;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Result of a sub-agent task execution
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    /// Final text response from the sub-agent
    pub response: Option<String>,
    /// Token usage during sub-agent execution
    pub usage: UsageStats,
    /// Number of agentic iterations performed
    pub iterations: u32,
    /// Whether the task completed successfully
    pub success: bool,
    /// Error message if the task failed
    pub error: Option<String>,
}

/// Trait for spawning sub-agent tasks
#[async_trait]
pub trait TaskSpawner: Send + Sync {
    /// Spawn a new sub-agent task with its own context window
    ///
    /// The sub-agent will:
    /// - Have its own independent conversation context
    /// - Have access to basic tools (Read, Write, Edit, Bash, Glob, Grep, LS, Cwd)
    /// - NOT have access to the Task tool (no recursion)
    /// - Forward streaming events to the provided channel
    /// - Respect the cancellation token from the parent
    async fn spawn_task(
        &self,
        prompt: String,
        task_type: Option<String>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        cancellation_token: CancellationToken,
    ) -> TaskExecutionResult;
}

/// Context passed to tool executor for Task tool access
pub struct TaskContext {
    /// The task spawner implementation
    pub spawner: Arc<dyn TaskSpawner>,
    /// Channel for streaming events back to the UI
    pub tx: mpsc::Sender<UnifiedStreamEvent>,
    /// Cancellation token from the parent orchestrator
    pub cancellation_token: CancellationToken,
}
