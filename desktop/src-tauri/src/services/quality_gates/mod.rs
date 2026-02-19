//! Quality Gates Service
//!
//! Provides automatic project type detection, quality gate execution,
//! three-phase pipeline orchestration, formatting, caching, and AI-powered
//! quality gates (verification, code review, DoR, DoD).
//! Supports Node.js, Rust, Python, and Go projects with appropriate validators.

mod detector;
mod runner;
mod validators;
pub mod ai_verify;
pub mod cache;
pub mod code_review;
pub mod dod;
pub mod dor;
pub mod format;
pub mod pipeline;
pub mod validation;

pub use detector::*;
pub use runner::*;
pub use validators::*;
pub use ai_verify::AiVerificationGate;
pub use cache::{GateCache, GateCacheKey};
pub use code_review::{CodeReviewGate, CodeReviewResult};
pub use dod::{DoDGate, DoDInput};
pub use dor::{DoRGate, StoryForValidation};
pub use format::FormatGate;
pub use validation::ValidationGate;
pub use pipeline::{
    GateMode, GatePhase, GatePipeline, PipelineConfig, PipelineGateResult, PipelinePhaseResult,
    PipelineResult,
};
