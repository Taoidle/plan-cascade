//! Mega Plan Orchestrator Module
//!
//! Provides multi-feature orchestration for large projects.
//! Manages worktrees, PRD generation, and parallel feature execution.

mod orchestrator;
mod prd_generator;

pub use orchestrator::{MegaOrchestrator, MegaOrchestratorConfig, MegaOrchestratorError};
pub use prd_generator::{
    PrdGenerator, PrdGeneratorConfig, PrdGeneratorError,
    PrdGenerationRequest, PrdGenerationResult, DesignDocContext,
};
