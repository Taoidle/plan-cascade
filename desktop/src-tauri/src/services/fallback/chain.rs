//! Agent Fallback Chain
//!
//! Implements fallback execution when primary agents fail.
//! Supports configurable fallback chains with failure classification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::services::phase::{Phase, PhaseManager};

/// Reasons an agent execution can fail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    /// Agent is not available/configured
    Unavailable,
    /// Agent timed out
    Timeout,
    /// Agent returned an error
    Error,
    /// Rate limited
    RateLimited,
    /// Network/connection error
    NetworkError,
    /// Invalid response
    InvalidResponse,
    /// User cancelled
    Cancelled,
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureReason::Unavailable => write!(f, "unavailable"),
            FailureReason::Timeout => write!(f, "timeout"),
            FailureReason::Error => write!(f, "error"),
            FailureReason::RateLimited => write!(f, "rate_limited"),
            FailureReason::NetworkError => write!(f, "network_error"),
            FailureReason::InvalidResponse => write!(f, "invalid_response"),
            FailureReason::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FailureReason {
    /// Check if this failure reason should trigger a fallback
    pub fn should_fallback(&self) -> bool {
        match self {
            FailureReason::Unavailable => true,
            FailureReason::Timeout => true,
            FailureReason::Error => true,
            FailureReason::RateLimited => true,
            FailureReason::NetworkError => true,
            FailureReason::InvalidResponse => true,
            FailureReason::Cancelled => false, // Don't fallback on user cancellation
        }
    }

    /// Classify an error message into a failure reason
    pub fn from_error_message(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("unavailable") || msg_lower.contains("not found") || msg_lower.contains("not configured") {
            FailureReason::Unavailable
        } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            FailureReason::Timeout
        } else if msg_lower.contains("rate limit") || msg_lower.contains("too many requests") || msg_lower.contains("429") {
            FailureReason::RateLimited
        } else if msg_lower.contains("network") || msg_lower.contains("connection") || msg_lower.contains("socket") {
            FailureReason::NetworkError
        } else if msg_lower.contains("invalid") || msg_lower.contains("parse") || msg_lower.contains("deserialize") {
            FailureReason::InvalidResponse
        } else if msg_lower.contains("cancel") || msg_lower.contains("abort") {
            FailureReason::Cancelled
        } else {
            FailureReason::Error
        }
    }
}

/// Errors from fallback execution
#[derive(Debug, Error)]
pub enum FallbackError {
    #[error("All agents in fallback chain failed")]
    AllAgentsFailed,

    #[error("No fallback agents configured")]
    NoFallbackAgents,

    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Execution cancelled")]
    Cancelled,
}

/// Result type for fallback operations
pub type FallbackResult<T> = Result<T, FallbackError>;

/// Configuration for fallback behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Maximum number of fallback attempts
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
    /// Delay between attempts in milliseconds
    #[serde(default = "default_delay_ms")]
    pub delay_between_attempts_ms: u64,
    /// Timeout per attempt in seconds
    #[serde(default = "default_timeout")]
    pub timeout_per_attempt_seconds: u64,
    /// Whether to log all attempts
    #[serde(default = "default_log_attempts")]
    pub log_all_attempts: bool,
}

fn default_max_attempts() -> usize {
    3
}

fn default_delay_ms() -> u64 {
    500
}

fn default_timeout() -> u64 {
    600
}

fn default_log_attempts() -> bool {
    true
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            delay_between_attempts_ms: default_delay_ms(),
            timeout_per_attempt_seconds: default_timeout(),
            log_all_attempts: default_log_attempts(),
        }
    }
}

/// Record of a single fallback attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackAttempt {
    /// Agent that was tried
    pub agent: String,
    /// Whether this attempt succeeded
    pub success: bool,
    /// Failure reason if failed
    pub failure_reason: Option<FailureReason>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Duration of the attempt in milliseconds
    pub duration_ms: u64,
    /// Timestamp when attempt started
    pub started_at: String,
}

