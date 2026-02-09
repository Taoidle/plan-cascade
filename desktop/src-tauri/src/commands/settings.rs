//! Settings Commands
//!
//! Commands for reading and updating application settings.

use tauri::State;

use crate::models::response::CommandResponse;
use crate::models::settings::{AppConfig, SettingsUpdate};
use crate::state::AppState;

/// Get current application settings
#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<CommandResponse<AppConfig>, String> {
    match state.get_config().await {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update application settings with a partial update
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    update: SettingsUpdate,
) -> Result<CommandResponse<AppConfig>, String> {
    match state.update_config(update).await {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}
