//! Plan Mode Service
//!
//! Provides the domain-agnostic task decomposition framework including:
//! - Pluggable domain adapters for different task types
//! - Task analysis and domain classification
//! - LLM-powered plan decomposition into steps with dependencies
//! - Parallel step execution with dependency-resolved batching
//! - Step output validation against completion criteria

pub mod adapter;
pub mod adapter_registry;
pub mod adapters;
pub mod analyzer;
pub mod clarifier;
pub mod planner;
pub mod step_executor;
pub mod types;
pub mod validator;

pub use adapter::DomainAdapter;
pub use adapter_registry::{AdapterInfo, AdapterRegistry};
pub use types::{
    calculate_plan_batches, ClarificationAnswer, ClarificationQuestion, CriterionResult,
    OutputFormat, Plan, PlanAnalysis, PlanBatch, PlanExecutionProgress, PlanExecutionReport,
    PlanModePhase, PlanModeProgressEvent, PlanModeSession, PlanPersonaRole, PlanStep,
    StepExecutionState, StepOutput, StepPriority, TaskDomain, PLAN_MODE_EVENT_CHANNEL,
};
