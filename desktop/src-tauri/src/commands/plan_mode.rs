//! Plan Mode Tauri Commands
//!
//! Provides the complete Plan Mode lifecycle as Tauri commands:
//! - enter/exit plan mode
//! - submit clarifications
//! - generate/approve plan
//! - execution status/cancel/report

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::commands::task_mode::{
    resolve_llm_provider, resolve_provider_config, resolve_search_provider_for_tools,
};
use crate::models::CommandResponse;
use crate::services::knowledge::observability;
use crate::services::plan_mode::adapter_registry::{AdapterInfo, AdapterRegistry};
use crate::services::plan_mode::types::{
    ClarificationAnswer, Plan, PlanAnalysis, PlanExecutionReport, PlanModePhase, PlanModeSession,
    StepExecutionState, StepOutput,
};
use crate::services::skills::model::InjectionPhase;
use crate::services::task_mode::context_provider::{
    ContextSourceConfig, MemorySourceConfig, SkillsSourceConfig,
};
use crate::services::workflow_kernel::{
    PlanClarificationSnapshot, WorkflowKernelState, WorkflowKernelUpdatedEvent,
    WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
use crate::state::AppState;
use tauri::{Emitter, Manager};

// ============================================================================
// State
// ============================================================================

/// Managed state for Plan Mode.
pub struct PlanModeState {
    sessions: Arc<RwLock<HashMap<String, PlanModeSession>>>,
    cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    operation_cancellation_tokens: Arc<RwLock<HashMap<String, (String, CancellationToken)>>>,
    adapter_registry: Arc<RwLock<AdapterRegistry>>,
}

impl PlanModeState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            operation_cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            adapter_registry: Arc::new(RwLock::new(AdapterRegistry::with_builtins())),
        }
    }

    pub async fn get_session_snapshot(&self, session_id: &str) -> Option<PlanModeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
}

const PLAN_OPERATION_CANCELLED_ERROR: &str = "Operation cancelled";

async fn register_plan_operation_token(
    state: &PlanModeState,
    session_id: &str,
) -> (String, CancellationToken) {
    let operation_id = uuid::Uuid::new_v4().to_string();
    let token = CancellationToken::new();

    let previous = {
        let mut tokens = state.operation_cancellation_tokens.write().await;
        tokens.insert(
            session_id.to_string(),
            (operation_id.clone(), token.clone()),
        )
    };
    if let Some((_, prev_token)) = previous {
        prev_token.cancel();
    }

    (operation_id, token)
}

async fn clear_plan_operation_token(state: &PlanModeState, session_id: &str, operation_id: &str) {
    let mut tokens = state.operation_cancellation_tokens.write().await;
    let should_remove = tokens
        .get(session_id)
        .map(|(current_id, _)| current_id == operation_id)
        .unwrap_or(false);
    if should_remove {
        tokens.remove(session_id);
    }
}

fn plan_phase_to_kernel_phase(phase: PlanModePhase) -> &'static str {
    match phase {
        PlanModePhase::Idle => "idle",
        PlanModePhase::Analyzing => "analyzing",
        PlanModePhase::Clarifying => "clarifying",
        PlanModePhase::Planning => "planning",
        PlanModePhase::ReviewingPlan => "reviewing_plan",
        PlanModePhase::Executing => "executing",
        PlanModePhase::Completed => "completed",
        PlanModePhase::Failed => "failed",
        PlanModePhase::Cancelled => "cancelled",
    }
}

fn map_clarification_input_type(
    input_type: &crate::services::plan_mode::types::ClarificationInputType,
) -> (String, Vec<String>) {
    match input_type {
        crate::services::plan_mode::types::ClarificationInputType::Text => {
            ("text".to_string(), Vec::new())
        }
        crate::services::plan_mode::types::ClarificationInputType::Textarea => {
            ("textarea".to_string(), Vec::new())
        }
        crate::services::plan_mode::types::ClarificationInputType::SingleSelect(options) => {
            ("single_select".to_string(), options.clone())
        }
        crate::services::plan_mode::types::ClarificationInputType::Boolean => {
            ("boolean".to_string(), Vec::new())
        }
    }
}

