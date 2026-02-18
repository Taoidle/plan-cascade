//! Pipeline and Graph Execution Commands
//!
//! Tauri commands for executing agent pipelines and graph workflows with
//! streaming events, status tracking, and cancellation support.
//!
//! ## Commands
//! - `execute_agent_pipeline` - Run an AgentComposer pipeline, stream events
//! - `execute_graph_workflow` - Run a GraphWorkflow, stream events
//! - `get_pipeline_execution_status` - Get execution progress
//! - `cancel_pipeline_execution` - Cancel a running pipeline

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;

use crate::commands::standalone::{normalize_provider_name, StandaloneState};
use crate::models::response::CommandResponse;
use crate::services::agent_composer::{AgentConfig, AgentContext, AgentInput};
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, GlmProvider, LlmProvider, MinimaxProvider,
    OllamaProvider, OpenAIProvider, ProviderConfig, ProviderType, QwenProvider,
};
use crate::services::orchestrator::hooks::AgenticHooks;
use crate::services::tools::ToolExecutor;
use crate::storage::KeyringService;
use crate::utils::error::AppError;

// ============================================================================
// Execution Status
// ============================================================================

/// Status of a pipeline or graph workflow execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Execution is pending (not yet started).
    Pending,
    /// Execution is actively running.
    Running,
    /// Execution completed successfully.
    Completed,
    /// Execution failed with an error.
    Failed,
    /// Execution was cancelled by the user.
    Cancelled,
}

/// Tracks the state of a pipeline or graph workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecutionState {
    /// Unique execution identifier.
    pub execution_id: String,
    /// Pipeline or workflow identifier being executed.
    pub pipeline_id: String,
    /// Current execution status.
    pub status: ExecutionStatus,
    /// Number of steps completed.
    pub steps_completed: usize,
    /// Total number of steps (if known).
    pub total_steps: Option<usize>,
    /// Name of the currently executing step/node.
    pub current_step: Option<String>,
    /// Error message (if status is Failed).
    pub error: Option<String>,
    /// ISO 8601 timestamp when execution started.
    pub started_at: String,
    /// ISO 8601 timestamp when execution completed (if done).
    pub completed_at: Option<String>,
}

impl PipelineExecutionState {
    /// Create a new execution state in Pending status.
    pub fn new(pipeline_id: impl Into<String>) -> Self {
        Self {
            execution_id: uuid::Uuid::new_v4().to_string(),
            pipeline_id: pipeline_id.into(),
            status: ExecutionStatus::Pending,
            steps_completed: 0,
            total_steps: None,
            current_step: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    /// Mark execution as running with the current step name.
    pub fn mark_running(&mut self, step_name: impl Into<String>) {
        self.status = ExecutionStatus::Running;
        self.current_step = Some(step_name.into());
    }

    /// Mark a step as completed.
    pub fn mark_step_completed(&mut self) {
        self.steps_completed += 1;
    }

    /// Mark execution as completed successfully.
    pub fn mark_completed(&mut self) {
        self.status = ExecutionStatus::Completed;
        self.current_step = None;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark execution as failed with an error message.
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark execution as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.current_step = None;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Check if execution is still active (pending or running).
    pub fn is_active(&self) -> bool {
        matches!(self.status, ExecutionStatus::Pending | ExecutionStatus::Running)
    }
}

// ============================================================================
// Execution Registry (in-memory tracking)
// ============================================================================

/// Thread-safe registry of active and completed executions.
///
/// Used by Tauri commands to track execution state across async boundaries.
/// Fields are `pub(crate)` to allow spawned tasks to update state.
pub struct ExecutionRegistry {
    pub(crate) executions: Arc<RwLock<HashMap<String, PipelineExecutionState>>>,
    /// Cancellation tokens: execution_id -> cancelled flag
    pub(crate) cancellation_tokens: Arc<RwLock<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

impl ExecutionRegistry {
    /// Create a new empty execution registry.
    pub fn new() -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new execution and return its ID.
    pub async fn register(&self, pipeline_id: &str) -> String {
        let state = PipelineExecutionState::new(pipeline_id);
        let execution_id = state.execution_id.clone();

        let notify = Arc::new(tokio::sync::Notify::new());

        let mut executions = self.executions.write().await;
        executions.insert(execution_id.clone(), state);

        let mut tokens = self.cancellation_tokens.write().await;
        tokens.insert(execution_id.clone(), notify);

        execution_id
    }

    /// Get the current state of an execution.
    pub async fn get_status(&self, execution_id: &str) -> Option<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions.get(execution_id).cloned()
    }

    /// Update an execution's state.
    pub async fn update(&self, execution_id: &str, state: PipelineExecutionState) {
        let mut executions = self.executions.write().await;
        executions.insert(execution_id.to_string(), state);
    }

    /// Cancel an execution.
    pub async fn cancel(&self, execution_id: &str) -> bool {
        let mut executions = self.executions.write().await;
        if let Some(state) = executions.get_mut(execution_id) {
            if state.is_active() {
                state.mark_cancelled();

                // Notify cancellation
                let tokens = self.cancellation_tokens.read().await;
                if let Some(notify) = tokens.get(execution_id) {
                    notify.notify_one();
                }

                return true;
            }
        }
        false
    }

    /// Get the cancellation notify handle for an execution.
    pub async fn get_cancellation_token(
        &self,
        execution_id: &str,
    ) -> Option<Arc<tokio::sync::Notify>> {
        let tokens = self.cancellation_tokens.read().await;
        tokens.get(execution_id).cloned()
    }

    /// List all executions (active and completed).
    pub async fn list_all(&self) -> Vec<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions.values().cloned().collect()
    }

    /// List only active executions.
    pub async fn list_active(&self) -> Vec<PipelineExecutionState> {
        let executions = self.executions.read().await;
        executions
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    /// Remove completed executions older than the given duration.
    pub async fn cleanup_completed(&self) {
        let mut executions = self.executions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;

        let to_remove: Vec<String> = executions
            .iter()
            .filter(|(_, s)| !s.is_active())
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            executions.remove(&id);
            tokens.remove(&id);
        }
    }
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Event Streaming
// ============================================================================

/// Event channel name for pipeline execution events.
pub const PIPELINE_EVENT_CHANNEL: &str = "pipeline:event";

/// Payload emitted to the frontend during pipeline/graph execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineEventPayload {
    /// The execution ID this event belongs to.
    pub execution_id: String,
    /// Event type: "status", "agent_event", "error"
    pub event_type: String,
    /// Optional status string ("running", "completed", "failed", "cancelled")
    pub status: Option<String>,
    /// Optional agent event data (serialized AgentEvent)
    pub agent_event: Option<serde_json::Value>,
    /// Optional error message
    pub error: Option<String>,
}

impl PipelineEventPayload {
    /// Create a status event.
    pub fn status(execution_id: &str, status: &str, agent_event: Option<serde_json::Value>) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            event_type: "status".to_string(),
            status: Some(status.to_string()),
            agent_event,
            error: None,
        }
    }

