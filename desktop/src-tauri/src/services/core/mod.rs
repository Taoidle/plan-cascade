//! Core Traits Module
//!
//! Defines the foundational trait hierarchy for the Plan Cascade Desktop backend
//! architecture. These traits represent the intended crate boundaries of a future
//! workspace restructuring:
//!
//! - `ExecutionContext` - Immutable, shared context for all execution scopes
//! - `ToolContext` - Extends ExecutionContext for tool-level execution
//! - `OrchestratorContext` - Extends ExecutionContext for orchestrator-level control
//!
//! Also includes:
//! - Unified `ToolDefinition`/`ToolExecutable`/`UnifiedTool` traits
//! - `EventActions` for immutable event + side-effect actions pattern
//! - `ContextCompactor` trait for pluggable context compaction
//! - Builder patterns for configuration structs
//!
//! Module organization mirrors intended crate boundaries:
//! - `core/` = core crate (traits, error types, context hierarchy)
//! - Tools, LLM, orchestrator etc. would be separate crates importing from core.

pub mod builders;
pub mod compaction;
pub mod context;
pub mod event_actions;
pub mod tool_trait;

// ── Context Hierarchy ────────────────────────────────────────────────
pub use context::{ExecutionContext, OrchestratorContext, ToolContext};

// ── Unified Tool Trait ───────────────────────────────────────────────
pub use tool_trait::{ToolDefinitionTrait, ToolExecutable, UnifiedTool, UnifiedToolRegistry};

// ── Event + Actions ──────────────────────────────────────────────────
pub use event_actions::{
    AgentEventWithActions, CheckpointRequest, EventActions, QualityGateActionResult,
};

// ── Pluggable Compaction ─────────────────────────────────────────────
pub use compaction::{
    CompactionConfig, CompactionResult, CompactionStrategy, ContextCompactor,
    LlmSummaryCompactor, SlidingWindowCompactor,
};

// ── Builder Pattern & Session State ──────────────────────────────────
pub use builders::{
    AgentConfigBuilder, BuiltAgentConfig, BuiltExecutionConfig, BuiltQualityGateConfig,
    ExecutionConfigBuilder, QualityGateConfigBuilder, SessionState, SessionStateKey,
};