fn map_pending_clarification(
    question: Option<&crate::services::plan_mode::types::ClarificationQuestion>,
) -> Option<PlanClarificationSnapshot> {
    question.map(|value| {
        let (input_type, options) = map_clarification_input_type(&value.input_type);
        PlanClarificationSnapshot {
            question_id: value.question_id.clone(),
            question: value.question.clone(),
            hint: value.hint.clone(),
            input_type,
            options,
            required: false,
        }
    })
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

async fn sync_kernel_plan_snapshot_and_emit(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    session: &PlanModeSession,
    source: &str,
) {
    let phase = Some(plan_phase_to_kernel_phase(session.phase).to_string());
    let pending = map_pending_clarification(session.current_question.as_ref());
    let kernel_session_ids = kernel_state
        .sync_plan_snapshot_by_linked_session(&session.session_id, phase, pending)
        .await
        .unwrap_or_default();
    emit_kernel_updates(app, kernel_state, &kernel_session_ids, source).await;
}

// ============================================================================
// Commands
// ============================================================================

/// Enter plan mode: create a session and run domain analysis.
#[tauri::command]
pub async fn enter_plan_mode(
    description: String,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    project_path: Option<String>,
    context_sources: Option<ContextSourceConfig>,
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Create initial session
    let mut session = PlanModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        phase: PlanModePhase::Analyzing,
        analysis: None,
        clarifications: vec![],
        current_question: None,
        plan: None,
        step_outputs: HashMap::new(),
        step_states: HashMap::new(),
        progress: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let session_id = session.session_id.clone();
    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;

    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR)),
        result = async {
            // Run analysis if provider is specified
            if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
                let llm_provider = resolve_llm_provider(prov, mdl, None, base_url.clone(), &app_state)
                    .await
                    .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

                let registry = state.adapter_registry.read().await;

                let locale_tag = normalize_locale(locale.as_deref());
                let lang_instruction = locale_instruction(locale_tag);
                let plan_context = build_plan_conversation_context(
                    &app_state,
                    &knowledge_state,
                    project_path.as_deref(),
                    Some(session_id.as_str()),
                    conversation_context.as_deref(),
                    context_sources.as_ref(),
                    &description,
                    InjectionPhase::Planning,
                )
                .await;
                let plan_context_ref = if plan_context.is_empty() {
                    None
                } else {
                    Some(plan_context.as_str())
                };

                match crate::services::plan_mode::analyzer::analyze_task(
                    &description,
                    plan_context_ref,
                    lang_instruction,
                    llm_provider.clone(),
                    &registry,
                )
                .await
                {
                    Ok(analysis) => {
                        if analysis.needs_clarification {
                            // Generate first clarification question
                            let adapter = registry
                                .get(&analysis.adapter_name)
                                .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

                            match crate::services::plan_mode::clarifier::generate_clarification_question(
                                &description,
                                &analysis,
                                &[],
                                plan_context_ref,
                                lang_instruction,
                                adapter.as_ref(),
                                llm_provider,
                            )
                            .await
                            {
                                Ok(Some(question)) => {
                                    session.current_question = Some(question);
                                    session.phase = PlanModePhase::Clarifying;
                                }
                                _ => {
                                    // Question generation failed or returned None — skip to planning
                                    session.phase = PlanModePhase::Planning;
                                }
                            }
                        } else {
                            session.phase = PlanModePhase::Planning;
                        }
                        session.analysis = Some(analysis);
                    }
                    Err(e) => {
                        // Analysis failed, but we can still proceed without it
                        session.phase = PlanModePhase::Planning;
                        session.analysis = Some(PlanAnalysis {
                            domain: crate::services::plan_mode::types::TaskDomain::General,
                            complexity: 5,
                            estimated_steps: 4,
                            needs_clarification: false,
                            reasoning: format!("Analysis failed: {e}. Proceeding with general approach."),
                            adapter_name: "general".to_string(),
                            suggested_approach: "Standard decomposition".to_string(),
                        });
                    }
                }
            } else {
                // No provider specified — skip analysis
                session.phase = PlanModePhase::Planning;
                session.analysis = Some(PlanAnalysis {
                    domain: crate::services::plan_mode::types::TaskDomain::General,
                    complexity: 5,
                    estimated_steps: 4,
                    needs_clarification: false,
                    reasoning: "No LLM provider configured for analysis. Using defaults.".to_string(),
                    adapter_name: "general".to_string(),
                    suggested_approach: "Standard decomposition".to_string(),
                });
            }

            // Store session
            {
                let mut sessions = state.sessions.write().await;
                sessions.insert(session.session_id.clone(), session.clone());
            }

            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &session,
                "plan_mode.enter_plan_mode",
            )
            .await;

            Ok(CommandResponse::ok(session))
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;
    result
}

