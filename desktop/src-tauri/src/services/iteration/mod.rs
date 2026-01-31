//! Iteration Loop Module
//!
//! Provides the auto-iteration system for story execution with quality gates.

mod loop_runner;

pub use loop_runner::{IterationLoop, IterationLoopConfig, IterationLoopError, IterationEvent};
