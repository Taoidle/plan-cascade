//! Spec Interview Service
//!
//! Multi-turn LLM-driven interview service for eliciting project requirements.
//! Produces spec.json and spec.md from completed interviews.
//!
//! ## Architecture
//! - `interview.rs` - Multi-turn conversation management with contextual question generation
//! - `state.rs` - SQLite-backed interview state persistence with resume support
//! - `compiler.rs` - Spec compilation to spec.json, spec.md, and PRD format

pub mod compiler;
pub mod interview;
pub mod state;

pub use compiler::{CompileOptions, SpecCompiler};
pub use interview::{InterviewManager, InterviewPhase, InterviewQuestion};
pub use state::{InterviewStateManager, InterviewTurn, PersistedInterviewState};
