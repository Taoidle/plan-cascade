//! Strategy Analysis Service
//!
//! Provides task complexity analysis and intent classification for
//! automatic execution strategy selection. Ports the Python
//! `strategy.py` and `intent_classifier.py` modules to Rust.
//!
//! ## Strategies
//! - **Direct**: Simple tasks, single-story execution
//! - **Hybrid Auto**: Medium tasks, multi-story PRD with dependencies
//! - **Hybrid Worktree**: Like Hybrid Auto but with Git worktree isolation
//! - **Mega Plan**: Complex projects, multi-feature orchestration

pub mod analyzer;
pub mod classifier;

pub use analyzer::{StrategyAnalyzer, StrategyDecision, ExecutionStrategy, DimensionScores};
pub use classifier::{IntentClassifier, IntentResult, Intent};
