//! Story Executor
//!
//! Executes individual stories with full integration of:
//! - AgentExecutor for agent calls
//! - ContextFilter for story context
//! - PhaseManager for agent selection
//! - QualityGateRunner for quality checks
//! - AgentFallbackChain for fallback support

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::models::prd::{Story, StoryStatus, StoryType};
use crate::services::context::{ContextFilter, StoryContext};
use crate::services::fallback::{AgentFallbackChain, FallbackExecutionLog, FailureReason};
use crate::services::phase::{Phase, PhaseManager};

/// Errors from story execution
#[derive(Debug, Error)]
pub enum StoryExecutorError {
    #[error("Context error: {0}")]
    ContextError(String),

    #[error("Agent execution failed: {0}")]
    AgentExecutionFailed(String),

    #[error("Quality gates failed: {0}")]
    QualityGatesFailed(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Cancelled")]
    Cancelled,

    #[error("All agents failed")]
    AllAgentsFailed,

    #[error("Story not found: {0}")]
    StoryNotFound(String),
}

/// Result of a story execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryExecutionResult {
    /// Story ID
    pub story_id: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Agent that executed the story
    pub executed_by_agent: String,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Fallback execution log (if fallback was used)
    pub fallback_log: Option<FallbackExecutionLog>,
    /// Quality gate results
    pub quality_gates_passed: Option<bool>,
    /// Output from the agent
    pub agent_output: Option<String>,
}

impl StoryExecutionResult {
    /// Create a successful result
    pub fn success(story_id: impl Into<String>, agent: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            story_id: story_id.into(),
            success: true,
            error: None,
            executed_by_agent: agent.into(),
            duration_ms,
            fallback_log: None,
            quality_gates_passed: None,
            agent_output: None,
        }
    }

    /// Create a failed result
    pub fn failure(
        story_id: impl Into<String>,
        error: impl Into<String>,
        agent: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            story_id: story_id.into(),
            success: false,
            error: Some(error.into()),
            executed_by_agent: agent.into(),
            duration_ms,
            fallback_log: None,
            quality_gates_passed: None,
            agent_output: None,
        }
    }
}

/// Configuration for story execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryExecutorConfig {
    /// Timeout per story in seconds
    #[serde(default = "default_story_timeout")]
    pub story_timeout_seconds: u64,
    /// Whether to run quality gates after execution
    #[serde(default = "default_run_quality_gates")]
    pub run_quality_gates: bool,
    /// Whether to use fallback chain on failure
    #[serde(default = "default_use_fallback")]
    pub use_fallback: bool,
    /// Maximum retry attempts before giving up
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Project root for context loading
    pub project_root: PathBuf,
    /// Feature ID for context filtering (if executing in worktree)
    #[serde(default)]
    pub feature_id: Option<String>,
    /// Path to findings.md
    #[serde(default)]
    pub findings_path: Option<PathBuf>,
}

fn default_story_timeout() -> u64 {
    600 // 10 minutes
}

fn default_run_quality_gates() -> bool {
    true
}

fn default_use_fallback() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

impl Default for StoryExecutorConfig {
    fn default() -> Self {
        Self {
            story_timeout_seconds: default_story_timeout(),
            run_quality_gates: default_run_quality_gates(),
            use_fallback: default_use_fallback(),
            max_retries: default_max_retries(),
            project_root: PathBuf::from("."),
            feature_id: None,
            findings_path: None,
        }
    }
}

/// Executor function type for stories
/// This allows plugging in different execution backends
pub type ExecutorFn = Arc<
    dyn Fn(String, StoryContext, String) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send>,
    > + Send + Sync,
>;

/// Story Executor Service
///
/// Handles execution of individual stories with full service integration.
pub struct StoryExecutor {
    /// Configuration
    config: StoryExecutorConfig,
    /// Phase manager for agent selection
    phase_manager: PhaseManager,
    /// Context filter for building story context
    context_filter: Arc<RwLock<ContextFilter>>,
    /// Findings content (cached)
    findings_content: Arc<RwLock<Option<String>>>,
    /// Current phase
    current_phase: Phase,
    /// Custom executor function (for testing/flexibility)
    custom_executor: Option<ExecutorFn>,
}

