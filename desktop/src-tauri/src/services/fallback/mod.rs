//! Agent Fallback Chain Service
//!
//! Provides fallback execution when primary agents fail.

mod chain;

pub use chain::{
    AgentFallbackChain, FailureReason, FallbackAttempt, FallbackConfig, FallbackError,
    FallbackExecutionLog, FallbackResult,
};
