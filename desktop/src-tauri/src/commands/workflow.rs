//! Workflow Kernel v2 Tauri Commands
//!
//! Unified command surface for Chat/Plan/Task workflow sessions.

use crate::models::CommandResponse;
use crate::services::plan_mode::types::{
    ClarificationInputType, PlanModePhase, PlanModeSession, StepExecutionState,
};
use crate::services::spec_interview::interview::InterviewSession;
use crate::services::workflow_kernel::{
    HandoffContextBundle, PlanClarificationSnapshot, PlanEditOperation, PlanSnapshotRehydrate,
    TaskInterviewSnapshot, TaskSnapshotRehydrate, UserInputIntent, WorkflowKernelState,
    WorkflowKernelUpdatedEvent, WorkflowMode, WorkflowSession, WorkflowSessionState,
    WorkflowStatus, WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
use crate::{commands::plan_mode::PlanModeState, commands::spec_interview::SpecInterviewState};
use crate::{commands::task_mode::TaskModeState, state::AppState};
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

async fn emit_kernel_update_for_session(
    app: &tauri::AppHandle,
    state: &WorkflowKernelState,
    session_id: &str,
    source: &str,
) -> Result<(), String> {
    let session_state = state.get_session_state(session_id).await?;
    emit_kernel_update(app, session_state, source)
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
    if let Some(plan_session_id) = recovered_session
        .linked_mode_sessions
        .get(&WorkflowMode::Plan)
    {
        if let Some(plan_session) = plan_mode_state.get_session_snapshot(plan_session_id).await {
            plan_snapshot = Some(map_plan_session_to_rehydrate(&plan_session));
        }
    }

    let mut task_snapshot: Option<TaskSnapshotRehydrate> = None;
    if let Some(task_session_id) = recovered_session
        .linked_mode_sessions
        .get(&WorkflowMode::Task)
    {
        if let Some(task_session) = task_mode_state.get_session_snapshot(task_session_id).await {
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
            task_snapshot = Some(map_task_session_to_rehydrate(
                &task_session,
                persisted_interview_id,
                pending_interview,
            ));
            if let Some(next_status) = map_task_status_to_kernel_status(&task_session.status) {
                let _ = state
                    .sync_task_snapshot_by_linked_session(
                        task_session_id,
                        None,
                        None,
                        None,
                        None,
                        Some(next_status),
                    )
                    .await;
            }
        }
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
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .link_mode_session(&session_id, mode, &mode_session_id)
        .await;
    Ok(match result {
        Ok(session) => {
            let _ = emit_kernel_update_for_session(
                &app,
                state.inner(),
                &session_id,
                "workflow_link_mode_session",
            )
            .await;
            CommandResponse::ok(session)
        }
        Err(error) => CommandResponse::err(error),
    })
}
