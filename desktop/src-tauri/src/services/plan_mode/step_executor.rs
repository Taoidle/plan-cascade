//! Step Executor
//!
//! Phase 5: Execute steps in dependency-resolved batches.
//! Runs steps in parallel within each batch using tokio tasks.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::services::analytics::send_message_tracked;
use crate::services::file_change_tracker::FileChangeTracker;
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{
    LlmRequestOptions, Message, MessageRole, ProviderConfig, ToolDefinition,
};
use crate::services::orchestrator::embedding_manager::EmbeddingManager;
use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::services::orchestrator::hnsw_index::HnswIndex;
use crate::services::orchestrator::index_store::IndexStore;
use crate::services::orchestrator::permission_gate::PermissionGate;
use crate::services::orchestrator::text_describes_pending_action;
use crate::services::orchestrator::{ExecutionKind, OrchestratorConfig, OrchestratorService};
use crate::services::skills::model::SkillMatch;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::definitions::get_tool_definitions_from_registry;
use crate::services::workflow_kernel::{
    WorkflowKernelState, WorkflowMode, WorkflowModeTranscriptUpdatedEvent,
    WorkflowSessionCatalogUpdatedEvent, WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL,
    WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL,
};
use crate::utils::error::{AppError, AppResult};

use super::adapter::DomainAdapter;
use super::types::{
    OutputFormat, Plan, PlanExecutionProgress, PlanExecutionReport, PlanModeProgressEvent,
    PlanRetryStats, PlanTerminalStatus, StepArtifactEvidence, StepEvidenceBundle,
    StepExecutionState, StepFileReadEvidence, StepOutcomeStatus, StepOutput,
    StepOutputQualityState, StepRuntimeStats, StepToolCallEvidence, StepValidationResult,
    PLAN_MODE_EVENT_CHANNEL,
};
use super::validator::{validate_step_output, validation_summary};

#[derive(Debug, Clone, Default)]
pub struct PlanExecutionResumeState {
    pub step_outputs: HashMap<String, StepOutput>,
    pub step_states: HashMap<String, StepExecutionState>,
    pub step_attempts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct PlanExecutionCheckpoint {
    pub step_outputs: HashMap<String, StepOutput>,
    pub step_states: HashMap<String, StepExecutionState>,
    pub step_attempts: HashMap<String, usize>,
    pub current_batch: usize,
    pub total_batches: usize,
}

pub type PlanExecutionProgressCallback =
    Arc<dyn Fn(PlanExecutionCheckpoint) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Configuration for step execution.
#[derive(Debug, Clone)]
pub struct StepExecutionConfig {
    /// Maximum parallel steps per batch
    pub max_parallel: usize,
    /// Optional cap for the derived soft limit of a single step execution.
    pub step_soft_limit_cap: u32,
    /// Maximum output tokens per context injection (~4000 chars per dep)
    pub max_dep_output_chars: usize,
    /// Total cap for all dependency outputs
    pub max_total_dep_chars: usize,
    /// Number of retries after the first attempt.
    pub max_retry_attempts: usize,
    /// Milliseconds to wait between retry rounds.
    pub retry_backoff_ms: u64,
    /// Whether to stop future batches after retry exhaustion.
    pub fail_batch_on_exhausted: bool,
}

impl Default for StepExecutionConfig {
    fn default() -> Self {
        Self {
            max_parallel: 4,
            step_soft_limit_cap: 96,
            max_dep_output_chars: 4000,
            max_total_dep_chars: 16000,
            max_retry_attempts: 2,
            retry_backoff_ms: 800,
            fail_batch_on_exhausted: true,
        }
    }
}

/// Shared runtime dependencies used to execute each step through orchestrator.
#[derive(Clone)]
pub struct StepExecutionRuntime {
    pub provider_config: ProviderConfig,
    pub project_root: PathBuf,
    pub kernel_session_id: Option<String>,
    pub mode_session_id: String,
    pub phase_id: String,
    pub file_change_tracker: Option<Arc<Mutex<FileChangeTracker>>>,
    pub index_store: Option<Arc<IndexStore>>,
    pub embedding_service: Option<Arc<EmbeddingService>>,
    pub embedding_manager: Option<Arc<EmbeddingManager>>,
    pub hnsw_index: Option<Arc<HnswIndex>>,
    pub permission_gate: Option<Arc<PermissionGate>>,
    pub search_provider: Option<(String, Option<String>)>,
    pub selected_skills: Vec<SkillMatch>,
    pub analytics_tx: Option<tokio::sync::mpsc::Sender<crate::services::analytics::TrackerMessage>>,
    pub analytics_cost_calculator: Option<Arc<crate::services::analytics::CostCalculator>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureCategory {
    IncompleteNarration,
    CriteriaUnmet,
    MaxIterations,
    InternalError,
}

impl FailureCategory {
    fn as_error_code(self) -> &'static str {
        match self {
            FailureCategory::IncompleteNarration => "incomplete_narration",
            FailureCategory::CriteriaUnmet => "criteria_unmet",
            FailureCategory::MaxIterations => "iteration_stalled",
            FailureCategory::InternalError => "internal_error",
        }
    }
}

fn normalize_tool_name(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_ascii_lowercase().replace('_', "");
    match normalized.as_str() {
        "read" | "readfile" => Some("Read"),
        "write" | "writefile" => Some("Write"),
        "edit" => Some("Edit"),
        "ls" => Some("LS"),
        "glob" => Some("Glob"),
        "grep" => Some("Grep"),
        "bash" => Some("Bash"),
        "websearch" => Some("WebSearch"),
        "webfetch" => Some("WebFetch"),
        "codebasesearch" => Some("CodebaseSearch"),
        "searchknowledge" => Some("SearchKnowledge"),
        "cwd" => Some("Cwd"),
        "browser" => Some("Browser"),
        "notebookedit" => Some("NotebookEdit"),
        "task" => Some("Task"),
        "analyze" => Some("Analyze"),
        _ => None,
    }
}

fn fallback_plan_tools() -> Vec<&'static str> {
    vec!["CodebaseSearch", "Read", "Grep", "LS", "Write", "WebSearch"]
}

fn resolve_step_tools(
    adapter: &dyn DomainAdapter,
    step: &super::types::PlanStep,
) -> Vec<ToolDefinition> {
    let requested = adapter.available_tools(step);
    let mut allowed: HashSet<String> = requested
        .iter()
        .filter_map(|name| normalize_tool_name(name))
        .map(|name| name.to_string())
        .collect();

    if allowed.is_empty() {
        allowed.extend(
            fallback_plan_tools()
                .into_iter()
                .map(|name| name.to_string()),
        );
    }

    let mut resolved: Vec<ToolDefinition> = get_tool_definitions_from_registry()
        .into_iter()
        .filter(|tool| allowed.contains(&tool.name))
        .collect();

    if resolved.is_empty() {
        let fallback: HashSet<&'static str> = fallback_plan_tools().into_iter().collect();
        resolved = get_tool_definitions_from_registry()
            .into_iter()
            .filter(|tool| fallback.contains(tool.name.as_str()))
            .collect();
    }

    resolved
}

