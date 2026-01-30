//! Plan Cascade Desktop - Rust Backend Library
//!
//! This library provides the core backend functionality for the Plan Cascade Desktop application.
//! It includes:
//! - Tauri command handlers for frontend IPC
//! - Business logic services
//! - Storage layer (SQLite, Keyring, Config)
//! - Data models and utilities

pub mod commands;
pub mod models;
pub mod services;
pub mod state;
pub mod storage;
pub mod utils;

// Re-export commonly used items from commands
pub use commands::{
    // Init commands
    init_app, get_version,
    // Health commands
    get_health,
    // Settings commands
    get_settings, update_settings,
    // Project commands
    list_projects, get_project, search_projects,
    // Session commands
    list_sessions, get_session, resume_session, search_sessions,
    // MCP commands
    list_mcp_servers, add_mcp_server, update_mcp_server, remove_mcp_server,
    test_mcp_server, toggle_mcp_server, import_from_claude_desktop,
};
// Re-export models (avoiding settings module conflict)
pub use models::response::*;
pub use models::settings::{AppConfig, SettingsUpdate};
pub use state::AppState;
pub use utils::error::{AppError, AppResult};
