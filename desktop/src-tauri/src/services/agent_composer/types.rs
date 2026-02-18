//! Core types for the Composable Agent Model
//!
//! Defines the Agent trait, AgentContext, AgentEvent, AgentInput, AgentConfig,
//! AgentPipeline, and AgentStep types that form the foundation of the
//! composable agent architecture.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::services::llm::LlmProvider;
use crate::services::orchestrator::hooks::AgenticHooks;
use crate::services::plugins::manager::PluginManager;
use crate::services::tools::ToolExecutor;
use crate::utils::error::AppResult;

// ============================================================================
// Agent Event Stream
// ============================================================================

/// Type alias for the asynchronous stream of agent events.
///
/// Each item is a `Result<AgentEvent, AppError>`, enabling the consumer to
/// handle both events and errors as they arrive from the agent.
pub type AgentEventStream = Pin<Box<dyn Stream<Item = AppResult<AgentEvent>> + Send>>;

// ============================================================================
// Agent Trait
// ============================================================================

/// Core trait for composable agents.
///
/// Agents are lightweight, composable execution units. Each agent takes an
/// `AgentContext` and returns an asynchronous stream of `AgentEvent`s. Agents
/// can be composed into pipelines using `SequentialAgent`, `ParallelAgent`,
/// and `ConditionalAgent`.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Returns the agent's unique name.
    fn name(&self) -> &str;

    /// Returns a human-readable description of the agent.
    fn description(&self) -> &str;

    /// Execute the agent with the given context, returning a stream of events.
    ///
    /// The returned stream will emit events as the agent progresses, ending
    /// with a `Done` event on successful completion.
    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream>;
}

// ============================================================================
// Agent Context
// ============================================================================

/// Context provided to an agent during execution.
///
/// Contains all the dependencies an agent needs: the LLM provider, tool
/// executor, hooks, input data, shared state, and configuration.
#[derive(Clone)]
pub struct AgentContext {
    /// Unique session identifier for this execution.
    pub session_id: String,
    /// Project root directory.
    pub project_root: PathBuf,
    /// LLM provider for model interactions.
    pub provider: Arc<dyn LlmProvider>,
    /// Tool executor for running tools (file I/O, shell, search, etc.).
    pub tool_executor: Arc<ToolExecutor>,
    /// Optional plugin manager for accessing installed plugins.
    pub plugin_manager: Option<Arc<PluginManager>>,
    /// Lifecycle hooks for cross-cutting concerns (memory, skills, guardrails).
    pub hooks: Arc<AgenticHooks>,
    /// Input data for this agent invocation.
    pub input: AgentInput,
    /// Shared state accessible by all agents in a pipeline.
    ///
    /// Agents can read and write to this map to pass data between steps.
    pub shared_state: Arc<RwLock<HashMap<String, Value>>>,
    /// Agent-specific configuration.
    pub config: AgentConfig,
}

// ============================================================================
// Agent Input
// ============================================================================

/// Input data for an agent invocation.
///
/// Supports multiple input formats to accommodate different use cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentInput {
    /// Plain text input (most common).
    Text(String),
    /// Structured JSON input for complex data.
    Structured(Value),
}

impl Default for AgentInput {
    fn default() -> Self {
        AgentInput::Text(String::new())
    }
}

