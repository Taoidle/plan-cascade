//! Task Mode Tauri Commands
//!
//! Provides the complete Task Mode lifecycle as Tauri commands:
//! - enter/exit task mode
//! - generate/approve task PRD
//! - execution status/cancel/report

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::models::CommandResponse;
use crate::services::strategy::analyzer::{analyze_task_for_mode, StrategyAnalysis};
use crate::services::task_mode::batch_executor::{
    BatchExecutionProgress, BatchExecutionResult, BatchExecutor, ExecutableStory, ExecutionBatch,
    ExecutionConfig, StoryContext, StoryExecutionContext, StoryExecutionOutcome,
    StoryExecutionState, TaskModeProgressEvent, TASK_MODE_EVENT_CHANNEL,
};
use crate::services::task_mode::agent_resolver::AgentResolver;
use crate::services::task_mode::prd_generator;

use crate::state::AppState;
use crate::storage::KeyringService;

// ============================================================================
// Types
// ============================================================================

/// Execution mode for story execution in Task Mode.
///
/// Determines whether stories are executed via external CLI tools
/// or directly via the built-in LLM API through OrchestratorService.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryExecutionMode {
    /// Use external CLI tools (claude, codex, aider)
    Cli,
    /// Use direct LLM API via OrchestratorService
    Llm,
}

/// Task mode session status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskModeStatus {
    /// Session initialized, ready for PRD generation
    Initialized,
    /// PRD is being generated
    GeneratingPrd,
    /// PRD generated, awaiting review/approval
    ReviewingPrd,
    /// Stories are being executed
    Executing,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
}

/// A story in the task PRD.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStory {
    /// Story ID
    pub id: String,
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Priority (high/medium/low)
    pub priority: String,
    /// Dependencies (story IDs)
    pub dependencies: Vec<String>,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
}

/// Task PRD (Product Requirements Document).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPrd {
    /// PRD title
    pub title: String,
    /// Overall description
    pub description: String,
    /// Stories
    pub stories: Vec<TaskStory>,
    /// Execution batches (calculated from dependencies)
    pub batches: Vec<ExecutionBatch>,
}

/// Task mode session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskModeSession {
    /// Unique session ID
    pub session_id: String,
    /// Task description
    pub description: String,
    /// Current status
    pub status: TaskModeStatus,
    /// Strategy analysis result
    pub strategy_analysis: Option<StrategyAnalysis>,
    /// Generated PRD
    pub prd: Option<TaskPrd>,
    /// Execution progress
    pub progress: Option<BatchExecutionProgress>,
    /// When the session was created
    pub created_at: String,
}

/// A quality dimension score for a single story (e.g., correctness, readability).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityDimensionScore {
    /// Story ID this score belongs to
    pub story_id: String,
    /// Quality dimension name (e.g., "correctness", "readability")
    pub dimension: String,
    /// Achieved score
    pub score: f64,
    /// Maximum possible score
    pub max_score: f64,
}

/// A timeline entry representing one story's execution span.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    /// Story ID
    pub story_id: String,
    /// Story title
    pub story_title: String,
    /// Batch index (0-based) this story belonged to
    pub batch_index: usize,
    /// Agent that executed the story
    pub agent: String,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Start offset from overall execution start in milliseconds
    pub start_offset_ms: u64,
    /// Final status ("completed", "failed", "cancelled")
    pub status: String,
    /// Summary of quality gate result (if gates were run)
    pub gate_result: Option<String>,
}

/// Aggregated performance metrics for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPerformanceEntry {
    /// Agent name
    pub agent_name: String,
    /// Number of stories assigned to this agent
    pub stories_assigned: usize,
    /// Number of stories completed successfully by this agent
    pub stories_completed: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Average duration in milliseconds for completed stories
    pub average_duration_ms: u64,
    /// Average quality score across all quality dimensions (if available)
    pub average_quality_score: Option<f64>,
}

/// Execution report after task mode completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    /// Session ID
    pub session_id: String,
    /// Total stories
    pub total_stories: usize,
    /// Stories completed
    pub stories_completed: usize,
    /// Stories failed
    pub stories_failed: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Agent assignments per story
    pub agent_assignments: HashMap<String, String>,
    /// Overall success
    pub success: bool,
    /// Per-story quality dimension scores (empty when no gate results available)
    pub quality_scores: Vec<QualityDimensionScore>,
    /// Timeline entries for waterfall visualization
    pub timeline: Vec<TimelineEntry>,
    /// Aggregated per-agent performance metrics
    pub agent_performance: Vec<AgentPerformanceEntry>,
}

/// Workflow configuration from the frontend orchestrator.
///
/// Passed through from the Simple Mode workflow to control execution behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWorkflowConfig {
    /// Flow level: quick/standard/full
    pub flow_level: Option<String>,
    /// TDD mode: off/flexible/strict
    pub tdd_mode: Option<String>,
    /// Whether to enable spec interview
    pub enable_interview: bool,
    /// Maximum parallel stories
    pub max_parallel: Option<usize>,
    /// Skip verification gates (--no-verify)
    pub skip_verification: bool,
    /// Skip code review gate (--no-review)
    pub skip_review: bool,
    /// Override all agents with this agent name
    pub global_agent_override: Option<String>,
    /// Override implementation agents only
    pub impl_agent_override: Option<String>,
}

/// Current task execution status for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionStatus {
    /// Session ID
    pub session_id: String,
    /// Current status
    pub status: TaskModeStatus,
    /// Current batch index
    pub current_batch: usize,
    /// Total batches
    pub total_batches: usize,
    /// Per-story status
    pub story_statuses: HashMap<String, String>,
    /// Stories completed
    pub stories_completed: usize,
    /// Stories failed
    pub stories_failed: usize,
}

// ============================================================================
// State
// ============================================================================

/// Managed Tauri state for task mode.
pub struct TaskModeState {
    session: Arc<RwLock<Option<TaskModeSession>>>,
    /// Cancellation token for the currently executing batch.
    cancellation_token: Arc<RwLock<Option<CancellationToken>>>,
    /// Final execution result (populated when execution completes).
    execution_result: Arc<RwLock<Option<BatchExecutionResult>>>,
}

impl TaskModeState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
            cancellation_token: Arc::new(RwLock::new(None)),
            execution_result: Arc::new(RwLock::new(None)),
        }
    }
}

// ============================================================================
// Commands
// ============================================================================

/// Enter task mode by creating a new session.
///
/// Initializes a TaskModeSession and stores it in managed state.
/// Also runs strategy analysis to provide mode recommendation.
#[tauri::command]
pub async fn enter_task_mode(
    description: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskModeSession>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Check if already in task mode
    {
        let session = state.session.read().await;
        if let Some(ref s) = *session {
            if !matches!(
                s.status,
                TaskModeStatus::Completed | TaskModeStatus::Failed | TaskModeStatus::Cancelled
            ) {
                return Ok(CommandResponse::err(format!(
                    "Already in task mode (session: {}). Exit first.",
                    s.session_id
                )));
            }
        }
    }

    // Run strategy analysis
    let analysis = analyze_task_for_mode(&description, None);

    let session = TaskModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        status: TaskModeStatus::Initialized,
        strategy_analysis: Some(analysis),
        prd: None,
        progress: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    // Store session
    {
        let mut s = state.session.write().await;
        *s = Some(session.clone());
    }

    Ok(CommandResponse::ok(session))
}

