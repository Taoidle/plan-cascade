//! Initialization Commands
//!
//! Commands for application initialization and setup.
//! On startup, initializes all backend services and scans for
//! interrupted executions that may need recovery. Also auto-starts
//! the remote gateway if configured.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::mcp::reconcile_and_connect_enabled_servers;
use crate::commands::mcp::McpRuntimeState;
use crate::commands::plugins::PluginState;
use crate::commands::remote::RemoteState;
use crate::commands::guardrails::GuardrailState;
use crate::commands::spec_interview::SpecInterviewState;
use crate::commands::standalone::StandaloneState;
use crate::commands::webhook::WebhookState;
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

const INIT_PROGRESS_EVENT: &str = "app-init-progress";
const INIT_TOTAL_STEPS: u8 = 6;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitStage {
    CoreState,
    Plugins,
    IndexManager,
    SpecInterview,
    RecoveryScan,
    RemoteGateway,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitProgressEvent {
    pub stage: InitStage,
    pub step_index: u8,
    pub total_steps: u8,
}

fn emit_init_progress(app: &AppHandle, stage: InitStage, step_index: u8) {
    if let Err(e) = app.emit(
        INIT_PROGRESS_EVENT,
        InitProgressEvent {
            stage,
            step_index,
            total_steps: INIT_TOTAL_STEPS,
        },
    ) {
        tracing::debug!(
            error = %e,
            stage = ?stage,
            step = step_index,
            "Failed to emit init progress event"
        );
    }
}

/// Initialize the application on startup
/// This command sets up all backend services, prepares the app for use,
/// initializes the IndexManager for background codebase indexing,
/// scans for any interrupted executions that need recovery,
/// and auto-starts the remote gateway if configured.
#[tauri::command]
pub async fn init_app(
    state: State<'_, AppState>,
    guardrail_state: State<'_, GuardrailState>,
    standalone_state: State<'_, StandaloneState>,
    remote_state: State<'_, RemoteState>,
    webhook_state: State<'_, WebhookState>,
    plugin_state: State<'_, PluginState>,
    spec_interview_state: State<'_, SpecInterviewState>,
    mcp_state: State<'_, McpRuntimeState>,
    app: AppHandle,
) -> Result<CommandResponse<InitResult>, String> {
    emit_init_progress(&app, InitStage::CoreState, 1);

    // Initialize all services
    match state.initialize().await {
        Ok(_) => {
            if let Ok(database) = state.with_database(|db| Ok(std::sync::Arc::new(db.clone()))).await {
                if let Err(e) = guardrail_state.initialize(database).await {
                    tracing::warn!("Guardrail initialization failed: {}", e);
                }
            }

            if let Err(e) = webhook_state.start_worker_if_needed(state.inner()).await {
                tracing::warn!("Webhook worker initialization failed: {}", e);
            }

            emit_init_progress(&app, InitStage::Plugins, 2);

            // Initialize the plugin system early (ADR-F003).
            // Plugin discovery is fast (directory scanning only) and does not
            // depend on the database or index. Initializing it before the
            // potentially slow ensure_indexed() prevents a race where the
            // frontend's plugin retry logic (~7.5 s) exhausts before init_app
            // finishes, causing "Plugin system not initialized" errors.
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

            emit_init_progress(&app, InitStage::IndexManager, 3);

            // Initialize IndexManager with the database pool.
            // Store in StandaloneState BEFORE calling ensure_indexed() so that
            // get_index_status queries can immediately retrieve the manager and
            // return the previous indexed state from SQLite (fixes status not
            // showing after app restart).
            if let Ok(pool) = state.with_database(|db| Ok(db.pool().clone())).await {
                let manager = IndexManager::new(pool);
                manager.set_app_handle(app.clone()).await;

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

            emit_init_progress(&app, InitStage::SpecInterview, 4);

            // Initialize the spec interview service
            if let Ok(pool) = state.with_database(|db| Ok(db.pool().clone())).await {
                if let Err(e) = spec_interview_state.initialize(pool).await {
                    tracing::warn!("Spec interview initialization failed: {}", e);
                }
            }

            emit_init_progress(&app, InitStage::RecoveryScan, 5);

            // Scan for incomplete executions after initialization
            let incomplete_tasks = state
                .with_database(|db| RecoveryDetector::detect(db))
                .await
                .unwrap_or_default();

            // Auto-connect enabled MCP servers in background. Non-fatal and non-blocking.
            {
                let manager = mcp_state.manager.clone();
                let registry = mcp_state.registry.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) =
                        reconcile_and_connect_enabled_servers(manager, registry, "startup").await
                    {
                        tracing::warn!("MCP init auto-connect failed: {}", e);
                    }
                });
            }

            emit_init_progress(&app, InitStage::RemoteGateway, 6);

            // Auto-start remote gateway if configured.
            // This is non-blocking: failures are logged but do not prevent app init.
            let remote_gateway_auto_started = match crate::commands::remote::try_auto_start_gateway(
                &remote_state,
                &state,
                &webhook_state,
                &app,
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
