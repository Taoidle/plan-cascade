//! Spec Interview Commands
//!
//! Tauri commands for the spec interview service.
//! Provides four commands: start, submit answer, get state, and compile.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::response::CommandResponse;
use crate::services::spec_interview::{
    CompileOptions, InterviewManager, InterviewStateManager, SpecCompiler,
};
use crate::services::spec_interview::interview::{InterviewConfig, InterviewSession};
use crate::services::spec_interview::compiler::CompiledSpec;
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
#[tauri::command]
pub async fn start_spec_interview(
    config: InterviewConfig,
    state: State<'_, SpecInterviewState>,
) -> Result<CommandResponse<InterviewSession>, String> {
    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    match mgr.start_interview(config) {
        Ok(session) => Ok(CommandResponse::ok(session)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Submit an answer to the current interview question
///
/// Records the answer, updates the spec data, and returns the next question.
/// If the interview is complete, returns the session with no current question.
#[tauri::command]
pub async fn submit_interview_answer(
    interview_id: String,
    answer: String,
    state: State<'_, SpecInterviewState>,
) -> Result<CommandResponse<InterviewSession>, String> {
    let mgr_lock = state.interview_manager.read().await;
    let mgr = match SpecInterviewState::ensure_initialized_sync(&mgr_lock) {
        Ok(m) => m,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    match mgr.submit_answer(&interview_id, &answer) {
        Ok(session) => Ok(CommandResponse::ok(session)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
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
) -> Result<CommandResponse<InterviewSession>, String> {
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
) -> Result<CommandResponse<CompiledSpec>, String> {
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