/// Generate a task PRD from the session description using an LLM provider.
///
/// Calls the configured LLM provider to decompose the task description into
/// stories with dependencies, priorities, and acceptance criteria.
/// Implements retry-with-repair per ADR-F002 for JSON parse failures.
///
/// # Parameters
/// - `session_id`: The active task mode session ID
/// - `provider`: LLM provider name (e.g., "anthropic", "openai", "ollama")
/// - `model`: Model identifier (e.g., "claude-3-5-sonnet-20241022")
/// - `api_key` / `apiKey`: Optional API key (falls back to OS keyring)
/// - `base_url` / `baseUrl`: Optional base URL override
#[tauri::command]
#[allow(non_snake_case)]
pub async fn generate_task_prd(
    session_id: String,
    provider: String,
    model: String,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    compiled_spec: Option<serde_json::Value>,
    state: tauri::State<'_, TaskModeState>,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<TaskPrd>, String> {
    // Validate and extract session
    let (description, status) = {
        let session_guard = state.session.read().await;
        match session_guard.as_ref() {
            Some(s) if s.session_id == session_id => {
                (s.description.clone(), s.status.clone())
            }
            _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
        }
    };

    if status != TaskModeStatus::Initialized {
        return Ok(CommandResponse::err(format!(
            "Cannot generate PRD in {:?} status",
            status
        )));
    }

    // Update status to GeneratingPrd
    {
        let mut session_guard = state.session.write().await;
        if let Some(s) = session_guard.as_mut() {
            s.status = TaskModeStatus::GeneratingPrd;
        }
    }

    // If compiled_spec is provided (from interview pipeline), convert directly
    if let Some(spec_value) = compiled_spec {
        match prd_generator::convert_compiled_prd_to_task_prd(spec_value) {
            Ok(prd) => {
                let mut session_guard = state.session.write().await;
                if let Some(s) = session_guard.as_mut() {
                    s.status = TaskModeStatus::ReviewingPrd;
                    s.prd = Some(prd.clone());
                }
                return Ok(CommandResponse::ok(prd));
            }
            Err(e) => {
                eprintln!("[generate_task_prd] compiled_spec conversion failed, falling back to LLM: {}", e);
                // Fall through to LLM generation
            }
        }
    }

    // Resolve provider configuration
    let llm_provider = match resolve_llm_provider(
        &provider,
        &model,
        api_key.or(apiKey),
        base_url.or(baseUrl),
        &app_state,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            // Reset status back to Initialized on failure
            let mut session_guard = state.session.write().await;
            if let Some(s) = session_guard.as_mut() {
                s.status = TaskModeStatus::Initialized;
            }
            return Ok(CommandResponse::err(e));
        }
    };

    // Call LLM for PRD generation
    let prd = match prd_generator::generate_prd_with_llm(llm_provider, &description).await {
        Ok(prd) => prd,
        Err(e) => {
            // Reset status back to Initialized on failure
            let mut session_guard = state.session.write().await;
            if let Some(s) = session_guard.as_mut() {
                s.status = TaskModeStatus::Initialized;
            }
            return Ok(CommandResponse::err(format!(
                "PRD generation failed: {}",
                e
            )));
        }
    };

    // Update session with generated PRD
    {
        let mut session_guard = state.session.write().await;
        if let Some(s) = session_guard.as_mut() {
            s.status = TaskModeStatus::ReviewingPrd;
            s.prd = Some(prd.clone());
        }
    }

    Ok(CommandResponse::ok(prd))
}

/// Resolve an LLM provider from frontend parameters and OS keyring.
///
/// Looks up the API key from the keyring if not provided explicitly.
/// Returns an Arc<dyn LlmProvider> ready for use.
async fn resolve_llm_provider(
    provider_name: &str,
    model: &str,
    explicit_api_key: Option<String>,
    explicit_base_url: Option<String>,
    app_state: &tauri::State<'_, AppState>,
) -> Result<Arc<dyn crate::services::llm::provider::LlmProvider>, String> {
    use crate::commands::standalone::normalize_provider_name;
    use crate::services::llm::types::{ProviderConfig, ProviderType};

    let canonical = normalize_provider_name(provider_name)
        .ok_or_else(|| format!("Unknown provider: {}", provider_name))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    // Resolve API key: explicit parameter > OS keyring
    let api_key = explicit_api_key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .or_else(|| {
            let keyring = KeyringService::new();
            keyring.get_api_key(canonical).ok().flatten()
        });

    // Validate API key for non-Ollama providers
    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err(format!(
            "API key not configured for provider '{}'. \
             Please configure it in Settings or pass it explicitly.",
            canonical
        ));
    }

    // Resolve base URL: explicit > DB settings > default
    let mut resolved_base_url = explicit_base_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());

    if resolved_base_url.is_none() {
        let key = format!("provider_{}_base_url", canonical);
        if let Ok(Some(db_url)) = app_state
            .with_database(|db| db.get_setting(&key))
            .await
        {
            if !db_url.is_empty() {
                resolved_base_url = Some(db_url);
            }
        }
    }

    // Resolve proxy
    let proxy = {
        let keyring = KeyringService::new();
        app_state
            .with_database(|db| {
                Ok(crate::commands::proxy::resolve_provider_proxy(
                    &keyring, db, canonical,
                ))
            })
            .await
            .unwrap_or(None)
    };

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: resolved_base_url,
        model: model.to_string(),
        proxy,
        ..Default::default()
    };

    Ok(prd_generator::create_provider(config))
}

/// Resolve a ProviderConfig from frontend parameters and OS keyring.
///
/// Same as `resolve_llm_provider` but returns the raw config instead of
/// an instantiated provider. Used by `execute_story_via_llm()` to create
/// OrchestratorService instances.
async fn resolve_provider_config(
    provider_name: &str,
    model: &str,
    app_state: &tauri::State<'_, AppState>,
) -> Result<crate::services::llm::types::ProviderConfig, String> {
    use crate::commands::standalone::normalize_provider_name;
    use crate::services::llm::types::{ProviderConfig, ProviderType};

    let canonical = normalize_provider_name(provider_name)
        .ok_or_else(|| format!("Unknown provider: {}", provider_name))?;

    let provider_type = match canonical {
        "anthropic" => ProviderType::Anthropic,
        "openai" => ProviderType::OpenAI,
        "deepseek" => ProviderType::DeepSeek,
        "glm" => ProviderType::Glm,
        "qwen" => ProviderType::Qwen,
        "minimax" => ProviderType::Minimax,
        "ollama" => ProviderType::Ollama,
        _ => return Err(format!("Unsupported provider: {}", canonical)),
    };

    let api_key = {
        let keyring = KeyringService::new();
        keyring.get_api_key(canonical).ok().flatten()
    };

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Err(format!(
            "API key not configured for provider '{}'.",
            canonical
        ));
    }

    let mut resolved_base_url: Option<String> = None;
    {
        let key = format!("provider_{}_base_url", canonical);
        if let Ok(Some(db_url)) = app_state
            .with_database(|db| db.get_setting(&key))
            .await
        {
            if !db_url.is_empty() {
                resolved_base_url = Some(db_url);
            }
        }
    }

    let proxy = {
        let keyring = KeyringService::new();
        app_state
            .with_database(|db| {
                Ok(crate::commands::proxy::resolve_provider_proxy(
                    &keyring, db, canonical,
                ))
            })
            .await
            .unwrap_or(None)
    };

    Ok(ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: resolved_base_url,
        model: model.to_string(),
        proxy,
        ..Default::default()
    })
}