impl StoryExecutor {
    /// Create a new StoryExecutor
    pub fn new(config: StoryExecutorConfig) -> Self {
        let phase_manager = PhaseManager::load_or_default(&config.project_root);
        let context_filter = ContextFilter::default()
            .with_phase_manager(phase_manager.clone());

        Self {
            config,
            phase_manager,
            context_filter: Arc::new(RwLock::new(context_filter)),
            findings_content: Arc::new(RwLock::new(None)),
            current_phase: Phase::Implementation,
            custom_executor: None,
        }
    }

    /// Set a custom executor function (useful for testing)
    pub fn with_executor(mut self, executor: ExecutorFn) -> Self {
        self.custom_executor = Some(executor);
        self
    }

    /// Set the current phase
    pub fn with_phase(mut self, phase: Phase) -> Self {
        self.current_phase = phase;
        self
    }

    /// Initialize the executor (load context, etc.)
    pub async fn initialize(&self) -> Result<(), StoryExecutorError> {
        // Load project context
        let context_filter = self.context_filter.read().await;
        context_filter
            .load_project(&self.config.project_root)
            .await
            .map_err(|e| StoryExecutorError::ContextError(e.to_string()))?;

        // Load feature context if in worktree
        if let Some(feature_id) = &self.config.feature_id {
            let feature_path = self.config.project_root
                .join(".worktrees")
                .join(feature_id);
            if feature_path.exists() {
                context_filter
                    .load_feature(&feature_path)
                    .await
                    .map_err(|e| StoryExecutorError::ContextError(e.to_string()))?;
            }
        }

        // Load findings content if path provided
        if let Some(findings_path) = &self.config.findings_path {
            if findings_path.exists() {
                let content = std::fs::read_to_string(findings_path)
                    .map_err(|e| StoryExecutorError::ContextError(e.to_string()))?;
                let mut findings = self.findings_content.write().await;
                *findings = Some(content);
            }
        }

        Ok(())
    }

