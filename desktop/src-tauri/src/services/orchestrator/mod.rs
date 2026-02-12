//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with SQLite persistence for crash recovery.

pub mod analysis_index;
mod analysis_merge;
mod analysis_scheduler;
mod analysis_store;
mod adaptive_scope;
pub mod background_indexer;
pub mod index_manager;
pub mod index_store;
pub mod tree_sitter_parser;
mod service;

pub use service::{
    ExecutionResult, OrchestratorConfig, OrchestratorService, ProviderInfo, SessionExecutionResult,
};
