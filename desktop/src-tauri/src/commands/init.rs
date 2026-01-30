//! Initialization Commands
//!
//! Commands for application initialization and setup.

use tauri::State;

use crate::models::response::CommandResponse;
use crate::state::AppState;

/// Initialize the application on startup
/// This command sets up all backend services and prepares the app for use.
#[tauri::command]
pub async fn init_app(state: State<'_, AppState>) -> Result<CommandResponse<String>, String> {
    // Initialize all services
    match state.initialize().await {
        Ok(_) => Ok(CommandResponse::ok("Application initialized successfully".to_string())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the application version
#[tauri::command]
pub fn get_version() -> CommandResponse<String> {
    CommandResponse::ok(env!("CARGO_PKG_VERSION").to_string())
}