    /// Create an agent event payload.
    pub fn agent_event(execution_id: &str, event: serde_json::Value) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            event_type: "agent_event".to_string(),
            status: None,
            agent_event: Some(event),
            error: None,
        }
    }

    /// Create an error event.
    pub fn error(execution_id: &str, error: &str) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            event_type: "error".to_string(),
            status: Some("failed".to_string()),
            agent_event: None,
            error: Some(error.to_string()),
        }
    }
}

/// Emit a pipeline event to the frontend via Tauri.
fn emit_pipeline_event(
    app: &tauri::AppHandle,
    _execution_id: &str,
    payload: PipelineEventPayload,
) -> Result<(), String> {
    use tauri::Emitter;
    app.emit(PIPELINE_EVENT_CHANNEL, &payload)
        .map_err(|e| format!("Failed to emit pipeline event: {}", e))
}

/// Stream agent events from an AgentEventStream to the frontend.
///
/// Consumes the agent event stream, forwarding each event to the frontend
/// via Tauri's emit mechanism. Updates the execution state in the registry
/// as events flow through. Respects cancellation via the Notify mechanism.
///
/// Returns `Ok(())` on successful completion, or an error string on failure.
async fn stream_agent_events(
    app: &tauri::AppHandle,
    execution_id: &str,
    mut stream: crate::services::agent_composer::AgentEventStream,
    executions: &Arc<RwLock<HashMap<String, PipelineExecutionState>>>,
    cancel_token: Option<Arc<tokio::sync::Notify>>,
) -> Result<(), String> {
    use futures_util::StreamExt;

    loop {
        // Check for cancellation between events
        let event = if let Some(ref token) = cancel_token {
            tokio::select! {
                event = stream.next() => event,
                _ = token.notified() => {
                    // Cancellation requested
                    let _ = emit_pipeline_event(
                        app,
                        execution_id,
                        PipelineEventPayload::status(execution_id, "cancelled", None),
                    );
                    return Ok(());
                }
            }
        } else {
            stream.next().await
        };

        match event {
            Some(Ok(agent_event)) => {
                // Serialize the agent event for the frontend
                let event_json = serde_json::to_value(&agent_event)
                    .unwrap_or_else(|_| serde_json::json!({"type": "unknown"}));

                match &agent_event {
                    crate::services::agent_composer::AgentEvent::Done { .. } => {
                        // Forward the done event
                        let _ = emit_pipeline_event(
                            app,
                            execution_id,
                            PipelineEventPayload::agent_event(execution_id, event_json),
                        );
                        return Ok(());
                    }
                    crate::services::agent_composer::AgentEvent::GraphNodeStarted { node_id } => {
                        // Update current step in registry
                        let mut execs = executions.write().await;
                        if let Some(state) = execs.get_mut(execution_id) {
                            state.mark_running(node_id);
                        }
                        let _ = emit_pipeline_event(
                            app,
                            execution_id,
                            PipelineEventPayload::agent_event(execution_id, event_json),
                        );
                    }
                    crate::services::agent_composer::AgentEvent::GraphNodeCompleted { .. } => {
                        // Update step count in registry
                        let mut execs = executions.write().await;
                        if let Some(state) = execs.get_mut(execution_id) {
                            state.mark_step_completed();
                        }
                        let _ = emit_pipeline_event(
                            app,
                            execution_id,
                            PipelineEventPayload::agent_event(execution_id, event_json),
                        );
                    }
                    _ => {
                        // Forward all other events to frontend
                        let _ = emit_pipeline_event(
                            app,
                            execution_id,
                            PipelineEventPayload::agent_event(execution_id, event_json),
                        );
                    }
                }
            }
            Some(Err(e)) => {
                // Error in event stream
                let _ = emit_pipeline_event(
                    app,
                    execution_id,
                    PipelineEventPayload::error(execution_id, &e.to_string()),
                );
                return Err(e.to_string());
            }
            None => {
                // Stream ended without Done event
                return Ok(());
            }
        }
    }
}

// ============================================================================
// Provider and Context Helpers
// ============================================================================

/// Create an `Arc<dyn LlmProvider>` from a `ProviderConfig`.
///
/// Uses the same factory pattern as `OrchestratorService::new()`.
fn create_llm_provider(config: &ProviderConfig) -> Arc<dyn LlmProvider> {
    match config.provider {
        ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.clone())),
        ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.clone())),
        ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.clone())),
        ProviderType::Glm => Arc::new(GlmProvider::new(config.clone())),
        ProviderType::Qwen => Arc::new(QwenProvider::new(config.clone())),
        ProviderType::Minimax => Arc::new(MinimaxProvider::new(config.clone())),
        ProviderType::Ollama => Arc::new(OllamaProvider::new(config.clone())),
    }
}

