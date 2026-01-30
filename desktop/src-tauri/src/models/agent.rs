//! Agent Models
//!
//! Data structures for AI agents with custom behaviors.

use serde::{Deserialize, Serialize};

/// An AI agent with custom system prompt and tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier (UUID)
    pub id: String,
    /// Display name for the agent
    pub name: String,
    /// Description of what the agent does
    pub description: Option<String>,
    /// System prompt that defines the agent's behavior
    pub system_prompt: String,
    /// Model to use (e.g., "claude-sonnet-4-20250514")
    pub model: String,
    /// List of allowed tool names (empty means all tools allowed)
    pub allowed_tools: Vec<String>,
    /// Creation timestamp (ISO 8601)
    pub created_at: Option<String>,
    /// Last update timestamp (ISO 8601)
    pub updated_at: Option<String>,
}

impl Agent {
    /// Create a new agent with required fields
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        system_prompt: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            system_prompt: system_prompt.into(),
            model: model.into(),
            allowed_tools: Vec::new(),
            created_at: Some(now.clone()),
            updated_at: Some(now),
        }
    }

    /// Create a new agent with a generated UUID
    pub fn create(
        name: impl Into<String>,
        system_prompt: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self::new(uuid::Uuid::new_v4().to_string(), name, system_prompt, model)
    }

    /// Builder pattern: set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder pattern: set allowed tools
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }
}

/// Request to create a new agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCreateRequest {
    /// Display name for the agent
    pub name: String,
    /// Description of what the agent does
    pub description: Option<String>,
    /// System prompt that defines the agent's behavior
    pub system_prompt: String,
    /// Model to use
    pub model: String,
    /// List of allowed tool names
    pub allowed_tools: Vec<String>,
}

impl AgentCreateRequest {
    /// Validate the create request
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Agent name cannot be empty".to_string());
        }
        if self.name.len() > 100 {
            return Err("Agent name cannot exceed 100 characters".to_string());
        }
        if self.system_prompt.trim().is_empty() {
            return Err("System prompt cannot be empty".to_string());
        }
        if self.system_prompt.len() > 100_000 {
            return Err("System prompt cannot exceed 100,000 characters".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("Model cannot be empty".to_string());
        }
        Ok(())
    }
}

/// Request to update an existing agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdateRequest {
    /// New display name (optional)
    pub name: Option<String>,
    /// New description (optional)
    pub description: Option<String>,
    /// New system prompt (optional)
    pub system_prompt: Option<String>,
    /// New model (optional)
    pub model: Option<String>,
    /// New allowed tools list (optional)
    pub allowed_tools: Option<Vec<String>>,
}

impl AgentUpdateRequest {
    /// Validate the update request
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err("Agent name cannot be empty".to_string());
            }
            if name.len() > 100 {
                return Err("Agent name cannot exceed 100 characters".to_string());
            }
        }
        if let Some(ref system_prompt) = self.system_prompt {
            if system_prompt.trim().is_empty() {
                return Err("System prompt cannot be empty".to_string());
            }
            if system_prompt.len() > 100_000 {
                return Err("System prompt cannot exceed 100,000 characters".to_string());
            }
        }
        if let Some(ref model) = self.model {
            if model.trim().is_empty() {
                return Err("Model cannot be empty".to_string());
            }
        }
        Ok(())
    }

    /// Check if the request has any fields to update
    pub fn has_updates(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.system_prompt.is_some()
            || self.model.is_some()
            || self.allowed_tools.is_some()
    }
}

/// Agent run status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// Run is pending execution
    Pending,
    /// Run is currently executing
    Running,
    /// Run completed successfully
    Completed,
    /// Run failed with an error
    Failed,
    /// Run was cancelled by user
    Cancelled,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown run status: {}", s)),
        }
    }
}

/// A single execution run of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    /// Unique run identifier
    pub id: String,
    /// Agent that was executed
    pub agent_id: String,
    /// User input that triggered the run
    pub input: String,
    /// Output produced by the agent
    pub output: Option<String>,
    /// Current status of the run
    pub status: RunStatus,
    /// Execution duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Number of input tokens used
    pub input_tokens: Option<u32>,
    /// Number of output tokens generated
    pub output_tokens: Option<u32>,
    /// Error message if the run failed
    pub error: Option<String>,
    /// When the run was created
    pub created_at: Option<String>,
    /// When the run completed
    pub completed_at: Option<String>,
}

