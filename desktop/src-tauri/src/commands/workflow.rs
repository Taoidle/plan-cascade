//! Workflow Kernel v2 Tauri Commands
//!
//! Unified command surface for Chat/Plan/Task workflow sessions.

use crate::models::CommandResponse;
use crate::services::workflow_kernel::{
    HandoffContextBundle, PlanEditOperation, UserInputIntent, WorkflowKernelState,
    WorkflowKernelUpdatedEvent, WorkflowMode, WorkflowSession, WorkflowSessionState,
    WORKFLOW_KERNEL_UPDATED_CHANNEL,
};
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
    app: tauri::AppHandle,
) -> Result<CommandResponse<WorkflowSessionState>, String> {
    let recover_result = state.recover_session(&session_id).await;
    if let Err(error) = recover_result {
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