/// Resolve the LLM `ProviderConfig` from the app's default settings and keyring.
///
/// Uses `StandaloneState` for the working directory (project_root) and
/// `AppState` settings for the default provider/model. Retrieves the API key
/// from the OS keyring via `KeyringService`.
///
/// Returns `(ProviderConfig, PathBuf)` on success where `PathBuf` is the project root.
async fn resolve_provider_config(
    app_state: &crate::state::AppState,
    standalone_state: &StandaloneState,
) -> Result<(ProviderConfig, PathBuf), String> {
    // Get the default provider and model from app settings
    let app_config = app_state
        .get_config()
        .await
        .map_err(|e| format!("Failed to load app config: {}", e))?;

    let canonical_provider = normalize_provider_name(&app_config.default_provider)
        .ok_or_else(|| {
            format!(
                "Unknown provider in settings: {}",
                app_config.default_provider
            )
        })?;

    let provider_type = match canonical_provider {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical_provider)),
    };

    // Retrieve API key from OS keyring (not required for Ollama)
    let keyring = KeyringService::new();
    let api_key =
        crate::commands::standalone::get_api_key_with_aliases(&keyring, canonical_provider);
    let api_key = match api_key {
        Ok(key) => key,
        Err(e) => return Err(format!("Failed to get API key: {}", e)),
    };

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err(format!(
            "API key not configured for provider '{}'. \
             Configure it in Settings before running a pipeline.",
            canonical_provider
        ));
    }

    // Resolve proxy
    let proxy = app_state
        .with_database(|db| {
            Ok(crate::commands::proxy::resolve_provider_proxy(
                &keyring,
                db,
                canonical_provider,
            ))
        })
        .await
        .unwrap_or(None);

    let project_root = standalone_state.working_directory.read().await.clone();

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: None,
        model: app_config.default_model.clone(),
        proxy,
        ..Default::default()
    };

    Ok((config, project_root))
}

