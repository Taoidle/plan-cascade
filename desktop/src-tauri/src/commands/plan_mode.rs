//! Plan Mode Tauri Commands
//!
//! Provides the complete Plan Mode lifecycle as Tauri commands:
//! - enter/exit plan mode
//! - submit clarifications
//! - generate/approve plan
//! - execution status/cancel/report

use std::collections::HashMap;
use std::fs;
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
    PlanRetryStats, StepExecutionState, StepOutput,
};
use crate::services::skills::model::InjectionPhase;
use crate::services::task_mode::context_provider::{
    ContextSourceConfig, MemorySourceConfig, SkillsSourceConfig,
};
use crate::services::workflow_kernel::{
    HandoffContextBundle, HandoffSummaryItem, PlanClarificationSnapshot, WorkflowKernelState,
    WorkflowKernelUpdatedEvent, WorkflowMode, WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
use crate::state::AppState;
use crate::utils::paths::ensure_plan_cascade_dir;
use tauri::{Emitter, Manager};

// ============================================================================
// State
// ============================================================================

/// Managed state for Plan Mode.
#[derive(Clone)]
pub struct PlanModeState {
    sessions: Arc<RwLock<HashMap<String, PlanModeSession>>>,
    cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    operation_cancellation_tokens: Arc<RwLock<HashMap<String, (String, CancellationToken)>>>,
    adapter_registry: Arc<RwLock<AdapterRegistry>>,
    storage_root: Arc<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanModeSessionRecordV1 {
    version: u32,
    session: PlanModeSession,
}

const PLAN_MODE_SESSION_RECORD_VERSION: u32 = 2;

impl PlanModeState {
    pub fn new() -> Self {
        Self::new_with_storage_dir(resolve_plan_mode_storage_root())
    }

    pub fn new_with_storage_dir(storage_root: PathBuf) -> Self {
        let sessions_dir = storage_root.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            operation_cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            adapter_registry: Arc::new(RwLock::new(AdapterRegistry::with_builtins())),
            storage_root: Arc::new(storage_root),
        }
    }

    pub async fn get_session_snapshot(&self, session_id: &str) -> Option<PlanModeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    pub async fn get_or_load_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<PlanModeSession>, String> {
        if let Some(snapshot) = self.get_session_snapshot(session_id).await {
            return Ok(Some(snapshot));
        }

        match self.read_persisted_session(session_id).await {
            Ok(Some(snapshot)) => {
                {
                    let mut sessions = self.sessions.write().await;
                    sessions.insert(session_id.to_string(), snapshot.clone());
                }
                Ok(Some(snapshot))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                let _ = self.delete_persisted_session(session_id).await;
                Err(error)
            }
        }
    }

    pub async fn persist_session_snapshot(&self, session: &PlanModeSession) -> Result<(), String> {
        let record = PlanModeSessionRecordV1 {
            version: PLAN_MODE_SESSION_RECORD_VERSION,
            session: session.clone(),
        };
        let encoded = serde_json::to_vec_pretty(&record)
            .map_err(|e| format!("Failed to encode plan mode session: {e}"))?;
        fs::write(self.session_file_path(&session.session_id), encoded).map_err(|e| {
            format!(
                "Failed to persist plan mode session '{}': {e}",
                session.session_id
            )
        })
    }

    pub async fn store_session_snapshot(&self, session: PlanModeSession) -> Result<(), String> {
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.session_id.clone(), session.clone());
        }
        self.persist_session_snapshot(&session).await
    }

    pub async fn delete_persisted_session(&self, session_id: &str) -> Result<(), String> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(path).map_err(|e| {
            format!("Failed to delete persisted plan mode session '{session_id}': {e}")
        })
    }

    async fn read_persisted_session(
        &self,
        session_id: &str,
    ) -> Result<Option<PlanModeSession>, String> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read(&path).map_err(|e| {
            format!("Failed to read persisted plan mode session '{session_id}': {e}")
        })?;
        let record: PlanModeSessionRecordV1 = serde_json::from_slice(&raw)
            .map_err(|e| format!("Persisted plan mode session '{session_id}' is corrupted: {e}"))?;
        if record.version != PLAN_MODE_SESSION_RECORD_VERSION {
            return Err(format!(
                "Unsupported plan mode session record version {} for '{}'",
                record.version, session_id
            ));
        }
        Ok(Some(record.session))
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.storage_root
            .join("sessions")
            .join(format!("{session_id}.json"))
    }
}

