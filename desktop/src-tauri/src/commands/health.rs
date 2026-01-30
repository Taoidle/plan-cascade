//! Health Check Commands
//!
//! Commands for checking the health status of backend services.

use tauri::State;

use crate::models::response::{CommandResponse, HealthResponse};
use crate::state::AppState;

/// Get the health status of all backend services
#[tauri::command]
pub async fn get_health(state: State<'_, AppState>) -> Result<CommandResponse<HealthResponse>, String> {
    let mut health = HealthResponse::default();

    // Check database health
    health.database = state.is_database_healthy();

    // Check keyring health
    health.keyring = state.is_keyring_healthy();

    // Check config health
    health.config = state.is_config_healthy();

    // Overall status
    health.status = if health.database && health.keyring && health.config {
        "healthy".to_string()
    } else {
        "degraded".to_string()
    };

    Ok(CommandResponse::ok(health))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_fields() {
        let health = HealthResponse::default();
        assert_eq!(health.service, "plan-cascade-desktop");
    }
}
