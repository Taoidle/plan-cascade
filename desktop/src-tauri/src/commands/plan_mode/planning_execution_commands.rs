use super::*;

/// Generate a plan using LLM decomposition.
#[tauri::command]
pub async fn generate_plan(
    request: GeneratePlanRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<Plan>, String> {
    let GeneratePlanRequest {
        session_id,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    // Extract session data
    let (description, domain, adapter_name, clarifications) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        let analysis = session
            .analysis
            .as_ref()
            .ok_or_else(|| "No analysis result available".to_string())?;

        (
            session.description.clone(),
            analysis.domain.clone(),
            analysis.adapter_name.clone(),
            session.clarifications.clone(),
        )
    };

    let (resolved_provider, resolved_model) =
        resolve_plan_provider_and_model(provider, model, &app_state).await;
    let prov = resolved_provider.as_str();
    let mdl = resolved_model.as_str();
    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR)),
        result = async {
            let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
                .await
                .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

            let registry = state.adapter_registry.read().await;
            let adapter = registry
                .get(&adapter_name)
                .unwrap_or_else(|| registry.find_for_domain(&domain));

            let locale_tag = normalize_locale(locale.as_deref());
            let lang_instruction = locale_instruction(locale_tag);
            let plan_context = build_plan_conversation_context(
                &app_state,
                &knowledge_state,
                project_path.as_deref(),
                Some(session_id.as_str()),
                conversation_context.as_deref(),
                context_sources.as_ref(),
                &description,
                InjectionPhase::Planning,
            )
            .await;
            let plan_context_ref = if plan_context.is_empty() {
                None
            } else {
                Some(plan_context.as_str())
            };

            match crate::services::plan_mode::planner::generate_plan(
                &description,
                &domain,
                adapter,
                &clarifications,
                plan_context_ref,
                lang_instruction,
                llm_provider,
            )
            .await
            {
                Ok(plan) => {
                    // Update session
                    let updated_session = {
                    let mut sessions = state.sessions.write().await;
                    let session = sessions
                        .get_mut(&session_id)
                        .ok_or_else(|| "No active plan mode session".to_string())?;
                    session.plan = Some(plan.clone());
                    session.phase = PlanModePhase::ReviewingPlan;
                        session.clone()
                    };

                    sync_kernel_plan_snapshot_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        &updated_session,
                        "plan_mode.generate_plan",
                    )
                    .await;

                    Ok(CommandResponse::ok(plan))
                }
                Err(e) => Ok(CommandResponse::err(format!("Plan generation failed: {e}"))),
            }
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;
    result
}

