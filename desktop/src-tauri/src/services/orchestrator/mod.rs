//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with SQLite persistence for crash recovery.

mod adaptive_scope;
pub mod analysis_index;
mod analysis_merge;
pub mod component_classifier;
mod analysis_scheduler;
mod analysis_store;
pub mod background_indexer;
pub mod embedding_config_builder;
pub mod embedding_manager;
pub mod embedding_provider;
pub mod embedding_provider_glm;
pub mod embedding_provider_ollama;
pub mod embedding_provider_openai;
pub mod embedding_provider_qwen;
pub mod embedding_provider_tfidf;
pub mod embedding_service;
pub mod event_actions_applicator;
pub mod hnsw_index;
pub mod hooks;
pub mod hybrid_search;
pub mod index_manager;
pub mod index_store;
pub mod lsp_client;
pub mod lsp_enricher;
pub mod lsp_registry;
pub mod permission_gate;
pub mod permissions;
mod service;
pub mod transfer;
pub mod tree_sitter_parser;

pub use hooks::{
    build_default_hooks, register_memory_hooks, register_skill_hooks, AgenticHooks,
    BeforeToolResult, HookContext, SessionSummary,
};
pub(crate) use service::text_describes_pending_action;
pub use service::{
    ExecutionResult, OrchestratorConfig, OrchestratorService, ProviderInfo, SessionExecutionResult,
};
