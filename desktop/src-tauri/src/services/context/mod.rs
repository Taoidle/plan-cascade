//! Context Management Service
//!
//! Filters and provides context for agent execution based on phase and agent type.

mod filter;

pub use filter::{ContextError, ContextFilter, ContextFilterConfig, ContextTag, StoryContext};
