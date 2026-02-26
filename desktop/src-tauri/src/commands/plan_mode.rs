//! Plan Mode Tauri Commands
//!
//! Provides the complete Plan Mode lifecycle as Tauri commands:
//! - enter/exit plan mode
//! - submit clarifications
//! - generate/approve plan
//! - execution status/cancel/report

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::commands::task_mode::resolve_llm_provider;
use crate::models::CommandResponse;
use crate::services::plan_mode::adapter_registry::{AdapterInfo, AdapterRegistry};
use crate::services::plan_mode::types::{
    ClarificationAnswer, Plan, PlanAnalysis, PlanExecutionProgress, PlanExecutionReport,
    PlanModePhase, PlanModeSession, StepExecutionState, StepOutput,
};
use crate::state::AppState;

// ============================================================================
// State
// ============================================================================

/// Managed state for Plan Mode.
pub struct PlanModeState {
    session: Arc<RwLock<Option<PlanModeSession>>>,
    cancellation_token: Arc<RwLock<Option<CancellationToken>>>,
    adapter_registry: Arc<RwLock<AdapterRegistry>>,
}

impl PlanModeState {
    pub fn new() -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
            cancellation_token: Arc::new(RwLock::new(None)),
            adapter_registry: Arc::new(RwLock::new(AdapterRegistry::with_builtins())),
        }
    }
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
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<PlanModeSession>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Check if already in plan mode
    {
        let session = state.session.read().await;
        if let Some(ref s) = *session {
            if !matches!(
                s.phase,
                PlanModePhase::Completed | PlanModePhase::Failed | PlanModePhase::Cancelled
            ) {
                return Ok(CommandResponse::err(format!(
                    "Already in plan mode (session: {}). Exit first.",
                    s.session_id
                )));
            }
        }
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

    // Run analysis if provider is specified
    if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
        let llm_provider = resolve_llm_provider(
            prov,
            mdl,
            None,
            base_url.clone(),
            &app_state,
        )
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

        let registry = state.adapter_registry.read().await;

        let locale_tag = normalize_locale(locale.as_deref());
        let lang_instruction = locale_instruction(locale_tag);

        match crate::services::plan_mode::analyzer::analyze_task(
            &description,
            conversation_context.as_deref(),
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
                        conversation_context.as_deref(),
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
        let mut s = state.session.write().await;
        *s = Some(session.clone());
    }

    Ok(CommandResponse::ok(session))
}

