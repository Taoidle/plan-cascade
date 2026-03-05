use super::*;

/// Enter task mode by creating a new session.
///
/// Initializes a TaskModeSession and stores it in managed state.
/// Also runs strategy analysis to provide mode recommendation.
#[tauri::command]
pub async fn enter_task_mode(
    description: String,
    state: tauri::State<'_, TaskModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<TaskModeSession>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Run strategy analysis
    let analysis = analyze_task_for_mode(&description, None);

    let session = TaskModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        status: TaskModeStatus::Initialized,
        strategy_analysis: Some(analysis),
        prd: None,
        exploration_result: None,
        progress: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    // Store session
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session.session_id.clone(), session.clone());
    }
    persist_task_session_best_effort(&state, &session, "enter_task_mode").await;

    sync_kernel_task_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &session,
        None,
        "task_mode.enter_task_mode",
    )
    .await;

    Ok(CommandResponse::ok(session))
}

/// Exit task mode and clean up session state.
#[tauri::command]
pub async fn exit_task_mode(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<bool>, String> {
    let removed = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&session_id).is_some()
    };
    let _ = state.delete_persisted_session(&session_id).await;
    if !removed {
        return Ok(CommandResponse::err(
            "Invalid session ID or no active session",
        ));
    }

    // Cancel and remove any active token for this session.
    if let Some(token) = {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.remove(&session_id)
    } {
        token.cancel();
    }

    // Cancel and remove any active pre-execution operation token for this session.
    if let Some((_, token)) = {
        let mut tokens = state.operation_cancellation_tokens.write().await;
        tokens.remove(&session_id)
    } {
        token.cancel();
    }

    // Drop any cached execution report for this session.
    {
        let mut results = state.execution_results.write().await;
        results.remove(&session_id);
    }

    Ok(CommandResponse::ok(true))
}