impl AgentInput {
    /// Extract the input as a text string.
    ///
    /// For `Text`, returns the string directly.
    /// For `Structured`, returns the JSON serialization.
    pub fn as_text(&self) -> String {
        match self {
            AgentInput::Text(s) => s.clone(),
            AgentInput::Structured(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
        }
    }
}

// ============================================================================
// Agent Event
// ============================================================================

/// Events emitted by agents during execution.
///
/// These are the building blocks for agent communication. Composite agents
/// (Sequential, Parallel) forward sub-agent events, potentially prefixed
/// with the sub-agent's name for disambiguation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Text content delta from the model.
    TextDelta {
        content: String,
    },
    /// Start of a tool call.
    ToolCall {
        name: String,
        args: String,
    },
    /// Result of a tool call.
    ToolResult {
        name: String,
        result: String,
    },
    /// Thinking/reasoning content delta.
    ThinkingDelta {
        content: String,
    },
    /// State update — agents can write key-value pairs to shared state.
    StateUpdate {
        key: String,
        value: Value,
    },
    /// Request to transfer execution to another agent.
    AgentTransfer {
        target: String,
        message: String,
    },
    /// A graph node has started execution.
    GraphNodeStarted {
        node_id: String,
    },
    /// A graph node has completed execution.
    GraphNodeCompleted {
        node_id: String,
        output: Option<String>,
    },
    /// A human review is required before continuing execution.
    HumanReviewRequired {
        node_id: String,
        context: String,
    },
    /// Rich content for dynamic UI rendering.
    ///
    /// Agents can emit structured data that the frontend renders as
    /// tables, charts, diffs, or action buttons. The `surface_id`
    /// enables replacing previously rendered content (update-in-place).
    RichContent {
        /// Component type: "table", "chart", "diff", "action_buttons", etc.
        component_type: String,
        /// Structured JSON payload consumed by the frontend renderer.
        data: Value,
        /// Optional surface identifier for update/replace semantics.
        /// When present, the frontend replaces the existing surface
        /// with this ID instead of appending a new element.
        #[serde(skip_serializing_if = "Option::is_none")]
        surface_id: Option<String>,
    },
    /// Agent execution completed successfully with optional output.
    Done {
        output: Option<String>,
    },
}

// ============================================================================
// Agent Config
// ============================================================================

/// Configuration options for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Maximum number of agentic loop iterations.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Maximum total tokens to consume.
    #[serde(default = "default_max_total_tokens")]
    pub max_total_tokens: u32,
    /// Whether to enable streaming output.
    #[serde(default = "default_streaming")]
    pub streaming: bool,
    /// Whether to enable automatic context compaction.
    #[serde(default = "default_enable_compaction")]
    pub enable_compaction: bool,
    /// LLM temperature setting.
    #[serde(default)]
    pub temperature: Option<f32>,
}

fn default_max_iterations() -> u32 {
    50
}

fn default_max_total_tokens() -> u32 {
    1_000_000
}

fn default_streaming() -> bool {
    true
}

fn default_enable_compaction() -> bool {
    true
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            max_total_tokens: default_max_total_tokens(),
            streaming: default_streaming(),
            enable_compaction: default_enable_compaction(),
            temperature: None,
        }
    }
}

// ============================================================================
// Agent Pipeline (Serializable definitions)
// ============================================================================

/// Serializable definition of an agent pipeline.
///
/// Pipelines describe compositions of agents that can be persisted to the
/// database and reconstructed at runtime by the `AgentRegistry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPipeline {
    /// Unique pipeline identifier.
    pub pipeline_id: String,
    /// Human-readable pipeline name.
    pub name: String,
    /// Optional description of what this pipeline does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ordered list of agent steps in this pipeline.
    pub steps: Vec<AgentStep>,
    /// When the pipeline was created (ISO 8601).
    pub created_at: String,
    /// When the pipeline was last updated (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// A single step in an agent pipeline.
///
/// Each variant corresponds to a different agent type, with its own
/// configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step_type", rename_all = "snake_case")]
pub enum AgentStep {
    /// An LLM-backed agent step.
    LlmStep(LlmStepConfig),
    /// A sequential composition of sub-steps.
    SequentialStep {
        name: String,
        steps: Vec<AgentStep>,
    },
    /// A parallel composition of sub-steps.
    ParallelStep {
        name: String,
        steps: Vec<AgentStep>,
    },
    /// A conditional branching step.
    ConditionalStep {
        name: String,
        /// The shared_state key to evaluate for branch selection.
        condition_key: String,
        /// Map of condition_value -> agent step.
        branches: HashMap<String, AgentStep>,
        /// Optional default branch if no key matches.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_branch: Option<Box<AgentStep>>,
    },
}

