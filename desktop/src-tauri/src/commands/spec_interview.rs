//! Spec Interview Commands
//!
//! Tauri commands for the spec interview service.
//! Provides four commands: start, submit answer, get state, and compile.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::spec_interview::compiler::CompiledSpec;
use crate::services::spec_interview::interview::{InterviewConfig, InterviewSession};
use crate::services::spec_interview::{
    CompileOptions, InterviewManager, InterviewStateManager, SpecCompiler,
};
use crate::state::AppState;
use crate::storage::database::DbPool;

/// State for the Spec Interview service, managed by Tauri
pub struct SpecInterviewState {
    pub interview_manager: Arc<RwLock<Option<InterviewManager>>>,
    pub state_manager: Arc<RwLock<Option<InterviewStateManager>>>,
    pool: Arc<RwLock<Option<DbPool>>>,
}

impl SpecInterviewState {
    pub fn new() -> Self {
        Self {
            interview_manager: Arc::new(RwLock::new(None)),
            state_manager: Arc::new(RwLock::new(None)),
            pool: Arc::new(RwLock::new(None)),
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

    match llm_provider {
        Some(provider) => {
            let config_clone = config.clone();
            let provider_for_translate = Some(Arc::clone(&provider));
            match mgr.start_interview_with_llm(config, provider).await {
                Ok(session) => Ok(CommandResponse::ok(session)),
                Err(e) => {
                    // Fall back to deterministic on LLM failure
                    tracing::warn!(error = %e, "LLM-driven interview start failed, falling back to deterministic");
                    match mgr.start_interview(config_clone) {
                        Ok(mut session) => {
                            translate_session_question(&mut session, &provider_for_translate).await;
                            Ok(CommandResponse::ok(session))
                        }
                        Err(e) => Ok(CommandResponse::err(e.to_string())),
                    }
                }
            }
        }
        None => match mgr.start_interview(config) {
            Ok(session) => Ok(CommandResponse::ok(session)),
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
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
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

    match llm_provider {
        Some(provider) => {
            let provider_for_translate = Some(Arc::clone(&provider));
            match mgr
                .submit_answer_with_llm(&interview_id, &answer, provider)
                .await
            {
                Ok(session) => Ok(CommandResponse::ok(session)),
                Err(e) => {
                    // Fall back to deterministic on LLM failure
                    tracing::warn!(error = %e, "LLM-driven answer submission failed, falling back to deterministic");
                    match mgr.submit_answer(&interview_id, &answer) {
                        Ok(mut session) => {
                            translate_session_question(&mut session, &provider_for_translate).await;
                            Ok(CommandResponse::ok(session))
                        }
                        Err(e) => Ok(CommandResponse::err(e.to_string())),
                    }
                }
            }
        }
        None => match mgr.submit_answer(&interview_id, &answer) {
            Ok(session) => Ok(CommandResponse::ok(session)),
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
    state: State<'_, SpecInterviewState>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<InterviewSession>, String> {
    if let Err(e) = state.ensure_initialized(&app_state).await {
        tracing::warn!("Spec interview lazy init failed: {}", e);
    }

    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    match mgr.get_interview_state(&interview_id) {
        Ok(session) => Ok(CommandResponse::ok(session)),
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
