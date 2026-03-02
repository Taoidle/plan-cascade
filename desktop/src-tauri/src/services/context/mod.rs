//! Context Management Service
//!
//! Filters and provides context for agent execution based on phase and agent type.

pub mod assembly;
pub mod events;
mod filter;

pub use filter::{ContextError, ContextFilter, ContextFilterConfig, ContextTag, StoryContext};
