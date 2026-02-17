//! Composable Agent Model
//!
//! Provides a lightweight Agent composition layer that enables building complex
//! multi-agent pipelines from simple building blocks:
//!
//! - **Agent trait**: Core interface with `name()`, `description()`, and `run()` methods
//! - **LlmAgent**: Wraps the existing OrchestratorService agentic loop
//! - **SequentialAgent**: Runs sub-agents in order, chaining outputs
//! - **ParallelAgent**: Runs sub-agents concurrently, merging event streams
//! - **ConditionalAgent**: Routes to branches based on shared state
//! - **ComposerRegistry**: Named agent storage with pipeline construction

pub mod conditional;
pub mod eval_types;
pub mod evaluation;
pub mod graph_types;
pub mod graph_workflow;
pub mod llm_agent;
pub mod parallel;
pub mod registry;
pub mod sequential;
pub mod types;

// Re-export core types
pub use types::{
    Agent, AgentConfig, AgentContext, AgentEvent, AgentEventStream, AgentInput, AgentPipeline,
    AgentPipelineInfo, AgentStep, LlmStepConfig,
};
pub use llm_agent::{LlmAgent, convert_stream_event};
pub use sequential::SequentialAgent;
pub use parallel::ParallelAgent;
pub use conditional::ConditionalAgent;
pub use registry::{AgentInfo, ComposerRegistry};

// Re-export graph workflow types
pub use graph_types::{
    GraphWorkflow, GraphNode, NodePosition, Edge, ConditionConfig,
    StateSchema, ChannelConfig, Reducer, GraphWorkflowInfo,
};

// Re-export evaluation types
pub use eval_types::{
    Evaluator, EvaluationCriteria, ToolTrajectoryConfig, ResponseSimilarityConfig,
    LlmJudgeConfig, EvaluationCase, ModelConfig, EvaluationRun, EvaluationRunInfo,
    EvaluatorInfo, EvaluationReport, TestResult,
};
