//! Quality Gates Service
//!
//! Provides automatic project type detection and quality gate execution.
//! Supports Node.js, Rust, Python, and Go projects with appropriate validators.

mod detector;
mod runner;
mod validators;

pub use detector::*;
pub use runner::*;
pub use validators::*;
