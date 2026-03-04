use super::*;

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
