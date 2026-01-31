//! Context Management Service
//!
//! Filters and provides context for agent execution based on phase and agent type.

mod filter;

pub use filter::{ContextFilter, ContextFilterConfig, ContextTag, StoryContext, ContextError};
