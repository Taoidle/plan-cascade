//! Standalone Orchestrator Module
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.

mod service;

pub use service::{OrchestratorService, OrchestratorConfig, ExecutionResult};