impl FallbackAttempt {
    /// Create a successful attempt record
    pub fn success(agent: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            agent: agent.into(),
            success: true,
            failure_reason: None,
            error_message: None,
            duration_ms,
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a failed attempt record
    pub fn failure(
        agent: impl Into<String>,
        reason: FailureReason,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            agent: agent.into(),
            success: false,
            failure_reason: Some(reason),
            error_message: Some(error.into()),
            duration_ms,
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Log of all fallback execution attempts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FallbackExecutionLog {
    /// All attempts made
    pub attempts: Vec<FallbackAttempt>,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Final agent that succeeded (if any)
    pub successful_agent: Option<String>,
    /// Whether execution ultimately succeeded
    pub overall_success: bool,
}

impl FallbackExecutionLog {
    /// Create a new log
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an attempt to the log
    pub fn add_attempt(&mut self, attempt: FallbackAttempt) {
        self.total_duration_ms += attempt.duration_ms;
        if attempt.success {
            self.successful_agent = Some(attempt.agent.clone());
            self.overall_success = true;
        }
        self.attempts.push(attempt);
    }

    /// Get the number of failed attempts before success
    pub fn failed_attempts_count(&self) -> usize {
        self.attempts.iter().filter(|a| !a.success).count()
    }
}

/// Agent Fallback Chain
///
/// Manages fallback execution when primary agents fail.
/// Integrates with PhaseManager for phase-specific fallback chains.
#[derive(Debug, Clone)]
pub struct AgentFallbackChain {
    /// Primary agent
    primary_agent: String,
    /// Fallback agents in order of preference
    fallback_agents: Vec<String>,
    /// Fallback configuration
    config: FallbackConfig,
}

impl AgentFallbackChain {
    /// Create a new fallback chain with the given primary agent
    pub fn new(primary_agent: impl Into<String>) -> Self {
        Self {
            primary_agent: primary_agent.into(),
            fallback_agents: vec!["claude-code".to_string()], // Default fallback
            config: FallbackConfig::default(),
        }
    }

    /// Create from PhaseManager for a specific phase
    pub fn from_phase_manager(phase_manager: &PhaseManager, phase: Phase) -> Self {
        let primary = phase_manager.get_agent_for_phase(phase).to_string();
        let fallbacks: Vec<String> = phase_manager
            .get_fallback_chain(phase)
            .into_iter()
            .map(|s| s.to_string())
            .filter(|s| s != &primary)
            .collect();

        Self {
            primary_agent: primary,
            fallback_agents: if fallbacks.is_empty() {
                vec!["claude-code".to_string()]
            } else {
                fallbacks
            },
            config: FallbackConfig {
                timeout_per_attempt_seconds: phase_manager.get_timeout(phase),
                max_attempts: phase_manager.get_max_retries(phase) as usize,
                ..Default::default()
            },
        }
    }

    /// Set the fallback agents
    pub fn with_fallbacks(mut self, agents: Vec<String>) -> Self {
        self.fallback_agents = agents;
        self
    }

    /// Add a fallback agent
    pub fn add_fallback(mut self, agent: impl Into<String>) -> Self {
        self.fallback_agents.push(agent.into());
        self
    }

    /// Set the configuration
    pub fn with_config(mut self, config: FallbackConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the primary agent
    pub fn primary_agent(&self) -> &str {
        &self.primary_agent
    }

    /// Get all agents in order (primary + fallbacks)
    pub fn all_agents(&self) -> Vec<&str> {
        let mut agents = vec![self.primary_agent.as_str()];
        agents.extend(self.fallback_agents.iter().map(|s| s.as_str()));
        agents
    }

    /// Get the next agent to try after a failure
    pub fn next_agent_after(&self, current: &str) -> Option<&str> {
        let all = self.all_agents();
        let current_idx = all.iter().position(|&a| a == current)?;
        all.get(current_idx + 1).copied()
    }

    /// Execute with fallback support
    ///
    /// Tries the primary agent first, then falls back to alternatives on failure.
    /// The executor function receives the agent name and returns a Result.
    pub async fn execute_with_fallback<F, Fut, T, E>(
        &self,
        mut executor: F,
    ) -> FallbackResult<(T, FallbackExecutionLog)>
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut log = FallbackExecutionLog::new();
        let start_time = Instant::now();
        let mut attempts = 0;

        for agent in self.all_agents() {
            if attempts >= self.config.max_attempts {
                warn!("Max fallback attempts ({}) reached", self.config.max_attempts);
                break;
            }

            info!("Attempting execution with agent: {}", agent);
            let attempt_start = Instant::now();

            match executor(agent).await {
                Ok(result) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    info!("Agent {} succeeded in {}ms", agent, duration_ms);
                    log.add_attempt(FallbackAttempt::success(agent, duration_ms));
                    return Ok((result, log));
                }
                Err(e) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();
                    let reason = FailureReason::from_error_message(&error_msg);

                    warn!(
                        "Agent {} failed: {} (reason: {})",
                        agent, error_msg, reason
                    );

                    log.add_attempt(FallbackAttempt::failure(
                        agent,
                        reason,
                        &error_msg,
                        duration_ms,
                    ));

                    // Check if we should continue fallback
                    if !reason.should_fallback() {
                        error!("Failure reason {} does not allow fallback", reason);
                        return Err(FallbackError::ExecutionFailed(error_msg));
                    }

                    attempts += 1;

                    // Delay before next attempt
                    if self.config.delay_between_attempts_ms > 0 && attempts < self.config.max_attempts {
                        debug!("Waiting {}ms before next attempt", self.config.delay_between_attempts_ms);
                        tokio::time::sleep(Duration::from_millis(self.config.delay_between_attempts_ms)).await;
                    }
                }
            }
        }

        error!("All agents in fallback chain failed after {} attempts", log.attempts.len());
        Err(FallbackError::AllAgentsFailed)
    }

    /// Synchronous version of execute_with_fallback for testing
    pub fn execute_with_fallback_sync<F, T, E>(
        &self,
        mut executor: F,
    ) -> FallbackResult<(T, FallbackExecutionLog)>
    where
        F: FnMut(&str) -> Result<T, E>,
        E: std::fmt::Display,
    {
        let mut log = FallbackExecutionLog::new();
        let mut attempts = 0;

        for agent in self.all_agents() {
            if attempts >= self.config.max_attempts {
                break;
            }

            let attempt_start = Instant::now();

            match executor(agent) {
                Ok(result) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    log.add_attempt(FallbackAttempt::success(agent, duration_ms));
                    return Ok((result, log));
                }
                Err(e) => {
                    let duration_ms = attempt_start.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();
                    let reason = FailureReason::from_error_message(&error_msg);

                    log.add_attempt(FallbackAttempt::failure(
                        agent,
                        reason,
                        &error_msg,
                        duration_ms,
                    ));

                    if !reason.should_fallback() {
                        return Err(FallbackError::ExecutionFailed(error_msg));
                    }

                    attempts += 1;
                }
            }
        }

        Err(FallbackError::AllAgentsFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_reason_display() {
        assert_eq!(FailureReason::Unavailable.to_string(), "unavailable");
        assert_eq!(FailureReason::Timeout.to_string(), "timeout");
        assert_eq!(FailureReason::RateLimited.to_string(), "rate_limited");
    }

    #[test]
    fn test_failure_reason_from_error() {
        assert_eq!(
            FailureReason::from_error_message("Agent not found"),
            FailureReason::Unavailable
        );
        assert_eq!(
            FailureReason::from_error_message("Request timed out"),
            FailureReason::Timeout
        );
        assert_eq!(
            FailureReason::from_error_message("Rate limit exceeded"),
            FailureReason::RateLimited
        );
        assert_eq!(
            FailureReason::from_error_message("Connection refused"),
            FailureReason::NetworkError
        );
        assert_eq!(
            FailureReason::from_error_message("Some random error"),
            FailureReason::Error
        );
    }

    #[test]
    fn test_failure_reason_should_fallback() {
        assert!(FailureReason::Unavailable.should_fallback());
        assert!(FailureReason::Timeout.should_fallback());
        assert!(FailureReason::Error.should_fallback());
        assert!(!FailureReason::Cancelled.should_fallback());
    }

    #[test]
    fn test_fallback_chain_creation() {
        let chain = AgentFallbackChain::new("primary-agent");
        assert_eq!(chain.primary_agent(), "primary-agent");
        assert!(chain.fallback_agents.contains(&"claude-code".to_string()));
    }

    #[test]
    fn test_fallback_chain_with_fallbacks() {
        let chain = AgentFallbackChain::new("codex")
            .with_fallbacks(vec!["aider".to_string(), "claude-code".to_string()]);

        let all = chain.all_agents();
        assert_eq!(all, vec!["codex", "aider", "claude-code"]);
    }

    #[test]
    fn test_next_agent_after() {
        let chain = AgentFallbackChain::new("primary")
            .with_fallbacks(vec!["fallback1".to_string(), "fallback2".to_string()]);

        assert_eq!(chain.next_agent_after("primary"), Some("fallback1"));
        assert_eq!(chain.next_agent_after("fallback1"), Some("fallback2"));
        assert_eq!(chain.next_agent_after("fallback2"), None);
        assert_eq!(chain.next_agent_after("unknown"), None);
    }

    #[test]
    fn test_from_phase_manager() {
        let phase_manager = PhaseManager::new();
        let chain = AgentFallbackChain::from_phase_manager(&phase_manager, Phase::Planning);

        assert_eq!(chain.primary_agent(), "codex");
        assert!(chain.fallback_agents.contains(&"claude-code".to_string()));
    }

    #[test]
    fn test_fallback_execution_log() {
        let mut log = FallbackExecutionLog::new();

        log.add_attempt(FallbackAttempt::failure(
            "agent1",
            FailureReason::Unavailable,
            "Not found",
            100,
        ));
        log.add_attempt(FallbackAttempt::success("agent2", 200));

        assert_eq!(log.attempts.len(), 2);
        assert_eq!(log.failed_attempts_count(), 1);
        assert_eq!(log.successful_agent, Some("agent2".to_string()));
        assert!(log.overall_success);
        assert_eq!(log.total_duration_ms, 300);
    }

    #[test]
    fn test_execute_with_fallback_success_first() {
        let chain = AgentFallbackChain::new("agent1")
            .with_fallbacks(vec!["agent2".to_string()]);

        let result: FallbackResult<(String, _)> = chain.execute_with_fallback_sync(|agent| {
            Ok::<_, String>(format!("success from {}", agent))
        });

        let (output, log) = result.unwrap();
        assert_eq!(output, "success from agent1");
        assert_eq!(log.attempts.len(), 1);
        assert!(log.overall_success);
    }

    #[test]
    fn test_execute_with_fallback_first_fails() {
        let chain = AgentFallbackChain::new("agent1")
            .with_fallbacks(vec!["agent2".to_string()]);

        let mut call_count = 0;
        let result: FallbackResult<(String, _)> = chain.execute_with_fallback_sync(|agent| {
            call_count += 1;
            if agent == "agent1" {
                Err("Agent not found".to_string())
            } else {
                Ok(format!("success from {}", agent))
            }
        });

        let (output, log) = result.unwrap();
        assert_eq!(output, "success from agent2");
        assert_eq!(log.attempts.len(), 2);
        assert_eq!(log.failed_attempts_count(), 1);
        assert!(log.overall_success);
    }

    #[test]
    fn test_execute_with_fallback_all_fail() {
        let chain = AgentFallbackChain::new("agent1")
            .with_fallbacks(vec!["agent2".to_string()])
            .with_config(FallbackConfig {
                max_attempts: 3,
                ..Default::default()
            });

        let result: FallbackResult<(String, _)> = chain.execute_with_fallback_sync(|_| {
            Err::<String, _>("Always fails".to_string())
        });

        assert!(matches!(result, Err(FallbackError::AllAgentsFailed)));
    }

    #[test]
    fn test_execute_with_fallback_cancelled_no_fallback() {
        let chain = AgentFallbackChain::new("agent1")
            .with_fallbacks(vec!["agent2".to_string()]);

        let result: FallbackResult<(String, _)> = chain.execute_with_fallback_sync(|_| {
            Err::<String, _>("User cancelled operation".to_string())
        });

        // Should not fallback on cancellation
        assert!(matches!(result, Err(FallbackError::ExecutionFailed(_))));
    }

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.delay_between_attempts_ms, 500);
        assert_eq!(config.timeout_per_attempt_seconds, 600);
        assert!(config.log_all_attempts);
    }

    #[test]
    fn test_fallback_attempt_creation() {
        let success = FallbackAttempt::success("agent", 100);
        assert!(success.success);
        assert!(success.failure_reason.is_none());

        let failure = FallbackAttempt::failure("agent", FailureReason::Timeout, "Timed out", 200);
        assert!(!failure.success);
        assert_eq!(failure.failure_reason, Some(FailureReason::Timeout));
    }
}
