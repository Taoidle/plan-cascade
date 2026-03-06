//! Workflow Kernel v2 Tauri Commands
//!
//! Unified command surface for Chat/Plan/Task workflow sessions.

use crate::models::CommandResponse;
use crate::services::plan_mode::types::{
    ClarificationInputType, PlanModePhase, PlanModeSession, StepExecutionState,
};
use crate::services::spec_interview::interview::InterviewSession;
use crate::services::workflow_kernel::{
    observability::{
        self, WorkflowFailureRecordInput, WorkflowObservabilitySnapshot,
    },
    HandoffContextBundle, PlanClarificationSnapshot, PlanEditOperation, PlanSnapshotRehydrate,
    TaskInterviewSnapshot, TaskSnapshotRehydrate, UserInputIntent, WorkflowKernelState,
    WorkflowKernelUpdatedEvent, WorkflowMode, WorkflowSession, WorkflowSessionState,
    WorkflowStatus, WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
use crate::{commands::plan_mode::PlanModeState, commands::spec_interview::SpecInterviewState};
use crate::{commands::task_mode::TaskModeState, state::AppState};
use serde::Deserialize;
use serde_json::json;
use tauri::Emitter;

fn build_kernel_update(
    session_state: WorkflowSessionState,
    source: &str,
) -> WorkflowKernelUpdatedEvent {
    let revision = (session_state.events.len() + session_state.checkpoints.len()) as u64;
    WorkflowKernelUpdatedEvent {
        session_state,
        revision,
        source: source.to_string(),
    }
}

fn emit_kernel_update(
    app: &tauri::AppHandle,
    session_state: WorkflowSessionState,
    source: &str,
) -> Result<(), String> {
    app.emit(
        WORKFLOW_KERNEL_UPDATED_CHANNEL,
        &build_kernel_update(session_state, source),
    )
    .map_err(|err| format!("Failed to emit workflow kernel update: {err}"))
}

fn workflow_mode_label(mode: WorkflowMode) -> &'static str {
    match mode {
        WorkflowMode::Chat => "chat",
        WorkflowMode::Plan => "plan",
        WorkflowMode::Task => "task",
    }
}

fn session_phase_for_mode(session: &WorkflowSession, mode: WorkflowMode) -> Option<String> {
    match mode {
        WorkflowMode::Chat => session.mode_snapshots.chat.as_ref().map(|chat| chat.phase.clone()),
        WorkflowMode::Plan => session.mode_snapshots.plan.as_ref().map(|plan| plan.phase.clone()),
        WorkflowMode::Task => session.mode_snapshots.task.as_ref().map(|task| task.phase.clone()),
    }
}

async fn emit_kernel_update_for_session(
    app: &tauri::AppHandle,
    state: &WorkflowKernelState,
    session_id: &str,
    source: &str,
) -> Result<(), String> {
    let session_state = state.get_session_state(session_id).await?;
    emit_kernel_update(app, session_state, source)
}

