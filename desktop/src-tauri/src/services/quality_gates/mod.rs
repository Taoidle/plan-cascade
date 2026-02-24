//! Quality Gates Service
//!
//! Provides automatic project type detection, quality gate execution,
//! three-phase pipeline orchestration, formatting, caching, and AI-powered
//! quality gates (verification, code review, DoR, DoD).
//! Supports Node.js, Rust, Python, and Go projects with appropriate validators.

pub mod ai_verify;
pub mod cache;
pub mod code_review;
mod detector;
pub mod dod;
pub mod dor;
pub mod format;
pub mod pipeline;
mod runner;
pub mod validation;
mod validators;

pub use ai_verify::AiVerificationGate;
pub use cache::{GateCache, GateCacheKey};
pub use code_review::{CodeReviewGate, CodeReviewResult};
pub use detector::*;
pub use dod::{DoDGate, DoDInput};
pub use dor::{DoRGate, StoryForValidation};
pub use format::FormatGate;
pub use pipeline::{
    GateMode, GatePhase, GatePipeline, PipelineConfig, PipelineGateResult, PipelinePhaseResult,
    PipelineResult,
};
pub use runner::*;
pub use validation::ValidationGate;
pub use validators::*;
