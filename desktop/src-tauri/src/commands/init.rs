//! Initialization Commands
//!
//! Commands for application initialization and setup.
//! On startup, initializes all backend services and scans for
//! interrupted executions that may need recovery. Also auto-starts
//! the remote gateway if configured.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::mcp::McpRuntimeState;
use crate::commands::plugins::PluginState;
use crate::commands::remote::RemoteState;
use crate::commands::spec_interview::SpecInterviewState;
use crate::commands::standalone::StandaloneState;
use crate::commands::webhook::WebhookState;
use crate::models::response::CommandResponse;
use crate::services::mcp::McpService;
use crate::services::orchestrator::index_manager::IndexManager;
use crate::services::recovery::detector::{IncompleteTask, RecoveryDetector};
use crate::services::tools::mcp_manager::McpManager;
use crate::services::tools::runtime_tools;
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
                    let service = match McpService::new() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to initialize MCP service for auto-connect: {}", e);
                            return;
                        }
                    };

                    let servers = match service.list_enabled_auto_connect_servers() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to list auto-connect MCP servers: {}", e);
                            return;
                        }
                    };

                    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
                    let mut join_set = tokio::task::JoinSet::new();

                    for server in servers {
                        let config = match McpManager::config_from_model(&server) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::warn!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    error = %e,
                                    "Skipping MCP server during init auto-connect"
                                );
                                let _ = service.mark_server_connection_error(&server.id, &e.to_string());
                                continue;
                            }
                        };

                        let permit = match semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => {
                                let msg = "transport: connector semaphore closed".to_string();
                                let _ = service.mark_server_connection_error(&server.id, &msg);
                                tracing::warn!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    error = %msg,
                                    "Skipping MCP server during init auto-connect"
                                );
                                continue;
                            }
                        };

                        let manager = manager.clone();
                        let registry = registry.clone();
                        join_set.spawn(async move {
                            let _permit = permit;
                            let started = std::time::Instant::now();
                            let result = tokio::time::timeout(
                                std::time::Duration::from_secs(10),
                                manager.connect_server_with_registry_lock(&config, registry),
                            )
                            .await;
                            let latency_ms = started.elapsed().as_millis() as u64;
                            (server, result, latency_ms)
                        });
                    }

                    let mut success_count = 0u32;
                    let mut failure_count = 0u32;
                    let mut latency_total_ms = 0u64;

                    while let Some(joined) = join_set.join_next().await {
                        match joined {
                            Ok((server, Ok(Ok(info)), latency_ms)) => {
                                let _ = service.mark_server_connected(&server.id);
                                success_count += 1;
                                latency_total_ms += latency_ms;
                                tracing::info!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    tool_count = info.qualified_tool_names.len(),
                                    latency_ms = latency_ms,
                                    "Auto-connected MCP server during init"
                                );
                            }
                            Ok((server, Ok(Err(e)), latency_ms)) => {
                                failure_count += 1;
                                tracing::warn!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    latency_ms = latency_ms,
                                    error = %e,
                                    "Failed to auto-connect MCP server during init"
                                );
                                let _ = service.mark_server_connection_error(&server.id, &e.to_string());
                            }
                            Ok((server, Err(_), latency_ms)) => {
                                failure_count += 1;
                                tracing::warn!(
                                    server_id = %server.id,
                                    server_name = %server.name,
                                    latency_ms = latency_ms,
                                    "Auto-connect MCP server timed out during init"
                                );
                                let _ = service.mark_server_connection_error(
                                    &server.id,
                                    "transport: connection timeout",
                                );
                            }
                            Err(e) => {
                                failure_count += 1;
                                tracing::warn!(
                                    error = %e,
                                    "MCP init auto-connect task join failed"
                                );
                            }
                        }
                    }

                    let registry_guard = registry.read().await;
                    runtime_tools::replace_from_registry(&registry_guard);

                    let average_latency_ms = if success_count > 0 {
                        latency_total_ms / u64::from(success_count)
                    } else {
                        0
                    };
                    tracing::info!(
                        success_count = success_count,
                        failure_count = failure_count,
                        average_latency_ms = average_latency_ms,
                        "MCP init auto-connect summary"
                    );
                });
            }

            emit_init_progress(&app, InitStage::RemoteGateway, 6);

            // Auto-start remote gateway if configured.
            // This is non-blocking: failures are logged but do not prevent app init.
            let remote_gateway_auto_started = match crate::commands::remote::try_auto_start_gateway(
                &remote_state,
                &state,
                &webhook_state,
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