async fn link_mode_session_and_rehydrate(
    session_id: &str,
    mode: WorkflowMode,
    mode_session_id: &str,
    state: &WorkflowKernelState,
    plan_mode_state: &PlanModeState,
    task_mode_state: &TaskModeState,
    interview_context: Option<(&SpecInterviewState, &AppState)>,
) -> Result<WorkflowSession, String> {
    let linked_session = state
        .link_mode_session(session_id, mode, mode_session_id)
        .await?;

    match mode {
        WorkflowMode::Plan => {
            let plan_session = plan_mode_state
                .get_or_load_session_snapshot(mode_session_id)
                .await
                .map_err(|error| {
                    format!(
                        "Failed to load linked plan session '{}': {}",
                        mode_session_id, error
                    )
                })?
                .ok_or_else(|| {
                    format!(
                        "Linked plan session '{}' not found for rehydrate",
                        mode_session_id
                    )
                })?;
            let snapshot = map_plan_session_to_rehydrate(&plan_session);
            state
                .sync_plan_snapshot_by_linked_session(
                    mode_session_id,
                    snapshot.phase.clone(),
                    snapshot.pending_clarification.clone(),
                    snapshot.running_step_id.clone(),
                    None,
                )
                .await
                .map_err(|error| {
                    format!(
                        "Failed to sync linked plan snapshot '{}': {}",
                        mode_session_id, error
                    )
                })?;
        }
        WorkflowMode::Task => {
            let task_session = task_mode_state
                .get_or_load_session_snapshot(mode_session_id)
                .await
                .map_err(|error| {
                    format!(
                        "Failed to load linked task session '{}': {}",
                        mode_session_id, error
                    )
                })?
                .ok_or_else(|| {
                    format!(
                        "Linked task session '{}' not found for rehydrate",
                        mode_session_id
                    )
                })?;

            let interview_session_id = linked_session
                .mode_snapshots
                .task
                .as_ref()
                .and_then(|task| task.interview_session_id.clone());
            let pending_interview = if let (Some((spec_interview_state, app_state)), Some(interview_id)) =
                (interview_context, interview_session_id.as_ref())
            {
                spec_interview_state
                    .get_session_snapshot(interview_id, app_state)
                    .await
                    .as_ref()
                    .and_then(map_task_interview_snapshot)
            } else {
                None
            };
            let snapshot = map_task_session_to_rehydrate(
                &task_session,
                interview_session_id.clone(),
                pending_interview.clone(),
            );
            state
                .sync_task_snapshot_by_linked_session(
                    mode_session_id,
                    snapshot.phase.clone(),
                    snapshot.current_story_id.clone(),
                    snapshot.completed_stories,
                    snapshot.failed_stories,
                    map_task_status_to_kernel_status(&task_session.status),
                )
                .await
                .map_err(|error| {
                    format!(
                        "Failed to sync linked task snapshot '{}': {}",
                        mode_session_id, error
                    )
                })?;

            if snapshot.interview_session_id.is_some() || snapshot.pending_interview.is_some() {
                state
                    .sync_task_interview_snapshot_by_linked_session(
                        mode_session_id,
                        snapshot.interview_session_id,
                        snapshot.phase,
                        snapshot.pending_interview,
                    )
                    .await
                    .map_err(|error| {
                        format!(
                            "Failed to sync linked task interview snapshot '{}': {}",
                            mode_session_id, error
                        )
                    })?;
            }
        }
        WorkflowMode::Chat => {}
    }

    Ok(linked_session)
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

fn map_plan_input_type(input_type: &ClarificationInputType) -> (String, Vec<String>) {
    match input_type {
        ClarificationInputType::Text => ("text".to_string(), Vec::new()),
        ClarificationInputType::Textarea => ("textarea".to_string(), Vec::new()),
        ClarificationInputType::SingleSelect(options) => {
            ("single_select".to_string(), options.clone())
        }
        ClarificationInputType::MultiSelect(options) => ("multi_select".to_string(), options.clone()),
        ClarificationInputType::Boolean => ("boolean".to_string(), Vec::new()),
    }
}

fn map_plan_session_to_rehydrate(session: &PlanModeSession) -> PlanSnapshotRehydrate {
    let pending_clarification = session.current_question.as_ref().map(|question| {
        let (input_type, options) = map_plan_input_type(&question.input_type);
        PlanClarificationSnapshot {
            question_id: question.question_id.clone(),
            question: question.question.clone(),
            hint: question.hint.clone(),
            input_type,
            options,
            required: false,
            allow_custom: question.allow_custom,
        }
    });

    let running_step_id = session.step_states.iter().find_map(|(step_id, state)| {
        if matches!(state, StepExecutionState::Running) {
            Some(step_id.clone())
        } else {
            None
        }
    });

    PlanSnapshotRehydrate {
        phase: Some(plan_phase_to_kernel_phase(session.phase).to_string()),
        running_step_id,
        pending_clarification,
    }
}

fn map_task_session_status_to_kernel_phase(
    status: &crate::commands::task_mode::TaskModeStatus,
) -> &'static str {
    match status {
        crate::commands::task_mode::TaskModeStatus::Initialized => "configuring",
        crate::commands::task_mode::TaskModeStatus::Exploring => "exploring",
        crate::commands::task_mode::TaskModeStatus::GeneratingPrd => "generating_prd",
        crate::commands::task_mode::TaskModeStatus::ReviewingPrd => "reviewing_prd",
        crate::commands::task_mode::TaskModeStatus::Executing => "executing",
        crate::commands::task_mode::TaskModeStatus::Completed => "completed",
        crate::commands::task_mode::TaskModeStatus::Failed => "failed",
        crate::commands::task_mode::TaskModeStatus::Cancelled => "cancelled",
    }
}

