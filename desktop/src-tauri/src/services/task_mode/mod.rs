//! Task Mode Service
//!
//! Provides the core task mode execution pipeline including:
//! - Agent priority chain resolver for selecting best agents per story/phase
//! - Batch parallel execution engine with topological sort
//! - Task mode session management types

pub mod agent_resolver;
pub mod batch_executor;
pub mod prd_generator;

pub use agent_resolver::{
    AgentAssignment, AgentOverrides, AgentResolver, AgentsConfig, ExecutionPhase,
    PhaseConfig as AgentPhaseConfig, StoryType,
};
pub use batch_executor::{
    calculate_batches, BatchExecutionProgress, BatchExecutionResult, BatchExecutor,
    ExecutableStory, ExecutionBatch, ExecutionConfig, RetryContext, StoryContext,
    StoryExecutionContext, StoryExecutionOutcome, StoryExecutionState, TaskModeProgressEvent,
    TASK_MODE_EVENT_CHANNEL,
};
