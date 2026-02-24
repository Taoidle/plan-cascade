//! Git Service Module
//!
//! Comprehensive git service for the Plan Cascade Desktop application.
//! Built on top of the existing GitOps foundation in services/worktree/git_ops.rs.

pub mod conflict;
pub mod graph;
pub mod llm_assist;
pub mod service;
pub mod types;
pub mod watcher;

pub use conflict::{parse_conflicts, resolve_conflict, resolve_file};
pub use graph::compute_graph_layout;
pub use llm_assist::GitLlmAssist;
pub use service::GitService;
pub use types::*;
pub use watcher::GitWatcher;
