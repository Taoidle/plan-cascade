//! Iteration Loop Module
//!
//! Provides the auto-iteration system for story execution with quality gates.

mod loop_runner;
mod story_executor;

pub use loop_runner::{
    IterationEvent, IterationLoop, IterationLoopConfig, IterationLoopError, QualityGateContext,
    QualityGateResult, QualityGateRunnerFn, StoryExecutionContext, StoryExecutorFn,
};
pub use story_executor::{
    RetryQueue, StoryExecutionResult, StoryExecutor, StoryExecutorConfig, StoryExecutorError,
    StoryRetryInfo,
};