/// Execute all steps in a plan according to batch ordering.
pub async fn execute_plan(
    session_id: &str,
    plan: &mut Plan,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
    runtime: Option<StepExecutionRuntime>,
    config: StepExecutionConfig,
    shared_context: Option<String>,
    language_instruction: String,
    app_handle: tauri::AppHandle,
    cancellation_token: CancellationToken,
    resume_state: Option<PlanExecutionResumeState>,
    progress_callback: Option<PlanExecutionProgressCallback>,
) -> AppResult<(
    HashMap<String, StepOutput>,
    HashMap<String, StepExecutionState>,
    HashMap<String, usize>,
)> {
    adapter.before_execution(plan);

    let mut restored_outputs = resume_state
        .as_ref()
        .map(|state| state.step_outputs.clone())
        .unwrap_or_default();
    let mut restored_states = resume_state
        .as_ref()
        .map(|state| state.step_states.clone())
        .unwrap_or_default();
    let restored_attempts = resume_state
        .as_ref()
        .map(|state| state.step_attempts.clone())
        .unwrap_or_default();

    restored_outputs.retain(|step_id, _| plan.steps.iter().any(|step| step.id == *step_id));
    for step in &plan.steps {
        restored_states
            .entry(step.id.clone())
            .and_modify(|state| {
                if !counts_as_completed(state) {
                    *state = StepExecutionState::Pending;
                }
            })
            .or_insert(StepExecutionState::Pending);
    }

    let step_outputs: Arc<RwLock<HashMap<String, StepOutput>>> =
        Arc::new(RwLock::new(restored_outputs));
    let step_states: Arc<RwLock<HashMap<String, StepExecutionState>>> =
        Arc::new(RwLock::new(restored_states));

    let total_batches = plan.batches.len();
    let total_steps = plan.steps.len().max(1);
    let semaphore = Arc::new(Semaphore::new(config.max_parallel));
    let run_id = uuid::Uuid::new_v4().to_string();
    let event_seq = Arc::new(AtomicU64::new(1));
    let step_attempts: Arc<RwLock<HashMap<String, usize>>> =
        Arc::new(RwLock::new(restored_attempts));

    let step_map: HashMap<String, &super::types::PlanStep> =
        plan.steps.iter().map(|s| (s.id.clone(), s)).collect();
    let mut blocked_reason: Option<String> = None;
    let mut cancellation_event_emitted = false;
    let mut last_batch_index = 0usize;
    let mut failure_fingerprints: HashMap<String, (FailureCategory, String, usize)> =
        HashMap::new();
    let mut max_iteration_strategy_retries: HashMap<String, usize> = HashMap::new();

    for (batch_idx, batch) in plan.batches.iter().enumerate() {
        last_batch_index = batch_idx;
        if cancellation_token.is_cancelled() {
            let mut states = step_states.write().await;
            for step in &plan.steps {
                if !states.get(&step.id).map_or(false, |s| s.is_terminal()) {
                    states.insert(step.id.clone(), StepExecutionState::Cancelled);
                }
            }
            let states_snapshot = states.clone();
            drop(states);
            let outputs_snapshot = step_outputs.read().await.clone();
            let attempts_snapshot = step_attempts.read().await.clone();
            let terminal_report = build_terminal_report(
                session_id,
                plan,
                &outputs_snapshot,
                &states_snapshot,
                "cancelled",
                Some("user".to_string()),
                &run_id,
                compute_retry_stats(&attempts_snapshot, &states_snapshot),
                adapter.as_ref(),
            );
            emit_event_with_metadata(
                &app_handle,
                PlanModeProgressEvent::execution_cancelled(
                    session_id,
                    batch_idx,
                    total_batches,
                    terminal_report,
                ),
                &run_id,
                event_seq.as_ref(),
            );
            cancellation_event_emitted = true;
            break;
        }

        let completed_so_far = {
            let states = step_states.read().await;
            states.values().filter(|s| counts_as_completed(s)).count()
        };
        let progress_pct = (completed_so_far as f64 / total_steps as f64) * 100.0;

        emit_event_with_metadata(
            &app_handle,
            PlanModeProgressEvent::batch_started(
                session_id,
                batch_idx,
                total_batches,
                progress_pct,
            ),
            &run_id,
            event_seq.as_ref(),
        );

        let batch_step_ids = {
            let states = step_states.read().await;
            batch
                .step_ids
                .iter()
                .filter(|step_id| {
                    !states
                        .get(step_id.as_str())
                        .map(counts_as_completed)
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        if !batch_step_ids.is_empty() {
            execute_batch_round(BatchRoundRequest {
                session_id,
                batch_index: batch_idx,
                total_batches,
                total_steps,
                target_step_ids: batch_step_ids,
                step_map: &step_map,
                plan,
                adapter: adapter.clone(),
                provider: provider.clone(),
                runtime: runtime.clone(),
                step_outputs: step_outputs.clone(),
                step_states: step_states.clone(),
                semaphore: semaphore.clone(),
                config: config.clone(),
                shared_context: shared_context.clone(),
                language_instruction: language_instruction.clone(),
                app_handle: app_handle.clone(),
                cancellation_token: cancellation_token.clone(),
                run_id: run_id.clone(),
                event_seq: event_seq.clone(),
                step_attempts: step_attempts.clone(),
            })
            .await;
        }

        if let Some(callback) = progress_callback.as_ref() {
            callback(PlanExecutionCheckpoint {
                step_outputs: step_outputs.read().await.clone(),
                step_states: step_states.read().await.clone(),
                step_attempts: step_attempts.read().await.clone(),
                current_batch: batch_idx,
                total_batches,
            })
            .await;
        }

        let mut failed_in_batch =
            collect_failed_steps_for_batch(&step_states, &batch.step_ids).await;
        update_failure_streaks_for_batch(&step_states, &failed_in_batch, &mut failure_fingerprints)
            .await;
        if failed_in_batch.is_empty() {
            continue;
        }

        for _retry_index in 1..=config.max_retry_attempts {
            if failed_in_batch.is_empty() || cancellation_token.is_cancelled() {
                break;
            }

            let attempts_snapshot = step_attempts.read().await.clone();
            let mut retryable_steps = Vec::new();
            for step_id in &failed_in_batch {
                let reason = {
                    let states = step_states.read().await;
                    match states.get(step_id) {
                        Some(StepExecutionState::HardFailed { reason }) => reason.clone(),
                        _ => "Retrying due to unresolved failure".to_string(),
                    }
                };
                let category = classify_failure_reason(&reason);
                let attempts = attempts_snapshot.get(step_id).copied().unwrap_or(1);
                let repeated_streak = failure_fingerprints
                    .get(step_id)
                    .map(|(_, _, streak)| *streak)
                    .unwrap_or(1);
                if repeated_streak >= 2 {
                    continue;
                }
                if matches!(category, FailureCategory::MaxIterations) {
                    let already_used = max_iteration_strategy_retries
                        .get(step_id)
                        .copied()
                        .unwrap_or(0);
                    if already_used >= 1 || attempts >= 2 {
                        continue;
                    }
                    max_iteration_strategy_retries.insert(step_id.clone(), already_used + 1);
                }
                retryable_steps.push(step_id.clone());
                let progress_pct = {
                    let snapshot = step_states.read().await;
                    progress_pct_from_states(&snapshot, total_steps)
                };
                emit_event_with_metadata(
                    &app_handle,
                    PlanModeProgressEvent::step_retrying(
                        session_id,
                        batch_idx,
                        total_batches,
                        step_id,
                        &reason,
                        progress_pct,
                    )
                    .with_attempt_metadata(Some(attempts), Some(category.as_error_code())),
                    &run_id,
                    event_seq.as_ref(),
                );
            }

            if retryable_steps.is_empty() {
                break;
            }

            if config.retry_backoff_ms > 0 {
                sleep(Duration::from_millis(config.retry_backoff_ms)).await;
            }

            execute_batch_round(BatchRoundRequest {
                session_id,
                batch_index: batch_idx,
                total_batches,
                total_steps,
                target_step_ids: retryable_steps,
                step_map: &step_map,
                plan,
                adapter: adapter.clone(),
                provider: provider.clone(),
                runtime: runtime.clone(),
                step_outputs: step_outputs.clone(),
                step_states: step_states.clone(),
                semaphore: semaphore.clone(),
                config: config.clone(),
                shared_context: shared_context.clone(),
                language_instruction: language_instruction.clone(),
                app_handle: app_handle.clone(),
                cancellation_token: cancellation_token.clone(),
                run_id: run_id.clone(),
                event_seq: event_seq.clone(),
                step_attempts: step_attempts.clone(),
            })
            .await;
            failed_in_batch = collect_failed_steps_for_batch(&step_states, &batch.step_ids).await;
            update_failure_streaks_for_batch(
                &step_states,
                &failed_in_batch,
                &mut failure_fingerprints,
            )
            .await;
        }

        if cancellation_token.is_cancelled() {
            break;
        }

        if !failed_in_batch.is_empty() && config.fail_batch_on_exhausted {
            let reason = format!(
                "Batch {} blocked because failed steps exhausted retries: {}",
                batch_idx,
                failed_in_batch.join(", ")
            );
            blocked_reason = Some(reason.clone());
            let progress_pct = {
                let snapshot = step_states.read().await;
                progress_pct_from_states(&snapshot, total_steps)
            };
            emit_event_with_metadata(
                &app_handle,
                PlanModeProgressEvent::batch_blocked(
                    session_id,
                    batch_idx,
                    total_batches,
                    &reason,
                    progress_pct,
                ),
                &run_id,
                event_seq.as_ref(),
            );
            break;
        }
    }

    let final_outputs = step_outputs.read().await.clone();
    let final_states = step_states.read().await.clone();
    let attempts_snapshot = step_attempts.read().await.clone();
    if cancellation_token.is_cancelled() && !cancellation_event_emitted {
        let mut states = step_states.write().await;
        for step in &plan.steps {
            if !states.get(&step.id).map_or(false, |s| s.is_terminal()) {
                states.insert(step.id.clone(), StepExecutionState::Cancelled);
            }
        }
        let states_snapshot = states.clone();
        drop(states);
        let outputs_snapshot = step_outputs.read().await.clone();
        let terminal_report = build_terminal_report(
            session_id,
            plan,
            &outputs_snapshot,
            &states_snapshot,
            "cancelled",
            Some("user".to_string()),
            &run_id,
            compute_retry_stats(&attempts_snapshot, &states_snapshot),
            adapter.as_ref(),
        );
        let emit_batch = last_batch_index.min(total_batches.saturating_sub(1));
        emit_event_with_metadata(
            &app_handle,
            PlanModeProgressEvent::execution_cancelled(
                session_id,
                emit_batch,
                total_batches,
                terminal_report,
            ),
            &run_id,
            event_seq.as_ref(),
        );
    }
    if !cancellation_token.is_cancelled() {
        let terminal_state = if blocked_reason.is_some() {
            "failed"
        } else {
            "completed"
        };
        let terminal_report = build_terminal_report(
            session_id,
            plan,
            &final_outputs,
            &final_states,
            terminal_state,
            None,
            &run_id,
            compute_retry_stats(&attempts_snapshot, &final_states),
            adapter.as_ref(),
        );
        let progress_pct = progress_pct_from_states(&final_states, total_steps);
        emit_event_with_metadata(
            &app_handle,
            PlanModeProgressEvent::execution_completed(
                session_id,
                total_batches,
                progress_pct,
                terminal_report,
            ),
            &run_id,
            event_seq.as_ref(),
        );
    }

    Ok((final_outputs, final_states, attempts_snapshot))
}

struct BatchRoundRequest<'a> {
    session_id: &'a str,
    batch_index: usize,
    total_batches: usize,
    total_steps: usize,
    target_step_ids: Vec<String>,
    step_map: &'a HashMap<String, &'a super::types::PlanStep>,
    plan: &'a Plan,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
    runtime: Option<StepExecutionRuntime>,
    step_outputs: Arc<RwLock<HashMap<String, StepOutput>>>,
    step_states: Arc<RwLock<HashMap<String, StepExecutionState>>>,
    semaphore: Arc<Semaphore>,
    config: StepExecutionConfig,
    shared_context: Option<String>,
    language_instruction: String,
    app_handle: tauri::AppHandle,
    cancellation_token: CancellationToken,
    run_id: String,
    event_seq: Arc<AtomicU64>,
    step_attempts: Arc<RwLock<HashMap<String, usize>>>,
}

async fn execute_batch_round(request: BatchRoundRequest<'_>) {
    let BatchRoundRequest {
        session_id,
        batch_index,
        total_batches,
        total_steps,
        target_step_ids,
        step_map,
        plan,
        adapter,
        provider,
        runtime,
        step_outputs,
        step_states,
        semaphore,
        config,
        shared_context,
        language_instruction,
        app_handle,
        cancellation_token,
        run_id,
        event_seq,
        step_attempts,
    } = request;

    let mut handles = Vec::new();

    for step_id in target_step_ids {
        let step = match step_map.get(step_id.as_str()) {
            Some(value) => (*value).clone(),
            None => continue,
        };

        let sem = semaphore.clone();
        let adapter_cloned = adapter.clone();
        let provider_cloned = provider.clone();
        let runtime_cloned = runtime.clone();
        let outputs = step_outputs.clone();
        let states = step_states.clone();
        let app = app_handle.clone();
        let cancel = cancellation_token.clone();
        let plan_clone = plan.clone();
        let cfg = config.clone();
        let shared_ctx = shared_context.clone();
        let lang_inst = language_instruction.clone();
        let sid = session_id.to_string();
        let run = run_id.clone();
        let seq = event_seq.clone();
        let attempts = step_attempts.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if cancel.is_cancelled() {
                let mut s = states.write().await;
                s.insert(step.id.clone(), StepExecutionState::Cancelled);
                return;
            }

            let attempt_count = {
                let mut tracker = attempts.write().await;
                let entry = tracker.entry(step.id.clone()).or_insert(0);
                *entry += 1;
                *entry
            };
            let retry_feedback = {
                let snapshot = states.read().await;
                match snapshot.get(step.id.as_str()) {
                    Some(StepExecutionState::HardFailed { reason })
                    | Some(StepExecutionState::SoftFailed { reason, .. })
                    | Some(StepExecutionState::NeedsReview { reason, .. }) => Some(reason.clone()),
                    _ => None,
                }
            };

            {
                let mut s = states.write().await;
                s.insert(step.id.clone(), StepExecutionState::Running);
            }

            let completed_count = {
                let s = states.read().await;
                s.values().filter(|st| counts_as_completed(st)).count()
            };
            let progress_pct = (completed_count as f64 / total_steps as f64) * 100.0;
            emit_event_with_metadata(
                &app,
                PlanModeProgressEvent::step_started(
                    &sid,
                    batch_index,
                    total_batches,
                    &step.id,
                    progress_pct,
                )
                .with_attempt_metadata(Some(attempt_count), None::<String>),
                &run,
                seq.as_ref(),
            );

            let started_at = Instant::now();
            let dep_outputs = {
                let outs = outputs.read().await;
                let mut deps = Vec::new();
                for dep_id in &step.dependencies {
                    if let Some(output) = outs.get(dep_id) {
                        let dep_title = plan_clone
                            .steps
                            .iter()
                            .find(|value| value.id == *dep_id)
                            .map(|value| value.title.clone())
                            .unwrap_or_else(|| dep_id.clone());
                        deps.push((dep_title, output.clone()));
                    }
                }
                deps
            };
            let dep_outputs = truncate_dep_outputs(
                dep_outputs,
                cfg.max_dep_output_chars,
                cfg.max_total_dep_chars,
            );
            let step_soft_limit = compute_step_iteration_limit(&step, cfg.step_soft_limit_cap);
            let retry_failure_category = retry_feedback.as_deref().map(classify_failure_reason);

            match execute_single_step(
                &step,
                &dep_outputs,
                &plan_clone,
                adapter_cloned.as_ref(),
                provider_cloned.as_ref(),
                runtime_cloned.as_ref(),
                shared_ctx.as_deref(),
                &lang_inst,
                retry_feedback.as_deref(),
                retry_failure_category,
                step_soft_limit,
                attempt_count,
                cancel.clone(),
            )
            .await
            {
                Ok(mut output) => {
                    if cancel.is_cancelled() {
                        let mut s = states.write().await;
                        s.insert(step.id.clone(), StepExecutionState::Cancelled);
                        return;
                    }

                    output.attempt_count = attempt_count;
                    output.evidence_bundle.runtime_stats.attempt_count = attempt_count;
                    let validation_provider = if let Some(runtime) = runtime_cloned.as_ref() {
                        if let (Some(tx), Some(calc)) = (
                            runtime.analytics_tx.as_ref(),
                            runtime.analytics_cost_calculator.as_ref(),
                        ) {
                            crate::services::analytics::wrap_provider_with_tracking(
                                provider_cloned.clone(),
                                tx.clone(),
                                Arc::clone(calc),
                                crate::models::analytics::AnalyticsAttribution {
                                    project_id: None,
                                    kernel_session_id: runtime.kernel_session_id.clone(),
                                    mode_session_id: Some(runtime.mode_session_id.clone()),
                                    workflow_mode: Some(
                                        crate::models::analytics::AnalyticsWorkflowMode::Plan,
                                    ),
                                    phase_id: Some("plan_validation".to_string()),
                                    execution_scope: Some(
                                        crate::models::analytics::AnalyticsExecutionScope::DirectLlm,
                                    ),
                                    execution_id: Some(format!(
                                        "plan:{}:{}:{}:validation",
                                        runtime.mode_session_id, step.id, attempt_count
                                    )),
                                    parent_execution_id: Some(format!(
                                        "plan:{}:{}:{}",
                                        runtime.mode_session_id, step.id, attempt_count
                                    )),
                                    agent_role: Some("plan_validation".to_string()),
                                    agent_name: None,
                                    step_id: Some(step.id.clone()),
                                    story_id: None,
                                    gate_id: None,
                                    attempt: Some(attempt_count as i64),
                                    request_sequence: Some(1),
                                    call_site: Some("plan_step.validation".to_string()),
                                    metadata_json: None,
                                },
                            )
                        } else {
                            provider_cloned.clone()
                        }
                    } else {
                        provider_cloned.clone()
                    };
                    let validation_result = validate_step_output(
                        &step,
                        &mut output,
                        adapter_cloned.clone(),
                        validation_provider,
                    )
                    .await;

                    if let Some(reason) =
                        detect_incomplete_output_reason(&step, &output, &validation_result)
                    {
                        let category =
                            classify_failure_reason(&format!("Step output incomplete: {reason}"));
                        output.quality_state = StepOutputQualityState::Incomplete;
                        output.incomplete_reason = Some(reason.clone());
                        output.error_code = Some(category.as_error_code().to_string());
                        emit_step_output_diagnostic_log(
                            "step_output_rejected",
                            &run,
                            &sid,
                            batch_index,
                            total_batches,
                            &step.id,
                            &step.title,
                            &output,
                        );
                        let mut s = states.write().await;
                        s.insert(
                            step.id.clone(),
                            StepExecutionState::HardFailed {
                                reason: format!("Step output incomplete: {reason}"),
                            },
                        );
                        let completed_count = s
                            .values()
                            .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                            .count();
                        let pct = (completed_count as f64 / total_steps as f64) * 100.0;
                        drop(s);
                        emit_event_with_metadata(
                            &app,
                            PlanModeProgressEvent::step_failed_with_output(
                                &sid,
                                batch_index,
                                total_batches,
                                &step.id,
                                &format!("Step output incomplete: {reason}"),
                                output,
                                pct,
                            )
                            .with_attempt_metadata(
                                Some(attempt_count),
                                Some(category.as_error_code()),
                            ),
                            &run,
                            seq.as_ref(),
                        );
                        return;
                    }

                    let duration_ms = started_at.elapsed().as_millis() as u64;
                    apply_validation_outcome_to_output(&mut output, &validation_result);
                    emit_step_output_diagnostic_log(
                        "step_output_accepted",
                        &run,
                        &sid,
                        batch_index,
                        total_batches,
                        &step.id,
                        &step.title,
                        &output,
                    );
                    let output_for_event = output.clone();

                    {
                        let mut s = states.write().await;
                        s.insert(
                            step.id.clone(),
                            state_from_validation(&validation_result, duration_ms),
                        );
                    }
                    {
                        let mut outs = outputs.write().await;
                        outs.insert(step.id.clone(), output);
                    }

                    let completed_count = {
                        let s = states.read().await;
                        s.values()
                            .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                            .count()
                    };
                    let pct = (completed_count as f64 / total_steps as f64) * 100.0;
                    emit_event_with_metadata(
                        &app,
                        step_event_from_validation(
                            &sid,
                            batch_index,
                            total_batches,
                            &step.id,
                            output_for_event,
                            pct,
                            &validation_result,
                        )
                        .with_attempt_metadata(Some(attempt_count), None::<String>),
                        &run,
                        seq.as_ref(),
                    );
                }
                Err(error) => {
                    if cancel.is_cancelled() {
                        let mut s = states.write().await;
                        s.insert(step.id.clone(), StepExecutionState::Cancelled);
                        return;
                    }
                    let reason = format!("{error}");
                    let category = classify_failure_reason(&reason);
                    {
                        let mut s = states.write().await;
                        s.insert(
                            step.id.clone(),
                            StepExecutionState::HardFailed {
                                reason: reason.clone(),
                            },
                        );
                    }

                    let completed_count = {
                        let s = states.read().await;
                        s.values()
                            .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                            .count()
                    };
                    let pct = (completed_count as f64 / total_steps as f64) * 100.0;
                    emit_event_with_metadata(
                        &app,
                        PlanModeProgressEvent::step_failed(
                            &sid,
                            batch_index,
                            total_batches,
                            &step.id,
                            &reason,
                            pct,
                        )
                        .with_attempt_metadata(Some(attempt_count), Some(category.as_error_code())),
                        &run,
                        seq.as_ref(),
                    );
                }
            }
        });

        handles.push(task);
    }

    for handle in handles {
        let _ = handle.await;
    }
}

