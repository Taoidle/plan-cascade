//! Dependency Analysis Module
//!
//! Provides dependency analysis for PRD stories, including batch generation
//! and circular dependency detection.

mod analyzer;

pub use analyzer::{Batch, DependencyAnalyzer, DependencyError};
