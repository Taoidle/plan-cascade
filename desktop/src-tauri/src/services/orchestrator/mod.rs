//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with SQLite persistence for crash recovery.

mod service;

pub use service::{
    OrchestratorService, OrchestratorConfig, ExecutionResult,
    SessionExecutionResult, ProviderInfo,
};