/// Configuration for an LLM agent step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStepConfig {
    /// Agent name.
    pub name: String,
    /// Optional system instruction/prompt for the LLM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Optional model override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional tool filter (only allow these tools).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Agent-specific configuration overrides.
    #[serde(default)]
    pub config: AgentConfig,
}

/// Summary information about an agent pipeline (for list views).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPipelineInfo {
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// Pipeline name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Number of steps in the pipeline.
    pub step_count: usize,
    /// When the pipeline was created.
    pub created_at: String,
    /// When the pipeline was last updated.
    pub updated_at: Option<String>,
}

impl From<&AgentPipeline> for AgentPipelineInfo {
    fn from(pipeline: &AgentPipeline) -> Self {
        Self {
            pipeline_id: pipeline.pipeline_id.clone(),
            name: pipeline.name.clone(),
            description: pipeline.description.clone(),
            step_count: pipeline.steps.len(),
            created_at: pipeline.created_at.clone(),
            updated_at: pipeline.updated_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_input_text() {
        let input = AgentInput::Text("Hello, world!".to_string());
        assert_eq!(input.as_text(), "Hello, world!");
    }

    #[test]
    fn test_agent_input_structured() {
        let val = serde_json::json!({"key": "value"});
        let input = AgentInput::Structured(val);
        let text = input.as_text();
        assert!(text.contains("key"));
        assert!(text.contains("value"));
    }

    #[test]
    fn test_agent_input_default() {
        let input = AgentInput::default();
        assert_eq!(input.as_text(), "");
    }

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_total_tokens, 1_000_000);
        assert!(config.streaming);
        assert!(config.enable_compaction);
        assert!(config.temperature.is_none());
    }

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text_delta\""));
        assert!(json.contains("\"content\":\"Hello\""));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_agent_event_done() {
        let event = AgentEvent::Done {
            output: Some("Result".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"done\""));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::Done { output } => assert_eq!(output, Some("Result".to_string())),
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_agent_event_tool_call() {
        let event = AgentEvent::ToolCall {
            name: "read_file".to_string(),
            args: r#"{"path":"/foo/bar"}"#.to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"tool_call\""));
        assert!(json.contains("\"name\":\"read_file\""));
    }

    #[test]
    fn test_agent_event_state_update() {
        let event = AgentEvent::StateUpdate {
            key: "result".to_string(),
            value: serde_json::json!(42),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"state_update\""));
    }

    #[test]
    fn test_agent_event_agent_transfer() {
        let event = AgentEvent::AgentTransfer {
            target: "reviewer".to_string(),
            message: "Please review".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"agent_transfer\""));
    }

    #[test]
    fn test_agent_pipeline_serialization() {
        let pipeline = AgentPipeline {
            pipeline_id: "p-1".to_string(),
            name: "Test Pipeline".to_string(),
            description: Some("A test".to_string()),
            steps: vec![AgentStep::LlmStep(LlmStepConfig {
                name: "llm-1".to_string(),
                instruction: Some("Be helpful".to_string()),
                model: None,
                tools: None,
                config: AgentConfig::default(),
            })],
            created_at: "2026-02-17T00:00:00Z".to_string(),
            updated_at: None,
        };

        let json = serde_json::to_string_pretty(&pipeline).unwrap();
        assert!(json.contains("Test Pipeline"));
        assert!(json.contains("llm_step"));

        let parsed: AgentPipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pipeline_id, "p-1");
        assert_eq!(parsed.steps.len(), 1);
    }

    #[test]
    fn test_agent_step_sequential() {
        let step = AgentStep::SequentialStep {
            name: "seq-1".to_string(),
            steps: vec![
                AgentStep::LlmStep(LlmStepConfig {
                    name: "step-a".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                AgentStep::LlmStep(LlmStepConfig {
                    name: "step-b".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
            ],
        };

        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("sequential_step"));
        let parsed: AgentStep = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentStep::SequentialStep { name, steps } => {
                assert_eq!(name, "seq-1");
                assert_eq!(steps.len(), 2);
            }
            _ => panic!("Expected SequentialStep"),
        }
    }

    #[test]
    fn test_agent_step_conditional() {
        let mut branches = HashMap::new();
        branches.insert(
            "yes".to_string(),
            AgentStep::LlmStep(LlmStepConfig {
                name: "yes-agent".to_string(),
                instruction: None,
                model: None,
                tools: None,
                config: AgentConfig::default(),
            }),
        );

        let step = AgentStep::ConditionalStep {
            name: "cond-1".to_string(),
            condition_key: "should_proceed".to_string(),
            branches,
            default_branch: None,
        };

        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("conditional_step"));
        assert!(json.contains("should_proceed"));
    }

    #[test]
    fn test_agent_event_rich_content_serialization() {
        let event = AgentEvent::RichContent {
            component_type: "table".to_string(),
            data: serde_json::json!({
                "columns": ["Name", "Status"],
                "rows": [["story-1", "completed"], ["story-2", "running"]]
            }),
            surface_id: Some("progress-table".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"rich_content\""));
        assert!(json.contains("\"component_type\":\"table\""));
        assert!(json.contains("\"surface_id\":\"progress-table\""));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::RichContent {
                component_type,
                data,
                surface_id,
            } => {
                assert_eq!(component_type, "table");
                assert!(data.get("columns").is_some());
                assert_eq!(surface_id, Some("progress-table".to_string()));
            }
            _ => panic!("Expected RichContent"),
        }
    }

    #[test]
    fn test_agent_event_rich_content_without_surface_id() {
        let event = AgentEvent::RichContent {
            component_type: "chart".to_string(),
            data: serde_json::json!({"progress": 75}),
            surface_id: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"rich_content\""));
        assert!(json.contains("\"component_type\":\"chart\""));
        // surface_id should be omitted when None
        assert!(!json.contains("surface_id"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::RichContent {
                component_type,
                surface_id,
                ..
            } => {
                assert_eq!(component_type, "chart");
                assert!(surface_id.is_none());
            }
            _ => panic!("Expected RichContent"),
        }
    }

    #[test]
    fn test_agent_event_rich_content_diff_type() {
        let event = AgentEvent::RichContent {
            component_type: "diff".to_string(),
            data: serde_json::json!({
                "old": "fn foo() {}",
                "new": "fn foo() -> i32 { 42 }",
                "file": "src/main.rs"
            }),
            surface_id: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::RichContent {
                component_type,
                data,
                ..
            } => {
                assert_eq!(component_type, "diff");
                assert_eq!(data["file"], "src/main.rs");
            }
            _ => panic!("Expected RichContent"),
        }
    }

    #[test]
    fn test_agent_event_rich_content_action_buttons() {
        let event = AgentEvent::RichContent {
            component_type: "action_buttons".to_string(),
            data: serde_json::json!({
                "actions": [
                    {"id": "approve", "label": "Approve", "variant": "primary"},
                    {"id": "retry", "label": "Retry", "variant": "secondary"},
                    {"id": "skip", "label": "Skip", "variant": "ghost"}
                ]
            }),
            surface_id: Some("review-actions".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::RichContent {
                component_type,
                data,
                ..
            } => {
                assert_eq!(component_type, "action_buttons");
                let actions = data["actions"].as_array().unwrap();
                assert_eq!(actions.len(), 3);
            }
            _ => panic!("Expected RichContent"),
        }
    }

    #[test]
    fn test_agent_pipeline_info_from() {
        let pipeline = AgentPipeline {
            pipeline_id: "p-1".to_string(),
            name: "My Pipeline".to_string(),
            description: Some("desc".to_string()),
            steps: vec![
                AgentStep::LlmStep(LlmStepConfig {
                    name: "s1".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
                AgentStep::LlmStep(LlmStepConfig {
                    name: "s2".to_string(),
                    instruction: None,
                    model: None,
                    tools: None,
                    config: AgentConfig::default(),
                }),
            ],
            created_at: "2026-01-01".to_string(),
            updated_at: None,
        };

        let info = AgentPipelineInfo::from(&pipeline);
        assert_eq!(info.pipeline_id, "p-1");
        assert_eq!(info.name, "My Pipeline");
        assert_eq!(info.step_count, 2);
    }
}
