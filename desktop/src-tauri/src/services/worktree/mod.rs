//! Worktree Service
//!
//! Git worktree management for isolated story execution.
//! Provides creation, listing, completion, and cleanup of worktrees.

mod config;
mod git_ops;
mod manager;

pub use config::*;
pub use git_ops::*;
pub use manager::*;
