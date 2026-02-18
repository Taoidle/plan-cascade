//! Plan Cascade Core
//!
//! Foundational traits, error types, and context hierarchy for the Plan Cascade
//! Desktop workspace. This crate has zero dependencies on application-level code
//! (Tauri, database, LLM providers, etc.).
//!
//! ## Module Organization
//!
//! - `error` - Core error types (`CoreError`, `CoreResult`)
//! - `context` - Execution context hierarchy (`ExecutionContext`, `ToolContext`, `OrchestratorContext`)
//! - `tool_trait` - Unified tool abstraction (`ToolDefinitionTrait`, `ToolExecutable`, `UnifiedTool`)
//! - `builders` - Builder patterns and session state types
//! - `proxy` - Proxy configuration data types shared across workspace crates
//! - `streaming` - Unified stream event types and adapter trait
//!
//! ## Design Principles
//!
//! 1. **Zero external dependencies beyond serde/async-trait/thiserror** - keeps build times minimal
//! 2. **Trait-based abstractions** - enables mocking, testing, and future crate splitting
//! 3. **Unidirectional dependency** - this crate depends on nothing else in the workspace

pub mod error;
pub mod context;
pub mod tool_trait;
pub mod builders;
pub mod proxy;
pub mod streaming;
pub mod event_actions;

// ── Error Types ────────────────────────────────────────────────────────
pub use error::{CoreError, CoreResult};

// ── Context Hierarchy ──────────────────────────────────────────────────
pub use context::{ExecutionContext, OrchestratorContext, ToolContext};

// ── Unified Tool Trait ─────────────────────────────────────────────────
pub use tool_trait::{ToolDefinitionTrait, ToolExecutable, UnifiedTool, UnifiedToolRegistry};

// ── Builder Pattern & Session State ────────────────────────────────────
pub use builders::{
    AgentConfigBuilder, BuiltAgentConfig, BuiltExecutionConfig, BuiltQualityGateConfig,
    ExecutionConfigBuilder, QualityGateConfigBuilder, SessionState, SessionStateKey,
};

// ── Proxy Types ────────────────────────────────────────────────────────
pub use proxy::{ProxyConfig, ProxyProtocol, ProxyStrategy};

// ── Streaming Types ────────────────────────────────────────────────────
pub use streaming::{AdapterError, StreamAdapter, UnifiedStreamEvent};

// ── Event Actions ─────────────────────────────────────────────────────
pub use event_actions::{CheckpointRequest, EventActions, QualityGateActionResult};