/// Submit a clarification answer and generate next question.
#[tauri::command]
pub async fn submit_plan_clarification(
    session_id: String,
    answer: ClarificationAnswer,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    project_path: Option<String>,
    context_sources: Option<ContextSourceConfig>,
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    // Snapshot data needed for question generation.
    let (description, analysis, mut clarifications, adapter_name, current_question_text) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::Clarifying {
            return Ok(CommandResponse::err("Not in clarifying phase"));
        }

        let analysis = session
            .analysis
            .clone()
            .ok_or_else(|| "No analysis available".to_string())?;

        (
            session.description.clone(),
            analysis,
            session.clarifications.clone(),
            session
                .analysis
                .as_ref()
                .map(|a| a.adapter_name.clone())
                .unwrap_or_default(),
            session
                .current_question
                .as_ref()
                .map(|q| q.question.clone()),
        )
    };

    let mut enriched_answer = answer;
    if let Some(question_text) = current_question_text {
        enriched_answer.question_text = question_text;
    }
    clarifications.push(enriched_answer);

    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;
    let next_question_result = tokio::select! {
        _ = operation_token.cancelled() => Err(PLAN_OPERATION_CANCELLED_ERROR.to_string()),
        result = async {
            if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
                let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
                    .await
                    .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

                let registry = state.adapter_registry.read().await;
                let adapter = registry
                    .get(&adapter_name)
                    .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

                let locale_tag = normalize_locale(locale.as_deref());
                let lang_instruction = locale_instruction(locale_tag);
                let plan_context = build_plan_conversation_context(
                    &app_state,
                    &knowledge_state,
                    project_path.as_deref(),
                    Some(session_id.as_str()),
                    conversation_context.as_deref(),
                    context_sources.as_ref(),
                    &description,
                    InjectionPhase::Planning,
                )
                .await;
                let plan_context_ref = if plan_context.is_empty() {
                    None
                } else {
                    Some(plan_context.as_str())
                };

                Ok(
                    crate::services::plan_mode::clarifier::generate_clarification_question(
                        &description,
                        &analysis,
                        &clarifications,
                        plan_context_ref,
                        lang_instruction,
                        adapter.as_ref(),
                        llm_provider,
                    )
                    .await
                    .unwrap_or(None),
                )
            } else {
                Ok(None)
            }
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;

    let next_question = match next_question_result {
        Ok(question) => question,
        Err(e) if e == PLAN_OPERATION_CANCELLED_ERROR => {
            return Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR));
        }
        Err(e) => return Err(e),
    };

    // Apply clarification only if operation completed.
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    if session.phase != PlanModePhase::Clarifying {
        return Ok(CommandResponse::err("Not in clarifying phase"));
    }

    session.clarifications = clarifications;
    match next_question {
        Some(q) => {
            session.current_question = Some(q);
        }
        None => {
            session.current_question = None;
            session.phase = PlanModePhase::Planning;
        }
    }

    let updated_session = session.clone();
    drop(sessions);

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &updated_session,
        "plan_mode.submit_plan_clarification",
    )
    .await;

    Ok(CommandResponse::ok(updated_session))
}

/// Skip clarification and proceed to planning.
#[tauri::command]
pub async fn skip_plan_clarification(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    session.phase = PlanModePhase::Planning;
    let result = session.clone();
    drop(sessions);

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &result,
        "plan_mode.skip_plan_clarification",
    )
    .await;

    Ok(CommandResponse::ok(result))
}

