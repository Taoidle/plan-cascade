//! Core Traits Module
//!
//! Re-exports foundational types from the `plan-cascade-core` workspace crate
//! and provides adapter/bridge modules that depend on application-level types.
//!
//! ## Extracted to `plan-cascade-core` crate
//!
//! The following modules now live in the standalone core crate:
//! - `context` - ExecutionContext trait, ToolContext, OrchestratorContext
//! - `tool_trait` - ToolDefinitionTrait, ToolExecutable, UnifiedTool, UnifiedToolRegistry
//! - `builders` - AgentConfigBuilder, ExecutionConfigBuilder, QualityGateConfigBuilder, SessionState
//! - `error` - CoreError, CoreResult
//!
//! ## Remaining in main crate (cross-service dependencies)
//!
//! - `adapter` - Bridges core traits with existing Tool/ToolRegistry/ToolExecutionContext
//! - `compaction` - Pluggable context compaction (depends on llm::types::Message)
//! - `event_actions` - Event + Actions pattern (depends on agent_composer::types::AgentEvent)

// ── Modules that remain in this crate (cross-service dependencies) ─────
pub mod adapter;
pub mod compaction;
pub mod event_actions;

// ── Re-exports from plan-cascade-core c────────────────────────────────
// These provide backward compatibility so that `crate::services::core::*`
// continues to work across the codebase without changing every import.

// Context hierarchy
pub use plan_cascade_core::context;
pub use plan_cascade_core::context::{ExecutionContext, OrchestratorContext, ToolContext};

// Unified tool traits
pub use plan_cascade_core::tool_trait;
pub use plan_cascade_core::tool_trait::{ToolDefinitionTrait, ToolExecutable, UnifiedTool, UnifiedToolRegistry};

// Adapters (Core <-> Existing Layer Bridge)
pub use adapter::{import_legacy_tools, ToolAdapter, UnifiedToolAdapter};

// Event + Actions
pub use event_actions::{
    AgentEventWithActions, CheckpointRequest, EventActions, QualityGateActionResult,
};

// Pluggable Compaction
pub use compaction::{
    CompactionConfig, CompactionResult, CompactionStrategy, ContextCompactor,
    LlmSummaryCompactor, SlidingWindowCompactor,
};

// Builder Pattern & Session State
pub use plan_cascade_core::builders;
pub use plan_cascade_core::builders::{
    AgentConfigBuilder, BuiltAgentConfig, BuiltExecutionConfig, BuiltQualityGateConfig,
    ExecutionConfigBuilder, QualityGateConfigBuilder, SessionState, SessionStateKey,
};
