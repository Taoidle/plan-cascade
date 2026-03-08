use super::*;

/// Enter task mode by creating a new session.
///
/// Initializes a TaskModeSession and stores it in managed state.
/// Also runs strategy analysis to provide mode recommendation.
#[tauri::command]
pub async fn enter_task_mode(
    request: EnterTaskModeRequest,
    state: tauri::State<'_, TaskModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<TaskModeSession>, String> {
    let EnterTaskModeRequest {
        description,
        kernel_session_id,
        locale,
    } = request;
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    let normalized_kernel_session_id = kernel_session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let task_entry_handoff = if let Some(kernel_session_id) = normalized_kernel_session_id.as_deref()
    {
        let entry = kernel_state
            .mode_entry_handoff_for_kernel_session(kernel_session_id, WorkflowMode::Task)
            .await
            .unwrap_or_default();
        if !entry.conversation_context.is_empty()
            || !entry.summary_items.is_empty()
            || !entry.artifact_refs.is_empty()
            || !entry.context_sources.is_empty()
            || !entry.metadata.is_empty()
        {
            Some(entry)
        } else {
            kernel_state
                .handoff_context_for_kernel_session(kernel_session_id)
                .await
        }
    } else {
        None
    };
    let analysis_input = task_entry_handoff
        .as_ref()
        .and_then(render_task_entry_handoff_context)
        .map(|handoff| format!("{description}\n\nCross-mode context:\n{handoff}"))
        .unwrap_or_else(|| description.clone());

    // Run strategy analysis
    let analysis = analyze_task_for_mode(&analysis_input, None);
    let locale_tag = normalize_locale(locale.as_deref());

    let session = TaskModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        kernel_session_id: normalized_kernel_session_id.clone(),
        locale: locale.clone(),
        description: description.clone(),
        status: TaskModeStatus::Initialized,
        strategy_analysis: Some(analysis),
        prd: None,
        exploration_result: None,
        progress: None,
        execution_resume_payload: None,
        cancel_requested: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    // Store session
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session.session_id.clone(), session.clone());
    }
    persist_task_session_best_effort(&state, &session, "enter_task_mode").await;
    if let Some(analysis) = session.strategy_analysis.as_ref() {
        publish_task_handoff_summary(
            kernel_state.inner(),
            session.kernel_session_id.as_deref(),
            HandoffSummaryItem {
                id: format!("task-strategy-{}", session.session_id),
                source_mode: WorkflowMode::Task,
                kind: "task_strategy".to_string(),
                title: match locale_tag {
                    "zh" => format!("{} 的任务策略", session.description),
                    "ja" => format!("{} のタスク戦略", session.description),
                    _ => format!("Task strategy for {}", session.description),
                },
                body: match locale_tag {
                    "zh" => format!(
                        "推荐模式：{}\n风险：{}\n预计故事数：{}\n推理：{}",
                        localized_task_execution_mode(locale_tag, &analysis.recommended_mode),
                        localized_task_risk_level(locale_tag, &analysis.risk_level),
                        analysis.estimated_stories,
                        analysis.reasoning
                    ),
                    "ja" => format!(
                        "推奨モード: {}\nリスク: {}\n推定ストーリー数: {}\n理由: {}",
                        localized_task_execution_mode(locale_tag, &analysis.recommended_mode),
                        localized_task_risk_level(locale_tag, &analysis.risk_level),
                        analysis.estimated_stories,
                        analysis.reasoning
                    ),
                    _ => format!(
                        "Recommended mode: {}\nRisk: {}\nEstimated stories: {}\nReasoning: {}",
                        localized_task_execution_mode(locale_tag, &analysis.recommended_mode),
                        localized_task_risk_level(locale_tag, &analysis.risk_level),
                        analysis.estimated_stories,
                        analysis.reasoning
                    ),
                },
                artifact_refs: Vec::new(),
                metadata: serde_json::Map::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await;
    }

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