fn map_task_status_to_kernel_status(
    status: &crate::commands::task_mode::TaskModeStatus,
) -> Option<WorkflowStatus> {
    match status {
        crate::commands::task_mode::TaskModeStatus::Completed => Some(WorkflowStatus::Completed),
        crate::commands::task_mode::TaskModeStatus::Failed => Some(WorkflowStatus::Failed),
        crate::commands::task_mode::TaskModeStatus::Cancelled => Some(WorkflowStatus::Cancelled),
        _ => None,
    }
}

fn map_task_interview_snapshot(interview: &InterviewSession) -> Option<TaskInterviewSnapshot> {
    let question = interview.current_question.as_ref()?;
    Some(TaskInterviewSnapshot {
        interview_id: interview.id.clone(),
        question_id: question.id.clone(),
        question: question.question.clone(),
        hint: question.hint.clone(),
        required: question.required,
        input_type: question.input_type.clone(),
        options: question.options.clone(),
        allow_custom: question.allow_custom,
        question_number: (interview.question_cursor.max(0) as u32).saturating_add(1),
        total_questions: interview.max_questions.max(0) as u32,
    })
}

fn infer_current_story_id(
    progress: &crate::services::task_mode::batch_executor::BatchExecutionProgress,
) -> Option<String> {
    for (story_id, status) in &progress.story_statuses {
        if status == "running" || status == "executing" {
            return Some(story_id.clone());
        }
    }
    None
}

fn map_task_session_to_rehydrate(
    session: &crate::commands::task_mode::TaskModeSession,
    interview_session_id: Option<String>,
    pending_interview: Option<TaskInterviewSnapshot>,
) -> TaskSnapshotRehydrate {
    let (current_story_id, completed_stories, failed_stories) = match session.progress.as_ref() {
        Some(progress) => (
            infer_current_story_id(progress),
            Some(progress.stories_completed as u64),
            Some(progress.stories_failed as u64),
        ),
        None => (None, Some(0), Some(0)),
    };

    TaskSnapshotRehydrate {
        phase: Some(map_task_session_status_to_kernel_phase(&session.status).to_string()),
        current_story_id,
        completed_stories,
        failed_stories,
        interview_session_id,
        pending_interview,
    }
}

fn mark_plan_session_interrupted(session: &mut PlanModeSession) -> bool {
    if session.phase != PlanModePhase::Executing {
        return false;
    }

    session.phase = PlanModePhase::Failed;
    session.step_states.insert(
        "_error".to_string(),
        StepExecutionState::Failed {
            reason: "interrupted_by_restart".to_string(),
        },
    );
    true
}