/// Approve a task PRD and trigger batch execution.
///
/// Validates the PRD structure, spawns execution as a background tokio task,
/// and returns immediately. Progress events are emitted via Tauri's
/// AppHandle::emit('task-mode-progress', payload) during execution.
#[tauri::command]
pub async fn approve_task_prd(
    app: tauri::AppHandle,
    session_id: String,
    prd: TaskPrd,
    state: tauri::State<'_, TaskModeState>,
    app_state: tauri::State<'_, AppState>,
    provider: Option<String>,
    model: Option<String>,
    execution_mode: Option<StoryExecutionMode>,
    workflow_config: Option<TaskWorkflowConfig>,
) -> Result<CommandResponse<bool>, String> {
    let mut session_guard = state.session.write().await;
    let session = match session_guard.as_mut() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if session.status != TaskModeStatus::ReviewingPrd {
        return Ok(CommandResponse::err(format!(
            "Cannot approve PRD in {:?} status",
            session.status
        )));
    }

    // Validate PRD
    if prd.stories.is_empty() {
        return Ok(CommandResponse::err("PRD must contain at least one story"));
    }

    // Calculate batches
    let stories: Vec<ExecutableStory> = prd
        .stories
        .iter()
        .map(|s| ExecutableStory {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            dependencies: s.dependencies.clone(),
            acceptance_criteria: s.acceptance_criteria.clone(),
            agent: None,
        })
        .collect();

    // Build execution config from workflow config overrides
    let mut config = ExecutionConfig::default();
    if let Some(ref wc) = workflow_config {
        if let Some(max_p) = wc.max_parallel {
            config.max_parallel = max_p;
        }
        config.skip_verification = wc.skip_verification;
        config.skip_review = wc.skip_review;
    }
    match crate::services::task_mode::calculate_batches(&stories, config.max_parallel) {
        Ok(batches) => {
            let mut approved_prd = prd;
            approved_prd.batches = batches;
            session.prd = Some(approved_prd);
            session.status = TaskModeStatus::Executing;

            // Create cancellation token for this execution
            let cancellation_token = CancellationToken::new();
            {
                let mut ct = state.cancellation_token.write().await;
                *ct = Some(cancellation_token.clone());
            }

            // Clear any previous execution result
            {
                let mut er = state.execution_result.write().await;
                *er = None;
            }

            // Clone what we need for the spawned background task
            let session_arc = state.session.clone();
            let result_arc = state.execution_result.clone();
            let sid = session_id.clone();
            let app_handle = app.clone();
            let stories_for_exec = stories.clone();

            // Resolve LLM provider config if provider/model specified
            let provider_config: Option<crate::services::llm::types::ProviderConfig> =
                if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
                    match resolve_provider_config(prov, mdl, &app_state).await {
                        Ok(cfg) => Some(cfg),
                        Err(e) => {
                            eprintln!("[approve_task_prd] LLM provider config resolution failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

            // Determine execution mode:
            // - If explicitly specified, use that
            // - If LLM provider config available, default to Llm
            // - Otherwise default to Cli
            let mode = execution_mode.unwrap_or_else(|| {
                if provider_config.is_some() {
                    StoryExecutionMode::Llm
                } else {
                    StoryExecutionMode::Cli
                }
            });

            // Resolve database pool for OrchestratorService (if using LLM mode)
            let db_pool = if matches!(mode, StoryExecutionMode::Llm) {
                app_state
                    .with_database(|db| Ok(db.pool().clone()))
                    .await
                    .ok()
            } else {
                None
            };

            // Spawn background tokio task for batch execution
            let exec_config = config;
            tokio::spawn(async move {
                let executor = BatchExecutor::new(
                    stories_for_exec,
                    exec_config,
                    cancellation_token,
                );
                let resolver = AgentResolver::with_defaults();

                // Create emit callback that sends events via Tauri AppHandle
                let app_for_emit = app_handle.clone();
                let emit = move |event: TaskModeProgressEvent| {
                    use tauri::Emitter;
                    let _ = app_for_emit.emit(TASK_MODE_EVENT_CHANNEL, &event);
                };

                // Resolve project path from current working directory
                let project_path = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."));

                // Create story executor that delegates to the appropriate backend.
                // In CLI mode, spawns external CLI tools. In LLM mode, uses OrchestratorService.
                let story_executor = build_story_executor(
                    app_handle.clone(),
                    mode,
                    provider_config,
                    db_pool,
                );

                let result = executor
                    .execute(&sid, &resolver, project_path, emit, story_executor)
                    .await;

                // Update session state based on result
                let mut session_guard = session_arc.write().await;
                if let Some(ref mut session) = *session_guard {
                    if session.session_id == sid {
                        match &result {
                            Ok(exec_result) => {
                                // Update progress
                                session.progress = Some(executor.get_progress().await);

                                if exec_result.cancelled {
                                    session.status = TaskModeStatus::Cancelled;
                                } else if exec_result.success {
                                    session.status = TaskModeStatus::Completed;
                                } else {
                                    session.status = TaskModeStatus::Failed;
                                }

                                // Store the result
                                let mut er = result_arc.write().await;
                                *er = Some(exec_result.clone());
                            }
                            Err(_) => {
                                session.status = TaskModeStatus::Failed;
                            }
                        }
                    }
                }
            });

            Ok(CommandResponse::ok(true))
        }
        Err(e) => Ok(CommandResponse::err(format!("PRD validation failed: {}", e))),
    }
}

/// Get the current task execution status.
#[tauri::command]
pub async fn get_task_execution_status(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskExecutionStatus>, String> {
    let session_guard = state.session.read().await;
    let session = match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: session
            .prd
            .as_ref()
            .map(|p| p.batches.len())
            .unwrap_or(0),
        stories_completed: 0,
        stories_failed: 0,
        total_stories: session
            .prd
            .as_ref()
            .map(|p| p.stories.len())
            .unwrap_or(0),
        story_statuses: HashMap::new(),
        current_phase: "idle".to_string(),
    });

    Ok(CommandResponse::ok(TaskExecutionStatus {
        session_id: session.session_id.clone(),
        status: session.status.clone(),
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        story_statuses: progress.story_statuses,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
    }))
}

/// Cancel the current task execution.
///
/// Triggers the CancellationToken to gracefully stop the background
/// batch execution task.
#[tauri::command]
pub async fn cancel_task_execution(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let session_guard = state.session.read().await;
    let session = match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if session.status != TaskModeStatus::Executing {
        return Ok(CommandResponse::err("No execution in progress to cancel"));
    }

    // Trigger the cancellation token
    let ct = state.cancellation_token.read().await;
    if let Some(ref token) = *ct {
        token.cancel();
    }

    // Note: The background task will update session.status to Cancelled
    // when it detects the cancellation token.
    Ok(CommandResponse::ok(true))
}

/// Get the execution report after completion.
///
/// Returns the final `BatchExecutionResult` populated by the background task.
#[tauri::command]
pub async fn get_task_execution_report(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<ExecutionReport>, String> {
    let session_guard = state.session.read().await;
    let session = match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => s,
        _ => return Ok(CommandResponse::err("Invalid session ID or no active session")),
    };

    if !matches!(
        session.status,
        TaskModeStatus::Completed | TaskModeStatus::Failed | TaskModeStatus::Cancelled
    ) {
        return Ok(CommandResponse::err("Execution has not finished yet"));
    }

    // Build a story title lookup from the PRD (if available)
    let story_title_map: HashMap<String, String> = session
        .prd
        .as_ref()
        .map(|prd| {
            prd.stories
                .iter()
                .map(|s| (s.id.clone(), s.title.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Build a story-to-batch-index lookup from the PRD batches
    let story_batch_map: HashMap<String, usize> = session
        .prd
        .as_ref()
        .map(|prd| {
            prd.batches
                .iter()
                .flat_map(|b| b.story_ids.iter().map(move |sid| (sid.clone(), b.index)))
                .collect()
        })
        .unwrap_or_default();

    // Try to get the real execution result
    let exec_result = state.execution_result.read().await;
    if let Some(ref result) = *exec_result {
        let agent_assignments: HashMap<String, String> = result
            .agent_assignments
            .iter()
            .map(|(id, a)| (id.clone(), a.agent_name.clone()))
            .collect();

        // --- Build timeline entries ---
        let mut timeline = Vec::new();
        // Estimate start_offset_ms per batch: sum durations of prior batches.
        // First, collect max duration per batch from completed stories.
        let mut batch_max_durations: HashMap<usize, u64> = HashMap::new();
        for (story_id, state) in &result.story_results {
            let batch_idx = story_batch_map.get(story_id).copied().unwrap_or(0);
            if let StoryExecutionState::Completed { duration_ms, .. } = state {
                let entry = batch_max_durations.entry(batch_idx).or_insert(0);
                if *duration_ms > *entry {
                    *entry = *duration_ms;
                }
            }
        }
        // Compute cumulative start offsets per batch index
        let max_batch_idx = story_batch_map.values().copied().max().unwrap_or(0);
        let mut batch_start_offsets: Vec<u64> = vec![0; max_batch_idx + 1];
        for i in 1..=max_batch_idx {
            batch_start_offsets[i] =
                batch_start_offsets[i - 1] + batch_max_durations.get(&(i - 1)).copied().unwrap_or(0);
        }

        for (story_id, story_state) in &result.story_results {
            let batch_idx = story_batch_map.get(story_id).copied().unwrap_or(0);
            let start_offset_ms = batch_start_offsets
                .get(batch_idx)
                .copied()
                .unwrap_or(0);
            let story_title = story_title_map
                .get(story_id)
                .cloned()
                .unwrap_or_else(|| story_id.clone());

            match story_state {
                StoryExecutionState::Completed {
                    agent,
                    duration_ms,
                    gate_result,
                } => {
                    let gate_summary = gate_result.as_ref().map(|pr| {
                        if pr.passed {
                            "passed".to_string()
                        } else {
                            format!(
                                "failed ({})",
                                pr.short_circuit_phase
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "validation".to_string())
                            )
                        }
                    });
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: agent.clone(),
                        duration_ms: *duration_ms,
                        start_offset_ms,
                        status: "completed".to_string(),
                        gate_result: gate_summary,
                    });
                }
                StoryExecutionState::Failed {
                    last_agent,
                    ..
                } => {
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: last_agent.clone(),
                        duration_ms: 0,
                        start_offset_ms,
                        status: "failed".to_string(),
                        gate_result: None,
                    });
                }
                StoryExecutionState::Cancelled => {
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: agent_assignments
                            .get(story_id)
                            .cloned()
                            .unwrap_or_default(),
                        duration_ms: 0,
                        start_offset_ms,
                        status: "cancelled".to_string(),
                        gate_result: None,
                    });
                }
                _ => {} // Pending/Running shouldn't appear in final results
            }
        }
        // Sort timeline by batch_index then story_id for deterministic output
        timeline.sort_by(|a, b| {
            a.batch_index
                .cmp(&b.batch_index)
                .then_with(|| a.story_id.cmp(&b.story_id))
        });

        // --- Build agent performance ---
        // Tracks: (assigned, completed, durations_vec)
        let mut agent_stats: HashMap<String, (usize, usize, Vec<u64>)> = HashMap::new();
        for (story_id, assignment) in &result.agent_assignments {
            let entry = agent_stats
                .entry(assignment.agent_name.clone())
                .or_insert((0, 0, Vec::new()));
            entry.0 += 1; // assigned
            if let Some(story_state) = result.story_results.get(story_id) {
                if let StoryExecutionState::Completed { duration_ms, .. } = story_state {
                    entry.1 += 1; // completed
                    entry.2.push(*duration_ms);
                }
            }
        }
        let agent_performance: Vec<AgentPerformanceEntry> = agent_stats
            .into_iter()
            .map(|(agent_name, (assigned, completed, durations))| {
                let success_rate = if assigned > 0 {
                    completed as f64 / assigned as f64
                } else {
                    0.0
                };
                let average_duration_ms = if !durations.is_empty() {
                    durations.iter().sum::<u64>() / durations.len() as u64
                } else {
                    0
                };
                AgentPerformanceEntry {
                    agent_name,
                    stories_assigned: assigned,
                    stories_completed: completed,
                    success_rate,
                    average_duration_ms,
                    average_quality_score: None, // populated below if quality scores exist
                }
            })
            .collect();

        // --- Build quality scores ---
        let quality_dimensions = [
            "correctness",
            "readability",
            "maintainability",
            "test_coverage",
            "security",
        ];
        let mut quality_scores = Vec::new();
        for (story_id, story_state) in &result.story_results {
            if let StoryExecutionState::Completed {
                gate_result: Some(pipeline_result),
                ..
            } = story_state
            {
                // Extract quality dimension scores from gate results.
                // Each gate that passed gets 100, failed gets 0. We map gate IDs
                // to quality dimensions where possible, and generate default
                // dimension scores based on overall pass/fail.
                let gate_results: Vec<_> = pipeline_result
                    .phase_results
                    .iter()
                    .flat_map(|pr| pr.gate_results.iter())
                    .collect();

                for dim in &quality_dimensions {
                    let score = compute_quality_dimension_score(dim, &gate_results, pipeline_result.passed);
                    quality_scores.push(QualityDimensionScore {
                        story_id: story_id.clone(),
                        dimension: dim.to_string(),
                        score,
                        max_score: 100.0,
                    });
                }
            }
        }

        // Compute average quality score per agent
        let mut agent_performance = agent_performance;
        for entry in &mut agent_performance {
            let agent_story_scores: Vec<f64> = quality_scores
                .iter()
                .filter(|qs| {
                    result
                        .agent_assignments
                        .get(&qs.story_id)
                        .map(|a| a.agent_name == entry.agent_name)
                        .unwrap_or(false)
                })
                .map(|qs| qs.score)
                .collect();
            if !agent_story_scores.is_empty() {
                let avg = agent_story_scores.iter().sum::<f64>() / agent_story_scores.len() as f64;
                entry.average_quality_score = Some(avg);
            }
        }

        return Ok(CommandResponse::ok(ExecutionReport {
            session_id: session.session_id.clone(),
            total_stories: result.total_stories,
            stories_completed: result.completed,
            stories_failed: result.failed,
            total_duration_ms: result.total_duration_ms,
            agent_assignments,
            success: result.success,
            quality_scores,
            timeline,
            agent_performance,
        }));
    }

    // Fallback to progress-based report (no BatchExecutionResult available)
    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: 0,
        stories_completed: 0,
        stories_failed: 0,
        total_stories: 0,
        story_statuses: HashMap::new(),
        current_phase: "complete".to_string(),
    });

    Ok(CommandResponse::ok(ExecutionReport {
        session_id: session.session_id.clone(),
        total_stories: progress.total_stories,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
        total_duration_ms: 0,
        agent_assignments: HashMap::new(),
        success: session.status == TaskModeStatus::Completed,
        quality_scores: Vec::new(),
        timeline: Vec::new(),
        agent_performance: Vec::new(),
    }))
}