async fn collect_failed_steps_for_batch(
    states: &Arc<RwLock<HashMap<String, StepExecutionState>>>,
    step_ids: &[String],
) -> Vec<String> {
    let snapshot = states.read().await;
    step_ids
        .iter()
        .filter_map(|step_id| match snapshot.get(step_id) {
            Some(StepExecutionState::HardFailed { .. }) => Some(step_id.clone()),
            _ => None,
        })
        .collect()
}

/// Retry a single failed/cancelled step and preserve existing outputs/states.
pub async fn retry_single_step(
    session_id: &str,
    plan: &Plan,
    step_id: &str,
    mut existing_outputs: HashMap<String, StepOutput>,
    mut existing_states: HashMap<String, StepExecutionState>,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
    runtime: Option<StepExecutionRuntime>,
    config: StepExecutionConfig,
    shared_context: Option<String>,
    language_instruction: String,
    app_handle: Option<tauri::AppHandle>,
    cancellation_token: CancellationToken,
) -> AppResult<(
    HashMap<String, StepOutput>,
    HashMap<String, StepExecutionState>,
)> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let step = plan
        .steps
        .iter()
        .find(|value| value.id == step_id)
        .ok_or_else(|| AppError::Internal(format!("Step not found in plan: {step_id}")))?;

    let total_batches = plan.batches.len().max(1);
    let current_batch = plan
        .batches
        .iter()
        .position(|batch| batch.step_ids.iter().any(|id| id == step_id))
        .unwrap_or(0);
    let total_steps = plan.steps.len().max(1);

    if cancellation_token.is_cancelled() {
        existing_states.insert(step.id.clone(), StepExecutionState::Cancelled);
        if let Some(handle) = app_handle.as_ref() {
            let terminal_report = build_terminal_report(
                session_id,
                plan,
                &existing_outputs,
                &existing_states,
                "cancelled",
                Some("user".to_string()),
                &run_id,
                PlanRetryStats::default(),
                adapter.as_ref(),
            );
            emit_event(
                handle,
                PlanModeProgressEvent::execution_cancelled(
                    session_id,
                    current_batch,
                    total_batches,
                    terminal_report,
                ),
            );
        }
        return Ok((existing_outputs, existing_states));
    }

    existing_states.insert(step.id.clone(), StepExecutionState::Running);
    let completed_before = existing_states
        .values()
        .filter(|value| matches!(value, StepExecutionState::Completed { .. }))
        .count();
    let started_pct = (completed_before as f64 / total_steps as f64) * 100.0;
    if let Some(handle) = app_handle.as_ref() {
        emit_event(
            handle,
            PlanModeProgressEvent::step_started(
                session_id,
                current_batch,
                total_batches,
                &step.id,
                started_pct,
            )
            .with_attempt_metadata(Some(1), None::<String>),
        );
    }

    let mut missing_dependencies = Vec::new();
    let mut dep_outputs = Vec::new();
    for dep_id in &step.dependencies {
        if let Some(output) = existing_outputs.get(dep_id) {
            let dep_title = plan
                .steps
                .iter()
                .find(|s| s.id == *dep_id)
                .map(|s| s.title.clone())
                .unwrap_or_else(|| dep_id.clone());
            dep_outputs.push((dep_title, output.clone()));
        } else {
            missing_dependencies.push(dep_id.clone());
        }
    }

    if !missing_dependencies.is_empty() {
        let reason = format!(
            "Missing dependency outputs for retry step '{}': {}",
            step.id,
            missing_dependencies.join(", ")
        );
        existing_states.insert(
            step.id.clone(),
            StepExecutionState::HardFailed {
                reason: reason.clone(),
            },
        );
        let completed_count = existing_states
            .values()
            .filter(|value| counts_as_completed(value))
            .count();
        let pct = (completed_count as f64 / total_steps as f64) * 100.0;
        if let Some(handle) = app_handle.as_ref() {
            let terminal_report = build_terminal_report(
                session_id,
                plan,
                &existing_outputs,
                &existing_states,
                "completed",
                None,
                &run_id,
                PlanRetryStats::default(),
                adapter.as_ref(),
            );
            emit_event(
                handle,
                PlanModeProgressEvent::step_failed(
                    session_id,
                    current_batch,
                    total_batches,
                    &step.id,
                    &reason,
                    pct,
                ),
            );
            emit_event(
                handle,
                PlanModeProgressEvent::execution_completed(
                    session_id,
                    total_batches,
                    pct,
                    terminal_report,
                ),
            );
        }
        return Ok((existing_outputs, existing_states));
    }

    let dep_outputs = truncate_dep_outputs(
        dep_outputs,
        config.max_dep_output_chars,
        config.max_total_dep_chars,
    );
    let started_at = Instant::now();

    match execute_single_step(
        step,
        &dep_outputs,
        plan,
        adapter.as_ref(),
        provider.as_ref(),
        runtime.as_ref(),
        shared_context.as_deref(),
        &language_instruction,
        None,
        None,
        compute_step_iteration_limit(step, config.step_soft_limit_cap),
        1,
        cancellation_token.clone(),
    )
    .await
    {
        Ok(mut output) => {
            if cancellation_token.is_cancelled() {
                existing_states.insert(step.id.clone(), StepExecutionState::Cancelled);
                if let Some(handle) = app_handle.as_ref() {
                    let terminal_report = build_terminal_report(
                        session_id,
                        plan,
                        &existing_outputs,
                        &existing_states,
                        "cancelled",
                        Some("user".to_string()),
                        &run_id,
                        PlanRetryStats::default(),
                        adapter.as_ref(),
                    );
                    emit_event(
                        handle,
                        PlanModeProgressEvent::execution_cancelled(
                            session_id,
                            current_batch,
                            total_batches,
                            terminal_report,
                        ),
                    );
                }
                return Ok((existing_outputs, existing_states));
            }

            output.attempt_count = 1;
            let validation_result =
                validate_step_output(step, &mut output, adapter.clone(), provider.clone()).await;
            if let Some(reason) = detect_incomplete_output_reason(step, &output, &validation_result)
            {
                let category =
                    classify_failure_reason(&format!("Step output incomplete: {reason}"));
                existing_states.insert(
                    step.id.clone(),
                    StepExecutionState::HardFailed {
                        reason: format!("Step output incomplete: {reason}"),
                    },
                );
                let completed_count = existing_states
                    .values()
                    .filter(|value| counts_as_completed(value))
                    .count();
                let pct = (completed_count as f64 / total_steps as f64) * 100.0;
                if let Some(handle) = app_handle.as_ref() {
                    let terminal_report = build_terminal_report(
                        session_id,
                        plan,
                        &existing_outputs,
                        &existing_states,
                        "completed",
                        None,
                        &run_id,
                        PlanRetryStats::default(),
                        adapter.as_ref(),
                    );
                    emit_event(
                        handle,
                        PlanModeProgressEvent::step_failed(
                            session_id,
                            current_batch,
                            total_batches,
                            &step.id,
                            &format!("Step output incomplete: {reason}"),
                            pct,
                        )
                        .with_attempt_metadata(Some(1), Some(category.as_error_code())),
                    );
                    emit_event(
                        handle,
                        PlanModeProgressEvent::execution_completed(
                            session_id,
                            total_batches,
                            pct,
                            terminal_report,
                        ),
                    );
                }
                return Ok((existing_outputs, existing_states));
            }

            apply_validation_outcome_to_output(&mut output, &validation_result);
            let duration_ms = started_at.elapsed().as_millis() as u64;
            let output_for_event = output.clone();
            existing_states.insert(
                step.id.clone(),
                state_from_validation(&validation_result, duration_ms),
            );
            existing_outputs.insert(step.id.clone(), output);
            let completed_count = existing_states
                .values()
                .filter(|value| counts_as_completed(value))
                .count();
            let pct = (completed_count as f64 / total_steps as f64) * 100.0;
            if let Some(handle) = app_handle.as_ref() {
                let terminal_report = build_terminal_report(
                    session_id,
                    plan,
                    &existing_outputs,
                    &existing_states,
                    "completed",
                    None,
                    &run_id,
                    PlanRetryStats::default(),
                    adapter.as_ref(),
                );
                emit_event(
                    handle,
                    step_event_from_validation(
                        session_id,
                        current_batch,
                        total_batches,
                        &step.id,
                        output_for_event,
                        pct,
                        &validation_result,
                    )
                    .with_attempt_metadata(Some(1), None::<String>),
                );
                emit_event(
                    handle,
                    PlanModeProgressEvent::execution_completed(
                        session_id,
                        total_batches,
                        pct,
                        terminal_report,
                    ),
                );
            }
        }
        Err(error) => {
            if cancellation_token.is_cancelled() {
                existing_states.insert(step.id.clone(), StepExecutionState::Cancelled);
                if let Some(handle) = app_handle.as_ref() {
                    let terminal_report = build_terminal_report(
                        session_id,
                        plan,
                        &existing_outputs,
                        &existing_states,
                        "cancelled",
                        Some("user".to_string()),
                        &run_id,
                        PlanRetryStats::default(),
                        adapter.as_ref(),
                    );
                    emit_event(
                        handle,
                        PlanModeProgressEvent::execution_cancelled(
                            session_id,
                            current_batch,
                            total_batches,
                            terminal_report,
                        ),
                    );
                }
                return Ok((existing_outputs, existing_states));
            }

            let reason = format!("{error}");
            let category = classify_failure_reason(&reason);
            existing_states.insert(
                step.id.clone(),
                StepExecutionState::HardFailed {
                    reason: reason.clone(),
                },
            );
            let completed_count = existing_states
                .values()
                .filter(|value| counts_as_completed(value))
                .count();
            let pct = (completed_count as f64 / total_steps as f64) * 100.0;
            if let Some(handle) = app_handle.as_ref() {
                let terminal_report = build_terminal_report(
                    session_id,
                    plan,
                    &existing_outputs,
                    &existing_states,
                    "completed",
                    None,
                    &run_id,
                    PlanRetryStats::default(),
                    adapter.as_ref(),
                );
                emit_event(
                    handle,
                    PlanModeProgressEvent::step_failed(
                        session_id,
                        current_batch,
                        total_batches,
                        &step.id,
                        &reason,
                        pct,
                    )
                    .with_attempt_metadata(Some(1), Some(category.as_error_code())),
                );
                emit_event(
                    handle,
                    PlanModeProgressEvent::execution_completed(
                        session_id,
                        total_batches,
                        pct,
                        terminal_report,
                    ),
                );
            }
        }
    }

    Ok((existing_outputs, existing_states))
}

