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

fn default_model_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514".to_string(),
        "openai" => "gpt-4o".to_string(),
        "deepseek" => "deepseek-chat".to_string(),
        "glm" => "glm-5".to_string(),
        "qwen" => "qwen3-max".to_string(),
        "minimax" => "MiniMax-M2.5".to_string(),
        "ollama" => "qwen2.5-coder:14b".to_string(),
        _ => "claude-sonnet-4-20250514".to_string(),
    }
}

async fn resolve_plan_provider_and_model(
    provider: Option<String>,
    model: Option<String>,
    app_state: &tauri::State<'_, AppState>,
) -> (String, String) {
    let provider_from_db = app_state
        .with_database(|db| db.get_setting("llm_provider"))
        .await
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let resolved_provider = provider
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(provider_from_db)
        .unwrap_or_else(|| "anthropic".to_string());

    let canonical_provider =
        crate::commands::standalone::normalize_provider_name(&resolved_provider)
            .map(|value| value.to_string())
            .unwrap_or_else(|| resolved_provider.trim().to_ascii_lowercase());

    let model_from_db = app_state
        .with_database(|db| db.get_setting("llm_model"))
        .await
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let resolved_model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(model_from_db)
        .unwrap_or_else(|| default_model_for_provider(&canonical_provider));

    (canonical_provider, resolved_model)
}

// ============================================================================
// Request Payloads
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnterPlanModeRequest {
    pub description: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub conversation_context: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitPlanClarificationRequest {
    pub session_id: String,
    pub answer: ClarificationAnswer,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub conversation_context: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratePlanRequest {
    pub session_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub conversation_context: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovePlanRequest {
    pub session_id: String,
    pub plan: Plan,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub conversation_context: Option<String>,
    pub locale: Option<String>,
}

// ============================================================================
// Commands
// ============================================================================

pub mod lifecycle_reporting_commands;
pub mod planning_execution_commands;
pub mod session_analysis_commands;

pub use lifecycle_reporting_commands::{
    cancel_plan_execution, cancel_plan_operation, exit_plan_mode, get_plan_execution_report,
    get_plan_execution_status, get_step_output, list_plan_adapters,
};
pub use planning_execution_commands::{approve_plan, generate_plan};
pub use session_analysis_commands::{
    enter_plan_mode, skip_plan_clarification, submit_plan_clarification,
};

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
            selection_mode: None,
        }),
        skills: Some(SkillsSourceConfig {
            enabled: true,
            selected_skill_ids: vec![],
            selection_mode: crate::services::task_mode::context_provider::SkillSelectionMode::Auto,
        }),
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlanContextBundle {
    pub rendered_context: String,
    pub effective_skill_ids: Vec<String>,
    pub blocked_tools: Vec<String>,
    pub selection_reason: String,
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
) -> PlanContextBundle {
    let mut bundle = PlanContextBundle::default();
    let mut sections = Vec::new();

    if let Some(ctx) = conversation_context {
        let trimmed = ctx.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
        }
    }

    let Some(project_path) = project_path.map(str::trim).filter(|p| !p.is_empty()) else {
        bundle.rendered_context = sections.join("\n\n");
        return bundle;
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
        Ok(resp) => {
            bundle.effective_skill_ids = resp.diagnostics.effective_skill_ids.clone();
            bundle.blocked_tools = resp.diagnostics.blocked_tools.clone();
            bundle.selection_reason = resp.diagnostics.selection_reason.clone();
            crate::commands::context_v2::split_assembly_into_slices(&resp)
        }
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

    bundle.rendered_context = sections.join("\n\n");
    bundle
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

#[cfg(test)]
mod tests {
    use super::{
        ApprovePlanRequest, EnterPlanModeRequest, GeneratePlanRequest,
        SubmitPlanClarificationRequest,
    };

    #[test]
    fn enter_plan_mode_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<EnterPlanModeRequest>(serde_json::json!({
            "description": "desc",
            "provider": "openai",
            "model": "gpt-4o",
            "baseUrl": "https://api.example.com",
            "projectPath": "/tmp/project",
            "contextSources": null,
            "conversationContext": "ctx",
            "locale": "en-US"
        }))
        .expect("camelCase enter_plan_mode payload should deserialize");
        assert_eq!(ok.description, "desc");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy = serde_json::from_value::<EnterPlanModeRequest>(serde_json::json!({
            "description": "desc",
            "base_url": "https://api.example.com",
            "project_path": "/tmp/project"
        }));
        assert!(legacy.is_err(), "legacy snake_case keys must be rejected");
    }

    #[test]
    fn submit_plan_clarification_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<SubmitPlanClarificationRequest>(serde_json::json!({
            "sessionId": "sid-1",
            "answer": { "questionId": "q1", "answer": "yes", "skipped": false },
            "provider": "openai",
            "model": "gpt-4o",
            "baseUrl": "https://api.example.com",
            "projectPath": "/tmp/project",
            "contextSources": null,
            "conversationContext": "ctx",
            "locale": "en-US"
        }))
        .expect("camelCase submit_plan_clarification payload should deserialize");
        assert_eq!(ok.session_id, "sid-1");

        let legacy = serde_json::from_value::<SubmitPlanClarificationRequest>(serde_json::json!({
            "session_id": "sid-1",
            "answer": { "questionId": "q1", "answer": "yes", "skipped": false }
        }));
        assert!(legacy.is_err(), "legacy snake_case keys must be rejected");
    }

    #[test]
    fn generate_plan_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<GeneratePlanRequest>(serde_json::json!({
            "sessionId": "sid-1",
            "provider": "openai",
            "model": "gpt-4o",
            "baseUrl": "https://api.example.com",
            "projectPath": "/tmp/project",
            "contextSources": null,
            "conversationContext": "ctx",
            "locale": "en-US"
        }))
        .expect("camelCase generate_plan payload should deserialize");
        assert_eq!(ok.session_id, "sid-1");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy = serde_json::from_value::<GeneratePlanRequest>(serde_json::json!({
            "session_id": "sid-1",
            "project_path": "/tmp/project"
        }));
        assert!(legacy.is_err(), "legacy snake_case keys must be rejected");
    }

    #[test]
    fn approve_plan_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<ApprovePlanRequest>(serde_json::json!({
            "sessionId": "sid-1",
            "plan": {
                "title": "Plan",
                "description": "Desc",
                "domain": "general",
                "adapterName": "general",
                "steps": [],
                "batches": []
            },
            "provider": "openai",
            "model": "gpt-4o",
            "baseUrl": "https://api.example.com",
            "projectPath": "/tmp/project",
            "contextSources": null,
            "conversationContext": "ctx",
            "locale": "en-US"
        }))
        .expect("camelCase approve_plan payload should deserialize");
        assert_eq!(ok.session_id, "sid-1");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy = serde_json::from_value::<ApprovePlanRequest>(serde_json::json!({
            "session_id": "sid-1",
            "plan": {
                "title": "Plan",
                "description": "Desc",
                "domain": "general",
                "adapterName": "general",
                "steps": [],
                "batches": []
            }
        }));
        assert!(legacy.is_err(), "legacy snake_case keys must be rejected");
    }
}