/// Generate a plan using LLM decomposition.
#[tauri::command]
pub async fn generate_plan(
    session_id: String,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    project_path: Option<String>,
    context_sources: Option<ContextSourceConfig>,
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<Plan>, String> {
    // Extract session data
    let (description, domain, adapter_name, clarifications) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        let analysis = session
            .analysis
            .as_ref()
            .ok_or_else(|| "No analysis result available".to_string())?;

        (
            session.description.clone(),
            analysis.domain.clone(),
            analysis.adapter_name.clone(),
            session.clarifications.clone(),
        )
    };

    let (prov, mdl) = match (&provider, &model) {
        (Some(p), Some(m)) => (p.as_str(), m.as_str()),
        _ => return Ok(CommandResponse::err("Provider and model are required")),
    };
    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR)),
        result = async {
            let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
                .await
                .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

            let registry = state.adapter_registry.read().await;
            let adapter = registry
                .get(&adapter_name)
                .unwrap_or_else(|| registry.find_for_domain(&domain));

            let locale_tag = normalize_locale(locale.as_deref());
            let lang_instruction = locale_instruction(locale_tag);
            let plan_context = build_plan_conversation_context(
                &app_state,
                &knowledge_state,
                project_path.as_deref(),
                Some(session_id.as_str()),
                conversation_context.as_deref(),
                context_sources.as_ref(),
                &description,
                InjectionPhase::Planning,
            )
            .await;
            let plan_context_ref = if plan_context.is_empty() {
                None
            } else {
                Some(plan_context.as_str())
            };

            match crate::services::plan_mode::planner::generate_plan(
                &description,
                &domain,
                adapter,
                &clarifications,
                plan_context_ref,
                lang_instruction,
                llm_provider,
            )
            .await
            {
                Ok(plan) => {
                    // Update session
                    let updated_session = {
                    let mut sessions = state.sessions.write().await;
                    let session = sessions
                        .get_mut(&session_id)
                        .ok_or_else(|| "No active plan mode session".to_string())?;
                    session.plan = Some(plan.clone());
                    session.phase = PlanModePhase::ReviewingPlan;
                        session.clone()
                    };

                    sync_kernel_plan_snapshot_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        &updated_session,
                        "plan_mode.generate_plan",
                    )
                    .await;

                    Ok(CommandResponse::ok(plan))
                }
                Err(e) => Ok(CommandResponse::err(format!("Plan generation failed: {e}"))),
            }
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;
    result
}

