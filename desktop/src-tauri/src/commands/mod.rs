//! Tauri Commands
//!
//! Contains all Tauri command handlers that can be called from the frontend.
//! These are the IPC entry points for the application.

pub mod agents;
pub mod analytics;
pub mod claude_code;
pub mod design;
pub mod embedding;
pub mod files;
pub mod guardrails;
pub mod health;
pub mod init;
pub mod lsp;
pub mod markdown;
pub mod mcp;
pub mod memory;
pub mod projects;
pub mod quality_gates;
pub mod recovery;
pub mod sessions;
pub mod settings;
pub mod skills;
pub mod spec_interview;
pub mod standalone;
pub mod strategy;
pub mod timeline;
pub mod worktree;

pub use agents::*;
pub use analytics::*;
pub use claude_code::*;
pub use design::*;
pub use embedding::*;
pub use files::*;
pub use guardrails::*;
pub use health::*;
pub use init::*;
pub use lsp::*;
pub use markdown::*;
pub use mcp::*;
pub use memory::*;
pub use projects::*;
pub use quality_gates::*;
pub use recovery::*;
pub use sessions::*;
pub use settings::*;
pub use skills::*;
pub use spec_interview::*;
pub use standalone::*;
pub use strategy::*;
pub use timeline::*;
pub use worktree::*;