/// Approve the plan and start execution.
#[tauri::command]
pub async fn approve_plan(
    request: ApprovePlanRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    standalone_state: tauri::State<'_, crate::commands::standalone::StandaloneState>,
    permission_state: tauri::State<'_, crate::commands::permissions::PermissionState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<bool>, String> {
    let ApprovePlanRequest {
        session_id,
        plan,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    // Validate
    let (adapter_name, task_description) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::ReviewingPlan {
            return Ok(CommandResponse::err("Not in reviewing phase"));
        }

        (plan.adapter_name.clone(), session.description.clone())
    };

    let (resolved_provider, resolved_model) =
        resolve_plan_provider_and_model(provider, model, &app_state).await;
    let prov = resolved_provider.as_str();
    let mdl = resolved_model.as_str();

    let provider_config = resolve_provider_config(prov, mdl, None, base_url.clone(), &app_state)
        .await
        .map_err(|e| format!("Failed to resolve provider config: {e}"))?;

    let llm_provider = resolve_llm_provider(prov, mdl, None, base_url, &app_state)
        .await
        .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

    let registry = state.adapter_registry.read().await;
    let adapter = registry
        .get(&adapter_name)
        .unwrap_or_else(|| registry.find_for_domain(&plan.domain));
    drop(registry);

    // Update session to executing
    let executing_session_snapshot = {
        let mut sessions = state.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;
        session.plan = Some(plan.clone());
        session.phase = PlanModePhase::Executing;
        session.clone()
    };

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &executing_session_snapshot,
        "plan_mode.approve_plan.executing",
    )
    .await;

    // Set cancellation token
    let cancel_token = CancellationToken::new();
    {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.insert(session_id.clone(), cancel_token.clone());
    }

    // Spawn execution as background task
    let sessions_arc = state.sessions.clone();
    let tokens_arc = state.cancellation_tokens.clone();
    let sid = session_id.clone();
    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag).to_string();
    let execution_context = build_plan_conversation_context(
        &app_state,
        &knowledge_state,
        project_path.as_deref(),
        Some(session_id.as_str()),
        conversation_context.as_deref(),
        context_sources.as_ref(),
        &task_description,
        InjectionPhase::Implementation,
    )
    .await;
    let execution_context = if execution_context.is_empty() {
        None
    } else {
        Some(execution_context)
    };

    let resolved_project_root = match project_path.as_deref().map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => standalone_state.working_directory.read().await.clone(),
    };
    let resolved_project_path = resolved_project_root.to_string_lossy().to_string();

    let (index_store, embedding_service, embedding_manager, hnsw_index) = {
        let manager_guard = standalone_state.index_manager.read().await;
        if let Some(manager) = &*manager_guard {
            (
                Some(manager.index_store_arc()),
                manager.get_embedding_service(&resolved_project_path).await,
                manager.get_embedding_manager(&resolved_project_path).await,
                manager.get_hnsw_index(&resolved_project_path).await,
            )
        } else {
            (None, None, None, None)
        }
    };

    let step_runtime = crate::services::plan_mode::step_executor::StepExecutionRuntime {
        provider_config,
        project_root: resolved_project_root,
        index_store,
        embedding_service,
        embedding_manager,
        hnsw_index,
        permission_gate: Some(permission_state.gate.clone()),
        search_provider: Some(resolve_search_provider_for_tools()),
    };

    tokio::spawn(async move {
        let config = crate::services::plan_mode::step_executor::StepExecutionConfig::default();

        let mut plan_mut = plan;
        let app_for_execute = app_handle.clone();

        let result = crate::services::plan_mode::step_executor::execute_plan(
            &sid,
            &mut plan_mut,
            adapter,
            llm_provider,
            Some(step_runtime),
            config,
            execution_context,
            lang_instruction,
            app_for_execute,
            cancel_token,
        )
        .await;

        // Update session with results
        let mut updated_session_snapshot: Option<PlanModeSession> = None;
        let mut sessions = sessions_arc.write().await;
        if let Some(session) = sessions.get_mut(&sid) {
            match result {
                Ok((outputs, states)) => {
                    let failed = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Failed { .. }));
                    let cancelled = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Cancelled));

                    session.step_outputs = outputs;
                    session.step_states = states;
                    session.plan = Some(plan_mut);

                    if cancelled {
                        session.phase = PlanModePhase::Cancelled;
                    } else if failed {
                        session.phase = PlanModePhase::Failed;
                    } else {
                        session.phase = PlanModePhase::Completed;
                    }
                }
                Err(e) => {
                    session.phase = PlanModePhase::Failed;
                    // Store error in a synthetic step state
                    session.step_states.insert(
                        "_error".to_string(),
                        StepExecutionState::Failed {
                            reason: format!("{e}"),
                        },
                    );
                }
            }
            updated_session_snapshot = Some(session.clone());
        }
        drop(sessions);

        if let Some(updated_session) = updated_session_snapshot {
            let kernel_state = app_handle.state::<WorkflowKernelState>();
            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &updated_session,
                "plan_mode.approve_plan.completed",
            )
            .await;
        }

        // Clear cancellation token
        let mut tokens = tokens_arc.write().await;
        tokens.remove(&sid);
    });

    Ok(CommandResponse::ok(true))
}