/// Build the current progress from step states.
pub fn build_progress(
    step_states: &HashMap<String, StepExecutionState>,
    current_batch: usize,
    total_batches: usize,
    total_steps: usize,
) -> PlanExecutionProgress {
    let steps_completed = step_states
        .values()
        .filter(|s| counts_as_completed(s))
        .count();
    let steps_failed = step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::HardFailed { .. }))
        .count();

    PlanExecutionProgress {
        current_batch,
        total_batches,
        steps_completed,
        steps_failed,
        total_steps,
        progress_pct: if total_steps > 0 {
            (steps_completed as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        },
    }
}

fn build_terminal_report(
    session_id: &str,
    plan: &Plan,
    step_outputs: &HashMap<String, StepOutput>,
    step_states: &HashMap<String, StepExecutionState>,
    terminal_state: &str,
    cancelled_by: Option<String>,
    run_id: &str,
    retry_stats: PlanRetryStats,
    adapter: &dyn DomainAdapter,
) -> PlanExecutionReport {
    let mut step_summaries: HashMap<String, String> = HashMap::new();
    for (step_id, output) in step_outputs {
        let summary = if output.summary.trim().is_empty() {
            let source = if output.full_content.trim().is_empty() {
                output.content.as_str()
            } else {
                output.full_content.as_str()
            };
            source.chars().take(240).collect::<String>()
        } else {
            output.summary.clone()
        };
        step_summaries.insert(step_id.clone(), summary);
    }

    let mut failure_reasons: HashMap<String, String> = HashMap::new();
    let mut steps_completed = 0usize;
    let mut steps_failed = 0usize;
    let mut steps_soft_failed = 0usize;
    let mut steps_needs_review = 0usize;
    let mut steps_cancelled = 0usize;
    let mut total_duration_ms = 0u64;
    for (step_id, state) in step_states {
        match state {
            StepExecutionState::Completed { duration_ms } => {
                steps_completed += 1;
                total_duration_ms = total_duration_ms.saturating_add(*duration_ms);
            }
            StepExecutionState::SoftFailed {
                reason,
                duration_ms,
            } => {
                steps_completed += 1;
                steps_soft_failed += 1;
                total_duration_ms = total_duration_ms.saturating_add(*duration_ms);
                failure_reasons.insert(step_id.clone(), reason.clone());
            }
            StepExecutionState::NeedsReview {
                reason,
                duration_ms,
            } => {
                steps_completed += 1;
                steps_needs_review += 1;
                total_duration_ms = total_duration_ms.saturating_add(*duration_ms);
                failure_reasons.insert(step_id.clone(), reason.clone());
            }
            StepExecutionState::HardFailed { reason } => {
                steps_failed += 1;
                failure_reasons.insert(step_id.clone(), reason.clone());
            }
            StepExecutionState::Cancelled => {
                steps_cancelled += 1;
            }
            _ => {}
        }
    }

    let total_steps = plan.steps.len();
    let steps_attempted = steps_completed + steps_failed + steps_cancelled;
    let normalized_terminal = if terminal_state == "cancelled" {
        "cancelled"
    } else if steps_failed > 0 {
        "failed"
    } else if steps_needs_review > 0 {
        "needs_review"
    } else if steps_soft_failed > 0 {
        "completed_with_warnings"
    } else {
        "completed"
    };
    let success = (normalized_terminal == "completed"
        || normalized_terminal == "completed_with_warnings")
        && steps_failed == 0
        && steps_completed == total_steps;

    let ordered_outputs: Vec<StepOutput> = plan
        .steps
        .iter()
        .filter_map(|step| step_outputs.get(&step.id).cloned())
        .collect();
    let adapter_conclusion = if normalized_terminal == "cancelled" {
        None
    } else {
        adapter
            .after_execution(plan, &ordered_outputs)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let final_conclusion_markdown = adapter_conclusion.unwrap_or_else(|| {
        synthesize_final_conclusion(
            plan,
            &step_summaries,
            &failure_reasons,
            normalized_terminal,
            steps_completed,
            total_steps,
        )
    });
    let highlights = build_report_highlights(plan, &step_summaries);
    let next_actions = build_next_actions(normalized_terminal, &failure_reasons);

    PlanExecutionReport {
        session_id: session_id.to_string(),
        plan_title: plan.title.clone(),
        success,
        terminal_state: normalized_terminal.to_string(),
        terminal_status: match normalized_terminal {
            "completed" => PlanTerminalStatus::Completed,
            "completed_with_warnings" => PlanTerminalStatus::CompletedWithWarnings,
            "needs_review" => PlanTerminalStatus::NeedsReview,
            "cancelled" => PlanTerminalStatus::Cancelled,
            _ => PlanTerminalStatus::Failed,
        },
        total_steps,
        steps_completed,
        steps_failed,
        steps_soft_failed,
        steps_needs_review,
        steps_cancelled,
        steps_attempted,
        steps_failed_before_cancel: if normalized_terminal == "cancelled" {
            steps_failed
        } else {
            0
        },
        total_duration_ms,
        step_summaries,
        failure_reasons,
        cancelled_by: if normalized_terminal == "cancelled" {
            Some(cancelled_by.unwrap_or_else(|| "user".to_string()))
        } else {
            None
        },
        run_id: run_id.to_string(),
        final_conclusion_markdown,
        highlights,
        next_actions,
        retry_stats,
        terminal_verdict_trace: vec![format!(
            "completed={}, hard_failed={}, soft_failed={}, needs_review={}",
            steps_completed, steps_failed, steps_soft_failed, steps_needs_review
        )],
    }
}

fn synthesize_final_conclusion(
    plan: &Plan,
    step_summaries: &HashMap<String, String>,
    failure_reasons: &HashMap<String, String>,
    terminal_state: &str,
    steps_completed: usize,
    total_steps: usize,
) -> String {
    let mut lines = vec![
        format!("# {} — Execution Summary", plan.title),
        format!(
            "- Terminal state: `{}`\n- Steps completed: {}/{}",
            terminal_state, steps_completed, total_steps
        ),
    ];

    if !step_summaries.is_empty() {
        lines.push("## Key step outcomes".to_string());
        let mut ids: Vec<_> = step_summaries.keys().cloned().collect();
        ids.sort();
        for step_id in ids.into_iter().take(6) {
            if let Some(summary) = step_summaries.get(&step_id) {
                lines.push(format!("- {}: {}", step_id, summary));
            }
        }
    }

    if !failure_reasons.is_empty() {
        lines.push("## Blocking issues".to_string());
        let mut ids: Vec<_> = failure_reasons.keys().cloned().collect();
        ids.sort();
        for step_id in ids {
            if let Some(reason) = failure_reasons.get(&step_id) {
                lines.push(format!("- {}: {}", step_id, reason));
            }
        }
    } else if terminal_state == "cancelled" {
        lines.push("Execution was cancelled before all planned steps finished.".to_string());
    } else {
        lines.push("All planned steps finished without terminal failures.".to_string());
    }

    lines.join("\n")
}

fn build_report_highlights(plan: &Plan, step_summaries: &HashMap<String, String>) -> Vec<String> {
    let mut highlights = Vec::new();
    for step in &plan.steps {
        if let Some(summary) = step_summaries.get(&step.id) {
            highlights.push(format!("{}: {}", step.title, summary));
            if highlights.len() >= 5 {
                break;
            }
        }
    }
    highlights
}

fn build_next_actions(
    terminal_state: &str,
    failure_reasons: &HashMap<String, String>,
) -> Vec<String> {
    if terminal_state == "completed" {
        return vec![
            "Review the generated outputs and consolidate them into final deliverables."
                .to_string(),
            "Open any listed artifacts and run domain-specific validation before publishing."
                .to_string(),
        ];
    }
    if terminal_state == "cancelled" {
        return vec![
            "Resume from the latest completed steps after addressing cancellation causes."
                .to_string(),
            "Re-run blocked steps first to avoid repeating completed work.".to_string(),
        ];
    }
    let mut actions = vec![
        "Prioritize failed steps and verify their dependency outputs are complete.".to_string(),
        "Retry with stricter output instructions and explicit completion-criteria checks."
            .to_string(),
    ];
    if !failure_reasons.is_empty() {
        actions.push(
            "Inspect failure reasons in the report before re-executing subsequent batches."
                .to_string(),
        );
    }
    actions
}

/// Execute a single step using the adapter and LLM provider.
async fn execute_single_step(
    step: &super::types::PlanStep,
    dep_outputs: &[(String, StepOutput)],
    plan: &Plan,
    adapter: &dyn DomainAdapter,
    provider: &dyn LlmProvider,
    runtime: Option<&StepExecutionRuntime>,
    shared_context: Option<&str>,
    language_instruction: &str,
    retry_feedback: Option<&str>,
    retry_failure_category: Option<FailureCategory>,
    soft_limit_override: u32,
    attempt_count: usize,
    cancellation_token: CancellationToken,
) -> AppResult<StepOutput> {
    if cancellation_token.is_cancelled() {
        return Err(AppError::internal("Execution cancelled"));
    }

    let persona = adapter.execution_persona(step);

    let context_section = shared_context
        .filter(|s| !s.trim().is_empty())
        .map(|ctx| {
            format!(
                "\n\n## Project Memory & Skills Context\n{}",
                truncate_context_for_system(ctx, 12_000)
            )
        })
        .unwrap_or_default();

    let system = format!(
        "{}\n\n{}{}\n\n## Output Language\n{}",
        persona.identity_prompt, persona.thinking_style, context_section, language_instruction
    );

    let user_prompt = adapter.step_execution_prompt(step, dep_outputs, plan);
    let completion_criteria_section = if step.completion_criteria.is_empty() {
        String::new()
    } else {
        let checklist = step
            .completion_criteria
            .iter()
            .enumerate()
            .map(|(idx, criterion)| format!("{}. {}", idx + 1, criterion))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n## Completion Criteria Checklist\n{checklist}")
    };
    let retry_section = retry_feedback
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let strategy_hint = match retry_failure_category {
                Some(FailureCategory::MaxIterations) => {
                    "\nUse minimal tool calls and prioritize writing the final deliverable first."
                }
                Some(FailureCategory::CriteriaUnmet) => {
                    "\nAdd a \"Criteria Coverage\" section mapping each criterion to concrete evidence."
                }
                Some(FailureCategory::IncompleteNarration) => {
                    "\nStart directly with the deliverable body. Do not narrate execution steps."
                }
                _ => "",
            };
            format!(
                "\n\n## Retry Requirements\nPrevious attempt failed because:\n{}\n\n\
                 Produce the final deliverable directly. Do not describe future actions. \
                 Explicitly satisfy each completion criterion.{}",
                value, strategy_hint
            )
        })
        .unwrap_or_default();
    let full_prompt = format!(
        "{user_prompt}{completion_criteria_section}\n\n---\n\
         Execute this step and produce the required output.\n\
         Address all completion criteria.\n\
         Return the final deliverable directly. Do not start with execution narration \
         like \"Let me...\", \"I will...\", \"让我...\", \"我将...\".\n\
         If you include a short preface, it must be followed by the complete deliverable.{retry_section}"
    );

    if let Some(runtime) = runtime {
        let config = OrchestratorConfig {
            provider: runtime.provider_config.clone(),
            system_prompt: Some(system),
            execution_kind: ExecutionKind::PlanStep,
            soft_limit_override: Some(soft_limit_override),
            max_total_tokens: 120_000,
            project_root: runtime.project_root.clone(),
            analysis_artifacts_root: dirs::home_dir()
                .unwrap_or_else(|| std::env::temp_dir())
                .join(".plan-cascade")
                .join("analysis-runs"),
            streaming: false,
            enable_compaction: true,
            analysis_profile: Default::default(),
            analysis_limits: Default::default(),
            analysis_session_id: None,
            project_id: None,
            compaction_config: Default::default(),
            task_type: Some("plan".to_string()),
            sub_agent_depth: None,
        };

        let mut orchestrator = OrchestratorService::new(config)
            .with_guardrail_hooks(crate::services::guardrail::shared_guardrail_registry());
        if let Some(tracker) = runtime.file_change_tracker.as_ref() {
            let turn_index = match tracker.lock() {
                Ok(mut guard) => {
                    let next = guard.turn_index() + 1;
                    guard.set_turn_index(next);
                    next
                }
                Err(_) => 0,
            };
            orchestrator = orchestrator
                .with_file_change_tracker(Arc::clone(tracker))
                .with_file_change_turn_index(turn_index);
        }
        orchestrator = orchestrator
            .with_file_change_source_mode(
                crate::services::file_change_tracker::FileChangeSourceMode::Plan,
            )
            .with_file_change_actor_metadata(
                crate::services::file_change_tracker::FileChangeActorKind::RootAgent,
                Some(format!("plan-step:{}", step.id)),
                Some("Main Agent".to_string()),
                None,
                runtime.kernel_session_id.clone(),
            );
        let mut tools = resolve_step_tools(adapter, step);
        if matches!(retry_failure_category, Some(FailureCategory::MaxIterations)) {
            let narrowed = narrow_tools_for_max_iteration_retry(&tools);
            if !narrowed.is_empty() {
                tools = narrowed;
            }
        }
        if tools.is_empty() {
            return Err(AppError::Internal(
                "Step execution has no available tools after whitelist filtering".to_string(),
            ));
        }
        if let Some((provider_name, api_key)) = runtime.search_provider.as_ref() {
            orchestrator = orchestrator.with_search_provider(provider_name, api_key.clone());
        }
        if let Some(store) = runtime.index_store.as_ref() {
            orchestrator = orchestrator.with_index_store(Arc::clone(store));
        }
        if let Some(svc) = runtime.embedding_service.as_ref() {
            orchestrator = orchestrator.with_embedding_service(Arc::clone(svc));
        }
        if let Some(mgr) = runtime.embedding_manager.as_ref() {
            orchestrator = orchestrator.with_embedding_manager(Arc::clone(mgr));
        }
        if let Some(hnsw) = runtime.hnsw_index.as_ref() {
            orchestrator = orchestrator.with_hnsw_index(Arc::clone(hnsw));
        }
        if let Some(gate) = runtime.permission_gate.as_ref() {
            orchestrator = orchestrator.with_permission_gate(Arc::clone(gate));
        }
        if !runtime.selected_skills.is_empty() {
            let selected_skills = Arc::new(RwLock::new(runtime.selected_skills.clone()));
            orchestrator = orchestrator.with_selected_skills(selected_skills);
        }
        if let (Some(tx), Some(calc)) = (
            runtime.analytics_tx.as_ref(),
            runtime.analytics_cost_calculator.as_ref(),
        ) {
            orchestrator = orchestrator
                .with_analytics_tracker(tx.clone())
                .with_analytics_cost_calculator(Arc::clone(calc))
                .with_analytics_attribution(crate::models::analytics::AnalyticsAttribution {
                    project_id: None,
                    kernel_session_id: runtime.kernel_session_id.clone(),
                    mode_session_id: Some(runtime.mode_session_id.clone()),
                    workflow_mode: Some(crate::models::analytics::AnalyticsWorkflowMode::Plan),
                    phase_id: Some(runtime.phase_id.clone()),
                    execution_scope: Some(
                        crate::models::analytics::AnalyticsExecutionScope::RootAgent,
                    ),
                    execution_id: Some(format!(
                        "plan:{}:{}:{}",
                        runtime.mode_session_id, step.id, attempt_count
                    )),
                    parent_execution_id: None,
                    agent_role: Some("plan_step".to_string()),
                    agent_name: None,
                    step_id: Some(step.id.clone()),
                    story_id: None,
                    gate_id: None,
                    attempt: Some(attempt_count as i64),
                    request_sequence: Some(1),
                    call_site: Some("plan_step.execution".to_string()),
                    metadata_json: None,
                });
        }

        let cancel_bridge = orchestrator.cancellation_token();
        let cancel_observer = cancellation_token.clone();
        let cancel_watch = tokio::spawn(async move {
            cancel_observer.cancelled().await;
            cancel_bridge.cancel();
        });

        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(128);
        let drain_events = tokio::spawn(async move { while rx.recv().await.is_some() {} });

        let result = orchestrator.execute_story(&full_prompt, &tools, tx).await;
        let files_read_summary = orchestrator.get_read_file_summary();

        cancel_watch.abort();
        drain_events.abort();

        if cancellation_token.is_cancelled() {
            return Err(AppError::internal("Execution cancelled"));
        }
        if !result.success {
            let raw_error = result
                .error
                .unwrap_or_else(|| "unknown orchestrator error".to_string());
            let category = classify_failure_reason(&raw_error);
            return Err(AppError::Internal(format!(
                "[{}] Step execution failed: {}",
                category.as_error_code(),
                raw_error
            )));
        }

        let content = result
            .response
            .unwrap_or_else(|| "No output produced".to_string());
        let mut output = build_step_output(
            step.id.clone(),
            content,
            OutputFormat::Markdown,
            Some(build_step_evidence_bundle(
                dep_outputs,
                files_read_summary,
                &tools,
                result.iterations,
                result.error.as_deref(),
                1,
            )),
        );
        output.iterations = result.iterations;
        if let Some(err) = result.error.as_deref() {
            if err.contains("Iteration hard limit") {
                output.stop_reason = Some("iteration_hard_limit_recovered".to_string());
                output.error_code = Some("iteration_hard_limit_recovered".to_string());
            }
        }
        return Ok(output);
    }

    let messages = vec![Message::text(MessageRole::User, full_prompt)];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = tokio::select! {
        _ = cancellation_token.cancelled() => {
            return Err(AppError::internal("Execution cancelled"));
        }
        response = send_message_tracked(provider, messages, Some(system), vec![], options) => {
            response.map_err(|e| AppError::Internal(format!("Step execution LLM error: {e}")))?
        }
    };

    if cancellation_token.is_cancelled() {
        return Err(AppError::internal("Execution cancelled"));
    }

    let content = response
        .content
        .unwrap_or_else(|| "No output produced".to_string());
    let mut output = build_step_output(
        step.id.clone(),
        content,
        OutputFormat::Markdown,
        Some(build_step_evidence_bundle(
            dep_outputs,
            Vec::new(),
            &[],
            1,
            None,
            1,
        )),
    );
    output.iterations = 1;
    Ok(output)
}

