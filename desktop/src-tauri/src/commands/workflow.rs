//! Workflow Kernel v2 Tauri Commands
//!
//! Unified command surface for Chat/Plan/Task workflow sessions.

use crate::models::CommandResponse;
use crate::services::workflow_kernel::{
    HandoffContextBundle, PlanEditOperation, UserInputIntent, WorkflowKernelState, WorkflowMode,
    WorkflowSession, WorkflowSessionState,
};

#[tauri::command]
pub async fn workflow_open_session(
    initial_mode: Option<WorkflowMode>,
    initial_context: Option<HandoffContextBundle>,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.open_session(initial_mode, initial_context).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_transition_mode(
    session_id: String,
    target_mode: WorkflowMode,
    handoff: Option<HandoffContextBundle>,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .transition_mode(&session_id, target_mode, handoff)
        .await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_submit_input(
    session_id: String,
    intent: UserInputIntent,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.submit_input(&session_id, intent).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
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
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .transition_and_submit_input(&session_id, target_mode, handoff, intent)
        .await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_apply_plan_edit(
    session_id: String,
    operation: PlanEditOperation,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.apply_plan_edit(&session_id, operation).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_execute_plan(
    session_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.execute_plan(&session_id).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_retry_step(
    session_id: String,
    step_id: String,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.retry_step(&session_id, &step_id).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_cancel_operation(
    session_id: String,
    reason: Option<String>,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state.cancel_operation(&session_id, reason).await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
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
) -> Result<CommandResponse<WorkflowSessionState>, String> {
    let recover_result = state.recover_session(&session_id).await;
    if let Err(error) = recover_result {
        return Ok(CommandResponse::err(error));
    }

    let state_result = state.get_session_state(&session_id).await;
    Ok(match state_result {
        Ok(session_state) => CommandResponse::ok(session_state),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn workflow_append_context_items(
    session_id: String,
    target_mode: WorkflowMode,
    handoff: HandoffContextBundle,
    state: tauri::State<'_, WorkflowKernelState>,
) -> Result<CommandResponse<WorkflowSession>, String> {
    let result = state
        .append_context_items(&session_id, target_mode, handoff)
        .await;
    Ok(match result {
        Ok(session) => CommandResponse::ok(session),
        Err(error) => CommandResponse::err(error),
    })
}
