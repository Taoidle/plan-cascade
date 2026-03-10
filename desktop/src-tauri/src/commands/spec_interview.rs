//! Spec Interview Commands
//!
//! Tauri commands for the spec interview service.
//! Provides four commands: start, submit answer, get state, and compile.

use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::spec_interview::compiler::CompiledSpec;
use crate::services::spec_interview::interview::{InterviewConfig, InterviewSession};
use crate::services::spec_interview::{
    CompileOptions, InterviewManager, InterviewStateManager, SpecCompiler,
};
use crate::services::workflow_kernel::{
    TaskInterviewSnapshot, WorkflowKernelState, WorkflowKernelUpdatedEvent, WorkflowMode,
    WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
use crate::state::AppState;
use crate::storage::database::DbPool;
use tauri::Emitter;

/// State for the Spec Interview service, managed by Tauri
pub struct SpecInterviewState {
    pub interview_manager: Arc<RwLock<Option<InterviewManager>>>,
    pub state_manager: Arc<RwLock<Option<InterviewStateManager>>>,
    pool: Arc<RwLock<Option<DbPool>>>,
    task_links: Arc<RwLock<HashMap<String, String>>>,
}

impl SpecInterviewState {
    pub fn new() -> Self {
        Self {
            interview_manager: Arc::new(RwLock::new(None)),
            state_manager: Arc::new(RwLock::new(None)),
            pool: Arc::new(RwLock::new(None)),
            task_links: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize with a database pool (called during app init)
    pub async fn initialize(&self, pool: DbPool) -> Result<(), String> {
        let state_mgr = InterviewStateManager::new(pool.clone());

        // Initialize the schema
        state_mgr.init_schema().map_err(|e| e.to_string())?;

        let interview_mgr = InterviewManager::new(state_mgr.clone());

        let mut sm_lock = self.state_manager.write().await;
        *sm_lock = Some(state_mgr);

        let mut im_lock = self.interview_manager.write().await;
        *im_lock = Some(interview_mgr);

        let mut pool_lock = self.pool.write().await;
        *pool_lock = Some(pool);

        Ok(())
    }

    fn ensure_initialized_sync(
        interview_mgr: &Option<InterviewManager>,
    ) -> Result<&InterviewManager, String> {
        interview_mgr.as_ref().ok_or_else(|| {
            "Spec interview service not initialized. Call init_app first.".to_string()
        })
    }

    /// Lazy initialization: if init_app failed to initialize this service
    /// (e.g. silent pool retrieval failure), try to initialize from AppState now.
    pub async fn ensure_initialized(&self, app_state: &AppState) -> Result<(), String> {
        // Fast path: already initialized
        {
            let mgr = self.interview_manager.read().await;
            if mgr.is_some() {
                return Ok(());
            }
        }
        // Slow path: initialize from database pool
        let pool = app_state
            .with_database(|db| Ok(db.pool().clone()))
            .await
            .map_err(|e| format!("Database not available for spec interview init: {}", e))?;
        self.initialize(pool).await
    }

    pub async fn get_session_snapshot(
        &self,
        interview_id: &str,
        app_state: &AppState,
    ) -> Option<InterviewSession> {
        if self.ensure_initialized(app_state).await.is_err() {
            return None;
        }
        let mgr_lock = self.interview_manager.read().await;
        let mgr = SpecInterviewState::ensure_initialized_sync(&mgr_lock).ok()?;
        mgr.get_interview_state(interview_id).ok()
    }

    pub async fn link_task_session(&self, interview_id: &str, task_session_id: &str) {
        let normalized_interview_id = interview_id.trim();
        let normalized_task_session_id = task_session_id.trim();
        if normalized_interview_id.is_empty() || normalized_task_session_id.is_empty() {
            return;
        }
        let mut links = self.task_links.write().await;
        links.insert(
            normalized_interview_id.to_string(),
            normalized_task_session_id.to_string(),
        );
    }

    pub async fn get_linked_task_session_id(&self, interview_id: &str) -> Option<String> {
        let links = self.task_links.read().await;
        links.get(interview_id).cloned()
    }
}

fn interview_phase_for_kernel(session: &InterviewSession) -> &'static str {
    if session.status == "in_progress" && session.current_question.is_some() {
        "interviewing"
    } else if session.status == "finalized"
        || session.phase == crate::services::spec_interview::interview::InterviewPhase::Complete
    {
        "generating_prd"
    } else {
        "interviewing"
    }
}

fn map_task_interview_snapshot(session: &InterviewSession) -> Option<TaskInterviewSnapshot> {
    let question = session.current_question.as_ref()?;
    Some(TaskInterviewSnapshot {
        interview_id: session.id.clone(),
        question_id: question.id.clone(),
        question: question.question.clone(),
        hint: question.hint.clone(),
        required: question.required,
        input_type: question.input_type.clone(),
        options: question.options.clone(),
        allow_custom: question.allow_custom,
        question_number: (session.question_cursor.max(0) as u32).saturating_add(1),
        total_questions: session.max_questions.max(0) as u32,
    })
}

fn normalize_task_session_id(value: Option<&str>) -> Option<String> {
    value
        .map(|raw| raw.trim())
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_string())
}

async fn emit_kernel_updates(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    kernel_session_ids: &[String],
    source: &str,
) {
    for kernel_session_id in kernel_session_ids {
        if let Ok(session_state) = kernel_state.get_session_state(kernel_session_id).await {
            let revision = (session_state.events.len() + session_state.checkpoints.len()) as u64;
            let payload = WorkflowKernelUpdatedEvent {
                session_state,
                revision,
                source: source.to_string(),
            };
            let _ = app.emit(WORKFLOW_KERNEL_UPDATED_CHANNEL, payload);
        }
    }
}

async fn sync_kernel_task_interview_and_emit(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    task_session_id: &str,
    interview_session: &InterviewSession,
    source: &str,
) {
    let kernel_session_ids = kernel_state
        .sync_task_interview_snapshot_by_linked_session(
            task_session_id,
            Some(interview_session.id.clone()),
            Some(interview_phase_for_kernel(interview_session).to_string()),
            map_task_interview_snapshot(interview_session),
        )
        .await
        .unwrap_or_default();
    emit_kernel_updates(app, kernel_state, &kernel_session_ids, source).await;
}

impl Default for SpecInterviewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start a new spec interview session
///
/// Creates a new interview with the given configuration and returns
/// the initial session state with the first question.
/// When provider params are given, uses LLM-driven BA for question generation.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn start_spec_interview(
    config: InterviewConfig,
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
    kernel_state: State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<InterviewSession>, String> {
    // Lazy init: if init_app failed to initialize spec interview, try now
    if let Err(e) = state.ensure_initialized(&app_state).await {
        tracing::warn!("Spec interview lazy init failed: {}", e);
    }

    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Try to resolve LLM provider for BA-driven interview
    let llm_provider = resolve_interview_provider(
        provider,
        model,
        api_key.or(apiKey),
        base_url.or(baseUrl),
        &app_state,
    )
    .await;
    let linked_task_session_id = normalize_task_session_id(config.task_session_id.as_deref());

    match llm_provider {
        Some(provider) => {
            let config_clone = config.clone();
            let provider = if let Some(task_session_id) = linked_task_session_id.as_deref() {
                crate::commands::task_mode::wrap_task_provider_with_tracking(
                    &app_handle,
                    app_state.inner(),
                    provider,
                    crate::commands::task_mode::build_task_analytics_attribution(
                        kernel_state
                            .linked_kernel_sessions_for_mode_session(
                                WorkflowMode::Task,
                                task_session_id,
                            )
                            .await
                            .into_iter()
                            .next(),
                        task_session_id,
                        "plan_interview",
                        crate::models::analytics::AnalyticsExecutionScope::DirectLlm,
                        format!("task:{}:interview:start", task_session_id),
                        None,
                        Some("task_interview".to_string()),
                        Some("business_analyst".to_string()),
                        None,
                        None,
                        Some(1),
                        "spec_interview.start",
                    ),
                )
                .await
            } else {
                provider
            };
            let provider_for_translate = Some(Arc::clone(&provider));
            match mgr.start_interview_with_llm(config, provider).await {
                Ok(session) => {
                    if let Some(task_session_id) = linked_task_session_id.as_deref() {
                        state.link_task_session(&session.id, task_session_id).await;
                        sync_kernel_task_interview_and_emit(
                            &app_handle,
                            kernel_state.inner(),
                            task_session_id,
                            &session,
                            "spec_interview.start_spec_interview.llm",
                        )
                        .await;
                    }
                    Ok(CommandResponse::ok(session))
                }
                Err(e) => {
                    // Fall back to deterministic on LLM failure
                    tracing::warn!(error = %e, "LLM-driven interview start failed, falling back to deterministic");
                    match mgr.start_interview(config_clone) {
                        Ok(mut session) => {
                            translate_session_question(&mut session, &provider_for_translate).await;
                            if let Some(task_session_id) = linked_task_session_id.as_deref() {
                                state.link_task_session(&session.id, task_session_id).await;
                                sync_kernel_task_interview_and_emit(
                                    &app_handle,
                                    kernel_state.inner(),
                                    task_session_id,
                                    &session,
                                    "spec_interview.start_spec_interview.fallback",
                                )
                                .await;
                            }
                            Ok(CommandResponse::ok(session))
                        }
                        Err(e) => Ok(CommandResponse::err(e.to_string())),
                    }
                }
            }
        }
        None => match mgr.start_interview(config) {
            Ok(session) => {
                if let Some(task_session_id) = linked_task_session_id.as_deref() {
                    state.link_task_session(&session.id, task_session_id).await;
                    sync_kernel_task_interview_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        task_session_id,
                        &session,
                        "spec_interview.start_spec_interview.deterministic",
                    )
                    .await;
                }
                Ok(CommandResponse::ok(session))
            }
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
    }
}

/// Submit an answer to the current interview question
///
/// Records the answer, updates the spec data, and returns the next question.
/// If the interview is complete, returns the session with no current question.
/// When provider params are given, uses LLM-driven BA for next question generation.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn submit_interview_answer(
    interview_id: String,
    answer: String,
    task_session_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
    kernel_state: State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<InterviewSession>, String> {
    // Lazy init: if init_app failed to initialize spec interview, try now
    if let Err(e) = state.ensure_initialized(&app_state).await {
        tracing::warn!("Spec interview lazy init failed: {}", e);
    }

    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Try to resolve LLM provider for BA-driven question generation
    let llm_provider = resolve_interview_provider(
        provider,
        model,
        api_key.or(apiKey),
        base_url.or(baseUrl),
        &app_state,
    )
    .await;
    let linked_task_session_id = match normalize_task_session_id(task_session_id.as_deref()) {
        Some(explicit) => Some(explicit),
        None => match state.get_linked_task_session_id(&interview_id).await {
            Some(mapped) => Some(mapped),
            None => {
                kernel_state
                    .find_linked_task_session_by_interview_session(&interview_id)
                    .await
            }
        },
    };

    match llm_provider {
        Some(provider) => {
            let provider = if let Some(task_session_id) = linked_task_session_id.as_deref() {
                crate::commands::task_mode::wrap_task_provider_with_tracking(
                    &app_handle,
                    app_state.inner(),
                    provider,
                    crate::commands::task_mode::build_task_analytics_attribution(
                        kernel_state
                            .linked_kernel_sessions_for_mode_session(
                                WorkflowMode::Task,
                                task_session_id,
                            )
                            .await
                            .into_iter()
                            .next(),
                        task_session_id,
                        "plan_interview",
                        crate::models::analytics::AnalyticsExecutionScope::DirectLlm,
                        format!("task:{}:interview:{}", task_session_id, interview_id),
                        None,
                        Some("task_interview".to_string()),
                        Some("business_analyst".to_string()),
                        None,
                        None,
                        Some(1),
                        "spec_interview.answer",
                    ),
                )
                .await
            } else {
                provider
            };
            let provider_for_translate = Some(Arc::clone(&provider));
            match mgr
                .submit_answer_with_llm(&interview_id, &answer, provider)
                .await
            {
                Ok(session) => {
                    if let Some(task_session_id) = linked_task_session_id.as_deref() {
                        state.link_task_session(&session.id, task_session_id).await;
                        sync_kernel_task_interview_and_emit(
                            &app_handle,
                            kernel_state.inner(),
                            task_session_id,
                            &session,
                            "spec_interview.submit_interview_answer.llm",
                        )
                        .await;
                    }
                    Ok(CommandResponse::ok(session))
                }
                Err(e) => {
                    // Fall back to deterministic on LLM failure
                    tracing::warn!(error = %e, "LLM-driven answer submission failed, falling back to deterministic");
                    match mgr.submit_answer(&interview_id, &answer) {
                        Ok(mut session) => {
                            translate_session_question(&mut session, &provider_for_translate).await;
                            if let Some(task_session_id) = linked_task_session_id.as_deref() {
                                state.link_task_session(&session.id, task_session_id).await;
                                sync_kernel_task_interview_and_emit(
                                    &app_handle,
                                    kernel_state.inner(),
                                    task_session_id,
                                    &session,
                                    "spec_interview.submit_interview_answer.fallback",
                                )
                                .await;
                            }
                            Ok(CommandResponse::ok(session))
                        }
                        Err(e) => Ok(CommandResponse::err(e.to_string())),
                    }
                }
            }
        }
        None => match mgr.submit_answer(&interview_id, &answer) {
            Ok(session) => {
                if let Some(task_session_id) = linked_task_session_id.as_deref() {
                    state.link_task_session(&session.id, task_session_id).await;
                    sync_kernel_task_interview_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        task_session_id,
                        &session,
                        "spec_interview.submit_interview_answer.deterministic",
                    )
                    .await;
                }
                Ok(CommandResponse::ok(session))
            }
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
    }
}

/// Get the current state of an interview
///
/// Returns the full interview session including history and current question.
/// Useful for resuming after restart.
#[tauri::command]
pub async fn get_interview_state(
    interview_id: String,
    task_session_id: Option<String>,
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
    kernel_state: State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<InterviewSession>, String> {
    if let Err(e) = state.ensure_initialized(&app_state).await {
        tracing::warn!("Spec interview lazy init failed: {}", e);
    }

    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let linked_task_session_id = match normalize_task_session_id(task_session_id.as_deref()) {
        Some(explicit) => Some(explicit),
        None => match state.get_linked_task_session_id(&interview_id).await {
            Some(mapped) => Some(mapped),
            None => {
                kernel_state
                    .find_linked_task_session_by_interview_session(&interview_id)
                    .await
            }
        },
    };

    match mgr.get_interview_state(&interview_id) {
        Ok(session) => {
            if let Some(task_session_id) = linked_task_session_id.as_deref() {
                state.link_task_session(&session.id, task_session_id).await;
                sync_kernel_task_interview_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    task_session_id,
                    &session,
                    "spec_interview.get_interview_state",
                )
                .await;
            }
            Ok(CommandResponse::ok(session))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Compile a completed interview into spec.json, spec.md, and prd.json
///
/// The interview must be in "finalized" status. Returns the compiled outputs.
#[tauri::command]
pub async fn compile_spec(
    interview_id: String,
    options: Option<CompileOptions>,
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<CompiledSpec>, String> {
    if let Err(e) = state.ensure_initialized(&app_state).await {
        tracing::warn!("Spec interview lazy init failed: {}", e);
    }

    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Get the spec data from the interview
    let spec_data = match mgr.get_spec_data(&interview_id) {
        Ok(data) => data,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let compile_options = options.unwrap_or_default();

    match SpecCompiler::compile(&spec_data, &compile_options) {
        Ok(compiled) => Ok(CommandResponse::ok(compiled)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Translate the current question in a session if locale is non-English and a provider is available.
///
/// This is used for deterministic-mode interviews to translate the fixed English questions.
/// LLM-driven mode already handles locale via prompt instructions.
async fn translate_session_question(
    session: &mut InterviewSession,
    llm_provider: &Option<Arc<dyn crate::services::llm::provider::LlmProvider>>,
) {
    if let (Some(ref mut q), Some(ref provider)) = (&mut session.current_question, llm_provider) {
        if session.locale != "en" && !session.locale.is_empty() {
            InterviewManager::translate_question(q, &session.locale, provider).await;
        }
    }
}

/// Resolve an LLM provider for interview BA usage.
///
/// Follows the same resolution pattern as task_mode commands:
/// provider: explicit param → database `llm_provider` → None (deterministic mode)
/// model: explicit param → database `llm_model` → provider-specific default
///
/// Returns None if no provider can be resolved (falls back to deterministic mode).
async fn resolve_interview_provider(
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    app_state: &State<'_, AppState>,
) -> Option<Arc<dyn crate::services::llm::provider::LlmProvider>> {
    // Resolve provider name: explicit > database settings > none
    let resolved_provider = match provider {
        Some(ref p) if !p.is_empty() => p.clone(),
        _ => match app_state
            .with_database(|db| db.get_setting("llm_provider"))
            .await
        {
            Ok(Some(p)) if !p.is_empty() => p,
            _ => return None, // No provider configured — use deterministic mode
        },
    };

    // Resolve model: explicit > database settings > provider-specific default
    let resolved_model = match model {
        Some(ref m) if !m.is_empty() => m.clone(),
        _ => match app_state
            .with_database(|db| db.get_setting("llm_model"))
            .await
        {
            Ok(Some(m)) if !m.is_empty() => m,
            _ => match resolved_provider.as_str() {
                "anthropic" => "claude-sonnet-4-20250514".to_string(),
                "openai" => "gpt-4o".to_string(),
                "deepseek" => "deepseek-chat".to_string(),
                "ollama" => "qwen2.5-coder:14b".to_string(),
                _ => "claude-sonnet-4-20250514".to_string(),
            },
        },
    };

    match crate::commands::task_mode::resolve_llm_provider(
        &resolved_provider,
        &resolved_model,
        api_key,
        base_url,
        app_state,
    )
    .await
    {
        Ok(provider) => Some(provider),
        Err(e) => {
            tracing::debug!(error = %e, "Could not resolve LLM provider for interview, using deterministic mode");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_interview_state_creation() {
        let state = SpecInterviewState::new();
        // Verify state is created without panic
        let _ = state;
    }

    #[test]
    fn test_interview_config_serialization() {
        let config = InterviewConfig {
            description: "Test project".to_string(),
            flow_level: "standard".to_string(),
            max_questions: 18,
            first_principles: false,
            project_path: Some("/tmp/test".to_string()),
            exploration_context: None,
            task_session_id: None,
            locale: "en".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("Test project"));
        assert!(json.contains("standard"));
    }

    #[test]
    fn test_compile_options_default() {
        let options = CompileOptions::default();
        assert!(options.description.is_empty());
        assert!(options.flow_level.is_none());
        assert!(!options.confirm);
        assert!(!options.no_confirm);
    }
}