/// Submit a clarification answer and generate next question.
#[tauri::command]
pub async fn submit_plan_clarification(
    session_id: String,
    answer: ClarificationAnswer,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<PlanModeSession>, String> {
    // Phase 1: Store the answer and extract data needed for question generation
    let (description, analysis, clarifications, adapter_name) = {
        let mut session_guard = state.session.write().await;
        let session = session_guard
            .as_mut()
            .filter(|s| s.session_id == session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::Clarifying {
            return Ok(CommandResponse::err("Not in clarifying phase"));
        }

        // Enrich answer with question text from current_question
        let mut enriched_answer = answer;
        if let Some(ref cq) = session.current_question {
            enriched_answer.question_text = cq.question.clone();
        }

        session.clarifications.push(enriched_answer);
        session.current_question = None;

        let analysis = session.analysis.clone()
            .ok_or_else(|| "No analysis available".to_string())?;

        (
            session.description.clone(),
            analysis,
            session.clarifications.clone(),
            session.analysis.as_ref().map(|a| a.adapter_name.clone()).unwrap_or_default(),
        )
    };

    // Phase 2: Generate next question (requires LLM call, done outside session lock)
    let next_question = if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
        let llm_provider = resolve_llm_provider(
            prov,
            mdl,
            None,
            base_url,
            &app_state,
        )
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

        let registry = state.adapter_registry.read().await;
        let adapter = registry
            .get(&adapter_name)
            .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

        let locale_tag = normalize_locale(locale.as_deref());
        let lang_instruction = locale_instruction(locale_tag);

        crate::services::plan_mode::clarifier::generate_clarification_question(
            &description,
            &analysis,
            &clarifications,
            None,
            lang_instruction,
            adapter.as_ref(),
            llm_provider,
        )
        .await
        .unwrap_or(None)
    } else {
        None
    };

    // Phase 3: Update session with next question or transition to planning
    let mut session_guard = state.session.write().await;
    let session = session_guard
        .as_mut()
        .filter(|s| s.session_id == session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    match next_question {
        Some(q) => {
            session.current_question = Some(q);
            // Stay in Clarifying phase
        }
        None => {
            // No more questions — transition to Planning
            session.current_question = None;
            session.phase = PlanModePhase::Planning;
        }
    }

    let result = session.clone();
    Ok(CommandResponse::ok(result))
}

/// Skip clarification and proceed to planning.
#[tauri::command]
pub async fn skip_plan_clarification(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let mut session_guard = state.session.write().await;
    let session = session_guard
        .as_mut()
        .filter(|s| s.session_id == session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    session.phase = PlanModePhase::Planning;
    let result = session.clone();

    Ok(CommandResponse::ok(result))
}

/// Generate a plan using LLM decomposition.
#[tauri::command]
pub async fn generate_plan(
    session_id: String,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    conversation_context: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<Plan>, String> {
    // Extract session data
    let (description, domain, adapter_name, clarifications) = {
        let session_guard = state.session.read().await;
        let session = session_guard
            .as_ref()
            .filter(|s| s.session_id == session_id)
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

    let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

    let registry = state.adapter_registry.read().await;
    let adapter = registry
        .get(&adapter_name)
        .unwrap_or_else(|| registry.find_for_domain(&domain));

    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag);

    match crate::services::plan_mode::planner::generate_plan(
        &description,
        &domain,
        adapter,
        &clarifications,
        conversation_context.as_deref(),
        lang_instruction,
        llm_provider,
    )
    .await
    {
        Ok(plan) => {
            // Update session
            let mut session_guard = state.session.write().await;
            if let Some(session) = session_guard
                .as_mut()
                .filter(|s| s.session_id == session_id)
            {
                session.plan = Some(plan.clone());
                session.phase = PlanModePhase::ReviewingPlan;
            }

            Ok(CommandResponse::ok(plan))
        }
        Err(e) => Ok(CommandResponse::err(format!("Plan generation failed: {e}"))),
    }
}

/// Approve the plan and start execution.
#[tauri::command]
pub async fn approve_plan(
    session_id: String,
    plan: Plan,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    locale: Option<String>,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<bool>, String> {
    // Validate
    let adapter_name = {
        let session_guard = state.session.read().await;
        let session = session_guard
            .as_ref()
            .filter(|s| s.session_id == session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::ReviewingPlan {
            return Ok(CommandResponse::err("Not in reviewing phase"));
        }

        plan.adapter_name.clone()
    };

    let (prov, mdl) = match (&provider, &model) {
        (Some(p), Some(m)) => (p.as_str(), m.as_str()),
        _ => return Ok(CommandResponse::err("Provider and model are required")),
    };

    let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

    let registry = state.adapter_registry.read().await;
    let adapter = registry
        .get(&adapter_name)
        .unwrap_or_else(|| registry.find_for_domain(&plan.domain));
    drop(registry);

    // Update session to executing
    {
        let mut session_guard = state.session.write().await;
        if let Some(session) = session_guard
            .as_mut()
            .filter(|s| s.session_id == session_id)
        {
            session.plan = Some(plan.clone());
            session.phase = PlanModePhase::Executing;
        }
    }

    // Set cancellation token
    let cancel_token = CancellationToken::new();
    {
        let mut ct = state.cancellation_token.write().await;
        *ct = Some(cancel_token.clone());
    }

    // Spawn execution as background task
    let session_arc = state.session.clone();
    let ct_arc = state.cancellation_token.clone();
    let sid = session_id.clone();
    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag).to_string();

    tokio::spawn(async move {
        let config =
            crate::services::plan_mode::step_executor::StepExecutionConfig::default();

        let mut plan_mut = plan;

        let result = crate::services::plan_mode::step_executor::execute_plan(
            &sid,
            &mut plan_mut,
            adapter,
            llm_provider,
            config,
            lang_instruction,
            app_handle,
            cancel_token,
        )
        .await;

        // Update session with results
        let mut session_guard = session_arc.write().await;
        if let Some(session) = session_guard
            .as_mut()
            .filter(|s| s.session_id == sid)
        {
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
        }

        // Clear cancellation token
        let mut ct = ct_arc.write().await;
        *ct = None;
    });

    Ok(CommandResponse::ok(true))
}

/// Get current execution status.
#[tauri::command]
pub async fn get_plan_execution_status(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionStatusResponse>, String> {
    let session_guard = state.session.read().await;
    let session = session_guard
        .as_ref()
        .filter(|s| s.session_id == session_id)
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
        let session_guard = state.session.read().await;
        let session = session_guard
            .as_ref()
            .filter(|s| s.session_id == session_id);
        if session.is_none() {
            return Ok(CommandResponse::err("No active plan mode session"));
        }
    }

    // Cancel via token
    let ct_guard = state.cancellation_token.read().await;
    if let Some(ref token) = *ct_guard {
        token.cancel();
    }

    Ok(CommandResponse::ok(true))
}

/// Get the final execution report.
#[tauri::command]
pub async fn get_plan_execution_report(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionReport>, String> {
    let session_guard = state.session.read().await;
    let session = session_guard
        .as_ref()
        .filter(|s| s.session_id == session_id)
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
    let session_guard = state.session.read().await;
    let session = session_guard
        .as_ref()
        .filter(|s| s.session_id == session_id)
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
    // Cancel any running execution
    {
        let ct_guard = state.cancellation_token.read().await;
        if let Some(ref token) = *ct_guard {
            token.cancel();
        }
    }

    // Clear session
    {
        let mut session_guard = state.session.write().await;
        if let Some(ref s) = *session_guard {
            if s.session_id == session_id {
                *session_guard = None;
            }
        }
    }

    // Clear cancellation token
    {
        let mut ct = state.cancellation_token.write().await;
        *ct = None;
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
