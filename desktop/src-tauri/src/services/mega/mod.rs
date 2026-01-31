//! Mega Plan Orchestrator Module
//!
//! Provides multi-feature orchestration for large projects.
//! Manages worktrees, PRD generation, and parallel feature execution.

mod orchestrator;

pub use orchestrator::{MegaOrchestrator, MegaOrchestratorConfig, MegaOrchestratorError};