fn resolve_plan_mode_storage_root() -> PathBuf {
    if let Ok(root) = ensure_plan_cascade_dir() {
        let path = root.join("plan-mode");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    let fallback = std::env::temp_dir().join("plan-cascade-plan-mode");
    let _ = fs::create_dir_all(&fallback);
    fallback
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

pub(crate) async fn persist_plan_session_best_effort(
    state: &PlanModeState,
    session: &PlanModeSession,
    source: &str,
) {
    if let Err(error) = state.persist_session_snapshot(session).await {
        eprintln!(
            "[plan_mode] failed to persist session '{}' at {}: {}",
            session.session_id, source, error
        );
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
        crate::services::plan_mode::types::ClarificationInputType::MultiSelect(options) => {
            ("multi_select".to_string(), options.clone())
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
            allow_custom: value.allow_custom,
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
    let running_step_id = session.step_states.iter().find_map(|(step_id, state)| {
        if matches!(state, StepExecutionState::Running) {
            Some(step_id.clone())
        } else {
            None
        }
    });
    let kernel_session_ids = kernel_state
        .sync_plan_snapshot_by_linked_session(
            &session.session_id,
            phase,
            pending,
            running_step_id,
            None,
        )
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

pub(crate) async fn resolve_plan_provider_and_model(
    provider: Option<String>,
    model: Option<String>,
    app_state: &AppState,
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
    pub kernel_session_id: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryPlanStepRequest {
    pub session_id: String,
    pub step_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<ContextSourceConfig>,
    pub conversation_context: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionResumePayload {
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
pub use planning_execution_commands::{approve_plan, generate_plan, retry_plan_step};
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
    plan_state: Option<&PlanModeState>,
    kernel_state: &WorkflowKernelState,
    project_path: Option<&str>,
    session_id: Option<&str>,
    kernel_session_id: Option<&str>,
    conversation_context: Option<&str>,
    context_sources: Option<&ContextSourceConfig>,
    query: &str,
    phase: InjectionPhase,
) -> PlanContextBundle {
    let mut bundle = PlanContextBundle::default();
    let mut sections = Vec::new();

    let kernel_conversation_context = if conversation_context
        .map(str::trim)
        .filter(|ctx| !ctx.is_empty())
        .is_some()
    {
        None
    } else if let Some(kernel_session_id) = kernel_session_id {
        let entry_handoff = kernel_state
            .mode_entry_handoff_for_kernel_session(kernel_session_id, WorkflowMode::Plan)
            .await
            .unwrap_or_default();
        if is_handoff_context_empty(&entry_handoff) {
            kernel_state
                .handoff_context_for_kernel_session(kernel_session_id)
                .await
                .map(|handoff| render_plan_handoff_context(&handoff))
        } else {
            Some(render_plan_handoff_context(&entry_handoff))
        }
    } else if let (Some(plan_state), Some(mode_session_id)) = (plan_state, session_id) {
        handoff_context_for_plan_session(kernel_state, plan_state, mode_session_id)
            .await
            .map(|handoff| render_plan_handoff_context(&handoff))
    } else {
        None
    };

    if let Some(ctx) = conversation_context.or(kernel_conversation_context.as_deref()) {
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

fn render_plan_handoff_context(
    handoff: &HandoffContextBundle,
) -> String {
    let mut sections = Vec::new();

    let conversation_section = handoff
        .conversation_context
        .iter()
        .map(|turn| {
            format!(
                "user: {}\nassistant: {}",
                turn.user.trim(),
                turn.assistant.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if !conversation_section.trim().is_empty() {
        sections.push(conversation_section);
    }

    if !handoff.summary_items.is_empty() {
        let rendered = handoff
            .summary_items
            .iter()
            .map(|item| {
                format!(
                    "## [{}:{}] {}\n{}",
                    mode_label(item.source_mode),
                    item.kind,
                    item.title,
                    item.body
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !rendered.trim().is_empty() {
            sections.push(format!("[summary-items]\n{}", rendered));
        }
    }

    if !handoff.artifact_refs.is_empty() {
        sections.push(format!(
            "[artifact-refs]\n{}",
            handoff.artifact_refs.join("\n")
        ));
    }

    if !handoff.context_sources.is_empty() {
        sections.push(format!(
            "[context-sources]\n{}",
            handoff.context_sources.join("\n")
        ));
    }

    if !handoff.metadata.is_empty() {
        if let Ok(metadata) = serde_json::to_string_pretty(&handoff.metadata) {
            sections.push(format!("[handoff-metadata]\n{}", metadata));
        }
    }

    sections.join("\n\n")
}

fn is_handoff_context_empty(handoff: &HandoffContextBundle) -> bool {
    handoff.conversation_context.is_empty()
        && handoff.summary_items.is_empty()
        && handoff.artifact_refs.is_empty()
        && handoff.context_sources.is_empty()
        && handoff.metadata.is_empty()
}

pub(crate) async fn handoff_context_for_plan_session(
    kernel_state: &WorkflowKernelState,
    plan_state: &PlanModeState,
    plan_session_id: &str,
) -> Option<HandoffContextBundle> {
    let plan_session = plan_state
        .get_or_load_session_snapshot(plan_session_id)
        .await
        .ok()
        .flatten()?;
    let kernel_session_id = plan_session.kernel_session_id?;
    let entry_handoff = kernel_state
        .mode_entry_handoff_for_kernel_session(&kernel_session_id, WorkflowMode::Plan)
        .await
        .unwrap_or_default();
    if !is_handoff_context_empty(&entry_handoff) {
        return Some(entry_handoff);
    }
    kernel_state
        .handoff_context_for_kernel_session(&kernel_session_id)
        .await
}

pub(crate) async fn publish_plan_handoff_summary(
    kernel_state: &WorkflowKernelState,
    kernel_session_id: Option<&str>,
    summary_item: HandoffSummaryItem,
) {
    let Some(kernel_session_id) = kernel_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let handoff = HandoffContextBundle {
        summary_items: vec![summary_item],
        ..HandoffContextBundle::default()
    };
    let _ = kernel_state
        .append_context_items(kernel_session_id, WorkflowMode::Plan, handoff)
        .await;
}

pub(crate) fn build_plan_analysis_summary_item(session: &PlanModeSession) -> Option<HandoffSummaryItem> {
    let analysis = session.analysis.as_ref()?;
    let locale_tag = normalize_locale(session.locale.as_deref());
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "adapterName".to_string(),
        serde_json::Value::String(analysis.adapter_name.clone()),
    );
    metadata.insert(
        "estimatedSteps".to_string(),
        serde_json::Value::Number((analysis.estimated_steps as u64).into()),
    );
    metadata.insert(
        "needsClarification".to_string(),
        serde_json::Value::Bool(analysis.needs_clarification),
    );
    Some(HandoffSummaryItem {
        id: format!("plan-analysis-{}", session.session_id),
        source_mode: WorkflowMode::Plan,
        kind: "plan_analysis".to_string(),
        title: match locale_tag {
            "zh" => format!("{} 的计划分析", session.description),
            "ja" => format!("{} のプラン分析", session.description),
            _ => format!("Plan analysis for {}", session.description),
        },
        body: match locale_tag {
            "zh" => format!(
                "领域：{}\n复杂度：{}\n预计步骤数：{}\n是否需要澄清：{}\n建议方法：{}\n{}",
                analysis.domain,
                analysis.complexity,
                analysis.estimated_steps,
                analysis.needs_clarification,
                analysis.suggested_approach,
                analysis.reasoning
            ),
            "ja" => format!(
                "ドメイン: {}\n複雑度: {}\n推定ステップ数: {}\n確認が必要: {}\n推奨アプローチ: {}\n{}",
                analysis.domain,
                analysis.complexity,
                analysis.estimated_steps,
                analysis.needs_clarification,
                analysis.suggested_approach,
                analysis.reasoning
            ),
            _ => format!(
                "Domain: {}\nComplexity: {}\nEstimated steps: {}\nNeeds clarification: {}\nApproach: {}\n{}",
                analysis.domain,
                analysis.complexity,
                analysis.estimated_steps,
                analysis.needs_clarification,
                analysis.suggested_approach,
                analysis.reasoning
            ),
        },
        artifact_refs: Vec::new(),
        metadata,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub(crate) fn build_plan_clarification_summary_item(
    session: &PlanModeSession,
) -> Option<HandoffSummaryItem> {
    if session.clarifications.is_empty() {
        return None;
    }
    let locale_tag = normalize_locale(session.locale.as_deref());
    let clarification_lines = session
        .clarifications
        .iter()
        .map(|answer| {
            format!(
                "- {} => {}",
                answer.question_text.trim(),
                answer.answer.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "clarificationCount".to_string(),
        serde_json::Value::Number((session.clarifications.len() as u64).into()),
    );
    Some(HandoffSummaryItem {
        id: format!("plan-clarification-{}", session.session_id),
        source_mode: WorkflowMode::Plan,
        kind: "plan_clarification".to_string(),
        title: match locale_tag {
            "zh" => format!("{} 的计划澄清", session.description),
            "ja" => format!("{} のプラン確認", session.description),
            _ => format!("Plan clarifications for {}", session.description),
        },
        body: clarification_lines,
        artifact_refs: Vec::new(),
        metadata,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub(crate) fn build_plan_output_summary_item(session: &PlanModeSession) -> Option<HandoffSummaryItem> {
    let plan = session.plan.as_ref()?;
    let locale_tag = normalize_locale(session.locale.as_deref());
    let step_lines = plan
        .steps
        .iter()
        .map(|step| format!("- {}: {}", step.id, step.title))
        .collect::<Vec<_>>()
        .join("\n");
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "stepCount".to_string(),
        serde_json::Value::Number((plan.steps.len() as u64).into()),
    );
    metadata.insert(
        "batchCount".to_string(),
        serde_json::Value::Number((plan.batches.len() as u64).into()),
    );
    Some(HandoffSummaryItem {
        id: format!("plan-output-{}", session.session_id),
        source_mode: WorkflowMode::Plan,
        kind: "plan_output".to_string(),
        title: match locale_tag {
            "zh" => format!("{} 的计划输出", session.description),
            "ja" => format!("{} のプラン出力", session.description),
            _ => format!("Plan output for {}", session.description),
        },
        body: match locale_tag {
            "zh" => format!(
                "计划标题：{}\n步骤数：{}\n批次数：{}\n{}",
                plan.title,
                plan.steps.len(),
                plan.batches.len(),
                step_lines
            ),
            "ja" => format!(
                "プランタイトル: {}\nステップ数: {}\nバッチ数: {}\n{}",
                plan.title,
                plan.steps.len(),
                plan.batches.len(),
                step_lines
            ),
            _ => format!(
                "Plan title: {}\nSteps: {}\nBatches: {}\n{}",
                plan.title,
                plan.steps.len(),
                plan.batches.len(),
                step_lines
            ),
        },
        artifact_refs: Vec::new(),
        metadata,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub(crate) fn build_plan_execution_summary_item(session: &PlanModeSession) -> Option<HandoffSummaryItem> {
    let locale_tag = normalize_locale(session.locale.as_deref());
    let progress = session.progress.as_ref();
    let total_steps = progress
        .map(|value| value.total_steps)
        .or_else(|| session.plan.as_ref().map(|plan| plan.steps.len()))
        .unwrap_or_default();
    let steps_completed = progress.map(|value| value.steps_completed).unwrap_or_else(|| {
        session
            .step_states
            .values()
            .filter(|state| matches!(state, StepExecutionState::Completed { .. }))
            .count()
    });
    let steps_failed = progress.map(|value| value.steps_failed).unwrap_or_else(|| {
        session
            .step_states
            .values()
            .filter(|state| matches!(state, StepExecutionState::Failed { .. }))
            .count()
    });
    let terminal_state = match session.phase {
        PlanModePhase::Completed => "completed",
        PlanModePhase::Cancelled => "cancelled",
        PlanModePhase::Failed => "failed",
        _ => "partial",
    };
    let localized_terminal_state = localized_plan_terminal_state(locale_tag, terminal_state);
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "terminalState".to_string(),
        serde_json::Value::String(terminal_state.to_string()),
    );
    metadata.insert(
        "stepsCompleted".to_string(),
        serde_json::Value::Number((steps_completed as u64).into()),
    );
    metadata.insert(
        "stepsFailed".to_string(),
        serde_json::Value::Number((steps_failed as u64).into()),
    );
    metadata.insert(
        "totalSteps".to_string(),
        serde_json::Value::Number((total_steps as u64).into()),
    );
    Some(HandoffSummaryItem {
        id: format!("plan-execution-{}", session.session_id),
        source_mode: WorkflowMode::Plan,
        kind: "plan_execution".to_string(),
        title: match locale_tag {
            "zh" => format!("{} 的计划执行：{}", session.description, localized_terminal_state),
            "ja" => format!("{} のプラン実行: {}", session.description, localized_terminal_state),
            _ => format!("Plan execution {} for {}", localized_terminal_state, session.description),
        },
        body: match locale_tag {
            "zh" => format!(
                "执行状态：{}\n已完成步骤：{}\n失败步骤：{}\n总步骤数：{}",
                localized_terminal_state, steps_completed, steps_failed, total_steps
            ),
            "ja" => format!(
                "実行状態: {}\n完了ステップ: {}\n失敗ステップ: {}\n総ステップ数: {}",
                localized_terminal_state, steps_completed, steps_failed, total_steps
            ),
            _ => format!(
                "Execution state: {}\nCompleted steps: {}\nFailed steps: {}\nTotal steps: {}",
                localized_terminal_state, steps_completed, steps_failed, total_steps
            ),
        },
        artifact_refs: Vec::new(),
        metadata,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn mode_label(mode: WorkflowMode) -> &'static str {
    match mode {
        WorkflowMode::Chat => "chat",
        WorkflowMode::Plan => "plan",
        WorkflowMode::Task => "task",
    }
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

fn localized_plan_copy(
    locale_tag: &str,
    en: &'static str,
    zh: &'static str,
    ja: &'static str,
) -> &'static str {
    match locale_tag {
        "zh" => zh,
        "ja" => ja,
        _ => en,
    }
}

fn localized_plan_terminal_state(locale_tag: &str, terminal_state: &str) -> String {
    match terminal_state {
        "completed" => localized_plan_copy(locale_tag, "completed", "已完成", "完了").to_string(),
        "cancelled" => localized_plan_copy(locale_tag, "cancelled", "已取消", "キャンセル済み").to_string(),
        "failed" => localized_plan_copy(locale_tag, "failed", "失败", "失敗").to_string(),
        "partial" => localized_plan_copy(locale_tag, "partial", "部分完成", "部分完了").to_string(),
        _ => terminal_state.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn retry_plan_step_request_accepts_camel_case_only() {
        let ok = serde_json::from_value::<RetryPlanStepRequest>(serde_json::json!({
            "sessionId": "sid-1",
            "stepId": "step-2",
            "provider": "openai",
            "model": "gpt-4o",
            "baseUrl": "https://api.example.com",
            "projectPath": "/tmp/project",
            "contextSources": null,
            "conversationContext": "ctx",
            "locale": "en-US"
        }))
        .expect("camelCase retry_plan_step payload should deserialize");
        assert_eq!(ok.session_id, "sid-1");
        assert_eq!(ok.step_id, "step-2");
        assert_eq!(ok.project_path.as_deref(), Some("/tmp/project"));

        let legacy = serde_json::from_value::<RetryPlanStepRequest>(serde_json::json!({
            "session_id": "sid-1",
            "step_id": "step-2",
        }));
        assert!(legacy.is_err(), "legacy snake_case keys must be rejected");
    }

    fn sample_plan_mode_session(session_id: &str) -> PlanModeSession {
        PlanModeSession {
            session_id: session_id.to_string(),
            kernel_session_id: Some("kernel-session-1".to_string()),
            locale: Some("en-US".to_string()),
            description: "sample".to_string(),
            phase: PlanModePhase::ReviewingPlan,
            analysis: None,
            clarifications: Vec::new(),
            current_question: None,
            plan: None,
            step_outputs: HashMap::new(),
            step_states: HashMap::new(),
            step_attempts: HashMap::new(),
            progress: None,
            execution_resume_payload: None,
            created_at: "2026-03-05T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn plan_mode_session_persistence_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = PlanModeState::new_with_storage_dir(temp_dir.path().to_path_buf());

        let snapshot = sample_plan_mode_session("plan-session-roundtrip");
        state
            .store_session_snapshot(snapshot.clone())
            .await
            .expect("persist snapshot");

        let restored = state
            .get_or_load_session_snapshot("plan-session-roundtrip")
            .await
            .expect("load snapshot")
            .expect("snapshot should exist");
        assert_eq!(restored.session_id, snapshot.session_id);
        assert_eq!(restored.phase, snapshot.phase);
    }

    #[tokio::test]
    async fn plan_mode_session_corruption_is_reported_and_cleaned_up() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = PlanModeState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let session_id = "plan-session-corrupted";
        let path = state.session_file_path(session_id);
        std::fs::write(&path, b"{not json").expect("write corrupted file");

        let result = state.get_or_load_session_snapshot(session_id).await;
        assert!(result.is_err(), "corrupted record should return error");
        assert!(
            !path.exists(),
            "corrupted persisted record should be removed after failed decode"
        );
    }

    #[tokio::test]
    async fn plan_mode_exit_cleanup_deletes_persisted_file() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let state = PlanModeState::new_with_storage_dir(temp_dir.path().to_path_buf());
        let session_id = "plan-session-cleanup";
        let snapshot = sample_plan_mode_session(session_id);
        state
            .store_session_snapshot(snapshot)
            .await
            .expect("persist snapshot");

        state
            .delete_persisted_session(session_id)
            .await
            .expect("delete snapshot");
        assert!(
            !state.session_file_path(session_id).exists(),
            "persisted plan session should be removed"
        );
    }
}
