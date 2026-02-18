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
    ExecutionConfig, StoryExecutionContext, StoryExecutionOutcome, TaskModeProgressEvent,
    TASK_MODE_EVENT_CHANNEL,
};
use crate::services::task_mode::agent_resolver::AgentResolver;
use crate::services::task_mode::prd_generator;

use crate::state::AppState;
use crate::storage::KeyringService;

// ============================================================================
// Types
// ============================================================================

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

    let config = ExecutionConfig::default();
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

            // Spawn background tokio task for batch execution
            tokio::spawn(async move {
                let executor = BatchExecutor::new(
                    stories_for_exec,
                    ExecutionConfig::default(),
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

                // Create story executor that delegates to the orchestrator service.
                // Each story is sent to the LLM agent for code generation and execution.
                let story_executor = build_story_executor(app_handle.clone());

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

    // Try to get the real execution result
    let exec_result = state.execution_result.read().await;
    if let Some(ref result) = *exec_result {
        let agent_assignments: HashMap<String, String> = result
            .agent_assignments
            .iter()
            .map(|(id, a)| (id.clone(), a.agent_name.clone()))
            .collect();

        return Ok(CommandResponse::ok(ExecutionReport {
            session_id: session.session_id.clone(),
            total_stories: result.total_stories,
            stories_completed: result.completed,
            stories_failed: result.failed,
            total_duration_ms: result.total_duration_ms,
            agent_assignments,
            success: result.success,
        }));
    }

    // Fallback to progress-based report
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

/// Build a story executor callback that runs each story through a CLI agent.
///
/// The returned callback creates an execution prompt from the story context
/// and spawns a `claude` CLI process (or other configured agent) for code
/// generation. The process runs in the project directory with the story
/// prompt piped to stdin. If the agent binary is not available, execution
/// fails (which triggers retry with a different agent if retry is enabled).
fn build_story_executor(
    app_handle: tauri::AppHandle,
) -> impl Fn(StoryExecutionContext) -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>>
       + Send
       + Sync
       + Clone
       + 'static {
    move |ctx: StoryExecutionContext| -> Pin<Box<dyn Future<Output = StoryExecutionOutcome> + Send>> {
        let app = app_handle.clone();
        Box::pin(async move {
            eprintln!(
                "[INFO] Executing story '{}' (attempt {}) with agent '{}' in {}",
                ctx.story_id,
                ctx.attempt,
                ctx.agent_name,
                ctx.project_path.display()
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

            // Build execution prompt from story context
            let prompt = build_story_prompt(&ctx);

            // Spawn agent CLI process to execute the story
            execute_story_via_agent(&ctx.agent_name, &prompt, &ctx.project_path).await
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
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"storiesCompleted\""));
        assert!(json.contains("\"totalDurationMs\""));
        assert!(json.contains("\"agentAssignments\""));
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
}
