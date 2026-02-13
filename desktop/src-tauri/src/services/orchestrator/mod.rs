//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with SQLite persistence for crash recovery.

mod adaptive_scope;
pub mod analysis_index;
mod analysis_merge;
mod analysis_scheduler;
mod analysis_store;
pub mod background_indexer;
pub mod embedding_service;
pub mod index_manager;
pub mod index_store;
mod service;
pub mod tree_sitter_parser;

pub use service::{
    ExecutionResult, OrchestratorConfig, OrchestratorService, ProviderInfo, SessionExecutionResult,
};
pub(crate) use service::text_describes_pending_action;
