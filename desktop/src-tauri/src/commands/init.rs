//! Initialization Commands
//!
//! Commands for application initialization and setup.
//! On startup, initializes all backend services and scans for
//! interrupted executions that may need recovery.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

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
}

/// Initialize the application on startup
/// This command sets up all backend services, prepares the app for use,
/// initializes the IndexManager for background codebase indexing,
/// and scans for any interrupted executions that need recovery.
#[tauri::command]
pub async fn init_app(
    state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
    app: AppHandle,
) -> Result<CommandResponse<InitResult>, String> {
    // Initialize all services
    match state.initialize().await {
        Ok(_) => {
            // Initialize IndexManager with the database pool
            if let Ok(pool) = state.with_database(|db| Ok(db.pool().clone())).await {
                let manager = IndexManager::new(pool);
                manager.set_app_handle(app).await;

                // If a working directory is already set, trigger indexing
                let working_dir = {
                    let wd = standalone_state.working_directory.read().await;
                    wd.to_string_lossy().to_string()
                };

                if !working_dir.is_empty() && working_dir != "." {
                    manager.ensure_indexed(&working_dir).await;
                }

                // Store in StandaloneState
                let mut mgr_lock = standalone_state.index_manager.write().await;
                *mgr_lock = Some(manager);
            }

            // Scan for incomplete executions after initialization
            let incomplete_tasks = state
                .with_database(|db| RecoveryDetector::detect(db))
                .await
                .unwrap_or_default();

            let task_count = incomplete_tasks.len();
            let message = if task_count > 0 {
                format!(
                    "Application initialized successfully. Found {} interrupted execution(s).",
                    task_count
                )
            } else {
                "Application initialized successfully".to_string()
            };

            Ok(CommandResponse::ok(InitResult {
                message,
                incomplete_tasks,
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
