//! Tauri Commands
//!
//! Contains all Tauri command handlers that can be called from the frontend.
//! These are the IPC entry points for the application.

pub mod health;
pub mod init;
pub mod settings;

pub use health::*;
pub use init::*;
pub use settings::*;