fn truncate_context_for_system(context: &str, max_chars: usize) -> String {
    if context.chars().count() <= max_chars {
        return context.to_string();
    }
    let truncated: String = context.chars().take(max_chars).collect();
    format!("{}\n\n[Context truncated for budget]", truncated)
}

/// Truncate dependency outputs to fit within budget.
fn truncate_dep_outputs(
    deps: Vec<(String, StepOutput)>,
    max_per_dep: usize,
    max_total: usize,
) -> Vec<(String, StepOutput)> {
    let mut total_chars = 0;
    let mut result = Vec::new();

    for (title, mut output) in deps {
        if total_chars >= max_total {
            break;
        }

        let remaining = max_total - total_chars;
        let limit = remaining.min(max_per_dep);
        let original_chars = output.content.chars().count();

        if original_chars > limit {
            let truncated: String = output.content.chars().take(limit).collect();
            output.content = format!(
                "{}...\n[Truncated — {} chars total]",
                truncated, original_chars
            );
            output.truncated = true;
            output.shown_length = output.content.chars().count();
        }

        total_chars += output.content.chars().count();
        result.push((title, output));
    }

    result
}

fn detect_incomplete_output_reason(
    step: &super::types::PlanStep,
    output: &StepOutput,
    validation_result: &StepValidationResult,
) -> Option<String> {
    let primary = if output.full_content.trim().is_empty() {
        output.content.trim()
    } else {
        output.full_content.trim()
    };

    if primary.is_empty() {
        return Some("Output is empty".to_string());
    }

    let stripped_preface = strip_leading_narration_preface(primary);
    let candidate = stripped_preface.as_deref().unwrap_or(primary).trim();
    if candidate.is_empty() {
        return Some("Output is an execution narration rather than a completed result".to_string());
    }

    let min_expected_length = dynamic_min_expected_length(step, candidate);
    let has_structured_result = content_looks_structured(candidate);
    let has_artifact_evidence = has_delivery_evidence(candidate);
    let has_substantial_result = candidate.chars().count() >= min_expected_length
        || has_structured_result
        || has_artifact_evidence;
    let starts_with_pending =
        text_describes_pending_action(candidate) || starts_with_pending_narration(candidate);

    if starts_with_pending && stripped_preface.is_none() && !has_substantial_result {
        return Some("Output is an execution narration rather than a completed result".to_string());
    }

    if candidate.chars().count() < min_expected_length
        && !has_structured_result
        && !has_artifact_evidence
    {
        return Some(format!(
            "Output too short for expected result ({} chars < {})",
            candidate.chars().count(),
            min_expected_length
        ));
    }

    if matches!(
        validation_result.outcome_status,
        StepOutcomeStatus::HardFailed
    ) {
        return Some(validation_summary(validation_result));
    }

    None
}