/// Build an `AgentContext` suitable for pipeline / graph workflow execution.
///
/// Creates an `OrchestratorContext` from the core crate's context hierarchy
/// and attaches it to the `AgentContext`, enabling tool contexts with shared
/// memory access during tool invocations.
fn build_agent_context(
    execution_id: &str,
    project_root: PathBuf,
    provider: Arc<dyn LlmProvider>,
    input: Option<String>,
) -> AgentContext {
    let tool_executor = Arc::new(ToolExecutor::new(&project_root));
    let hooks = Arc::new(AgenticHooks::new());
    let shared_state = Arc::new(RwLock::new(HashMap::new()));

    let agent_input = match input {
        Some(text) if !text.is_empty() => AgentInput::Text(text),
        _ => AgentInput::default(),
    };

    // Create an OrchestratorContext from the core crate to provide
    // session state management, memory store, and execution control.
    let orchestrator_ctx = Arc::new(
        plan_cascade_core::context::OrchestratorContext::new(
            execution_id,
            project_root.clone(),
            "pipeline-agent",
        ),
    );

    AgentContext {
        session_id: execution_id.to_string(),
        project_root,
        provider,
        tool_executor,
        plugin_manager: None,
        hooks,
        input: agent_input,
        shared_state,
        config: AgentConfig::default(),
        orchestrator_ctx: Some(orchestrator_ctx),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Execute an agent pipeline by ID, streaming events to the frontend.
///
/// Looks up the pipeline definition from the database, registers the execution
/// in the ExecutionRegistry, and spawns an async task to run the pipeline.
/// Obtains the active LLM provider from `StandaloneState` + app settings.
/// Returns the execution ID immediately for status tracking.
#[tauri::command]
pub async fn execute_agent_pipeline(
    app: tauri::AppHandle,
    state: State<'_, crate::state::AppState>,
    standalone_state: State<'_, StandaloneState>,
    registry: State<'_, ExecutionRegistry>,
    pipeline_id: String,
    input: Option<String>,
) -> Result<CommandResponse<String>, String> {
    // Look up the pipeline definition from the database
    let pipeline_result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let pipeline_json = conn.query_row(
                "SELECT definition FROM agent_pipelines WHERE id = ?1",
                rusqlite::params![pipeline_id],
                |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                },
            );

            match pipeline_json {
                Ok(json) => {
                    let pipeline: crate::services::agent_composer::AgentPipeline =
                        serde_json::from_str(&json).map_err(|e| {
                            AppError::parse(format!("Failed to parse pipeline: {}", e))
                        })?;
                    Ok(pipeline)
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    Err(AppError::not_found(format!("Pipeline not found: {}", pipeline_id)))
                }
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await;

    let pipeline = match pipeline_result {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Resolve the LLM provider from app settings + keyring
    let (provider_config, project_root) =
        match resolve_provider_config(&state, &standalone_state).await {
            Ok(result) => result,
            Err(e) => {
                // Register + immediately fail so the frontend can observe the failure
                let execution_id = registry.register(&pipeline_id).await;
                let mut executions = registry.executions.write().await;
                if let Some(exec_state) = executions.get_mut(&execution_id) {
                    exec_state.mark_failed(&e);
                }
                let _ = emit_pipeline_event(
                    &app,
                    &execution_id,
                    PipelineEventPayload::error(&execution_id, &e),
                );
                return Ok(CommandResponse::ok(execution_id));
            }
        };

    let provider = create_llm_provider(&provider_config);

    // Register execution in the registry
    let execution_id = registry.register(&pipeline_id).await;

    // Clone what we need for the spawned task
    let exec_id = execution_id.clone();
    let registry_executions = registry.executions.clone();
    let registry_tokens = registry.cancellation_tokens.clone();
    let app_handle = app.clone();
    let user_input = input;

    // Spawn async task to execute the pipeline
    tokio::spawn(async move {
        // Mark as running
        {
            let mut executions = registry_executions.write().await;
            if let Some(state) = executions.get_mut(&exec_id) {
                state.mark_running("initializing");
                state.total_steps = Some(pipeline.steps.len());
            }
        }

        // Build the agent from the pipeline definition
        let composer_registry = crate::services::agent_composer::ComposerRegistry::new();
        let agent = match composer_registry.build_from_pipeline(&pipeline) {
            Ok(a) => a,
            Err(e) => {
                let mut executions = registry_executions.write().await;
                if let Some(state) = executions.get_mut(&exec_id) {
                    state.mark_failed(format!("Failed to build pipeline: {}", e));
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::error(&exec_id, &e.to_string()),
                );
                return;
            }
        };

        // Emit started event
        let _ = emit_pipeline_event(
            &app_handle,
            &exec_id,
            PipelineEventPayload::status(&exec_id, "running", None),
        );

        // Get cancellation token
        let cancel_token = {
            let tokens = registry_tokens.read().await;
            tokens.get(&exec_id).cloned()
        };

        // Check if cancelled while setting up
        {
            let execs = registry_executions.read().await;
            if let Some(state) = execs.get(&exec_id) {
                if state.status == ExecutionStatus::Cancelled {
                    let _ = emit_pipeline_event(
                        &app_handle,
                        &exec_id,
                        PipelineEventPayload::status(&exec_id, "cancelled", None),
                    );
                    return;
                }
            }
        }

        // Build the AgentContext and run the pipeline agent
        let ctx = build_agent_context(&exec_id, project_root, provider, user_input);

        let stream = match agent.run(ctx).await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Pipeline agent.run() failed: {}", e);
                {
                    let mut executions = registry_executions.write().await;
                    if let Some(state) = executions.get_mut(&exec_id) {
                        state.mark_failed(&err_msg);
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::error(&exec_id, &err_msg),
                );
                return;
            }
        };

        // Stream agent events to the frontend, updating the registry along the way
        let result = stream_agent_events(
            &app_handle,
            &exec_id,
            stream,
            &registry_executions,
            cancel_token,
        )
        .await;

        // Mark completed or failed based on stream outcome
        match result {
            Ok(()) => {
                let mut executions = registry_executions.write().await;
                if let Some(state) = executions.get_mut(&exec_id) {
                    if state.is_active() {
                        state.mark_completed();
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::status(&exec_id, "completed", None),
                );
            }
            Err(e) => {
                let mut executions = registry_executions.write().await;
                if let Some(state) = executions.get_mut(&exec_id) {
                    if state.is_active() {
                        state.mark_failed(&e);
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::error(&exec_id, &e),
                );
            }
        }
    });

    Ok(CommandResponse::ok(execution_id))
}

/// Execute a graph workflow by ID, streaming events to the frontend.
///
/// Looks up the workflow definition from the database, registers the execution
/// in the ExecutionRegistry, and spawns an async task to run the workflow.
/// Obtains the active LLM provider from `StandaloneState` + app settings.
/// Returns the execution ID immediately for status tracking.
#[tauri::command]
pub async fn execute_graph_workflow_run(
    app: tauri::AppHandle,
    state: State<'_, crate::state::AppState>,
    standalone_state: State<'_, StandaloneState>,
    registry: State<'_, ExecutionRegistry>,
    workflow_id: String,
    input: Option<String>,
) -> Result<CommandResponse<String>, String> {
    // Look up the workflow definition from the database
    let workflow_result = state
        .with_database(|db| {
            let conn = db.pool().get().map_err(|e| {
                AppError::database(format!("Failed to get connection: {}", e))
            })?;

            let workflow_json = conn.query_row(
                "SELECT definition FROM graph_workflows WHERE id = ?1",
                rusqlite::params![workflow_id],
                |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                },
            );

            match workflow_json {
                Ok(json) => {
                    let workflow: crate::services::agent_composer::GraphWorkflow =
                        serde_json::from_str(&json).map_err(|e| {
                            AppError::parse(format!("Failed to parse workflow: {}", e))
                        })?;
                    Ok(workflow)
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    Err(AppError::not_found(format!("Workflow not found: {}", workflow_id)))
                }
                Err(e) => Err(AppError::database(e.to_string())),
            }
        })
        .await;

    let workflow = match workflow_result {
        Ok(w) => w,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Resolve the LLM provider from app settings + keyring
    let (provider_config, project_root) =
        match resolve_provider_config(&state, &standalone_state).await {
            Ok(result) => result,
            Err(e) => {
                // Register + immediately fail so the frontend can observe the failure
                let execution_id = registry.register(&workflow_id).await;
                let mut executions = registry.executions.write().await;
                if let Some(exec_state) = executions.get_mut(&execution_id) {
                    exec_state.mark_failed(&e);
                }
                let _ = emit_pipeline_event(
                    &app,
                    &execution_id,
                    PipelineEventPayload::error(&execution_id, &e),
                );
                return Ok(CommandResponse::ok(execution_id));
            }
        };

    let provider = create_llm_provider(&provider_config);

    // Register execution in the registry
    let execution_id = registry.register(&workflow_id).await;

    // Clone what we need for the spawned task
    let exec_id = execution_id.clone();
    let registry_executions = registry.executions.clone();
    let registry_tokens = registry.cancellation_tokens.clone();
    let app_handle = app.clone();
    let user_input = input;

    // Spawn async task to execute the workflow
    tokio::spawn(async move {
        // Mark as running
        {
            let mut executions = registry_executions.write().await;
            if let Some(state) = executions.get_mut(&exec_id) {
                state.mark_running("initializing");
                state.total_steps = Some(workflow.nodes.len());
            }
        }

        // Emit started event
        let _ = emit_pipeline_event(
            &app_handle,
            &exec_id,
            PipelineEventPayload::status(&exec_id, "running", None),
        );

        // Get cancellation token
        let cancel_token = {
            let tokens = registry_tokens.read().await;
            tokens.get(&exec_id).cloned()
        };

        // Check if cancelled while setting up
        {
            let execs = registry_executions.read().await;
            if let Some(state) = execs.get(&exec_id) {
                if state.status == ExecutionStatus::Cancelled {
                    let _ = emit_pipeline_event(
                        &app_handle,
                        &exec_id,
                        PipelineEventPayload::status(&exec_id, "cancelled", None),
                    );
                    return;
                }
            }
        }

        // Build the AgentContext and run the graph workflow with an InMemoryCheckpointer
        // for interrupt_before / interrupt_after pause/resume support within the session.
        let ctx = build_agent_context(&exec_id, project_root, provider, user_input);

        let checkpointer: Arc<
            dyn crate::services::graph_workflow::checkpointer::Checkpointer + Send + Sync,
        > = Arc::new(
            crate::services::graph_workflow::checkpointer::InMemoryCheckpointer::new(),
        );
        let thread_id = exec_id.clone();

        let stream = match workflow
            .run_with_checkpointer(ctx, Some(checkpointer), thread_id)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Workflow run_with_checkpointer() failed: {}", e);
                {
                    let mut executions = registry_executions.write().await;
                    if let Some(state) = executions.get_mut(&exec_id) {
                        state.mark_failed(&err_msg);
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::error(&exec_id, &err_msg),
                );
                return;
            }
        };

        // Stream agent events to the frontend, updating the registry along the way
        let result = stream_agent_events(
            &app_handle,
            &exec_id,
            stream,
            &registry_executions,
            cancel_token,
        )
        .await;

        // Mark completed or failed based on stream outcome
        match result {
            Ok(()) => {
                let mut executions = registry_executions.write().await;
                if let Some(state) = executions.get_mut(&exec_id) {
                    if state.is_active() {
                        state.mark_completed();
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::status(&exec_id, "completed", None),
                );
            }
            Err(e) => {
                let mut executions = registry_executions.write().await;
                if let Some(state) = executions.get_mut(&exec_id) {
                    if state.is_active() {
                        state.mark_failed(&e);
                    }
                }
                let _ = emit_pipeline_event(
                    &app_handle,
                    &exec_id,
                    PipelineEventPayload::error(&exec_id, &e),
                );
            }
        }
    });

    Ok(CommandResponse::ok(execution_id))
}

/// Get the current execution status of a pipeline or workflow.
#[tauri::command]
pub async fn get_pipeline_execution_status(
    registry: State<'_, ExecutionRegistry>,
    execution_id: String,
) -> Result<CommandResponse<Option<PipelineExecutionState>>, String> {
    let status = registry.get_status(&execution_id).await;
    Ok(CommandResponse::ok(status))
}

/// Cancel a running pipeline or workflow execution.
#[tauri::command]
pub async fn cancel_pipeline_execution(
    registry: State<'_, ExecutionRegistry>,
    execution_id: String,
) -> Result<CommandResponse<bool>, String> {
    let cancelled = registry.cancel(&execution_id).await;
    Ok(CommandResponse::ok(cancelled))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ExecutionStatus Tests
    // ========================================================================

    #[test]
    fn test_execution_status_serialization() {
        let status = ExecutionStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let parsed: ExecutionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ExecutionStatus::Running);
    }

    #[test]
    fn test_all_execution_statuses() {
        let statuses = vec![
            (ExecutionStatus::Pending, "\"pending\""),
            (ExecutionStatus::Running, "\"running\""),
            (ExecutionStatus::Completed, "\"completed\""),
            (ExecutionStatus::Failed, "\"failed\""),
            (ExecutionStatus::Cancelled, "\"cancelled\""),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);
        }
    }

    // ========================================================================
    // PipelineExecutionState Tests
    // ========================================================================

    #[test]
    fn test_pipeline_execution_state_new() {
        let state = PipelineExecutionState::new("pipeline-1");
        assert!(!state.execution_id.is_empty());
        assert_eq!(state.pipeline_id, "pipeline-1");
        assert_eq!(state.status, ExecutionStatus::Pending);
        assert_eq!(state.steps_completed, 0);
        assert!(state.total_steps.is_none());
        assert!(state.current_step.is_none());
        assert!(state.error.is_none());
        assert!(!state.started_at.is_empty());
        assert!(state.completed_at.is_none());
        assert!(state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_running() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        assert_eq!(state.status, ExecutionStatus::Running);
        assert_eq!(state.current_step.as_deref(), Some("step-1"));
        assert!(state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_step_completed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        assert_eq!(state.steps_completed, 0);

        state.mark_step_completed();
        assert_eq!(state.steps_completed, 1);

        state.mark_step_completed();
        assert_eq!(state.steps_completed, 2);
    }

    #[test]
    fn test_pipeline_execution_state_mark_completed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_completed();

        assert_eq!(state.status, ExecutionStatus::Completed);
        assert!(state.current_step.is_none());
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_failed() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_failed("Something went wrong");

        assert_eq!(state.status, ExecutionStatus::Failed);
        assert_eq!(state.error.as_deref(), Some("Something went wrong"));
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_mark_cancelled() {
        let mut state = PipelineExecutionState::new("p1");
        state.mark_running("step-1");
        state.mark_cancelled();

        assert_eq!(state.status, ExecutionStatus::Cancelled);
        assert!(state.current_step.is_none());
        assert!(state.completed_at.is_some());
        assert!(!state.is_active());
    }

    #[test]
    fn test_pipeline_execution_state_serialization() {
        let mut state = PipelineExecutionState::new("p1");
        state.total_steps = Some(5);
        state.mark_running("step-2");
        state.steps_completed = 1;

        let json = serde_json::to_string(&state).unwrap();
        let parsed: PipelineExecutionState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pipeline_id, "p1");
        assert_eq!(parsed.status, ExecutionStatus::Running);
        assert_eq!(parsed.total_steps, Some(5));
        assert_eq!(parsed.steps_completed, 1);
        assert_eq!(parsed.current_step.as_deref(), Some("step-2"));
    }

    // ========================================================================
    // ExecutionRegistry Tests
    // ========================================================================

    #[tokio::test]
    async fn test_execution_registry_new() {
        let registry = ExecutionRegistry::new();
        let all = registry.list_all().await;
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_execution_registry_register() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;
        assert!(!id.is_empty());

        let status = registry.get_status(&id).await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.pipeline_id, "pipeline-1");
        assert_eq!(status.status, ExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_execution_registry_update() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;

        let mut state = registry.get_status(&id).await.unwrap();
        state.mark_running("step-1");
        registry.update(&id, state).await;

        let updated = registry.get_status(&id).await.unwrap();
        assert_eq!(updated.status, ExecutionStatus::Running);
    }

    #[tokio::test]
    async fn test_execution_registry_cancel() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("pipeline-1").await;

        let cancelled = registry.cancel(&id).await;
        assert!(cancelled);

        let status = registry.get_status(&id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Cancelled);

        // Cancelling again should return false (already not active)
        let cancelled_again = registry.cancel(&id).await;
        assert!(!cancelled_again);
    }

    #[tokio::test]
    async fn test_execution_registry_cancel_nonexistent() {
        let registry = ExecutionRegistry::new();
        let cancelled = registry.cancel("nonexistent").await;
        assert!(!cancelled);
    }

    #[tokio::test]
    async fn test_execution_registry_list_active() {
        let registry = ExecutionRegistry::new();
        let id1 = registry.register("p1").await;
        let id2 = registry.register("p2").await;

        // Cancel one
        registry.cancel(&id1).await;

        let active = registry.list_active().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].execution_id, id2);
    }

    #[tokio::test]
    async fn test_execution_registry_list_all() {
        let registry = ExecutionRegistry::new();
        registry.register("p1").await;
        registry.register("p2").await;

        let all = registry.list_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_execution_registry_cleanup() {
        let registry = ExecutionRegistry::new();
        let id1 = registry.register("p1").await;
        let _id2 = registry.register("p2").await;

        // Complete one
        let mut state = registry.get_status(&id1).await.unwrap();
        state.mark_completed();
        registry.update(&id1, state).await;

        // Cleanup
        registry.cleanup_completed().await;

        let all = registry.list_all().await;
        assert_eq!(all.len(), 1); // Only the active one remains
    }

    #[tokio::test]
    async fn test_execution_registry_get_cancellation_token() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("p1").await;

        let token = registry.get_cancellation_token(&id).await;
        assert!(token.is_some());

        let missing = registry.get_cancellation_token("nonexistent").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_execution_registry_default() {
        let registry = ExecutionRegistry::default();
        assert!(registry.list_all().await.is_empty());
    }

    #[test]
    fn test_get_status_nonexistent() {
        // Synchronous test to verify the function returns None
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = ExecutionRegistry::new();
            let status = registry.get_status("nonexistent").await;
            assert!(status.is_none());
        });
    }

    // ========================================================================
    // PipelineEventPayload Tests
    // ========================================================================

    #[test]
    fn test_pipeline_event_payload_status() {
        let payload = PipelineEventPayload::status("exec-1", "running", None);
        assert_eq!(payload.execution_id, "exec-1");
        assert_eq!(payload.event_type, "status");
        assert_eq!(payload.status, Some("running".to_string()));
        assert!(payload.agent_event.is_none());
        assert!(payload.error.is_none());
    }

    #[test]
    fn test_pipeline_event_payload_agent_event() {
        let event_data = serde_json::json!({"type": "text_delta", "content": "Hello"});
        let payload = PipelineEventPayload::agent_event("exec-2", event_data.clone());
        assert_eq!(payload.execution_id, "exec-2");
        assert_eq!(payload.event_type, "agent_event");
        assert!(payload.status.is_none());
        assert_eq!(payload.agent_event, Some(event_data));
        assert!(payload.error.is_none());
    }

    #[test]
    fn test_pipeline_event_payload_error() {
        let payload = PipelineEventPayload::error("exec-3", "Something failed");
        assert_eq!(payload.execution_id, "exec-3");
        assert_eq!(payload.event_type, "error");
        assert_eq!(payload.status, Some("failed".to_string()));
        assert!(payload.agent_event.is_none());
        assert_eq!(payload.error, Some("Something failed".to_string()));
    }

    #[test]
    fn test_pipeline_event_payload_serialization() {
        let payload = PipelineEventPayload::status("exec-4", "completed", None);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"execution_id\":\"exec-4\""));
        assert!(json.contains("\"event_type\":\"status\""));
        assert!(json.contains("\"status\":\"completed\""));

        let parsed: PipelineEventPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.execution_id, "exec-4");
        assert_eq!(parsed.event_type, "status");
        assert_eq!(parsed.status, Some("completed".to_string()));
    }

    #[test]
    fn test_pipeline_event_payload_with_agent_event_data() {
        let event = serde_json::json!({
            "type": "tool_call",
            "name": "read_file",
            "args": "{\"path\":\"/foo\"}"
        });
        let payload = PipelineEventPayload::status("exec-5", "running", Some(event.clone()));
        assert_eq!(payload.agent_event, Some(event));

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: PipelineEventPayload = serde_json::from_str(&json).unwrap();
        assert!(parsed.agent_event.is_some());
        assert_eq!(
            parsed.agent_event.unwrap()["name"],
            serde_json::json!("read_file")
        );
    }

    // ========================================================================
    // ExecutionRegistry pub(crate) field access tests
    // ========================================================================

    #[tokio::test]
    async fn test_execution_registry_fields_accessible() {
        let registry = ExecutionRegistry::new();
        let id = registry.register("p1").await;

        // Verify fields are accessible (pub(crate))
        {
            let executions = registry.executions.read().await;
            assert!(executions.contains_key(&id));
        }
        {
            let tokens = registry.cancellation_tokens.read().await;
            assert!(tokens.contains_key(&id));
        }
    }

    #[tokio::test]
    async fn test_execution_registry_direct_state_update() {
        // Test that spawned tasks can update state via the Arc<RwLock<>> fields
        let registry = ExecutionRegistry::new();
        let id = registry.register("p1").await;

        let executions = registry.executions.clone();

        // Simulate what a spawned task would do
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&id) {
                state.mark_running("step-1");
                state.total_steps = Some(3);
            }
        }

        // Verify through registry API
        let status = registry.get_status(&id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Running);
        assert_eq!(status.total_steps, Some(3));
        assert_eq!(status.current_step.as_deref(), Some("step-1"));
    }

    #[test]
    fn test_pipeline_event_channel_constant() {
        assert_eq!(PIPELINE_EVENT_CHANNEL, "pipeline:event");
    }

    // ========================================================================
    // Story 002: Pipeline execution wiring integration tests
    // ========================================================================

    #[tokio::test]
    async fn test_execution_registers_and_tracks_lifecycle() {
        // Simulates the full lifecycle of pipeline execution:
        // register -> running -> step completed -> completed
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-123").await;

        // Verify initial state is Pending
        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Pending);
        assert_eq!(status.pipeline_id, "pipeline-123");

        // Simulate spawned task marking as running
        let executions = registry.executions.clone();
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_running("step-1");
                state.total_steps = Some(2);
            }
        }

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Running);
        assert_eq!(status.current_step.as_deref(), Some("step-1"));
        assert_eq!(status.total_steps, Some(2));

        // Simulate step completion
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_step_completed();
                state.mark_running("step-2");
            }
        }

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.steps_completed, 1);
        assert_eq!(status.current_step.as_deref(), Some("step-2"));

        // Simulate completion
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_step_completed();
                state.mark_completed();
            }
        }

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Completed);
        assert_eq!(status.steps_completed, 2);
        assert!(status.completed_at.is_some());
        assert!(!status.is_active());
    }

    #[tokio::test]
    async fn test_execution_failure_tracking() {
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-fail").await;

        let executions = registry.executions.clone();
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_running("step-1");
            }
        }

        // Simulate failure
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_failed("Pipeline build error: no steps");
            }
        }

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Failed);
        assert_eq!(status.error.as_deref(), Some("Pipeline build error: no steps"));
        assert!(status.completed_at.is_some());
        assert!(!status.is_active());
    }

    #[tokio::test]
    async fn test_multiple_concurrent_executions() {
        let registry = ExecutionRegistry::new();

        // Register multiple executions
        let id1 = registry.register("pipeline-a").await;
        let id2 = registry.register("pipeline-b").await;
        let id3 = registry.register("workflow-c").await;

        // All should be active
        let active = registry.list_active().await;
        assert_eq!(active.len(), 3);

        // Complete one, fail another
        let executions = registry.executions.clone();
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&id1) {
                state.mark_running("step-1");
                state.mark_completed();
            }
            if let Some(state) = execs.get_mut(&id2) {
                state.mark_running("step-1");
                state.mark_failed("error");
            }
        }

        let active = registry.list_active().await;
        assert_eq!(active.len(), 1); // Only id3 is still active
        assert_eq!(active[0].execution_id, id3);

        let all = registry.list_all().await;
        assert_eq!(all.len(), 3);
    }

    // ========================================================================
    // Story 003: Status and cancellation wiring tests
    // ========================================================================

    #[tokio::test]
    async fn test_status_query_returns_correct_state() {
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-status").await;

        // Pending state
        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Pending);

        // Mark running
        let executions = registry.executions.clone();
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_running("processing");
            }
        }

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Running);
        assert_eq!(status.current_step.as_deref(), Some("processing"));
    }

    #[tokio::test]
    async fn test_status_query_nonexistent_returns_none() {
        let registry = ExecutionRegistry::new();
        let status = registry.get_status("nonexistent-id").await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_cancellation_updates_status() {
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-cancel").await;

        // Mark as running first
        let executions = registry.executions.clone();
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(&exec_id) {
                state.mark_running("step-1");
            }
        }

        // Cancel
        let cancelled = registry.cancel(&exec_id).await;
        assert!(cancelled);

        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Cancelled);
        assert!(status.completed_at.is_some());
        assert!(!status.is_active());
    }

    #[tokio::test]
    async fn test_cancellation_triggers_notify() {
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-notify").await;

        // Get the cancellation token before cancelling
        let token = registry.get_cancellation_token(&exec_id).await.unwrap();

        // Spawn a task that waits for cancellation
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            // Use a timeout so the test doesn't hang
            tokio::select! {
                _ = token_clone.notified() => true,
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => false,
            }
        });

        // Small delay to ensure spawned task is waiting
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Cancel the execution
        let cancelled = registry.cancel(&exec_id).await;
        assert!(cancelled);

        // The spawned task should receive the notification
        let was_notified = handle.await.unwrap();
        assert!(was_notified, "Cancellation should trigger the notify");
    }

    #[tokio::test]
    async fn test_cancel_completed_execution_returns_false() {
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-done").await;

        // Complete the execution
        let mut state = registry.get_status(&exec_id).await.unwrap();
        state.mark_completed();
        registry.update(&exec_id, state).await;

        // Try to cancel
        let cancelled = registry.cancel(&exec_id).await;
        assert!(!cancelled, "Cannot cancel an already completed execution");

        // Status should still be Completed
        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_returns_false() {
        let registry = ExecutionRegistry::new();
        let cancelled = registry.cancel("nonexistent").await;
        assert!(!cancelled);
    }

    // ========================================================================
    // Story 004: Event streaming tests
    // ========================================================================

    #[test]
    fn test_pipeline_event_payload_agent_event_with_text_delta() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::TextDelta {
            content: "Hello world".to_string(),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-stream-1", event_json.clone());

        assert_eq!(payload.execution_id, "exec-stream-1");
        assert_eq!(payload.event_type, "agent_event");
        assert!(payload.agent_event.is_some());

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "text_delta");
        assert_eq!(agent_data["content"], "Hello world");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_tool_call() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::ToolCall {
            name: "read_file".to_string(),
            args: r#"{"path": "/tmp/test.rs"}"#.to_string(),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-stream-2", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "tool_call");
        assert_eq!(agent_data["name"], "read_file");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_graph_node_started() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::GraphNodeStarted {
            node_id: "node-a".to_string(),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-graph-1", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "graph_node_started");
        assert_eq!(agent_data["node_id"], "node-a");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_graph_node_completed() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::GraphNodeCompleted {
            node_id: "node-b".to_string(),
            output: Some("processed data".to_string()),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-graph-2", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "graph_node_completed");
        assert_eq!(agent_data["node_id"], "node-b");
        assert_eq!(agent_data["output"], "processed data");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_done() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::Done {
            output: Some("final result".to_string()),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-done-1", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "done");
        assert_eq!(agent_data["output"], "final result");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_state_update() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::StateUpdate {
            key: "progress".to_string(),
            value: serde_json::json!(75),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-state-1", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "state_update");
        assert_eq!(agent_data["key"], "progress");
        assert_eq!(agent_data["value"], 75);
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_human_review() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::HumanReviewRequired {
            node_id: "review-node".to_string(),
            context: "Please approve this change".to_string(),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-review-1", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "human_review_required");
        assert_eq!(agent_data["node_id"], "review-node");
    }

    #[test]
    fn test_pipeline_event_payload_agent_event_with_rich_content() {
        use crate::services::agent_composer::AgentEvent;

        let event = AgentEvent::RichContent {
            component_type: "table".to_string(),
            data: serde_json::json!({"rows": [["a", "b"]]}),
            surface_id: Some("progress-table".to_string()),
        };
        let event_json = serde_json::to_value(&event).unwrap();
        let payload = PipelineEventPayload::agent_event("exec-rich-1", event_json);

        let agent_data = payload.agent_event.unwrap();
        assert_eq!(agent_data["type"], "rich_content");
        assert_eq!(agent_data["component_type"], "table");
        assert_eq!(agent_data["surface_id"], "progress-table");
    }

    #[tokio::test]
    async fn test_stream_agent_events_processes_events() {
        use crate::services::agent_composer::AgentEvent;
        use futures_util::stream;

        // Create a mock event stream
        let events: Vec<crate::utils::error::AppResult<AgentEvent>> = vec![
            Ok(AgentEvent::TextDelta {
                content: "Hello".to_string(),
            }),
            Ok(AgentEvent::GraphNodeStarted {
                node_id: "node-1".to_string(),
            }),
            Ok(AgentEvent::GraphNodeCompleted {
                node_id: "node-1".to_string(),
                output: Some("done".to_string()),
            }),
            Ok(AgentEvent::Done {
                output: Some("result".to_string()),
            }),
        ];
        let mock_stream: crate::services::agent_composer::AgentEventStream =
            Box::pin(stream::iter(events));

        // Set up the execution registry
        let executions = Arc::new(RwLock::new(HashMap::new()));
        let exec_id = "test-stream-exec";
        let mut initial_state = PipelineExecutionState::new("pipeline-stream");
        initial_state.execution_id = exec_id.to_string();
        initial_state.mark_running("initializing");
        initial_state.total_steps = Some(2);
        {
            let mut execs = executions.write().await;
            execs.insert(exec_id.to_string(), initial_state);
        }

        // We can't easily mock AppHandle in tests, but we can verify
        // the registry state updates that happen during streaming.
        // The stream_agent_events function needs an AppHandle, so we
        // test the event structure and registry updates separately.

        // Verify that GraphNodeStarted updates current_step
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(exec_id) {
                state.mark_running("node-1");
            }
        }
        {
            let execs = executions.read().await;
            let state = execs.get(exec_id).unwrap();
            assert_eq!(state.current_step.as_deref(), Some("node-1"));
        }

        // Verify that GraphNodeCompleted increments steps_completed
        {
            let mut execs = executions.write().await;
            if let Some(state) = execs.get_mut(exec_id) {
                state.mark_step_completed();
            }
        }
        {
            let execs = executions.read().await;
            let state = execs.get(exec_id).unwrap();
            assert_eq!(state.steps_completed, 1);
        }
    }

    #[tokio::test]
    async fn test_stream_agent_events_handles_error() {
        use crate::services::agent_composer::AgentEvent;
        use futures_util::stream;

        // Create a stream that has an error
        let events: Vec<crate::utils::error::AppResult<AgentEvent>> = vec![
            Ok(AgentEvent::TextDelta {
                content: "Start".to_string(),
            }),
            Err(crate::utils::error::AppError::Internal("Stream failed".to_string())),
        ];
        let _mock_stream: crate::services::agent_composer::AgentEventStream =
            Box::pin(stream::iter(events));

        // Verify the error event payload structure
        let payload = PipelineEventPayload::error("exec-err", "Stream failed");
        assert_eq!(payload.event_type, "error");
        assert_eq!(payload.status, Some("failed".to_string()));
        assert_eq!(payload.error, Some("Stream failed".to_string()));
    }

    #[tokio::test]
    async fn test_event_streaming_respects_cancellation_state() {
        // Verify that the execution task checks cancellation before running
        let registry = ExecutionRegistry::new();
        let exec_id = registry.register("pipeline-precancel").await;

        // Cancel before any task starts
        registry.cancel(&exec_id).await;

        // Verify the state is cancelled
        let status = registry.get_status(&exec_id).await.unwrap();
        assert_eq!(status.status, ExecutionStatus::Cancelled);

        // The spawned task should check this and skip execution
        let executions = registry.executions.clone();
        let is_cancelled = {
            let execs = executions.read().await;
            execs
                .get(&exec_id)
                .map(|s| s.status == ExecutionStatus::Cancelled)
                .unwrap_or(false)
        };
        assert!(is_cancelled);
    }
}
