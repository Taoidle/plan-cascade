//! Task Mode Service
//!
//! Provides the core task mode execution pipeline including:
//! - Agent priority chain resolver for selecting best agents per story/phase
//! - Batch parallel execution engine with topological sort
//! - Task mode session management types

pub mod agent_resolver;
pub mod batch_executor;

pub use agent_resolver::{
    AgentAssignment, AgentOverrides, AgentResolver, AgentsConfig, ExecutionPhase,
    PhaseConfig as AgentPhaseConfig, StoryType,
};
pub use batch_executor::{
    BatchExecutionProgress, BatchExecutionResult, BatchExecutor, ExecutableStory, ExecutionBatch,
    ExecutionConfig, RetryContext, StoryExecutionState, calculate_batches,
};