fn apply_validation_outcome_to_output(
    output: &mut StepOutput,
    validation_result: &StepValidationResult,
) {
    output.validation_result = validation_result.clone();
    output.outcome_status = validation_result.outcome_status.clone();
    output.review_reason = validation_result.review_reason.clone();
    match validation_result.outcome_status {
        StepOutcomeStatus::Completed => {
            output.quality_state = StepOutputQualityState::Complete;
            output.incomplete_reason = None;
            output.error_code = None;
        }
        StepOutcomeStatus::SoftFailed => {
            output.quality_state = StepOutputQualityState::Incomplete;
            output.incomplete_reason = Some(validation_result.summary.clone());
            output.error_code = Some("soft_validation_failed".to_string());
        }
        StepOutcomeStatus::NeedsReview => {
            output.quality_state = StepOutputQualityState::Incomplete;
            output.incomplete_reason = Some(validation_result.summary.clone());
            output.error_code = Some("needs_review".to_string());
        }
        StepOutcomeStatus::HardFailed => {
            output.quality_state = StepOutputQualityState::Incomplete;
            output.incomplete_reason = Some(validation_result.summary.clone());
            output.error_code = Some("hard_validation_failed".to_string());
        }
    }
}

fn state_from_validation(
    validation_result: &StepValidationResult,
    duration_ms: u64,
) -> StepExecutionState {
    match validation_result.outcome_status {
        StepOutcomeStatus::Completed => StepExecutionState::Completed { duration_ms },
        StepOutcomeStatus::SoftFailed => StepExecutionState::SoftFailed {
            reason: validation_result.summary.clone(),
            duration_ms,
        },
        StepOutcomeStatus::NeedsReview => StepExecutionState::NeedsReview {
            reason: validation_result.summary.clone(),
            duration_ms,
        },
        StepOutcomeStatus::HardFailed => StepExecutionState::HardFailed {
            reason: validation_result.summary.clone(),
        },
    }
}

fn step_event_from_validation(
    session_id: &str,
    current_batch: usize,
    total_batches: usize,
    step_id: &str,
    step_output: StepOutput,
    progress_pct: f64,
    validation_result: &StepValidationResult,
) -> PlanModeProgressEvent {
    match validation_result.outcome_status {
        StepOutcomeStatus::Completed
        | StepOutcomeStatus::SoftFailed
        | StepOutcomeStatus::NeedsReview => PlanModeProgressEvent::step_completed(
            session_id,
            current_batch,
            total_batches,
            step_id,
            step_output,
            progress_pct,
        ),
        StepOutcomeStatus::HardFailed => PlanModeProgressEvent::step_failed_with_output(
            session_id,
            current_batch,
            total_batches,
            step_id,
            &validation_result.summary,
            step_output,
            progress_pct,
        ),
    }
}

fn dynamic_min_expected_length(step: &super::types::PlanStep, candidate: &str) -> usize {
    let criteria_count = step.completion_criteria.len();
    let base: usize = if criteria_count > 3 || step.expected_output.chars().count() > 120 {
        120
    } else if criteria_count > 1 || step.expected_output.chars().count() > 24 {
        90
    } else {
        60
    };
    if content_looks_structured(candidate) {
        base.saturating_sub(20)
    } else {
        base
    }
}

fn has_delivery_evidence(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    content.contains("```")
        || content.contains("## ")
        || content.contains("- ")
        || content.contains("1.")
        || lower.contains("test plan")
        || lower.contains("validation")
        || lower.contains("verify")
        || lower.contains("file:")
        || lower.contains("path:")
        || lower.contains("src/")
        || lower.contains(".tsx")
        || lower.contains(".ts")
        || lower.contains(".rs")
}

fn classify_failure_reason(reason: &str) -> FailureCategory {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("execution narration rather than a completed result") {
        return FailureCategory::IncompleteNarration;
    }
    if lower.contains("completion criteria unmet") {
        return FailureCategory::CriteriaUnmet;
    }
    if lower.contains("iteration hard limit") || lower.contains("iteration stalled") {
        return FailureCategory::MaxIterations;
    }
    FailureCategory::InternalError
}

fn compute_step_iteration_limit(step: &super::types::PlanStep, global_max: u32) -> u32 {
    let expected_len = step.expected_output.chars().count();
    let expected_output_len_bucket = if expected_len <= 24 {
        0
    } else if expected_len <= 120 {
        1
    } else if expected_len <= 280 {
        2
    } else {
        3
    };
    let score =
        (step.completion_criteria.len() * 2) + step.dependencies.len() + expected_output_len_bucket;
    let recommended = if score <= 4 {
        24
    } else if score <= 8 {
        36
    } else {
        48
    };
    recommended.min(global_max.clamp(12, 96))
}

fn narrow_tools_for_max_iteration_retry(tools: &[ToolDefinition]) -> Vec<ToolDefinition> {
    let preferred = ["Read", "Write", "Edit", "Grep", "LS", "Glob", "Cwd"];
    tools
        .iter()
        .filter(|tool| preferred.iter().any(|name| *name == tool.name))
        .cloned()
        .collect()
}

async fn update_failure_streaks_for_batch(
    states: &Arc<RwLock<HashMap<String, StepExecutionState>>>,
    step_ids: &[String],
    history: &mut HashMap<String, (FailureCategory, String, usize)>,
) {
    let snapshot = states.read().await;
    for step_id in step_ids {
        let Some(StepExecutionState::HardFailed { reason }) = snapshot.get(step_id) else {
            history.remove(step_id);
            continue;
        };
        let category = classify_failure_reason(reason);
        let fingerprint = format!("{}::{reason}", category.as_error_code());
        let streak = history
            .get(step_id)
            .map(|(_, prev, prev_streak)| {
                if prev == &fingerprint {
                    prev_streak.saturating_add(1)
                } else {
                    1
                }
            })
            .unwrap_or(1);
        history.insert(step_id.clone(), (category, fingerprint, streak));
    }
}

fn strip_leading_narration_preface(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    let (first_line, remainder) = trimmed.split_once('\n')?;
    let first_line = first_line.trim();
    if first_line.is_empty() {
        return None;
    }
    let is_narration_preface =
        text_describes_pending_action(first_line) || starts_with_pending_narration(first_line);
    if !is_narration_preface {
        return None;
    }

    let mut body = remainder.trim_start();
    if body.starts_with("---") || body.starts_with("***") || body.starts_with("```") {
        if let Some((_, after)) = body.split_once('\n') {
            body = after.trim_start();
        }
    }
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

fn content_looks_structured(content: &str) -> bool {
    let non_empty_lines = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    non_empty_lines >= 3
        || content.contains('\n')
            && (content.contains("- ")
                || content.contains("* ")
                || content.contains("1.")
                || content.contains("##")
                || content.contains("```"))
}

fn starts_with_pending_narration(content: &str) -> bool {
    let normalized = content.trim().to_lowercase();
    let en_prefixes = [
        "let me ",
        "i will ",
        "i'll ",
        "i am going to ",
        "i'm going to ",
        "i need to ",
    ];
    if en_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return true;
    }

    let zh_prefixes = ["让我", "我将", "我会", "我先", "接下来", "下一步"];
    zh_prefixes
        .iter()
        .any(|prefix| content.trim_start().starts_with(prefix))
}

fn progress_pct_from_states(
    states: &HashMap<String, StepExecutionState>,
    total_steps: usize,
) -> f64 {
    if total_steps == 0 {
        return 0.0;
    }
    let completed = states
        .values()
        .filter(|state| counts_as_completed(state))
        .count();
    (completed as f64 / total_steps as f64) * 100.0
}

fn counts_as_completed(state: &StepExecutionState) -> bool {
    matches!(
        state,
        StepExecutionState::Completed { .. }
            | StepExecutionState::SoftFailed { .. }
            | StepExecutionState::NeedsReview { .. }
    )
}

fn is_hard_failed(state: &StepExecutionState) -> bool {
    matches!(state, StepExecutionState::HardFailed { .. })
}

fn compute_retry_stats(
    step_attempts: &HashMap<String, usize>,
    step_states: &HashMap<String, StepExecutionState>,
) -> PlanRetryStats {
    let mut total_retries = 0usize;
    let mut steps_retried = 0usize;
    for attempts in step_attempts.values() {
        if *attempts > 1 {
            steps_retried += 1;
            total_retries += attempts.saturating_sub(1);
        }
    }

    let exhausted_failures = step_states
        .iter()
        .filter(|(step_id, state)| {
            matches!(state, StepExecutionState::HardFailed { .. })
                && step_attempts
                    .get(step_id.as_str())
                    .map(|attempts| *attempts > 1)
                    .unwrap_or(false)
        })
        .count();

    PlanRetryStats {
        total_retries,
        steps_retried,
        exhausted_failures,
    }
}

/// Emit a plan mode progress event.
fn emit_event(handle: &tauri::AppHandle, event: PlanModeProgressEvent) {
    let _ = handle.emit(PLAN_MODE_EVENT_CHANNEL, &event);
    let app_handle = handle.clone();
    tokio::spawn(async move {
        append_plan_progress_transcript(&app_handle, &event).await;
    });
}

fn emit_event_with_metadata(
    handle: &tauri::AppHandle,
    event: PlanModeProgressEvent,
    run_id: &str,
    event_seq: &AtomicU64,
) {
    let seq = next_event_seq(event_seq);
    emit_event(
        handle,
        event.with_observability(run_id, seq, "plan_step_executor"),
    );
}

fn next_event_seq(counter: &AtomicU64) -> u64 {
    counter.fetch_add(1, Ordering::Relaxed)
}

fn transcript_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

fn build_card_transcript_line(card_type: &str, data: Value, interactive: bool) -> Value {
    let timestamp = transcript_timestamp();
    let card_payload = json!({
        "cardType": card_type,
        "cardId": format!("{card_type}-{timestamp}"),
        "data": data,
        "interactive": interactive,
    });
    let content = serde_json::to_string(&card_payload).unwrap_or_else(|_| "{}".to_string());
    json!({
        "id": timestamp,
        "type": "card",
        "content": content,
        "timestamp": timestamp,
        "cardPayload": card_payload,
    })
}

fn output_format_label(format: &OutputFormat) -> &'static str {
    match format {
        OutputFormat::Text => "text",
        OutputFormat::Markdown => "markdown",
        OutputFormat::Json => "json",
        OutputFormat::Html => "html",
        OutputFormat::Code => "code",
    }
}

