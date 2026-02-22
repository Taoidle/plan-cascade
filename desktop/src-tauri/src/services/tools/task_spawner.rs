//! Task Spawner
//!
//! Defines the trait and types for spawning sub-agent tasks with independent context windows.
//! Supports multiple sub-agent types with differentiated tool sets and natural multi-level
//! nesting (controlled by tool permissions rather than hardcoded depth limits).

use crate::services::llm::types::UsageStats;
use crate::services::streaming::unified::UnifiedStreamEvent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Maximum nesting depth for sub-agents. Root = 0, coordinator sub = 1, sub-sub = 2.
/// At depth >= max_depth, general-purpose agents cannot spawn further sub-agents.
pub const MAX_SUB_AGENT_DEPTH: u32 = 3;

/// Sub-agent type that determines tool access and behavioral role.
///
/// Different types have different tool sets:
/// - `GeneralPurpose`: All tools including Task — can act as coordinator
/// - `Explore`: Read tools + Bash (for git commands) for codebase exploration
/// - `Plan`: Same tools as Explore, focused on architecture design
/// - `Bash`: Only Bash + Cwd for shell command execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubAgentType {
    /// All tools including Task — coordinator that can spawn sub-agents
    GeneralPurpose,
    /// Read-only tools (Read, Glob, Grep, LS, Cwd, CodebaseSearch, WebFetch, WebSearch)
    Explore,
    /// Same read-only tools as Explore, focused on architecture design
    Plan,
    /// Only Bash + Cwd
    Bash,
}

impl SubAgentType {
    /// Parse from string with backward-compatible aliases.
    pub fn from_str_compat(s: &str) -> Self {
        match s {
            "general-purpose" | "general_purpose" => Self::GeneralPurpose,
            "explore" => Self::Explore,
            "plan" => Self::Plan,
            "bash" => Self::Bash,
            "analyze" => Self::Plan,           // backward compat
            "implement" => Self::GeneralPurpose, // backward compat
            _ => Self::Explore,
        }
    }

    /// Whether this type can spawn further sub-agents (has Task tool).
    pub fn can_spawn_subagents(&self) -> bool {
        matches!(self, Self::GeneralPurpose)
    }

    /// Tool names this type is allowed to use.
    pub fn allowed_tools(&self) -> &'static [&'static str] {
        match self {
            Self::GeneralPurpose => &[
                "Read", "Write", "Edit", "Bash", "Glob", "Grep", "LS", "Cwd",
                "Task", "WebFetch", "WebSearch", "NotebookEdit", "CodebaseSearch", "Browser",
            ],
            Self::Explore | Self::Plan => &[
                "Read", "Glob", "Grep", "LS", "Cwd", "CodebaseSearch", "Bash", "WebFetch", "WebSearch",
            ],
            Self::Bash => &["Bash", "Cwd"],
        }
    }

    /// Legacy task_type string for existing code paths.
    pub fn legacy_task_type(&self) -> &'static str {
        match self {
            Self::GeneralPurpose => "implement",
            Self::Explore => "explore",
            Self::Plan => "analyze",
            Self::Bash => "implement",
        }
    }
}

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
    /// - Have tool access determined by `subagent_type`
    /// - Forward streaming events to the provided channel
    /// - Respect the cancellation token from the parent
    async fn spawn_task(
        &self,
        prompt: String,
        subagent_type: SubAgentType,
        depth: u32,
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
    /// Current nesting depth (root = 0)
    pub depth: u32,
    /// Maximum allowed nesting depth
    pub max_depth: u32,
    /// Semaphore controlling the maximum number of concurrent sub-agent LLM calls.
    /// Prevents QPS bursts on rate-limited providers (e.g., GLM, Qwen).
    pub llm_semaphore: Arc<tokio::sync::Semaphore>,
}

/// Create a forwarding channel that wraps all sub-agent events as `SubAgentEvent`.
///
/// Events that already carry sub-agent identity (`SubAgentStart`, `SubAgentEnd`,
/// `SubAgentEvent`) and internal signals (`Usage`, `Complete`) are forwarded as-is.
/// All other events (TextDelta, ToolStart, ToolResult, etc.) are serialized and
/// wrapped in `SubAgentEvent { sub_agent_id, depth, event_type, event_data }`.
pub fn create_tagged_channel(
    sub_agent_id: String,
    depth: u32,
    parent_tx: mpsc::Sender<UnifiedStreamEvent>,
) -> mpsc::Sender<UnifiedStreamEvent> {
    let (child_tx, mut child_rx) = mpsc::channel::<UnifiedStreamEvent>(256);
    tokio::spawn(async move {
        while let Some(event) = child_rx.recv().await {
            let to_send = match &event {
                // Lifecycle events already carry sub_agent_id — forward as-is
                UnifiedStreamEvent::SubAgentStart { .. }
                | UnifiedStreamEvent::SubAgentEnd { .. }
                | UnifiedStreamEvent::SubAgentEvent { .. }
                // Internal signal events don't need wrapping
                | UnifiedStreamEvent::Usage { .. }
                | UnifiedStreamEvent::Complete { .. } => event,
                // All other events get wrapped as SubAgentEvent
                _other => {
                    if let Ok(json) = serde_json::to_value(&event) {
                        let event_type = json
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let mut event_data = json;
                        if let Some(obj) = event_data.as_object_mut() {
                            obj.remove("type");
                        }
                        UnifiedStreamEvent::SubAgentEvent {
                            sub_agent_id: sub_agent_id.clone(),
                            depth,
                            event_type,
                            event_data,
                        }
                    } else {
                        event // serialization failed — fall back to raw forwarding
                    }
                }
            };
            if parent_tx.send(to_send).await.is_err() {
                break;
            }
        }
    });
    child_tx
}
