use super::*;

/// Enter plan mode: create a session and run domain analysis.
#[tauri::command]
pub async fn enter_plan_mode(
    request: EnterPlanModeRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let EnterPlanModeRequest {
        description,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    // Create initial session
    let mut session = PlanModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        phase: PlanModePhase::Analyzing,
        analysis: None,
        clarifications: vec![],
        current_question: None,
        plan: None,
        step_outputs: HashMap::new(),
        step_states: HashMap::new(),
        progress: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let session_id = session.session_id.clone();
    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;

    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR)),
        result = async {
            let (resolved_provider, resolved_model) =
                resolve_plan_provider_and_model(provider, model, app_state.inner()).await;
            let llm_provider = match resolve_llm_provider(
                &resolved_provider,
                &resolved_model,
                None,
                base_url.clone(),
                &app_state,
            )
            .await
            {
                Ok(provider) => provider,
                Err(e) => {
                    // Provider resolution failed, but we can still proceed with a safe fallback.
                    session.phase = PlanModePhase::Planning;
                    session.analysis = Some(PlanAnalysis {
                        domain: crate::services::plan_mode::types::TaskDomain::General,
                        complexity: 5,
                        estimated_steps: 4,
                        needs_clarification: false,
                        reasoning: format!(
                            "Provider resolution failed: {e}. Proceeding with general approach."
                        ),
                        adapter_name: "general".to_string(),
                        suggested_approach: "Standard decomposition".to_string(),
                    });
                    // Store session
                    {
                        let mut sessions = state.sessions.write().await;
                        sessions.insert(session.session_id.clone(), session.clone());
                    }
                    persist_plan_session_best_effort(&state, &session, "enter_plan_mode.provider_resolution_failed")
                        .await;

                    sync_kernel_plan_snapshot_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        &session,
                        "plan_mode.enter_plan_mode",
                    )
                    .await;

                    return Ok(CommandResponse::ok(session));
                }
            };

            let registry = state.adapter_registry.read().await;

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
            let plan_context_ref = if plan_context.rendered_context.is_empty() {
                None
            } else {
                Some(plan_context.rendered_context.as_str())
            };

            match crate::services::plan_mode::analyzer::analyze_task(
                &description,
                plan_context_ref,
                lang_instruction,
                llm_provider.clone(),
                &registry,
            )
            .await
            {
                Ok(analysis) => {
                    if analysis.needs_clarification {
                        // Generate first clarification question
                        let adapter = registry
                            .get(&analysis.adapter_name)
                            .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

                        match crate::services::plan_mode::clarifier::generate_clarification_question(
                            &description,
                            &analysis,
                            &[],
                            plan_context_ref,
                            lang_instruction,
                            adapter.as_ref(),
                            llm_provider,
                        )
                        .await
                        {
                            Ok(Some(question)) => {
                                session.current_question = Some(question);
                                session.phase = PlanModePhase::Clarifying;
                            }
                            _ => {
                                // Question generation failed or returned None — skip to planning
                                session.phase = PlanModePhase::Planning;
                            }
                        }
                    } else {
                        session.phase = PlanModePhase::Planning;
                    }
                    session.analysis = Some(analysis);
                }
                Err(e) => {
                    // Analysis failed, but we can still proceed without it
                    session.phase = PlanModePhase::Planning;
                    session.analysis = Some(PlanAnalysis {
                        domain: crate::services::plan_mode::types::TaskDomain::General,
                        complexity: 5,
                        estimated_steps: 4,
                        needs_clarification: false,
                        reasoning: format!("Analysis failed: {e}. Proceeding with general approach."),
                        adapter_name: "general".to_string(),
                        suggested_approach: "Standard decomposition".to_string(),
                    });
                }
            }

            // Store session
            {
                let mut sessions = state.sessions.write().await;
                sessions.insert(session.session_id.clone(), session.clone());
            }
            persist_plan_session_best_effort(&state, &session, "enter_plan_mode.completed").await;

            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &session,
                "plan_mode.enter_plan_mode",
            )
            .await;

            Ok(CommandResponse::ok(session))
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;
    result
}

