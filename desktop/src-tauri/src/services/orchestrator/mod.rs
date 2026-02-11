//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with SQLite persistence for crash recovery.

mod analysis_index;
mod analysis_merge;
mod analysis_scheduler;
mod analysis_store;
mod service;

pub use service::{
    ExecutionResult, OrchestratorConfig, OrchestratorService, ProviderInfo, SessionExecutionResult,
};