impl AgentRun {
    /// Create a new pending run
    pub fn new(agent_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            input: input.into(),
            output: None,
            status: RunStatus::Pending,
            duration_ms: None,
            input_tokens: None,
            output_tokens: None,
            error: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
        }
    }

    /// Mark the run as started
    pub fn start(&mut self) {
        self.status = RunStatus::Running;
    }

    /// Mark the run as completed with output
    pub fn complete(&mut self, output: String, duration_ms: u64, input_tokens: u32, output_tokens: u32) {
        self.status = RunStatus::Completed;
        self.output = Some(output);
        self.duration_ms = Some(duration_ms);
        self.input_tokens = Some(input_tokens);
        self.output_tokens = Some(output_tokens);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark the run as failed with an error
    pub fn fail(&mut self, error: String, duration_ms: u64) {
        self.status = RunStatus::Failed;
        self.error = Some(error);
        self.duration_ms = Some(duration_ms);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark the run as cancelled
    pub fn cancel(&mut self, duration_ms: u64) {
        self.status = RunStatus::Cancelled;
        self.duration_ms = Some(duration_ms);
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Statistics for an agent
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentStats {
    /// Total number of runs
    pub total_runs: u32,
    /// Number of successful runs
    pub completed_runs: u32,
    /// Number of failed runs
    pub failed_runs: u32,
    /// Number of cancelled runs
    pub cancelled_runs: u32,
    /// Success rate (completed / total) as percentage
    pub success_rate: f64,
    /// Average execution duration in milliseconds
    pub avg_duration_ms: f64,
    /// Total input tokens used
    pub total_input_tokens: u64,
    /// Total output tokens generated
    pub total_output_tokens: u64,
    /// Timestamp of the last run
    pub last_run_at: Option<String>,
}

impl AgentStats {
    /// Calculate success rate
    pub fn calculate_success_rate(&mut self) {
        if self.total_runs > 0 {
            self.success_rate = (self.completed_runs as f64 / self.total_runs as f64) * 100.0;
        } else {
            self.success_rate = 0.0;
        }
    }
}

/// Paginated list of agent runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunList {
    /// List of runs
    pub runs: Vec<AgentRun>,
    /// Total count of runs (for pagination)
    pub total: u32,
    /// Current offset
    pub offset: u32,
    /// Number of items per page
    pub limit: u32,
}

/// Agent with statistics (for UI display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWithStats {
    #[serde(flatten)]
    pub agent: Agent,
    pub stats: AgentStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = Agent::create("Test Agent", "You are a helpful assistant.", "claude-sonnet-4-20250514");
        assert!(!agent.id.is_empty());
        assert_eq!(agent.name, "Test Agent");
        assert_eq!(agent.system_prompt, "You are a helpful assistant.");
        assert!(agent.created_at.is_some());
    }

    #[test]
    fn test_agent_builder() {
        let agent = Agent::create("Test", "Prompt", "model")
            .with_description("A test agent")
            .with_allowed_tools(vec!["read_file".to_string(), "write_file".to_string()]);

        assert_eq!(agent.description, Some("A test agent".to_string()));
        assert_eq!(agent.allowed_tools.len(), 2);
    }

    #[test]
    fn test_create_request_validation() {
        let valid = AgentCreateRequest {
            name: "Test".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };
        assert!(valid.validate().is_ok());

        let empty_name = AgentCreateRequest {
            name: "".to_string(),
            description: None,
            system_prompt: "Prompt".to_string(),
            model: "model".to_string(),
            allowed_tools: vec![],
        };
        assert!(empty_name.validate().is_err());
    }

    #[test]
    fn test_run_status_parsing() {
        assert_eq!("pending".parse::<RunStatus>().unwrap(), RunStatus::Pending);
        assert_eq!("COMPLETED".parse::<RunStatus>().unwrap(), RunStatus::Completed);
        assert!("invalid".parse::<RunStatus>().is_err());
    }

    #[test]
    fn test_agent_run_lifecycle() {
        let mut run = AgentRun::new("agent-1", "Hello");
        assert_eq!(run.status, RunStatus::Pending);

        run.start();
        assert_eq!(run.status, RunStatus::Running);

        run.complete("World".to_string(), 1000, 10, 5);
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.output, Some("World".to_string()));
        assert!(run.completed_at.is_some());
    }

    #[test]
    fn test_agent_stats() {
        let mut stats = AgentStats {
            total_runs: 10,
            completed_runs: 8,
            failed_runs: 2,
            ..Default::default()
        };
        stats.calculate_success_rate();
        assert!((stats.success_rate - 80.0).abs() < 0.01);
    }
}
