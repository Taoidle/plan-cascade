//! Quality Gates Service
//!
//! Provides automatic project type detection and quality gate execution.
//! Supports Node.js, Rust, Python, and Go projects with appropriate validators.

mod detector;
mod validators;
mod runner;

pub use detector::*;
pub use validators::*;
pub use runner::*;