/// Approve the plan and start execution.
#[tauri::command]
pub async fn approve_plan(
    session_id: String,
    plan: Plan,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    project_path: Option<String>,
    context_sources: Option<ContextSourceConfig>,
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    standalone_state: tauri::State<'_, crate::commands::standalone::StandaloneState>,
    permission_state: tauri::State<'_, crate::commands::permissions::PermissionState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<bool>, String> {
    // Validate
    let (adapter_name, task_description) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::ReviewingPlan {
            return Ok(CommandResponse::err("Not in reviewing phase"));
        }

        (plan.adapter_name.clone(), session.description.clone())
    };

    let (prov, mdl) = match (&provider, &model) {
        (Some(p), Some(m)) => (p.as_str(), m.as_str()),
        _ => return Ok(CommandResponse::err("Provider and model are required")),
    };

    let provider_config = resolve_provider_config(prov, mdl, None, base_url.clone(), &app_state)
        .await
        .map_err(|e| format!("Failed to resolve provider config: {e}"))?;

    let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

    let registry = state.adapter_registry.read().await;
    let adapter = registry
        .get(&adapter_name)
        .unwrap_or_else(|| registry.find_for_domain(&plan.domain));
    drop(registry);

    // Update session to executing
    let executing_session_snapshot = {
        let mut sessions = state.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;
        session.plan = Some(plan.clone());
        session.phase = PlanModePhase::Executing;
        session.clone()
    };

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &executing_session_snapshot,
        "plan_mode.approve_plan.executing",
    )
    .await;

    // Set cancellation token
    let cancel_token = CancellationToken::new();
    {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.insert(session_id.clone(), cancel_token.clone());
    }

    // Spawn execution as background task
    let sessions_arc = state.sessions.clone();
    let tokens_arc = state.cancellation_tokens.clone();
    let sid = session_id.clone();
    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag).to_string();
    let execution_context = build_plan_conversation_context(
        &app_state,
        &knowledge_state,
        project_path.as_deref(),
        Some(session_id.as_str()),
        conversation_context.as_deref(),
        context_sources.as_ref(),
        &task_description,
        InjectionPhase::Implementation,
    )
    .await;
    let execution_context = if execution_context.is_empty() {
        None
    } else {
        Some(execution_context)
    };

    let resolved_project_root = match project_path.as_deref().map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => standalone_state.working_directory.read().await.clone(),
    };
    let resolved_project_path = resolved_project_root.to_string_lossy().to_string();

    let (index_store, embedding_service, embedding_manager, hnsw_index) = {
        let manager_guard = standalone_state.index_manager.read().await;
        if let Some(manager) = &*manager_guard {
            (
                Some(manager.index_store_arc()),
                manager.get_embedding_service(&resolved_project_path).await,
                manager.get_embedding_manager(&resolved_project_path).await,
                manager.get_hnsw_index(&resolved_project_path).await,
            )
        } else {
            (None, None, None, None)
        }
    };

    let step_runtime = crate::services::plan_mode::step_executor::StepExecutionRuntime {
        provider_config,
        project_root: resolved_project_root,
        index_store,
        embedding_service,
        embedding_manager,
        hnsw_index,
        permission_gate: Some(permission_state.gate.clone()),
        search_provider: Some(resolve_search_provider_for_tools()),
    };

    tokio::spawn(async move {
        let config = crate::services::plan_mode::step_executor::StepExecutionConfig::default();

        let mut plan_mut = plan;
        let app_for_execute = app_handle.clone();

        let result = crate::services::plan_mode::step_executor::execute_plan(
            &sid,
            &mut plan_mut,
            adapter,
            llm_provider,
            Some(step_runtime),
            config,
            execution_context,
            lang_instruction,
            app_for_execute,
            cancel_token,
        )
        .await;

        // Update session with results
        let mut updated_session_snapshot: Option<PlanModeSession> = None;
        let mut sessions = sessions_arc.write().await;
        if let Some(session) = sessions.get_mut(&sid) {
            match result {
                Ok((outputs, states)) => {
                    let failed = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Failed { .. }));
                    let cancelled = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Cancelled));

                    session.step_outputs = outputs;
                    session.step_states = states;
                    session.plan = Some(plan_mut);

                    if cancelled {
                        session.phase = PlanModePhase::Cancelled;
                    } else if failed {
                        session.phase = PlanModePhase::Failed;
                    } else {
                        session.phase = PlanModePhase::Completed;
                    }
                }
                Err(e) => {
                    session.phase = PlanModePhase::Failed;
                    // Store error in a synthetic step state
                    session.step_states.insert(
                        "_error".to_string(),
                        StepExecutionState::Failed {
                            reason: format!("{e}"),
                        },
                    );
                }
            }
            updated_session_snapshot = Some(session.clone());
        }
        drop(sessions);

        if let Some(updated_session) = updated_session_snapshot {
            let kernel_state = app_handle.state::<WorkflowKernelState>();
            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &updated_session,
                "plan_mode.approve_plan.completed",
            )
            .await;
        }

        // Clear cancellation token
        let mut tokens = tokens_arc.write().await;
        tokens.remove(&sid);
    });

    Ok(CommandResponse::ok(true))
}

/// Get current execution status.
#[tauri::command]
pub async fn get_plan_execution_status(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionStatusResponse>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    let total_steps = session.plan.as_ref().map_or(0, |p| p.steps.len());
    let total_batches = session.plan.as_ref().map_or(0, |p| p.batches.len());

    let steps_completed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Failed { .. }))
        .count();

    Ok(CommandResponse::ok(PlanExecutionStatusResponse {
        session_id: session.session_id.clone(),
        phase: session.phase,
        total_steps,
        steps_completed,
        steps_failed,
        total_batches,
        progress_pct: if total_steps > 0 {
            (steps_completed as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        },
    }))
}

