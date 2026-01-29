//! Tauri Commands
//!
//! Defines commands that can be called from the frontend to interact
//! with the Python sidecar server.

use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync::Mutex;
use tauri::State;

/// Global state for managing the sidecar process
pub struct SidecarState {
    process: Mutex<Option<Child>>,
    port: Mutex<u16>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            process: Mutex::new(None),
            port: Mutex::new(8765),
        }
    }
}

/// Response structure for API calls
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub service: String,
}

/// Execution request
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub description: String,
    pub mode: String,
    pub project_path: Option<String>,
    pub use_worktree: bool,
}

/// Execution status response
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub task_description: String,
    pub current_story_id: Option<String>,
    pub stories: Vec<StoryStatus>,
    pub progress: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoryStatus {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
}

/// Start the Python sidecar server
#[tauri::command]
pub async fn start_sidecar(state: State<'_, SidecarState>) -> Result<ApiResponse<String>, String> {
    let mut process_guard = state.process.lock().map_err(|e| e.to_string())?;

    // Check if already running
    if process_guard.is_some() {
        return Ok(ApiResponse::ok("Sidecar already running".to_string()));
    }

    // Get the port
    let port = *state.port.lock().map_err(|e| e.to_string())?;

    // Start the Python server
    // In production, this would use the bundled Python executable
    let child = std::process::Command::new("python")
        .args([
            "-m",
            "plan_cascade_server.main",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .spawn()
        .map_err(|e| format!("Failed to start sidecar: {}", e))?;

    *process_guard = Some(child);

    Ok(ApiResponse::ok(format!(
        "Sidecar started on port {}",
        port
    )))
}

/// Stop the Python sidecar server
#[tauri::command]
pub async fn stop_sidecar(state: State<'_, SidecarState>) -> Result<ApiResponse<String>, String> {
    let mut process_guard = state.process.lock().map_err(|e| e.to_string())?;

    if let Some(mut child) = process_guard.take() {
        child.kill().map_err(|e| format!("Failed to stop sidecar: {}", e))?;
        Ok(ApiResponse::ok("Sidecar stopped".to_string()))
    } else {
        Ok(ApiResponse::err("Sidecar not running"))
    }
}

/// Execute a task via the sidecar
#[tauri::command]
pub async fn execute_task(
    state: State<'_, SidecarState>,
    request: ExecuteRequest,
) -> Result<ApiResponse<String>, String> {
    let port = *state.port.lock().map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{}/api/execute", port);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status().is_success() {
        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(ApiResponse::ok(
            data.get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ))
    } else {
        Ok(ApiResponse::err(format!(
            "Server returned status {}",
            response.status()
        )))
    }
}

/// Get the current execution status
#[tauri::command]
pub async fn get_status(
    state: State<'_, SidecarState>,
) -> Result<ApiResponse<StatusResponse>, String> {
    let port = *state.port.lock().map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{}/api/status", port);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if response.status().is_success() {
        let status: StatusResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(ApiResponse::ok(status))
    } else {
        Ok(ApiResponse::err(format!(
            "Server returned status {}",
            response.status()
        )))
    }
}

/// Check the health of the sidecar
#[tauri::command]
pub async fn get_health(
    state: State<'_, SidecarState>,
) -> Result<ApiResponse<HealthResponse>, String> {
    let port = *state.port.lock().map_err(|e| e.to_string())?;
    let url = format!("http://127.0.0.1:{}/api/health", port);

    let client = reqwest::Client::new();

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let health: HealthResponse = response
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse response: {}", e))?;

                Ok(ApiResponse::ok(health))
            } else {
                Ok(ApiResponse::err("Sidecar unhealthy"))
            }
        }
        Err(_) => Ok(ApiResponse::err("Sidecar not responding")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_ok() {
        let response = ApiResponse::ok("test".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("test".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_err() {
        let response: ApiResponse<String> = ApiResponse::err("error message");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("error message".to_string()));
    }
}