fn build_plan_progress_transcript_lines(event: &PlanModeProgressEvent) -> Vec<Value> {
    match event.event_type.as_str() {
        "batch_started" | "step_started" | "step_failed" | "step_retrying" | "batch_blocked" => {
            vec![build_card_transcript_line(
                "plan_step_update",
                json!({
                    "eventType": event.event_type,
                    "currentBatch": event.current_batch,
                    "totalBatches": event.total_batches,
                    "stepId": event.step_id,
                    "stepTitle": event.step_id,
                    "stepStatus": event.step_status,
                    "progressPct": event.progress_pct,
                    "error": event.error,
                    "attemptCount": event.attempt_count,
                    "errorCode": event.error_code,
                    "diagnostics": event.step_output.as_ref().map(|output| json!({
                        "summary": output.summary,
                        "content": output.content,
                        "fullContent": output.full_content,
                        "format": output_format_label(&output.format),
                        "truncated": output.truncated,
                        "originalLength": output.original_length,
                        "shownLength": output.shown_length,
                        "qualityState": match output.quality_state {
                            StepOutputQualityState::Complete => "complete",
                            StepOutputQualityState::Incomplete => "incomplete",
                        },
                        "incompleteReason": output.incomplete_reason,
                        "attemptCount": output.attempt_count,
                        "toolEvidence": output.tool_evidence,
                        "iterations": output.iterations,
                        "stopReason": output.stop_reason,
                        "errorCode": output.error_code,
                    })),
                }),
                false,
            )]
        }
        "step_completed" => {
            let mut lines = vec![build_card_transcript_line(
                "plan_step_update",
                json!({
                    "eventType": "step_completed",
                    "currentBatch": event.current_batch,
                    "totalBatches": event.total_batches,
                    "stepId": event.step_id,
                    "stepTitle": event.step_id,
                    "stepStatus": event.step_status,
                    "progressPct": event.progress_pct,
                    "error": event.error,
                    "attemptCount": event.attempt_count,
                    "errorCode": event.error_code,
                }),
                false,
            )];
            if let Some(output) = event.step_output.as_ref() {
                let criteria_met: Vec<Value> = output
                    .criteria_met
                    .iter()
                    .map(|criterion| {
                        json!({
                            "criterion": criterion.criterion,
                            "met": criterion.met,
                            "explanation": criterion.explanation,
                        })
                    })
                    .collect();
                lines.push(build_card_transcript_line(
                    "plan_step_output",
                    json!({
                        "stepId": output.step_id,
                        "stepTitle": output.step_id,
                        "summary": output.summary,
                        "content": output.content,
                        "fullContent": output.full_content,
                        "format": output_format_label(&output.format),
                        "truncated": output.truncated,
                        "originalLength": output.original_length,
                        "shownLength": output.shown_length,
                        "artifacts": output.artifacts,
                        "qualityState": match output.quality_state {
                            StepOutputQualityState::Complete => "complete",
                            StepOutputQualityState::Incomplete => "incomplete",
                        },
                        "incompleteReason": output.incomplete_reason,
                        "attemptCount": output.attempt_count,
                        "toolEvidence": output.tool_evidence,
                        "iterations": output.iterations,
                        "stopReason": output.stop_reason,
                        "errorCode": output.error_code,
                        "criteriaMet": criteria_met,
                    }),
                    false,
                ));
            }
            lines
        }
        "execution_cancelled" => vec![build_card_transcript_line(
            "workflow_info",
            json!({
                "message": "Plan execution cancelled.",
                "level": "warning",
            }),
            false,
        )],
        _ => Vec::new(),
    }
}

async fn append_plan_progress_transcript(app: &tauri::AppHandle, event: &PlanModeProgressEvent) {
    let kernel_state = app.state::<WorkflowKernelState>().inner().clone();
    let lines = build_plan_progress_transcript_lines(event);
    if lines.is_empty() {
        return;
    }

    let kernel_session_ids = kernel_state
        .linked_kernel_sessions_for_mode_session(WorkflowMode::Plan, &event.session_id)
        .await;
    if kernel_session_ids.is_empty() {
        return;
    }

    for kernel_session_id in &kernel_session_ids {
        if let Ok(transcript) = kernel_state
            .append_mode_transcript(kernel_session_id, WorkflowMode::Plan, lines.clone())
            .await
        {
            let _ = app.emit(
                WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL,
                WorkflowModeTranscriptUpdatedEvent {
                    session_id: transcript.session_id,
                    mode: transcript.mode,
                    revision: transcript.revision,
                    appended_lines: lines.clone(),
                    replace_from_line_id: None,
                    lines: transcript.lines.clone(),
                    source: "plan_step_executor.progress_event".to_string(),
                },
            );
        }
    }

    if let Ok(catalog_state) = kernel_state.get_session_catalog_state().await {
        let _ = app.emit(
            WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL,
            WorkflowSessionCatalogUpdatedEvent {
                active_session_id: catalog_state.active_session_id,
                sessions: catalog_state.sessions,
                source: "plan_step_executor.progress_event".to_string(),
            },
        );
    }
}

fn emit_step_output_diagnostic_log(
    stage: &str,
    run_id: &str,
    session_id: &str,
    batch_index: usize,
    total_batches: usize,
    step_id: &str,
    step_title: &str,
    output: &StepOutput,
) {
    tracing::info!(
        target: "plan_mode_diagnostics",
        stage,
        run_id,
        session_id,
        batch_index,
        total_batches,
        step_id,
        step_title,
        attempt_count = output.attempt_count,
        quality_state = ?output.quality_state,
        incomplete_reason = ?output.incomplete_reason,
        original_length = output.original_length,
        shown_length = output.shown_length,
        truncated = output.truncated,
        iterations = output.iterations,
        stop_reason = ?output.stop_reason,
        error_code = ?output.error_code,
        full_content = %output.full_content,
        summary = %output.summary,
        "plan step output diagnostic"
    );
}

fn build_step_output(
    step_id: String,
    content: String,
    format: OutputFormat,
    evidence_bundle: Option<StepEvidenceBundle>,
) -> StepOutput {
    let original_length = content.chars().count();
    let summary = summarize_output(&content);
    let artifacts = extract_artifacts(&content);
    let tool_evidence = extract_tool_evidence(&content, &artifacts);
    let evidence_bundle = evidence_bundle
        .unwrap_or_else(|| build_step_evidence_bundle(&[], Vec::new(), &[], 0, None, 1));
    StepOutput {
        step_id,
        summary,
        full_content: content.clone(),
        content,
        format,
        criteria_met: vec![],
        artifacts,
        truncated: false,
        original_length,
        shown_length: original_length,
        quality_state: StepOutputQualityState::Complete,
        incomplete_reason: None,
        attempt_count: 1,
        tool_evidence,
        iterations: 0,
        stop_reason: None,
        error_code: None,
        evidence_summary: super::validation_engine::summarize_evidence(&evidence_bundle),
        evidence_bundle,
        validation_result: StepValidationResult::default(),
        outcome_status: StepOutcomeStatus::Completed,
        review_reason: None,
    }
}

fn build_step_evidence_bundle(
    dep_outputs: &[(String, StepOutput)],
    files_read_summary: Vec<(String, usize, u64)>,
    tools: &[ToolDefinition],
    iterations: u32,
    stop_reason: Option<&str>,
    attempt_count: usize,
) -> StepEvidenceBundle {
    let coverage_markers = dep_outputs
        .iter()
        .map(|(title, _)| format!("dependency:{title}"))
        .collect::<Vec<_>>();
    let files_read = files_read_summary
        .into_iter()
        .map(|(path, read_count, bytes)| StepFileReadEvidence {
            path,
            read_count,
            bytes,
            matched_required_path: false,
        })
        .collect::<Vec<_>>();
    let artifacts = dep_outputs
        .iter()
        .flat_map(|(_, output)| output.artifacts.iter().cloned())
        .map(|value| StepArtifactEvidence {
            artifact_type: artifact_type_from_value(&value).to_string(),
            value,
        })
        .collect::<Vec<_>>();
    let tool_calls = tools
        .iter()
        .map(|tool| StepToolCallEvidence {
            tool_name: tool.name.clone(),
            args_summary: "available_in_step".to_string(),
            timestamp_ms: transcript_timestamp(),
        })
        .collect::<Vec<_>>();

    StepEvidenceBundle {
        tool_calls,
        files_read,
        files_written: Vec::new(),
        search_queries: Vec::new(),
        artifacts,
        dependency_inputs: dep_outputs.iter().map(|(title, _)| title.clone()).collect(),
        runtime_stats: StepRuntimeStats {
            iterations,
            stop_reason: stop_reason.map(|value| value.to_string()),
            attempt_count,
        },
        coverage_markers,
    }
}

fn artifact_type_from_value(value: &str) -> &'static str {
    if value.ends_with(".md") {
        "report"
    } else if value.ends_with(".json") {
        "json"
    } else if value.ends_with(".rs")
        || value.ends_with(".ts")
        || value.ends_with(".tsx")
        || value.ends_with(".js")
    {
        "code"
    } else {
        "artifact"
    }
}

fn summarize_output(content: &str) -> String {
    let first_non_empty_line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("No output produced");
    let mut summary = first_non_empty_line.to_string();
    if summary.chars().count() > 200 {
        summary = format!(
            "{}...",
            summary.chars().take(200).collect::<String>().trim_end()
        );
    }
    summary
}

fn extract_artifacts(content: &str) -> Vec<String> {
    let mut artifacts: Vec<String> = Vec::new();
    let mut in_tick = false;
    let mut current = String::new();
    for ch in content.chars() {
        if ch == '`' {
            if in_tick {
                let candidate = current.trim();
                if is_artifact_candidate(candidate)
                    && !artifacts.iter().any(|value| value == candidate)
                {
                    artifacts.push(candidate.to_string());
                }
                current.clear();
                in_tick = false;
            } else {
                current.clear();
                in_tick = true;
            }
            continue;
        }
        if in_tick {
            current.push(ch);
        }
    }

    if artifacts.len() > 12 {
        artifacts.truncate(12);
    }
    artifacts
}

fn extract_tool_evidence(content: &str, artifacts: &[String]) -> Vec<String> {
    let mut evidence = Vec::new();
    if content.contains("```") {
        evidence.push("code_block".to_string());
    }
    if content.contains("http://") || content.contains("https://") {
        evidence.push("url_reference".to_string());
    }
    if content.contains("Read(") || content.contains("Write(") || content.contains("Grep(") {
        evidence.push("tool_call_mention".to_string());
    }
    for artifact in artifacts.iter().take(6) {
        evidence.push(format!("artifact:{artifact}"));
    }
    if evidence.len() > 10 {
        evidence.truncate(10);
    }
    evidence
}

