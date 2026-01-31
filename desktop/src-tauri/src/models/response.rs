//! Response Types
//!
//! Standard response types for all Tauri commands.

use serde::{Deserialize, Serialize};

/// Generic command response for all Tauri commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> CommandResponse<T> {
    /// Create a successful response with data
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create an error response with message
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

impl<T> From<Result<T, crate::utils::error::AppError>> for CommandResponse<T> {
    fn from(result: Result<T, crate::utils::error::AppError>) -> Self {
        match result {
            Ok(data) => Self::ok(data),
            Err(e) => Self::err(e.to_string()),
        }
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub service: String,
    pub database: bool,
    pub keyring: bool,
    pub config: bool,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            service: "plan-cascade-desktop".to_string(),
            database: false,
            keyring: false,
            config: false,
        }
    }
}

/// Execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub description: String,
    pub mode: String,
    pub project_path: Option<String>,
    pub use_worktree: bool,
}

/// Execution status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub task_description: String,
    pub current_story_id: Option<String>,
    pub stories: Vec<StoryExecutionStatus>,
    pub progress: f64,
}

/// Individual story execution status (for StatusResponse)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryExecutionStatus {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_response_ok() {
        let response = CommandResponse::ok("test".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("test".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_command_response_err() {
        let response: CommandResponse<String> = CommandResponse::err("error message");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("error message".to_string()));
    }

    #[test]
    fn test_health_response_default() {
        let health = HealthResponse::default();
        assert_eq!(health.status, "healthy");
        assert_eq!(health.service, "plan-cascade-desktop");
    }
}
