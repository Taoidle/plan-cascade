//! Plan Cascade Quality Gates
//!
//! Core types, pipeline orchestrator, project detection, and validators for
//! the quality gate system. This crate provides the foundational types that
//! can be compiled independently:
//!
//! - `models` - Quality gate data types (ProjectType, GateStatus, QualityGate, GateResult, etc.)
//! - `pipeline` - Three-phase pipeline orchestrator (GatePipeline, GatePhase, GateMode, etc.)
//! - `detector` - Automatic project type detection
//! - `validators` - Pre-configured quality gate definitions per project type
//!
//! Heavy-dependency features (SQLite-backed cache, SQLite-backed runner,
//! AI-powered gates) live in the main crate's `services::quality_gates` module.

pub mod detector;
pub mod models;
pub mod pipeline;
pub mod validators;

// Re-export core model types
pub use models::{
    CustomGateConfig, GateResult, GateStatus, GatesSummary, ProjectDetectionResult,
    ProjectMetadata, ProjectType, QualityGate, StoredGateResult,
};

// Re-export pipeline types
pub use pipeline::{
    GateExecutor, GateMode, GatePhase, GatePipeline, PhaseGateConfig, PipelineConfig,
    PipelineGateResult, PipelinePhaseResult, PipelineResult,
};

// Re-export detector
pub use detector::{detect_project_type, ProjectDetector};

// Re-export validators
pub use validators::ValidatorRegistry;
