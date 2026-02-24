//! Initialization Commands
//!
//! Commands for application initialization and setup.
//! On startup, initializes all backend services and scans for
//! interrupted executions that may need recovery. Also auto-starts
//! the remote gateway if configured.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::plugins::PluginState;
use crate::commands::remote::RemoteState;
use crate::commands::spec_interview::SpecInterviewState;
use crate::commands::standalone::StandaloneState;
use crate::models::response::CommandResponse;
use crate::services::orchestrator::index_manager::IndexManager;
use crate::services::recovery::detector::{IncompleteTask, RecoveryDetector};
use crate::state::AppState;

/// Result of application initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResult {
    /// Success message
    pub message: String,
    /// Incomplete tasks detected during initialization
    pub incomplete_tasks: Vec<IncompleteTask>,
    /// Whether the remote gateway was auto-started
    #[serde(default)]
    pub remote_gateway_auto_started: bool,
}

/// Initialize the application on startup
/// This command sets up all backend services, prepares the app for use,
/// initializes the IndexManager for background codebase indexing,
/// scans for any interrupted executions that need recovery,
/// and auto-starts the remote gateway if configured.
#[tauri::command]
pub async fn init_app(
    state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
    remote_state: State<'_, RemoteState>,
    plugin_state: State<'_, PluginState>,
    spec_interview_state: State<'_, SpecInterviewState>,
    app: AppHandle,
) -> Result<CommandResponse<InitResult>, String> {
    // Initialize all services
    match state.initialize().await {
        Ok(_) => {
            // Initialize IndexManager with the database pool.
            // Store in StandaloneState BEFORE calling ensure_indexed() so that
            // get_index_status queries can immediately retrieve the manager and
            // return the previous indexed state from SQLite (fixes status not
            // showing after app restart).
            if let Ok(pool) = state.with_database(|db| Ok(db.pool().clone())).await {
                let manager = IndexManager::new(pool);
                manager.set_app_handle(app).await;

                // Store immediately so frontend can query status
                {
                    let mut mgr_lock = standalone_state.index_manager.write().await;
                    *mgr_lock = Some(manager);
                }

                // If a working directory is already set, trigger indexing
                // via the stored reference
                let working_dir = {
                    let wd = standalone_state.working_directory.read().await;
                    wd.to_string_lossy().to_string()
                };

                if !working_dir.is_empty() && working_dir != "." {
                    let mgr_lock = standalone_state.index_manager.read().await;
                    if let Some(ref manager) = *mgr_lock {
                        manager.ensure_indexed(&working_dir).await;
                    }
                }
            }

            // Initialize the spec interview service
            if let Ok(pool) = state.with_database(|db| Ok(db.pool().clone())).await {
                if let Err(e) = spec_interview_state.initialize(pool).await {
                    tracing::warn!("Spec interview initialization failed: {}", e);
                }
            }

            // Initialize the plugin system (ADR-F003)
            {
                let plugin_root = {
                    let wd = standalone_state.working_directory.read().await;
                    let wd_str = wd.to_string_lossy().to_string();
                    if wd_str.is_empty() || wd_str == "." {
                        // Fall back to user home directory
                        dirs::home_dir()
                            .map(|h| h.to_string_lossy().to_string())
                            .unwrap_or_else(|| ".".to_string())
                    } else {
                        wd_str
                    }
                };
                plugin_state.initialize(&plugin_root).await;
                tracing::info!("Plugin system initialized with root: {}", plugin_root);
            }

            // Scan for incomplete executions after initialization
            let incomplete_tasks = state
                .with_database(|db| RecoveryDetector::detect(db))
                .await
                .unwrap_or_default();

            // Auto-start remote gateway if configured.
            // This is non-blocking: failures are logged but do not prevent app init.
            let remote_gateway_auto_started = match crate::commands::remote::try_auto_start_gateway(
                &remote_state,
                &state,
            )
            .await
            {
                Ok(started) => started,
                Err(e) => {
                    tracing::warn!("Remote gateway auto-start failed: {}", e);
                    false
                }
            };

            let task_count = incomplete_tasks.len();
            let mut message = if task_count > 0 {
                format!(
                    "Application initialized successfully. Found {} interrupted execution(s).",
                    task_count
                )
            } else {
                "Application initialized successfully".to_string()
            };

            if remote_gateway_auto_started {
                message.push_str(" Remote gateway auto-started.");
            }

            Ok(CommandResponse::ok(InitResult {
                message,
                incomplete_tasks,
                remote_gateway_auto_started,
            }))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get the application version
#[tauri::command]
pub fn get_version() -> CommandResponse<String> {
    CommandResponse::ok(env!("CARGO_PKG_VERSION").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_result_serialization() {
        let result = InitResult {
            message: "test".to_string(),
            incomplete_tasks: vec![],
            remote_gateway_auto_started: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("remote_gateway_auto_started"));
        assert!(json.contains("true"));

        let parsed: InitResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.remote_gateway_auto_started);
    }

    #[test]
    fn test_init_result_backward_compat() {
        // Old JSON without remote_gateway_auto_started should default to false
        let old_json = r#"{"message":"test","incomplete_tasks":[]}"#;
        let parsed: InitResult = serde_json::from_str(old_json).unwrap();
        assert!(!parsed.remote_gateway_auto_started);
    }
}
