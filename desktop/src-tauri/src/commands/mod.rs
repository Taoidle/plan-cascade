//! Tauri Commands
//!
//! Contains all Tauri command handlers that can be called from the frontend.
//! These are the IPC entry points for the application.

pub mod health;
pub mod init;
pub mod mcp;
pub mod projects;
pub mod sessions;
pub mod settings;
pub mod standalone;

pub use health::*;
pub use init::*;
pub use mcp::*;
pub use projects::*;
pub use sessions::*;
pub use settings::*;
pub use standalone::*;