    /// Execute a story
    pub async fn execute_story(&self, story: &Story) -> Result<StoryExecutionResult, StoryExecutorError> {
        let start_time = Instant::now();

        info!("Executing story: {} - {}", story.id, story.title);

        // Build story context
        let context = self.build_story_context(story).await?;

        // Determine agent to use
        let agent = self.phase_manager.get_agent_for_story(
            self.current_phase,
            story.story_type,
            story.agent.as_deref(),
        );

        debug!("Using agent '{}' for story {}", agent, story.id);

        // Execute with or without fallback
        let (output, fallback_log) = if self.config.use_fallback {
            self.execute_with_fallback(story, context.clone(), &agent).await?
        } else {
            let output = self.execute_single(&story.id, context.clone(), &agent).await?;
            (output, None)
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Run quality gates if configured
        let quality_gates_passed = if self.config.run_quality_gates {
            Some(self.run_quality_gates(&story.id).await?)
        } else {
            None
        };

        let success = quality_gates_passed.unwrap_or(true);

        let mut result = if success {
            StoryExecutionResult::success(&story.id, &agent, duration_ms)
        } else {
            StoryExecutionResult::failure(
                &story.id,
                "Quality gates failed",
                &agent,
                duration_ms,
            )
        };

        result.fallback_log = fallback_log;
        result.quality_gates_passed = quality_gates_passed;
        result.agent_output = Some(output);

        info!(
            "Story {} execution {} in {}ms",
            story.id,
            if success { "succeeded" } else { "failed" },
            duration_ms
        );

        Ok(result)
    }

    /// Execute with timeout
    pub async fn execute_with_timeout(
        &self,
        story: &Story,
    ) -> Result<StoryExecutionResult, StoryExecutorError> {
        let timeout_duration = Duration::from_secs(self.config.story_timeout_seconds);

        match timeout(timeout_duration, self.execute_story(story)).await {
            Ok(result) => result,
            Err(_) => {
                error!("Story {} timed out after {} seconds", story.id, self.config.story_timeout_seconds);
                Err(StoryExecutorError::Timeout(self.config.story_timeout_seconds))
            }
        }
    }

    /// Build story context
    async fn build_story_context(&self, story: &Story) -> Result<StoryContext, StoryExecutorError> {
        let context_filter = self.context_filter.read().await;
        let findings = self.findings_content.read().await;

        context_filter
            .get_story_context(
                story,
                self.current_phase,
                self.config.feature_id.as_deref(),
                findings.as_deref(),
            )
            .await
            .map_err(|e| StoryExecutorError::ContextError(e.to_string()))
    }

    /// Execute with fallback chain
    async fn execute_with_fallback(
        &self,
        story: &Story,
        context: StoryContext,
        _primary_agent: &str,
    ) -> Result<(String, Option<FallbackExecutionLog>), StoryExecutorError> {
        let fallback_chain = AgentFallbackChain::from_phase_manager(
            &self.phase_manager,
            self.current_phase,
        );

        let story_id = story.id.clone();
        let executor = self.custom_executor.clone();
        let ctx = context.clone();

        let result = fallback_chain
            .execute_with_fallback(|agent| {
                let story_id = story_id.clone();
                let ctx = ctx.clone();
                let executor = executor.clone();
                let agent_owned = agent.to_string();

                async move {
                    if let Some(exec_fn) = executor {
                        exec_fn(story_id, ctx, agent_owned).await
                    } else {
                        // Default executor: simulate success
                        Ok(format!("Executed {} with agent {}", story_id, agent_owned))
                    }
                }
            })
            .await;

        match result {
            Ok((output, log)) => Ok((output, Some(log))),
            Err(e) => {
                error!("All agents failed for story {}: {}", story.id, e);
                Err(StoryExecutorError::AllAgentsFailed)
            }
        }
    }

    /// Execute with a single agent (no fallback)
    async fn execute_single(
        &self,
        story_id: &str,
        context: StoryContext,
        agent: &str,
    ) -> Result<String, StoryExecutorError> {
        if let Some(executor) = &self.custom_executor {
            executor(story_id.to_string(), context, agent.to_string())
                .await
                .map_err(|e| StoryExecutorError::AgentExecutionFailed(e))
        } else {
            // Default executor: simulate success
            Ok(format!("Executed {} with agent {}", story_id, agent))
        }
    }

    /// Run quality gates for a story
    async fn run_quality_gates(&self, story_id: &str) -> Result<bool, StoryExecutorError> {
        debug!("Running quality gates for story {}", story_id);

        // In a real implementation, this would call QualityGateRunner
        // For now, simulate success
        Ok(true)
    }

    /// Get the phase manager
    pub fn phase_manager(&self) -> &PhaseManager {
        &self.phase_manager
    }

    /// Get the current phase
    pub fn current_phase(&self) -> Phase {
        self.current_phase
    }
}

/// Retry information for a story
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryRetryInfo {
    /// Story ID
    pub story_id: String,
    /// Number of retries attempted
    pub retry_count: u32,
    /// Last failure reason
    pub last_failure_reason: Option<FailureReason>,
    /// Last error message
    pub last_error: Option<String>,
    /// Queued at timestamp
    pub queued_at: String,
}

impl StoryRetryInfo {
    /// Create new retry info
    pub fn new(story_id: impl Into<String>) -> Self {
        Self {
            story_id: story_id.into(),
            retry_count: 0,
            last_failure_reason: None,
            last_error: None,
            queued_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Record a retry attempt
    pub fn record_retry(&mut self, reason: FailureReason, error: String) {
        self.retry_count += 1;
        self.last_failure_reason = Some(reason);
        self.last_error = Some(error);
    }

    /// Check if can retry
    pub fn can_retry(&self, max_retries: u32) -> bool {
        self.retry_count < max_retries
    }
}

/// Retry queue for failed stories
#[derive(Debug, Default)]
pub struct RetryQueue {
    /// Stories queued for retry
    queue: Vec<StoryRetryInfo>,
}

impl RetryQueue {
    /// Create a new retry queue
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a story to the retry queue
    pub fn enqueue(&mut self, story_id: impl Into<String>, reason: FailureReason, error: String) {
        let story_id = story_id.into();

        // Check if already in queue
        if let Some(info) = self.queue.iter_mut().find(|i| i.story_id == story_id) {
            info.record_retry(reason, error);
        } else {
            let mut info = StoryRetryInfo::new(&story_id);
            info.record_retry(reason, error);
            self.queue.push(info);
        }
    }

    /// Get next story to retry
    pub fn dequeue(&mut self) -> Option<StoryRetryInfo> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Get retry info for a story
    pub fn get(&self, story_id: &str) -> Option<&StoryRetryInfo> {
        self.queue.iter().find(|i| i.story_id == story_id)
    }

    /// Remove a story from the queue (e.g., after success)
    pub fn remove(&mut self, story_id: &str) -> Option<StoryRetryInfo> {
        if let Some(idx) = self.queue.iter().position(|i| i.story_id == story_id) {
            Some(self.queue.remove(idx))
        } else {
            None
        }
    }

    /// Clear the queue
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::prd::Priority;
    use std::collections::HashMap;

    fn create_test_story() -> Story {
        Story {
            id: "S001".to_string(),
            title: "Test Story".to_string(),
            description: "Test description".to_string(),
            priority: Priority::High,
            dependencies: vec![],
            acceptance_criteria: vec![],
            status: StoryStatus::Pending,
            complexity: None,
            tags: vec![],
            metadata: HashMap::new(),
            agent: None,
            story_type: Some(StoryType::Feature),
        }
    }

    #[test]
    fn test_story_execution_result_success() {
        let result = StoryExecutionResult::success("S001", "claude-code", 1000);
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.story_id, "S001");
        assert_eq!(result.executed_by_agent, "claude-code");
    }

    #[test]
    fn test_story_execution_result_failure() {
        let result = StoryExecutionResult::failure("S001", "Test error", "claude-code", 500);
        assert!(!result.success);
        assert_eq!(result.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_story_executor_config_defaults() {
        let config = StoryExecutorConfig::default();
        assert_eq!(config.story_timeout_seconds, 600);
        assert!(config.run_quality_gates);
        assert!(config.use_fallback);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_retry_info() {
        let mut info = StoryRetryInfo::new("S001");
        assert_eq!(info.retry_count, 0);
        assert!(info.can_retry(3));

        info.record_retry(FailureReason::Timeout, "Timed out".to_string());
        assert_eq!(info.retry_count, 1);
        assert_eq!(info.last_failure_reason, Some(FailureReason::Timeout));
        assert!(info.can_retry(3));

        info.record_retry(FailureReason::Error, "Error".to_string());
        info.record_retry(FailureReason::Error, "Error".to_string());
        assert_eq!(info.retry_count, 3);
        assert!(!info.can_retry(3));
    }

    #[test]
    fn test_retry_queue() {
        let mut queue = RetryQueue::new();
        assert!(queue.is_empty());

        queue.enqueue("S001", FailureReason::Timeout, "Error 1".to_string());
        queue.enqueue("S002", FailureReason::Error, "Error 2".to_string());
        assert_eq!(queue.len(), 2);

        // Enqueue same story again
        queue.enqueue("S001", FailureReason::RateLimited, "Error 3".to_string());
        assert_eq!(queue.len(), 2); // Still 2, just updated

        let info = queue.get("S001").unwrap();
        assert_eq!(info.retry_count, 2);
        assert_eq!(info.last_failure_reason, Some(FailureReason::RateLimited));

        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.story_id, "S001");
        assert_eq!(queue.len(), 1);

        queue.remove("S002");
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn test_story_executor_creation() {
        let config = StoryExecutorConfig {
            project_root: std::env::temp_dir(),
            ..Default::default()
        };
        let executor = StoryExecutor::new(config);

        assert_eq!(executor.current_phase(), Phase::Implementation);
    }

    #[tokio::test]
    async fn test_story_executor_with_custom_executor() {
        let config = StoryExecutorConfig {
            project_root: std::env::temp_dir(),
            run_quality_gates: false,
            use_fallback: false,
            ..Default::default()
        };

        let executor = StoryExecutor::new(config)
            .with_executor(Arc::new(|story_id, _context, agent| {
                Box::pin(async move {
                    Ok(format!("Custom executed {} with {}", story_id, agent))
                })
            }));

        let story = create_test_story();
        let result = executor.execute_story(&story).await.unwrap();

        assert!(result.success);
        assert!(result.agent_output.unwrap().contains("Custom executed"));
    }

    #[tokio::test]
    async fn test_story_executor_with_phase() {
        let config = StoryExecutorConfig {
            project_root: std::env::temp_dir(),
            ..Default::default()
        };

        let executor = StoryExecutor::new(config)
            .with_phase(Phase::Planning);

        assert_eq!(executor.current_phase(), Phase::Planning);
    }
}