/// Submit a clarification answer and generate next question.
#[tauri::command]
pub async fn submit_plan_clarification(
    request: SubmitPlanClarificationRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let SubmitPlanClarificationRequest {
        session_id,
        answer,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    // Snapshot data needed for question generation.
    let (description, analysis, mut clarifications, adapter_name, current_question_text) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::Clarifying {
            return Ok(CommandResponse::err("Not in clarifying phase"));
        }

        let analysis = session
            .analysis
            .clone()
            .ok_or_else(|| "No analysis available".to_string())?;

        (
            session.description.clone(),
            analysis,
            session.clarifications.clone(),
            session
                .analysis
                .as_ref()
                .map(|a| a.adapter_name.clone())
                .unwrap_or_default(),
            session
                .current_question
                .as_ref()
                .map(|q| q.question.clone()),
        )
    };

    let mut enriched_answer = answer;
    if let Some(question_text) = current_question_text {
        enriched_answer.question_text = question_text;
    }
    clarifications.push(enriched_answer);

    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;
    let next_question_result = tokio::select! {
        _ = operation_token.cancelled() => Err(PLAN_OPERATION_CANCELLED_ERROR.to_string()),
        result = async {
            let (resolved_provider, resolved_model) =
                resolve_plan_provider_and_model(provider, model, app_state.inner()).await;
            let llm_provider = resolve_llm_provider(
                &resolved_provider,
                &resolved_model,
                None,
                base_url,
                &app_state,
            )
            .await
            .map_err(|e| format!("Failed to resolve LLM provider: {e}"))?;

            let registry = state.adapter_registry.read().await;
            let adapter = registry
                .get(&adapter_name)
                .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

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
            let plan_context_ref = if plan_context.rendered_context.is_empty() {
                None
            } else {
                Some(plan_context.rendered_context.as_str())
            };

            Ok(
                crate::services::plan_mode::clarifier::generate_clarification_question(
                    &description,
                    &analysis,
                    &clarifications,
                    plan_context_ref,
                    lang_instruction,
                    adapter.as_ref(),
                    llm_provider,
                )
                .await
                .unwrap_or(None),
            )
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;

    let next_question = match next_question_result {
        Ok(question) => question,
        Err(e) if e == PLAN_OPERATION_CANCELLED_ERROR => {
            return Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR));
        }
        Err(e) => return Err(e),
    };

    // Apply clarification only if operation completed.
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    if session.phase != PlanModePhase::Clarifying {
        return Ok(CommandResponse::err("Not in clarifying phase"));
    }

    session.clarifications = clarifications;
    match next_question {
        Some(q) => {
            session.current_question = Some(q);
        }
        None => {
            session.current_question = None;
            session.phase = PlanModePhase::Planning;
        }
    }

    let updated_session = session.clone();
    drop(sessions);
    persist_plan_session_best_effort(&state, &updated_session, "submit_plan_clarification").await;

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &updated_session,
        "plan_mode.submit_plan_clarification",
    )
    .await;

    Ok(CommandResponse::ok(updated_session))
}

/// Skip clarification and proceed to planning.
#[tauri::command]
pub async fn skip_plan_clarification(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    session.phase = PlanModePhase::Planning;
    let result = session.clone();
    drop(sessions);
    persist_plan_session_best_effort(&state, &result, "skip_plan_clarification").await;

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &result,
        "plan_mode.skip_plan_clarification",
    )
    .await;

    Ok(CommandResponse::ok(result))
}

#[cfg(test)]
mod tests {
    use super::resolve_plan_provider_and_model;
    use crate::state::AppState;

    #[tokio::test]
    async fn resolves_default_model_when_request_model_is_missing() {
        let app_state = AppState::new();
        app_state
            .initialize()
            .await
            .expect("app state should initialize");

        let (provider, model) = resolve_plan_provider_and_model(
            Some("openai".to_string()),
            None,
            &app_state,
        )
        .await;

        assert_eq!(provider, "openai");
        assert!(!model.trim().is_empty(), "resolved model should not be empty");
    }

    #[tokio::test]
    async fn resolves_provider_and_model_when_both_are_missing() {
        let app_state = AppState::new();
        app_state
            .initialize()
            .await
            .expect("app state should initialize");

        let (provider, model) = resolve_plan_provider_and_model(None, None, &app_state).await;

        assert!(!provider.trim().is_empty(), "resolved provider should not be empty");
        assert!(!model.trim().is_empty(), "resolved model should not be empty");
    }
}