/// Exit task mode and clean up session state.
#[tauri::command]
pub async fn exit_task_mode(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut session_guard = state.session.write().await;
    match session_guard.as_ref() {
        Some(s) if s.session_id == session_id => {
            // Cancel any running execution
            {
                let ct = state.cancellation_token.read().await;
                if let Some(ref token) = *ct {
                    token.cancel();
                }
            }

            *session_guard = None;

            // Clean up cancellation token and execution result
            {
                let mut ct = state.cancellation_token.write().await;
                *ct = None;
            }
            {
                let mut er = state.execution_result.write().await;
                *er = None;
            }

            Ok(CommandResponse::ok(true))
        }
        _ => Ok(CommandResponse::err("Invalid session ID or no active session")),
    }
}

// ============================================================================
// Story Executor
// ============================================================================

/// Load story-specific context from the project's design_doc.json.
///
/// Reads `design_doc.json` from the project root and extracts the story
/// mapping for the given `story_id` from the `story_mappings` section.
/// Returns `None` gracefully when the file does not exist, cannot be parsed,
/// or the story ID is not present in the mappings.
fn load_story_context(project_path: &std::path::Path, story_id: &str) -> Option<StoryContext> {
    let design_doc_path = project_path.join("design_doc.json");
    let content = std::fs::read_to_string(&design_doc_path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mappings = doc.get("story_mappings")?.as_object()?;
    let mapping = mappings.get(story_id)?;

    Some(StoryContext {
        relevant_files: mapping
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        components: mapping
            .get("components")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        design_decisions: mapping
            .get("decisions")
            .or_else(|| mapping.get("design_decisions"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        additional_context: mapping
            .get("additional_context")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

/// Build a story executor callback that runs each story through CLI or LLM.
///
/// In CLI mode, spawns external CLI tools (claude, codex, aider).
/// In LLM mode, uses OrchestratorService for direct LLM API execution.
fn build_story_executor(
    app_handle: tauri::AppHandle,
    mode: StoryExecutionMode,
    provider_config: Option<crate::services::llm::types::ProviderConfig>,
    db_pool: Option<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
) -> impl Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
       + Send
       + Sync
       + Clone
       + 'static {
    move |ctx: StoryExecutionContext| -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>> {
        let app = app_handle.clone();
        let mode = mode.clone();
        let provider_config = provider_config.clone();
        let db_pool = db_pool.clone();
        Box::pin(async move {
            eprintln!(
                "[INFO] Executing story '{}' (attempt {}) with agent '{}' in {} [mode: {:?}]",
                ctx.story_id,
                ctx.attempt,
                ctx.agent_name,
                ctx.project_path.display(),
                mode,
            );

            // Emit story execution event for frontend tracking
            {
                use tauri::Emitter;
                let _ = app.emit(
                    TASK_MODE_EVENT_CHANNEL,
                    &TaskModeProgressEvent {
                        session_id: String::new(),
                        event_type: "story_executing".to_string(),
                        current_batch: 0,
                        total_batches: 0,
                        story_id: Some(ctx.story_id.clone()),
                        story_status: Some("executing".to_string()),
                        agent_name: Some(ctx.agent_name.clone()),
                        gate_results: None,
                        error: None,
                        progress_pct: 0.0,
                    },
                );
            }

            // Load story-specific context from design_doc.json if available
            let mut ctx = ctx;
            if ctx.story_context.is_none() {
                ctx.story_context = load_story_context(&ctx.project_path, &ctx.story_id);
            }

            // Build execution prompt from story context
            let prompt = build_story_prompt(&ctx);

            match mode {
                StoryExecutionMode::Cli => {
                    // Spawn agent CLI process to execute the story
                    execute_story_via_agent(&ctx.agent_name, &prompt, &ctx.project_path).await
                }
                StoryExecutionMode::Llm => {
                    // Execute via OrchestratorService using direct LLM API
                    execute_story_via_llm(
                        provider_config.as_ref(),
                        &prompt,
                        &ctx.project_path,
                        db_pool.as_ref(),
                    )
                    .await
                }
            }
        })
    }
}

/// Execute a story by spawning an agent CLI process in the project directory.
///
/// Supports multiple agent backends: `claude`, `codex`, `aider`, etc.
/// The prompt is passed via the `-p` flag. The process exit code determines
/// success or failure.
async fn execute_story_via_agent(
    agent_name: &str,
    prompt: &str,
    project_path: &std::path::Path,
) -> StoryExecutionOutcome {
    use tokio::process::Command;

    // Resolve agent CLI command and arguments
    let (command, args) = match agent_name {
        name if name.starts_with("claude") => (
            "claude".to_string(),
            vec![
                "-p".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "text".to_string(),
            ],
        ),
        "codex" => (
            "codex".to_string(),
            vec!["--prompt".to_string(), prompt.to_string()],
        ),
        "aider" => (
            "aider".to_string(),
            vec![
                "--message".to_string(),
                prompt.to_string(),
                "--yes".to_string(),
            ],
        ),
        other => (
            other.to_string(),
            vec!["-p".to_string(), prompt.to_string()],
        ),
    };

    eprintln!("[INFO] Spawning agent '{}' in {}", command, project_path.display());

    let result = Command::new(&command)
        .args(&args)
        .current_dir(project_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => {
            eprintln!("[INFO] Agent '{}' completed successfully", command);
            StoryExecutionOutcome {
                success: true,
                error: None,
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!(
                "Agent '{}' exited with code {}: {}",
                command,
                output.status.code().unwrap_or(-1),
                stderr.chars().take(500).collect::<String>()
            );
            eprintln!("[WARN] {}", error_msg);
            StoryExecutionOutcome {
                success: false,
                error: Some(error_msg),
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to spawn agent '{}': {}", command, e);
            eprintln!("[WARN] {}", error_msg);
            StoryExecutionOutcome {
                success: false,
                error: Some(error_msg),
            }
        }
    }
}

/// Execute a story via the OrchestratorService using direct LLM API.
///
/// Creates a fresh OrchestratorService instance for each story execution,
/// runs the full agentic loop (tool use, code generation, etc.), and
/// maps the result to StoryExecutionOutcome.
async fn execute_story_via_llm(
    provider_config: Option<&crate::services::llm::types::ProviderConfig>,
    prompt: &str,
    project_path: &std::path::Path,
    db_pool: Option<&r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
) -> StoryExecutionOutcome {
    use crate::services::orchestrator::{OrchestratorConfig, OrchestratorService};
    use crate::services::streaming::UnifiedStreamEvent;

    let provider_config = match provider_config {
        Some(cfg) => cfg.clone(),
        None => {
            return StoryExecutionOutcome {
                success: false,
                error: Some("No LLM provider config available for story execution".to_string()),
            };
        }
    };

    let analysis_artifacts_root = dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".plan-cascade")
        .join("analysis-runs");

    let config = OrchestratorConfig {
        provider: provider_config,
        system_prompt: Some(
            "You are an expert software engineer executing a story task. \
             Use the provided tools to implement the required changes. \
             Read relevant files, make code changes, and run tests to verify."
                .to_string(),
        ),
        max_iterations: 50,
        max_total_tokens: 1_000_000,
        project_root: project_path.to_path_buf(),
        analysis_artifacts_root,
        streaming: true,
        enable_compaction: true,
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let mut orchestrator = OrchestratorService::new(config);

    // Wire database pool for CodebaseSearch if available
    if let Some(pool) = db_pool {
        orchestrator = orchestrator.with_database(pool.clone());
    }

    // Create channel for event collection (events are discarded for story execution)
    let (tx, mut rx) = tokio::sync::mpsc::channel::<UnifiedStreamEvent>(256);

    // Drain events in background to prevent channel backpressure
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Events are discarded — story execution doesn't stream to frontend
        }
    });

    eprintln!(
        "[INFO] Executing story via LLM in {}",
        project_path.display()
    );

    // Run the full agentic loop
    let result = orchestrator.execute(prompt.to_string(), tx).await;

    if result.success {
        eprintln!("[INFO] LLM story execution completed successfully");
        StoryExecutionOutcome {
            success: true,
            error: None,
        }
    } else {
        let error_msg = result
            .error
            .unwrap_or_else(|| "LLM execution failed with no error details".to_string());
        eprintln!("[WARN] LLM story execution failed: {}", error_msg);
        StoryExecutionOutcome {
            success: false,
            error: Some(error_msg),
        }
    }
}

/// Build an execution prompt from story context for the LLM agent.
fn build_story_prompt(ctx: &StoryExecutionContext) -> String {
    let criteria = ctx
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c))
        .collect::<Vec<_>>()
        .join("\n");

    let mut prompt = format!(
        "Execute the following task:\n\n\
         ## {} ({})\n\n\
         {}\n\n\
         ## Acceptance Criteria\n\
         {}\n\n\
         ## Instructions\n\
         - Implement all acceptance criteria\n\
         - Run relevant tests to verify correctness\n\
         - Ensure code compiles without errors",
        ctx.story_title, ctx.story_id, ctx.story_description, criteria,
    );

    if let Some(ref story_ctx) = ctx.story_context {
        prompt.push_str("\n\n## Relevant Context\n");
        if !story_ctx.relevant_files.is_empty() {
            prompt.push_str("\n### Relevant Files\n");
            for f in &story_ctx.relevant_files {
                prompt.push_str(&format!("- {}\n", f));
            }
        }
        if !story_ctx.components.is_empty() {
            prompt.push_str("\n### Components\n");
            for c in &story_ctx.components {
                prompt.push_str(&format!("- {}\n", c));
            }
        }
        if !story_ctx.design_decisions.is_empty() {
            prompt.push_str("\n### Design Decisions\n");
            for d in &story_ctx.design_decisions {
                prompt.push_str(&format!("- {}\n", d));
            }
        }
        if let Some(ref extra) = story_ctx.additional_context {
            prompt.push_str(&format!("\n### Additional Context\n{}\n", extra));
        }
    }

    if let Some(ref retry) = ctx.retry_context {
        prompt.push_str(&format!(
            "\n\n## Retry Context (Attempt {})\n\
             Previous attempt failed: {}\n\
             Previous agent: {}\n\
             Please analyze the failure and try a different approach.",
            retry.attempt, retry.failure_reason, retry.previous_agent,
        ));
    }

    prompt
}

// ============================================================================
// Quality Score Helpers
// ============================================================================

/// Compute a quality dimension score from gate results.
///
/// Maps quality dimensions to relevant gates where possible:
/// - "correctness" -> overall pipeline pass/fail
/// - "readability" -> "code_review" gate result
/// - "maintainability" -> "code_review" gate result
/// - "test_coverage" -> "ai_verify" gate result
/// - "security" -> "ai_verify" gate result
///
/// When a specific gate is found, the score is 100 if passed, 0 if failed.
/// When no relevant gate is found, falls back to 80 if the overall pipeline
/// passed, or 20 if it failed (partial credit for completing execution).
fn compute_quality_dimension_score(
    dimension: &str,
    gate_results: &[&crate::services::quality_gates::pipeline::PipelineGateResult],
    overall_passed: bool,
) -> f64 {
    let relevant_gate_id = match dimension {
        "correctness" => None, // based on overall pass/fail
        "readability" | "maintainability" => Some("code_review"),
        "test_coverage" | "security" => Some("ai_verify"),
        _ => None,
    };

    if let Some(gate_id) = relevant_gate_id {
        if let Some(gate) = gate_results.iter().find(|g| g.gate_id == gate_id) {
            return if gate.passed { 100.0 } else { 0.0 };
        }
    }

    // Fallback: derive from overall pipeline result
    if overall_passed {
        80.0
    } else {
        20.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test state
    fn test_state() -> TaskModeState {
        TaskModeState::new()
    }

    // We can't easily test Tauri commands directly without a Tauri app context,
    // so we test the core types and logic here.

    #[test]
    fn test_task_mode_status_serialization() {
        let json = serde_json::to_string(&TaskModeStatus::Initialized).unwrap();
        assert_eq!(json, "\"initialized\"");
        let json = serde_json::to_string(&TaskModeStatus::Executing).unwrap();
        assert_eq!(json, "\"executing\"");
        let json = serde_json::to_string(&TaskModeStatus::Completed).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn test_task_story_serialization() {
        let story = TaskStory {
            id: "story-001".to_string(),
            title: "Test Story".to_string(),
            description: "A test".to_string(),
            priority: "high".to_string(),
            dependencies: vec![],
            acceptance_criteria: vec!["Criterion 1".to_string()],
        };
        let json = serde_json::to_string(&story).unwrap();
        assert!(json.contains("\"acceptanceCriteria\""));
    }

    #[test]
    fn test_task_prd_serialization() {
        let prd = TaskPrd {
            title: "Test PRD".to_string(),
            description: "A test PRD".to_string(),
            stories: vec![],
            batches: vec![],
        };
        let json = serde_json::to_string(&prd).unwrap();
        assert!(json.contains("\"stories\""));
        assert!(json.contains("\"batches\""));
    }

    #[test]
    fn test_task_mode_session_serialization() {
        let session = TaskModeSession {
            session_id: "test-123".to_string(),
            description: "Build a feature".to_string(),
            status: TaskModeStatus::Initialized,
            strategy_analysis: None,
            prd: None,
            progress: None,
            created_at: "2026-02-18T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"strategyAnalysis\""));
        assert!(json.contains("\"createdAt\""));
    }

    #[test]
    fn test_execution_report_serialization() {
        let report = ExecutionReport {
            session_id: "test-123".to_string(),
            total_stories: 5,
            stories_completed: 4,
            stories_failed: 1,
            total_duration_ms: 10000,
            agent_assignments: HashMap::new(),
            success: false,
            quality_scores: vec![QualityDimensionScore {
                story_id: "s1".to_string(),
                dimension: "correctness".to_string(),
                score: 80.0,
                max_score: 100.0,
            }],
            timeline: vec![TimelineEntry {
                story_id: "s1".to_string(),
                story_title: "Story 1".to_string(),
                batch_index: 0,
                agent: "claude-sonnet".to_string(),
                duration_ms: 5000,
                start_offset_ms: 0,
                status: "completed".to_string(),
                gate_result: Some("passed".to_string()),
            }],
            agent_performance: vec![AgentPerformanceEntry {
                agent_name: "claude-sonnet".to_string(),
                stories_assigned: 3,
                stories_completed: 2,
                success_rate: 0.666,
                average_duration_ms: 4500,
                average_quality_score: Some(85.0),
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"totalDurationMs\""));
        assert!(json.contains("\"agentAssignments\""));
        assert!(json.contains("\"qualityScores\""));
        assert!(json.contains("\"timeline\""));
        assert!(json.contains("\"agentPerformance\""));
    }

    #[test]
    fn test_quality_dimension_score_serialization() {
        let score = QualityDimensionScore {
            story_id: "story-001".to_string(),
            dimension: "correctness".to_string(),
            score: 95.0,
            max_score: 100.0,
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"dimension\""));
        assert!(json.contains("\"score\""));
        assert!(json.contains("\"maxScore\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["storyId"], "story-001");
        assert_eq!(parsed["maxScore"], 100.0);
    }

    #[test]
    fn test_timeline_entry_serialization() {
        let entry = TimelineEntry {
            story_id: "story-002".to_string(),
            story_title: "Implement Auth".to_string(),
            batch_index: 1,
            agent: "claude-sonnet".to_string(),
            duration_ms: 8500,
            start_offset_ms: 3000,
            status: "completed".to_string(),
            gate_result: Some("passed".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"storyId\""));
        assert!(json.contains("\"storyTitle\""));
        assert!(json.contains("\"batchIndex\""));
        assert!(json.contains("\"durationMs\""));
        assert!(json.contains("\"startOffsetMs\""));
        assert!(json.contains("\"gateResult\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["storyId"], "story-002");
        assert_eq!(parsed["batchIndex"], 1);
        assert_eq!(parsed["durationMs"], 8500);
        assert_eq!(parsed["startOffsetMs"], 3000);
    }

    #[test]
    fn test_agent_performance_entry_serialization() {
        let entry = AgentPerformanceEntry {
            agent_name: "claude-sonnet".to_string(),
            stories_assigned: 5,
            stories_completed: 4,
            success_rate: 0.8,
            average_duration_ms: 6000,
            average_quality_score: Some(87.5),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"agentName\""));
        assert!(json.contains("\"storiesAssigned\""));
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"successRate\""));
        assert!(json.contains("\"averageDurationMs\""));
        assert!(json.contains("\"averageQualityScore\""));
        // Verify exact camelCase field names
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["agentName"], "claude-sonnet");
        assert_eq!(parsed["storiesAssigned"], 5);
        assert_eq!(parsed["averageDurationMs"], 6000);
    }

    #[test]
    fn test_agent_performance_entry_null_quality_score() {
        let entry = AgentPerformanceEntry {
            agent_name: "codex".to_string(),
            stories_assigned: 2,
            stories_completed: 0,
            success_rate: 0.0,
            average_duration_ms: 0,
            average_quality_score: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["averageQualityScore"].is_null());
    }

    #[test]
    fn test_execution_report_empty_enriched_fields() {
        // Simulates the fallback report with empty new fields
        let report = ExecutionReport {
            session_id: "test-fallback".to_string(),
            total_stories: 3,
            stories_completed: 0,
            stories_failed: 0,
            total_duration_ms: 0,
            agent_assignments: HashMap::new(),
            success: false,
            quality_scores: Vec::new(),
            timeline: Vec::new(),
            agent_performance: Vec::new(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["qualityScores"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["timeline"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["agentPerformance"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_timeline_entry_with_null_gate_result() {
        let entry = TimelineEntry {
            story_id: "s1".to_string(),
            story_title: "Story".to_string(),
            batch_index: 0,
            agent: "agent".to_string(),
            duration_ms: 0,
            start_offset_ms: 0,
            status: "failed".to_string(),
            gate_result: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["gateResult"].is_null());
    }

    #[test]
    fn test_quality_dimension_score_roundtrip() {
        let score = QualityDimensionScore {
            story_id: "s1".to_string(),
            dimension: "security".to_string(),
            score: 100.0,
            max_score: 100.0,
        };
        let json = serde_json::to_string(&score).unwrap();
        let deserialized: QualityDimensionScore = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.story_id, "s1");
        assert_eq!(deserialized.dimension, "security");
        assert_eq!(deserialized.score, 100.0);
        assert_eq!(deserialized.max_score, 100.0);
    }

    #[test]
    fn test_compute_quality_dimension_score_helper() {
        use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

        // Test with matching code_review gate that passed
        let gate = PipelineGateResult::passed(
            "code_review",
            "Code Review",
            GatePhase::PostValidation,
            100,
        );
        let results = vec![&gate];
        assert_eq!(
            compute_quality_dimension_score("readability", &results, true),
            100.0
        );
        assert_eq!(
            compute_quality_dimension_score("maintainability", &results, true),
            100.0
        );

        // Test with matching ai_verify gate that passed
        let gate2 = PipelineGateResult::passed(
            "ai_verify",
            "AI Verify",
            GatePhase::PostValidation,
            50,
        );
        let results2 = vec![&gate2];
        assert_eq!(
            compute_quality_dimension_score("test_coverage", &results2, true),
            100.0
        );
        assert_eq!(
            compute_quality_dimension_score("security", &results2, true),
            100.0
        );

        // Test correctness (uses overall pass/fail, no gate mapping)
        assert_eq!(
            compute_quality_dimension_score("correctness", &results, true),
            80.0
        );
        assert_eq!(
            compute_quality_dimension_score("correctness", &results, false),
            20.0
        );

        // Test fallback when no relevant gate found
        let empty: Vec<&PipelineGateResult> = vec![];
        assert_eq!(
            compute_quality_dimension_score("readability", &empty, true),
            80.0
        );
        assert_eq!(
            compute_quality_dimension_score("readability", &empty, false),
            20.0
        );
    }

    #[test]
    fn test_task_execution_status_serialization() {
        let status = TaskExecutionStatus {
            session_id: "test-123".to_string(),
            status: TaskModeStatus::Executing,
            current_batch: 1,
            total_batches: 3,
            story_statuses: HashMap::new(),
            stories_completed: 2,
            stories_failed: 0,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"currentBatch\""));
        assert!(json.contains("\"storyStatuses\""));
    }

    #[tokio::test]
    async fn test_state_creation() {
        let state = test_state();
        let session = state.session.read().await;
        assert!(session.is_none());
    }

    // ========================================================================
    // StoryContext & load_story_context Tests
    // ========================================================================

    /// Helper to create a StoryExecutionContext for prompt tests.
    fn make_story_ctx(story_context: Option<StoryContext>) -> StoryExecutionContext {
        StoryExecutionContext {
            story_id: "story-001".to_string(),
            story_title: "Add Login Feature".to_string(),
            story_description: "Implement user login with OAuth".to_string(),
            acceptance_criteria: vec![
                "Users can log in".to_string(),
                "OAuth tokens are stored securely".to_string(),
            ],
            agent_name: "claude-sonnet".to_string(),
            project_path: std::path::PathBuf::from("/tmp/test-project"),
            attempt: 1,
            retry_context: None,
            story_context,
        }
    }

    #[test]
    fn test_load_story_context_with_valid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-001": {
                    "files": ["src/auth.rs", "src/oauth.rs"],
                    "components": ["AuthService", "OAuthProvider"],
                    "decisions": ["ADR-001: Use OAuth2"],
                    "additional_context": "Requires HTTPS in production"
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.relevant_files, vec!["src/auth.rs", "src/oauth.rs"]);
        assert_eq!(ctx.components, vec!["AuthService", "OAuthProvider"]);
        assert_eq!(ctx.design_decisions, vec!["ADR-001: Use OAuth2"]);
        assert_eq!(
            ctx.additional_context.as_deref(),
            Some("Requires HTTPS in production")
        );
    }

    #[test]
    fn test_load_story_context_with_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        // No design_doc.json created
        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_missing_story() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-999": {
                    "files": ["src/other.rs"],
                    "components": [],
                    "decisions": []
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_design_decisions_field() {
        // Test fallback: "design_decisions" instead of "decisions"
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "story_mappings": {
                "story-002": {
                    "files": [],
                    "components": [],
                    "design_decisions": ["ADR-F002: Retry with repair"]
                }
            }
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-002");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(
            ctx.design_decisions,
            vec!["ADR-F002: Retry with repair"]
        );
    }

    #[test]
    fn test_load_story_context_with_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("design_doc.json"), "not valid json {{{")
            .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_load_story_context_with_no_story_mappings_key() {
        let tmp = tempfile::tempdir().unwrap();
        let design_doc = serde_json::json!({
            "title": "Design Doc",
            "components": []
        });
        std::fs::write(
            tmp.path().join("design_doc.json"),
            serde_json::to_string_pretty(&design_doc).unwrap(),
        )
        .unwrap();

        let ctx = load_story_context(tmp.path(), "story-001");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_build_story_prompt_without_context() {
        let ctx = make_story_ctx(None);
        let prompt = build_story_prompt(&ctx);

        assert!(prompt.contains("Add Login Feature"));
        assert!(prompt.contains("story-001"));
        assert!(prompt.contains("Implement user login with OAuth"));
        assert!(prompt.contains("1. Users can log in"));
        assert!(prompt.contains("2. OAuth tokens are stored securely"));
        // Should NOT contain context section
        assert!(!prompt.contains("## Relevant Context"));
    }

    #[test]
    fn test_build_story_prompt_with_context() {
        let story_ctx = StoryContext {
            relevant_files: vec!["src/auth.rs".to_string(), "src/oauth.rs".to_string()],
            components: vec!["AuthService".to_string()],
            design_decisions: vec!["ADR-001: Use OAuth2".to_string()],
            additional_context: Some("Must support Google and GitHub".to_string()),
        };
        let ctx = make_story_ctx(Some(story_ctx));
        let prompt = build_story_prompt(&ctx);

        // Standard prompt parts
        assert!(prompt.contains("Add Login Feature"));
        assert!(prompt.contains("story-001"));

        // Context section
        assert!(prompt.contains("## Relevant Context"));
        assert!(prompt.contains("### Relevant Files"));
        assert!(prompt.contains("- src/auth.rs"));
        assert!(prompt.contains("- src/oauth.rs"));
        assert!(prompt.contains("### Components"));
        assert!(prompt.contains("- AuthService"));
        assert!(prompt.contains("### Design Decisions"));
        assert!(prompt.contains("- ADR-001: Use OAuth2"));
        assert!(prompt.contains("### Additional Context"));
        assert!(prompt.contains("Must support Google and GitHub"));
    }

    #[test]
    fn test_build_story_prompt_with_partial_context() {
        // Only relevant_files, no components/decisions/additional
        let story_ctx = StoryContext {
            relevant_files: vec!["src/main.rs".to_string()],
            components: vec![],
            design_decisions: vec![],
            additional_context: None,
        };
        let ctx = make_story_ctx(Some(story_ctx));
        let prompt = build_story_prompt(&ctx);

        assert!(prompt.contains("## Relevant Context"));
        assert!(prompt.contains("### Relevant Files"));
        assert!(prompt.contains("- src/main.rs"));
        // Empty sections should be omitted
        assert!(!prompt.contains("### Components"));
        assert!(!prompt.contains("### Design Decisions"));
        assert!(!prompt.contains("### Additional Context"));
    }

    #[test]
    fn test_story_context_serialization_roundtrip() {
        let ctx = StoryContext {
            relevant_files: vec!["src/lib.rs".to_string(), "tests/test.rs".to_string()],
            components: vec!["Parser".to_string(), "Lexer".to_string()],
            design_decisions: vec!["ADR-003: Use tree-sitter".to_string()],
            additional_context: Some("Performance-critical path".to_string()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: StoryContext = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.relevant_files, ctx.relevant_files);
        assert_eq!(deserialized.components, ctx.components);
        assert_eq!(deserialized.design_decisions, ctx.design_decisions);
        assert_eq!(deserialized.additional_context, ctx.additional_context);

        // Verify camelCase serialization
        assert!(json.contains("\"relevantFiles\""));
        assert!(json.contains("\"designDecisions\""));
        assert!(json.contains("\"additionalContext\""));
    }

    #[test]
    fn test_story_context_serialization_with_none_additional() {
        let ctx = StoryContext {
            relevant_files: vec![],
            components: vec![],
            design_decisions: vec![],
            additional_context: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: StoryContext = serde_json::from_str(&json).unwrap();

        assert!(deserialized.relevant_files.is_empty());
        assert!(deserialized.additional_context.is_none());
    }
}