/// Cancel plan execution.
#[tauri::command]
pub async fn cancel_plan_execution(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    // Verify session
    {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id);
        if session.is_none() {
            return Ok(CommandResponse::err("No active plan mode session"));
        }
    }

    // Cancel via token
    let ct_guard = state.cancellation_tokens.read().await;
    if let Some(token) = ct_guard.get(&session_id) {
        token.cancel();
    } else {
        return Ok(CommandResponse::err("No execution in progress to cancel"));
    }

    Ok(CommandResponse::ok(true))
}

/// Cancel a running plan pre-execution operation (analysis/clarification/planning).
#[tauri::command]
pub async fn cancel_plan_operation(
    session_id: Option<String>,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut cancelled_any = false;
    let tokens = state.operation_cancellation_tokens.read().await;

    match session_id {
        Some(sid) => {
            if let Some((_, token)) = tokens.get(&sid) {
                token.cancel();
                cancelled_any = true;
            }
        }
        None => {
            for (_, token) in tokens.values() {
                token.cancel();
                cancelled_any = true;
            }
        }
    }

    if cancelled_any {
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(
            "No plan operation in progress to cancel",
        ))
    }
}

/// Get the final execution report.
#[tauri::command]
pub async fn get_plan_execution_report(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionReport>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    let plan = session
        .plan
        .as_ref()
        .ok_or_else(|| "No plan generated".to_string())?;

    let total_steps = plan.steps.len();
    let steps_completed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Failed { .. }))
        .count();

    let total_duration_ms: u64 = session
        .step_states
        .values()
        .filter_map(|s| match s {
            StepExecutionState::Completed { duration_ms } => Some(*duration_ms),
            _ => None,
        })
        .sum();

    let step_summaries: HashMap<String, String> = session
        .step_outputs
        .iter()
        .map(|(id, output)| {
            let summary = if output.content.len() > 200 {
                format!("{}...", &output.content[..200])
            } else {
                output.content.clone()
            };
            (id.clone(), summary)
        })
        .collect();

    Ok(CommandResponse::ok(PlanExecutionReport {
        session_id: session.session_id.clone(),
        plan_title: plan.title.clone(),
        success: steps_failed == 0 && steps_completed == total_steps,
        total_steps,
        steps_completed,
        steps_failed,
        total_duration_ms,
        step_summaries,
    }))
}

/// Get a single step's output.
#[tauri::command]
pub async fn get_step_output(
    session_id: String,
    step_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<StepOutput>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    match session.step_outputs.get(&step_id) {
        Some(output) => Ok(CommandResponse::ok(output.clone())),
        None => Ok(CommandResponse::err(format!(
            "No output for step '{}'",
            step_id
        ))),
    }
}

/// Exit plan mode and clean up.
#[tauri::command]
pub async fn exit_plan_mode(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    let removed_session = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&session_id).is_some()
    };
    if !removed_session {
        return Ok(CommandResponse::err("No active plan mode session"));
    }

    let removed_token = {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.remove(&session_id)
    };
    if let Some(token) = removed_token {
        token.cancel();
    }

    let removed_operation_token = {
        let mut tokens = state.operation_cancellation_tokens.write().await;
        tokens.remove(&session_id)
    };
    if let Some((_, token)) = removed_operation_token {
        token.cancel();
    }

    Ok(CommandResponse::ok(true))
}