fn mark_task_session_interrupted(session: &mut crate::commands::task_mode::TaskModeSession) -> bool {
    if session.status != crate::commands::task_mode::TaskModeStatus::Executing {
        return false;
    }

    session.status = crate::commands::task_mode::TaskModeStatus::Failed;
    if let Some(progress) = session.progress.as_mut() {
        progress.current_phase = "failed".to_string();
    }
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInteractiveActionFailureRecordRequest {
    pub card: String,
    pub action: String,
    pub error_code: String,
    pub message: Option<String>,
    pub mode: Option<String>,
    pub kernel_session_id: Option<String>,
    pub mode_session_id: Option<String>,
    pub phase_before: Option<String>,
    pub phase_after: Option<String>,
}

fn normalize_optional_value(value: Option<String>) -> Option<String> {
    value.map(|item| item.trim().to_string()).filter(|item| !item.is_empty())
}

#[tauri::command]
pub async fn workflow_open_session(
    initial_mode: Option<WorkflowMode>,
    initial_context: Option<HandoffContextBundle>,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.open_session(initial_mode, initial_context).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session.session_id,
                "workflow_open_session",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_transition_mode(
    session_id: String,
    target_mode: WorkflowMode,
    handoff: Option<HandoffContextBundle>,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .transition_mode(&session_id, target_mode, handoff)
        .await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_transition_mode",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_submit_input(
    session_id: String,
    intent: UserInputIntent,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.submit_input(&session_id, intent).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_submit_input",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_transition_and_submit_input(
    session_id: String,
    target_mode: WorkflowMode,
    handoff: Option<HandoffContextBundle>,
    intent: UserInputIntent,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .transition_and_submit_input(&session_id, target_mode, handoff, intent)
        .await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_transition_and_submit_input",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_apply_plan_edit(
    session_id: String,
    operation: PlanEditOperation,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.apply_plan_edit(&session_id, operation).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_apply_plan_edit",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_execute_plan(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.execute_plan(&session_id).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_execute_plan",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_retry_step(
    session_id: String,
    step_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.retry_step(&session_id, &step_id).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_retry_step",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_cancel_operation(
    session_id: String,
    reason: Option<String>,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.cancel_operation(&session_id, reason).await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_cancel_operation",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_get_session_state(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSessionState>, String> {
    let result = state.get_session_state(&session_id).await;
    Ok(match result {
        Ok(session_state) => CommandResponse::ok(session_state),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_get_observability_snapshot(
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<WorkflowObservabilitySnapshot>, String> {
    let snapshot = app_state
        .with_database(observability::read_metrics_snapshot)
        .await
        .map_err(|error| error.to_string());
    Ok(match snapshot {
        Ok(snapshot) => CommandResponse::ok(snapshot),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_record_interactive_action_failure(
    request: WorkflowInteractiveActionFailureRecordRequest,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let record = WorkflowFailureRecordInput {
        action: request.action.trim().to_string(),
        card: Some(request.card.trim().to_string()),
        mode: normalize_optional_value(request.mode),
        kernel_session_id: normalize_optional_value(request.kernel_session_id),
        mode_session_id: normalize_optional_value(request.mode_session_id),
        phase_before: normalize_optional_value(request.phase_before),
        phase_after: normalize_optional_value(request.phase_after),
        error_code: Some(request.error_code.trim().to_string()),
        message: normalize_optional_value(request.message),
        timestamp: None,
    };

    tracing::warn!(
        event = "interactive_action_failure",
        kernelSessionId = %record.kernel_session_id.clone().unwrap_or_default(),
        modeSessionId = %record.mode_session_id.clone().unwrap_or_default(),
        mode = %record.mode.clone().unwrap_or_default(),
        phase_before = %record.phase_before.clone().unwrap_or_default(),
        phase_after = %record.phase_after.clone().unwrap_or_default(),
        action = %record.action,
        errorCode = %record.error_code.clone().unwrap_or_default(),
        card = %record.card.clone().unwrap_or_default(),
    );

    let persisted = app_state
        .with_database(|db| observability::record_interactive_action_failure(db, &record))
        .await
        .map_err(|error| error.to_string());
    Ok(match persisted {
        Ok(_) => CommandResponse::ok(true),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_recover_session(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    plan_mode_state: tauri::State<'_, PlanModeState>,
    task_mode_state: tauri::State<'_, TaskModeState>,
    spec_interview_state: tauri::State<'_, SpecInterviewState>,
    app_state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSessionState>, String> {
    let recovered_session = match state.recover_session(&session_id).await {
        Ok(session) => session,
        Err(error) => {
            return Ok(CommandResponse::err(error));
        }
    };

    let mut plan_snapshot: Option<PlanSnapshotRehydrate> = None;
    let mut plan_interrupted = false;
    let mut recovery_warnings: Vec<String> = Vec::new();
    if let Some(plan_session_id) = recovered_session
        .linked_mode_sessions
        .get(&WorkflowMode::Plan)
    {
        let loaded_plan_session = match plan_mode_state
            .get_or_load_session_snapshot(plan_session_id)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                recovery_warnings.push(error);
                None
            }
        };

        if let Some(mut plan_session) = loaded_plan_session {
            if mark_plan_session_interrupted(&mut plan_session) {
                plan_interrupted = true;
                if let Err(error) = plan_mode_state.store_session_snapshot(plan_session.clone()).await {
                    recovery_warnings.push(format!(
                        "Failed to persist interrupted plan session '{}': {}",
                        plan_session_id, error
                    ));
                }
            }

            let snapshot = map_plan_session_to_rehydrate(&plan_session);
            let _ = state
                .sync_plan_snapshot_by_linked_session(
                    plan_session_id,
                    snapshot.phase.clone(),
                    snapshot.pending_clarification.clone(),
                    snapshot.running_step_id.clone(),
                    if plan_interrupted {
                        Some(WorkflowStatus::Failed)
                    } else {
                        None
                    },
                )
                .await;
            plan_snapshot = Some(snapshot);
        }
    }

    let mut task_snapshot: Option<TaskSnapshotRehydrate> = None;
    let mut task_interrupted = false;
    if let Some(task_session_id) = recovered_session
        .linked_mode_sessions
        .get(&WorkflowMode::Task)
    {
        let loaded_task_session = match task_mode_state
            .get_or_load_session_snapshot(task_session_id)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                recovery_warnings.push(error);
                None
            }
        };

        if let Some(mut task_session) = loaded_task_session {
            let persisted_interview_id = recovered_session
                .mode_snapshots
                .task
                .as_ref()
                .and_then(|task| task.interview_session_id.clone());
            let interview_session =
                if let Some(interview_session_id) = persisted_interview_id.as_ref() {
                    spec_interview_state
                        .get_session_snapshot(interview_session_id, app_state.inner())
                        .await
                } else {
                    None
                };
            let pending_interview = interview_session
                .as_ref()
                .and_then(map_task_interview_snapshot);
            if mark_task_session_interrupted(&mut task_session) {
                task_interrupted = true;
                if let Err(error) = task_mode_state.store_session_snapshot(task_session.clone()).await {
                    recovery_warnings.push(format!(
                        "Failed to persist interrupted task session '{}': {}",
                        task_session_id, error
                    ));
                }
            }

            let snapshot = map_task_session_to_rehydrate(
                &task_session,
                persisted_interview_id,
                pending_interview,
            );
            let next_status = if task_interrupted {
                Some(WorkflowStatus::Failed)
            } else {
                map_task_status_to_kernel_status(&task_session.status)
            };
            let _ = state
                .sync_task_snapshot_by_linked_session(
                    task_session_id,
                    snapshot.phase.clone(),
                    snapshot.current_story_id.clone(),
                    snapshot.completed_stories,
                    snapshot.failed_stories,
                    next_status,
                )
                .await;
            task_snapshot = Some(snapshot);
        }
    }

    if plan_interrupted {
        let _ = state
            .submit_input(
                &session_id,
                UserInputIntent {
                    intent_type: crate::services::workflow_kernel::UserInputIntentType::SystemPhaseUpdate,
                    content: "phase:failed".to_string(),
                    metadata: json!({
                        "mode": "plan",
                        "phase": "failed",
                        "reasonCode": "interrupted_by_restart"
                    }),
                },
            )
            .await;
    }
    if task_interrupted {
        let _ = state
            .submit_input(
                &session_id,
                UserInputIntent {
                    intent_type: crate::services::workflow_kernel::UserInputIntentType::SystemPhaseUpdate,
                    content: "phase:failed".to_string(),
                    metadata: json!({
                        "mode": "task",
                        "phase": "failed",
                        "reasonCode": "interrupted_by_restart"
                    }),
                },
            )
            .await;
    }
    if !recovery_warnings.is_empty() {
        eprintln!(
            "[workflow_recover_session] recovered with warnings: {}",
            recovery_warnings.join(" | ")
        );
    }

    let rehydrate_result = state
        .rehydrate_from_linked_sessions(&session_id, plan_snapshot, task_snapshot)
        .await;
    if let Err(error) = rehydrate_result {
        return Ok(CommandResponse::err(error));
    }

    let state_result = state.get_session_state(&session_id).await;
    Ok(match state_result {
        Ok(session_state) => {
            let _ = emit_kernel_update(&app, session_state.clone(), "workflow_recover_session");
            CommandResponse::ok(session_state)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_append_context_items(
    session_id: String,
    target_mode: WorkflowMode,
    handoff: HandoffContextBundle,
    state: tauri::State<'_, WorkflowKernelState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .append_context_items(&session_id, target_mode, handoff)
        .await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_append_context_items",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_link_mode_session(
    session_id: String,
    mode: WorkflowMode,
    mode_session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
    plan_mode_state: tauri::State<'_, PlanModeState>,
    task_mode_state: tauri::State<'_, TaskModeState>,
    spec_interview_state: tauri::State<'_, SpecInterviewState>,
    app_state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let normalized_mode_session_id = mode_session_id.trim().to_string();
    let phase_before = state
        .get_session(&session_id)
        .await
        .ok()
        .and_then(|session| session_phase_for_mode(&session, mode));
    tracing::info!(
        event = "workflow_link_rehydrate_start",
        kernelSessionId = %session_id,
        modeSessionId = %normalized_mode_session_id,
        mode = workflow_mode_label(mode),
        phase_before = %phase_before.clone().unwrap_or_default(),
        action = "workflow_link_mode_session"
    );

    let result = link_mode_session_and_rehydrate(
        &session_id,
        mode,
        &normalized_mode_session_id,
        state.inner(),
        plan_mode_state.inner(),
        task_mode_state.inner(),
        Some((spec_interview_state.inner(), app_state.inner())),
    )
    .await;
    Ok(match result {
        Ok(session) => {
            let phase_after = state
                .get_session(&session_id)
                .await
                .ok()
                .and_then(|kernel_session| session_phase_for_mode(&kernel_session, mode));
            let record = WorkflowFailureRecordInput {
                action: "workflow_link_mode_session".to_string(),
                card: None,
                mode: Some(workflow_mode_label(mode).to_string()),
                kernel_session_id: Some(session_id.clone()),
                mode_session_id: Some(normalized_mode_session_id.clone()),
                phase_before: phase_before.clone(),
                phase_after: phase_after.clone(),
                error_code: None,
                message: None,
                timestamp: None,
            };
            let _ = app_state
                .with_database(|db| observability::record_link_rehydrate(db, &record, true))
                .await;
            tracing::info!(
                event = "workflow_link_rehydrate_success",
                kernelSessionId = %session_id,
                modeSessionId = %normalized_mode_session_id,
                mode = workflow_mode_label(mode),
                phase_before = %record.phase_before.clone().unwrap_or_default(),
                phase_after = %phase_after.unwrap_or_default(),
                action = "workflow_link_mode_session",
                errorCode = ""
            );
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_link_mode_session",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => {
            let phase_after = state
                .get_session(&session_id)
                .await
                .ok()
                .and_then(|kernel_session| session_phase_for_mode(&kernel_session, mode));
            let error_message = error;
            let record = WorkflowFailureRecordInput {
                action: "workflow_link_mode_session".to_string(),
                card: None,
                mode: Some(workflow_mode_label(mode).to_string()),
                kernel_session_id: Some(session_id.clone()),
                mode_session_id: Some(normalized_mode_session_id.clone()),
                phase_before: phase_before.clone(),
                phase_after: phase_after.clone(),
                error_code: Some("mode_session_link_failed".to_string()),
                message: Some(error_message.clone()),
                timestamp: None,
            };
            let _ = app_state
                .with_database(|db| observability::record_link_rehydrate(db, &record, false))
                .await;
            tracing::warn!(
                event = "workflow_link_rehydrate_failure",
                kernelSessionId = %session_id,
                modeSessionId = %normalized_mode_session_id,
                mode = workflow_mode_label(mode),
                phase_before = %record.phase_before.clone().unwrap_or_default(),
                phase_after = %phase_after.unwrap_or_default(),
                action = "workflow_link_mode_session",
                errorCode = "mode_session_link_failed",
            );
            CommandResponse::err(error_message)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::task_mode::TaskModeStatus;
    use crate::services::plan_mode::types::{PlanExecutionProgress, PlanModeSession};
    use crate::services::task_mode::batch_executor::BatchExecutionProgress;
    use std::collections::HashMap;

    fn sample_plan_session(phase: PlanModePhase) -> PlanModeSession {
        PlanModeSession {
            session_id: "plan-session-1".to_string(),
            description: "sample".to_string(),
            phase,
            analysis: None,
            clarifications: Vec::new(),
            current_question: None,
            plan: None,
            step_outputs: HashMap::new(),
            step_states: HashMap::new(),
            step_attempts: HashMap::new(),
            progress: Some(PlanExecutionProgress {
                current_batch: 0,
                total_batches: 1,
                steps_completed: 0,
                steps_failed: 0,
                total_steps: 2,
                progress_pct: 0.0,
            }),
            created_at: "2026-03-05T00:00:00Z".to_string(),
        }
    }

    fn sample_task_session(status: TaskModeStatus) -> crate::commands::task_mode::TaskModeSession {
        crate::commands::task_mode::TaskModeSession {
            session_id: "task-session-1".to_string(),
            description: "sample".to_string(),
            status,
            strategy_analysis: None,
            prd: None,
            exploration_result: None,
            progress: Some(BatchExecutionProgress {
                current_batch: 1,
                total_batches: 1,
                stories_completed: 1,
                stories_failed: 0,
                total_stories: 3,
                current_phase: "executing".to_string(),
                story_statuses: HashMap::from([
                    ("S001".to_string(), "completed".to_string()),
                    ("S002".to_string(), "running".to_string()),
                    ("S003".to_string(), "pending".to_string()),
                ]),
            }),
            created_at: "2026-03-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn mark_plan_session_interrupted_sets_failed_reason() {
        let mut session = sample_plan_session(PlanModePhase::Executing);

        let changed = mark_plan_session_interrupted(&mut session);

        assert!(changed);
        assert_eq!(session.phase, PlanModePhase::Failed);
        match session.step_states.get("_error") {
            Some(StepExecutionState::Failed { reason }) => {
                assert_eq!(reason, "interrupted_by_restart");
            }
            _ => panic!("expected _error step with interrupted_by_restart reason"),
        }
    }

    #[test]
    fn mark_plan_session_interrupted_is_noop_for_non_executing() {
        let mut session = sample_plan_session(PlanModePhase::ReviewingPlan);

        let changed = mark_plan_session_interrupted(&mut session);

        assert!(!changed);
        assert_eq!(session.phase, PlanModePhase::ReviewingPlan);
        assert!(session.step_states.is_empty());
    }

    #[test]
    fn mark_task_session_interrupted_sets_failed_phase() {
        let mut session = sample_task_session(TaskModeStatus::Executing);

        let changed = mark_task_session_interrupted(&mut session);

        assert!(changed);
        assert_eq!(session.status, TaskModeStatus::Failed);
        assert_eq!(
            session.progress.as_ref().map(|progress| progress.current_phase.as_str()),
            Some("failed")
        );
    }

    #[test]
    fn mark_task_session_interrupted_is_noop_for_non_executing() {
        let mut session = sample_task_session(TaskModeStatus::ReviewingPrd);

        let changed = mark_task_session_interrupted(&mut session);

        assert!(!changed);
        assert_eq!(session.status, TaskModeStatus::ReviewingPrd);
        assert_eq!(
            session.progress.as_ref().map(|progress| progress.current_phase.as_str()),
            Some("executing")
        );
    }

    #[tokio::test]
    async fn link_mode_session_and_rehydrate_plan_syncs_kernel_phase() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().join("kernel"));
        let plan_state = PlanModeState::new_with_storage_dir(temp_dir.path().join("plan"));
        let task_state = TaskModeState::new_with_storage_dir(temp_dir.path().join("task"));

        let session = kernel
            .open_session(Some(WorkflowMode::Plan), None)
            .await
            .expect("open kernel session");
        let session_id = session.session_id;

        let mut plan_session = sample_plan_session(PlanModePhase::ReviewingPlan);
        plan_session.session_id = "plan-session-link-test".to_string();
        plan_state
            .store_session_snapshot(plan_session)
            .await
            .expect("store plan session");

        link_mode_session_and_rehydrate(
            &session_id,
            WorkflowMode::Plan,
            "plan-session-link-test",
            &kernel,
            &plan_state,
            &task_state,
            None,
        )
        .await
        .expect("link and rehydrate plan session");

        let state = kernel
            .get_session_state(&session_id)
            .await
            .expect("load kernel state");
        let plan_phase = state
            .session
            .mode_snapshots
            .plan
            .as_ref()
            .map(|snapshot| snapshot.phase.as_str());
        assert_eq!(plan_phase, Some("reviewing_plan"));
    }

    #[tokio::test]
    async fn link_mode_session_and_rehydrate_task_syncs_kernel_phase() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let kernel = WorkflowKernelState::new_with_storage_dir(temp_dir.path().join("kernel"));
        let plan_state = PlanModeState::new_with_storage_dir(temp_dir.path().join("plan"));
        let task_state = TaskModeState::new_with_storage_dir(temp_dir.path().join("task"));

        let session = kernel
            .open_session(Some(WorkflowMode::Task), None)
            .await
            .expect("open kernel session");
        let session_id = session.session_id;

        let mut task_session = sample_task_session(TaskModeStatus::ReviewingPrd);
        task_session.session_id = "task-session-link-test".to_string();
        task_state
            .store_session_snapshot(task_session)
            .await
            .expect("store task session");

        link_mode_session_and_rehydrate(
            &session_id,
            WorkflowMode::Task,
            "task-session-link-test",
            &kernel,
            &plan_state,
            &task_state,
            None,
        )
        .await
        .expect("link and rehydrate task session");

        let state = kernel
            .get_session_state(&session_id)
            .await
            .expect("load kernel state");
        let task_phase = state
            .session
            .mode_snapshots
            .task
            .as_ref()
            .map(|snapshot| snapshot.phase.as_str());
        assert_eq!(task_phase, Some("reviewing_prd"));
    }
}
