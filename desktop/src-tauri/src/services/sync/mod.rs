//! File System Sync Service
//!
//! Provides real-time file system watching for the Plan Cascade Desktop application.
//! Uses the `notify` crate for cross-platform file system event monitoring.
//!
//! Features:
//! - Watch ~/.claude/projects/ for project changes
//! - Watch current project directories for file modifications
//! - Watch prd.json and progress.txt for story updates
//! - Debounce rapid changes (100ms threshold)
//! - Broadcast events via Tauri IPC to all windows

mod events;
mod watcher;

pub use events::*;
pub use watcher::*;