/// List available domain adapters.
#[tauri::command]
pub async fn list_plan_adapters(
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<Vec<AdapterInfo>>, String> {
    let registry = state.adapter_registry.read().await;
    Ok(CommandResponse::ok(registry.list()))
}

fn default_plan_context_sources() -> ContextSourceConfig {
    ContextSourceConfig {
        project_id: "default".to_string(),
        knowledge: None,
        memory: Some(MemorySourceConfig {
            enabled: true,
            selected_categories: vec![],
            selected_memory_ids: vec![],
            excluded_memory_ids: vec![],
            selected_scopes: vec![],
            session_id: None,
            statuses: vec![],
            review_mode: None,
        }),
        skills: Some(SkillsSourceConfig {
            enabled: true,
            selected_skill_ids: vec![],
            selection_mode: crate::services::task_mode::context_provider::SkillSelectionMode::Auto,
        }),
    }
}

async fn build_plan_conversation_context(
    app_state: &AppState,
    knowledge_state: &crate::commands::knowledge::KnowledgeState,
    project_path: Option<&str>,
    session_id: Option<&str>,
    conversation_context: Option<&str>,
    context_sources: Option<&ContextSourceConfig>,
    query: &str,
    phase: InjectionPhase,
) -> String {
    let mut sections = Vec::new();

    if let Some(ctx) = conversation_context {
        let trimmed = ctx.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
        }
    }

    let Some(project_path) = project_path.map(str::trim).filter(|p| !p.is_empty()) else {
        return sections.join("\n\n");
    };

    let config = context_sources
        .cloned()
        .unwrap_or_else(default_plan_context_sources);
    let knowledge_requested = config
        .knowledge
        .as_ref()
        .map(|k| k.enabled)
        .unwrap_or(false);

    let context_phase = match phase {
        InjectionPhase::Planning => "planning",
        InjectionPhase::Implementation => "implementation",
        InjectionPhase::Retry => "implementation",
        InjectionPhase::Always => "analysis",
    };
    let request = crate::commands::context_v2::PrepareTurnContextV2Request {
        project_path: project_path.to_string(),
        query: query.to_string(),
        project_id: if config.project_id.trim().is_empty() {
            None
        } else {
            Some(config.project_id.clone())
        },
        session_id: session_id.map(|id| id.to_string()),
        mode: Some("plan".to_string()),
        turn_id: None,
        intent: None,
        phase: Some(context_phase.to_string()),
        conversation_history: Vec::new(),
        context_sources: Some(config.clone()),
        rules: Vec::new(),
        manual_blocks: Vec::new(),
        input_token_budget: None,
        reserved_output_tokens: None,
        hard_limit: None,
        compaction_policy: None,
        fault_injection: None,
        enforce_user_skill_selection: true,
    };
    let assembled = crate::commands::context_v2::assemble_turn_context_internal(
        request,
        app_state,
        knowledge_state,
    )
    .await;
    let slices = match assembled {
        Ok(resp) => crate::commands::context_v2::split_assembly_into_slices(&resp),
        Err(err) => {
            tracing::warn!(
                "[plan_mode] Context V2 assembly failed, using empty context: {}",
                err
            );
            crate::commands::context_v2::ContextAssemblySlices::default()
        }
    };
    if knowledge_requested {
        let knowledge_hit = !slices.knowledge_block.is_empty();
        let _ = app_state
            .with_database(|db| observability::record_plan_knowledge(db, knowledge_hit))
            .await;
    }

    if !slices.knowledge_block.is_empty() {
        sections.push(format!(
            "[context-source] knowledge:retrieved\n{}",
            slices.knowledge_block
        ));
    }
    if !slices.memory_block.is_empty() {
        sections.push(slices.memory_block);
    }
    if !slices.skills_block.is_empty() {
        sections.push(slices.skills_block);
    }

    sections.join("\n\n")
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionStatusResponse {
    pub session_id: String,
    pub phase: PlanModePhase,
    pub total_steps: usize,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub total_batches: usize,
    pub progress_pct: f64,
}

// ============================================================================
// Locale Helpers
// ============================================================================

fn normalize_locale(locale: Option<&str>) -> &'static str {
    let normalized = locale.unwrap_or("en").to_lowercase();
    if normalized.starts_with("zh") {
        "zh"
    } else if normalized.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

pub(crate) fn locale_instruction(locale_tag: &str) -> &'static str {
    match locale_tag {
        "zh" => {
            "CRITICAL: Your final answer MUST be in Simplified Chinese. Keep code symbols, identifiers, and file paths unchanged."
        }
        "ja" => {
            "CRITICAL: Your final answer MUST be in Japanese. Keep code symbols, identifiers, and file paths unchanged."
        }
        _ => "CRITICAL: Your final answer MUST be in English. Keep code symbols, identifiers, and file paths unchanged.",
    }
}