fn is_artifact_candidate(candidate: &str) -> bool {
    if candidate.is_empty() || candidate.len() > 260 {
        return false;
    }
    if candidate.contains('\n') {
        return false;
    }
    candidate.contains('/')
        || candidate.ends_with(".md")
        || candidate.ends_with(".ts")
        || candidate.ends_with(".tsx")
        || candidate.ends_with(".rs")
        || candidate.ends_with(".json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::types::{
        LlmError, LlmRequestOptions, LlmResponse, LlmResult, Message, ProviderConfig, StopReason,
        ToolDefinition, UsageStats,
    };
    use crate::services::plan_mode::adapters::general::GeneralAdapter;
    use crate::services::plan_mode::types::{PlanStep, StepPriority};
    use async_trait::async_trait;
    use plan_cascade_core::streaming::UnifiedStreamEvent;
    use std::collections::HashMap as StdHashMap;
    use tokio::sync::mpsc;

    struct MockRetryProvider {
        config: ProviderConfig,
        content: String,
    }

    impl MockRetryProvider {
        fn new(content: impl Into<String>) -> Self {
            Self {
                config: ProviderConfig::default(),
                content: content.into(),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockRetryProvider {
        fn name(&self) -> &'static str {
            "mock-retry-provider"
        }

        fn model(&self) -> &str {
            &self.config.model
        }

        fn supports_thinking(&self) -> bool {
            false
        }

        fn supports_tools(&self) -> bool {
            false
        }

        async fn send_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.content.clone()),
                thinking: None,
                tool_calls: vec![],
                stop_reason: StopReason::EndTurn,
                usage: UsageStats::default(),
                model: self.config.model.clone(),
                search_citations: vec![],
            })
        }

        async fn stream_message(
            &self,
            _messages: Vec<Message>,
            _system: Option<String>,
            _tools: Vec<ToolDefinition>,
            _tx: mpsc::Sender<UnifiedStreamEvent>,
            _request_options: LlmRequestOptions,
        ) -> LlmResult<LlmResponse> {
            Err(LlmError::Other {
                message: "streaming not needed in this test".to_string(),
            })
        }

        async fn health_check(&self) -> LlmResult<()> {
            Ok(())
        }

        fn config(&self) -> &ProviderConfig {
            &self.config
        }
    }

    fn sample_step(id: &str, title: &str, description: &str, priority: StepPriority) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            priority,
            dependencies: vec![],
            deliverable: Default::default(),
            evidence_requirements: Default::default(),
            quality_requirements: Default::default(),
            validation_profile: Default::default(),
            failure_policy: Default::default(),
            completion_criteria: vec![],
            expected_output: String::new(),
            metadata: StdHashMap::new(),
        }
    }

    fn sample_output(step_id: &str, content: &str, format: OutputFormat) -> StepOutput {
        let text = content.to_string();
        let len = text.len();
        StepOutput {
            step_id: step_id.to_string(),
            content: text.clone(),
            summary: "summary".to_string(),
            full_content: text,
            format,
            criteria_met: vec![],
            artifacts: vec![],
            truncated: false,
            original_length: len,
            shown_length: len,
            quality_state: StepOutputQualityState::Complete,
            incomplete_reason: None,
            attempt_count: 1,
            tool_evidence: vec![],
            iterations: 0,
            stop_reason: None,
            error_code: None,
            evidence_bundle: Default::default(),
            evidence_summary: Default::default(),
            validation_result: Default::default(),
            outcome_status: StepOutcomeStatus::Completed,
            review_reason: None,
        }
    }

    #[test]
    fn test_truncate_dep_outputs() {
        fn make_output(step_id: &str, text: String) -> StepOutput {
            let len = text.len();
            StepOutput {
                step_id: step_id.to_string(),
                content: text.clone(),
                summary: text.chars().take(200).collect(),
                full_content: text,
                format: OutputFormat::Text,
                criteria_met: vec![],
                artifacts: vec![],
                truncated: false,
                original_length: len,
                shown_length: len,
                quality_state: StepOutputQualityState::Complete,
                incomplete_reason: None,
                attempt_count: 1,
                tool_evidence: vec![],
                iterations: 0,
                stop_reason: None,
                error_code: None,
                evidence_bundle: Default::default(),
                evidence_summary: Default::default(),
                validation_result: Default::default(),
                outcome_status: StepOutcomeStatus::Completed,
                review_reason: None,
            }
        }

        let deps = vec![
            ("Step 1".to_string(), make_output("s1", "A".repeat(5000))),
            ("Step 2".to_string(), make_output("s2", "B".repeat(3000))),
        ];

        let result = truncate_dep_outputs(deps, 4000, 6000);
        assert_eq!(result.len(), 2);
        assert!(result[0].1.content.len() <= 4100); // 4000 + truncation message
        assert!(result[1].1.content.len() <= 2100); // Remaining budget
    }

    #[test]
    fn test_build_progress() {
        let mut states = HashMap::new();
        states.insert(
            "s1".to_string(),
            StepExecutionState::Completed { duration_ms: 100 },
        );
        states.insert(
            "s2".to_string(),
            StepExecutionState::HardFailed {
                reason: "err".to_string(),
            },
        );
        states.insert("s3".to_string(), StepExecutionState::Pending);

        let progress = build_progress(&states, 1, 2, 3);
        assert_eq!(progress.steps_completed, 1);
        assert_eq!(progress.steps_failed, 1);
        assert_eq!(progress.total_steps, 3);
        assert!((progress.progress_pct - 33.33).abs() < 1.0);
    }

    #[test]
    fn test_resolve_step_tools_maps_plan_tool_whitelist() {
        let adapter = GeneralAdapter;
        let step = sample_step(
            "s1",
            "Inspect codebase",
            "Read relevant code",
            StepPriority::Medium,
        );

        let tools = resolve_step_tools(&adapter, &step);
        let names: HashSet<String> = tools.into_iter().map(|tool| tool.name).collect();
        assert!(names.contains("CodebaseSearch"));
        assert!(names.contains("Read"));
        assert!(names.contains("Grep"));
        assert!(names.contains("LS"));
    }

    fn build_single_step_plan(step: super::super::types::PlanStep) -> Plan {
        Plan {
            title: "Retry Plan".to_string(),
            description: "Retry test plan".to_string(),
            domain: super::super::types::TaskDomain::General,
            adapter_name: "general".to_string(),
            batches: vec![super::super::types::PlanBatch {
                index: 0,
                step_ids: vec![step.id.clone()],
            }],
            steps: vec![step],
            execution_config: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_retry_single_step_success_updates_output_and_state() {
        let mut step = sample_step("s1", "Retry Step", "Run retry", StepPriority::Medium);
        step.completion_criteria = vec!["done".to_string()];
        step.expected_output = "output".to_string();
        let plan = build_single_step_plan(step);
        let mut states = HashMap::new();
        states.insert(
            "s1".to_string(),
            StepExecutionState::HardFailed {
                reason: "previous failure".to_string(),
            },
        );

        let (outputs, states) = retry_single_step(
            "sid-1",
            &plan,
            "s1",
            HashMap::new(),
            states,
            Arc::new(GeneralAdapter),
            Arc::new(MockRetryProvider::new(
                "Result summary:\n- Completed the retry step successfully.\n- Provided concrete output content for validation.\n- Confirmed all required criteria are addressed.",
            )),
            None,
            StepExecutionConfig::default(),
            None,
            "Use English".to_string(),
            None,
            CancellationToken::new(),
        )
        .await
        .expect("retry should succeed");

        assert!(matches!(
            states.get("s1"),
            Some(StepExecutionState::Completed { .. })
        ));
        assert_eq!(
            outputs
                .get("s1")
                .and_then(|output| output.content.split('\n').next()),
            Some("Result summary:")
        );
    }

    #[tokio::test]
    async fn test_retry_single_step_fails_when_dependency_output_missing() {
        let mut step = sample_step(
            "s2",
            "Dependent Step",
            "Needs prior output",
            StepPriority::Medium,
        );
        step.dependencies = vec!["s1".to_string()];
        step.expected_output = "output".to_string();
        let plan = build_single_step_plan(step);
        let mut states = HashMap::new();
        states.insert(
            "s2".to_string(),
            StepExecutionState::HardFailed {
                reason: "previous failure".to_string(),
            },
        );

        let (_outputs, states) = retry_single_step(
            "sid-1",
            &plan,
            "s2",
            HashMap::new(),
            states,
            Arc::new(GeneralAdapter),
            Arc::new(MockRetryProvider::new("unused")),
            None,
            StepExecutionConfig::default(),
            None,
            "Use English".to_string(),
            None,
            CancellationToken::new(),
        )
        .await
        .expect("retry call should return updated states");

        match states.get("s2") {
            Some(StepExecutionState::HardFailed { reason }) => {
                assert!(reason.contains("Missing dependency outputs"));
                assert!(reason.contains("s1"));
            }
            other => panic!("expected failed state, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_retry_single_step_marks_cancelled_when_token_is_cancelled() {
        let mut step = sample_step("s1", "Retry Step", "Run retry", StepPriority::Medium);
        step.expected_output = "output".to_string();
        let plan = build_single_step_plan(step);
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let (_outputs, states) = retry_single_step(
            "sid-1",
            &plan,
            "s1",
            HashMap::new(),
            HashMap::new(),
            Arc::new(GeneralAdapter),
            Arc::new(MockRetryProvider::new("unused")),
            None,
            StepExecutionConfig::default(),
            None,
            "Use English".to_string(),
            None,
            cancel_token,
        )
        .await
        .expect("retry call should return updated states");

        assert!(matches!(
            states.get("s1"),
            Some(StepExecutionState::Cancelled)
        ));
    }

    #[test]
    fn test_detect_incomplete_output_allows_preface_with_substantial_result() {
        let mut step = sample_step(
            "s1",
            "Deliver plan",
            "Create final plan output",
            StepPriority::Medium,
        );
        step.completion_criteria = vec!["Provide roadmap".to_string()];
        step.expected_output = "Detailed markdown plan".to_string();
        let output = sample_output(
            "s1",
            "由于写入功能暂时不可用，我将直接输出完整文档：\n\n---\n\n# 方案\n- 目标\n- 路线\n- 验收",
            OutputFormat::Markdown,
        );

        let reason = detect_incomplete_output_reason(&step, &output, &output.validation_result);
        assert!(
            reason.is_none(),
            "substantial content after narration preface should pass quality gate"
        );
    }

    #[test]
    fn test_detect_incomplete_output_rejects_pure_narration() {
        let mut step = sample_step(
            "s1",
            "Deliver plan",
            "Create final plan output",
            StepPriority::Medium,
        );
        step.expected_output = "Detailed markdown plan".to_string();
        let output = sample_output(
            "s1",
            "让我先整理一下，然后我会给出最终方案。",
            OutputFormat::Markdown,
        );

        let reason = detect_incomplete_output_reason(&step, &output, &output.validation_result);
        assert!(matches!(
            reason.as_deref(),
            Some("Output is an execution narration rather than a completed result")
        ));
    }

    #[test]
    fn test_compute_step_iteration_limit_respects_cap() {
        let mut step = sample_step("s-max", "Complex", "complex", StepPriority::High);
        step.dependencies = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        step.completion_criteria = vec![
            "c1".to_string(),
            "c2".to_string(),
            "c3".to_string(),
            "c4".to_string(),
        ];
        step.expected_output = "X".repeat(400);
        assert_eq!(compute_step_iteration_limit(&step, 96), 48);
        assert_eq!(compute_step_iteration_limit(&step, 36), 36);
    }

    #[test]
    fn test_build_terminal_report_cancelled_tracks_attempted_and_failed_before_cancel() {
        let plan = Plan {
            title: "Cancel Plan".to_string(),
            description: "desc".to_string(),
            domain: super::super::types::TaskDomain::General,
            adapter_name: "general".to_string(),
            execution_config: Default::default(),
            batches: vec![super::super::types::PlanBatch {
                index: 0,
                step_ids: vec!["s1".to_string(), "s2".to_string(), "s3".to_string()],
            }],
            steps: vec![
                sample_step("s1", "one", "one", StepPriority::Medium),
                sample_step("s2", "two", "two", StepPriority::Medium),
                sample_step("s3", "three", "three", StepPriority::Medium),
            ],
        };

        let mut states = HashMap::new();
        states.insert(
            "s1".to_string(),
            StepExecutionState::Completed { duration_ms: 120 },
        );
        states.insert(
            "s2".to_string(),
            StepExecutionState::HardFailed {
                reason: "failed".to_string(),
            },
        );
        states.insert("s3".to_string(), StepExecutionState::Cancelled);

        let report = build_terminal_report(
            "sid-1",
            &plan,
            &HashMap::new(),
            &states,
            "cancelled",
            None,
            "run-1",
            PlanRetryStats {
                total_retries: 0,
                steps_retried: 0,
                exhausted_failures: 0,
            },
            &GeneralAdapter,
        );

        assert_eq!(report.terminal_state, "cancelled");
        assert_eq!(report.steps_completed, 1);
        assert_eq!(report.steps_failed, 1);
        assert_eq!(report.steps_cancelled, 1);
        assert_eq!(report.steps_attempted, 3);
        assert_eq!(report.steps_failed_before_cancel, 1);
        assert_eq!(report.cancelled_by.as_deref(), Some("user"));
    }

    #[test]
    fn test_detect_incomplete_output_allows_short_structured_delivery_evidence() {
        let mut step = sample_step("s2", "Deliver", "deliver", StepPriority::Medium);
        step.completion_criteria = vec!["Provide output".to_string()];
        step.expected_output = "markdown".to_string();
        let mut output = sample_output(
            "s2",
            "## Result\n- file: src/app.tsx\n- validation: run pnpm test",
            OutputFormat::Markdown,
        );
        output.artifacts = vec!["src/app.tsx".to_string()];
        assert!(
            detect_incomplete_output_reason(&step, &output, &output.validation_result).is_none()
        );
    }
}
