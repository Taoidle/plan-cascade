use super::*;

async fn generate_plan_clarification_question(
    state: &PlanModeState,
    app_state: &AppState,
    knowledge_state: &crate::commands::knowledge::KnowledgeState,
    kernel_state: &WorkflowKernelState,
    tracker_components: Option<(
        tokio::sync::mpsc::Sender<crate::services::analytics::TrackerMessage>,
        Arc<crate::services::analytics::CostCalculator>,
    )>,
    session_id: &str,
    description: &str,
    analysis: &PlanAnalysis,
    clarifications: &[ClarificationAnswer],
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    agent_ref: Option<String>,
    agent_source: Option<String>,
    project_path: Option<String>,
    context_sources: Option<ContextSourceConfig>,
    conversation_context: Option<String>,
    locale: Option<String>,
) -> Result<
    (
        ResolvedPlanPhaseAgent,
        Option<crate::services::plan_mode::ClarificationQuestion>,
    ),
    String,
> {
    let resolved_agent = resolve_plan_phase_agent(
        "plan_clarification",
        agent_ref.as_deref(),
        agent_source.as_deref(),
        provider,
        model,
        base_url,
        app_state,
        false,
    )
    .await?;
    let provider_config =
        build_llm_provider_from_plan_phase_agent(&resolved_agent, app_state).await?;
    let base_provider =
        crate::services::task_mode::prd_generator::create_provider(provider_config.clone());

    let registry = state.adapter_registry.read().await;
    let adapter = registry
        .get(&analysis.adapter_name)
        .unwrap_or_else(|| registry.find_for_domain(&analysis.domain));

    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag);
    let plan_context = build_plan_conversation_context(
        app_state,
        knowledge_state,
        Some(state),
        kernel_state,
        project_path.as_deref(),
        Some(session_id),
        None,
        conversation_context.as_deref(),
        context_sources.as_ref(),
        description,
        InjectionPhase::Planning,
        Some(&provider_config),
        Some(&resolved_agent),
        false,
        false,
    )
    .await;
    let plan_context_ref = if plan_context.rendered_context.is_empty() {
        None
    } else {
        Some(plan_context.rendered_context.as_str())
    };

    let llm_provider = if let Some((tx, calc)) = tracker_components {
        crate::services::analytics::wrap_provider_with_tracking(
            base_provider,
            tx,
            calc,
            crate::models::analytics::AnalyticsAttribution {
                project_id: None,
                kernel_session_id: None,
                mode_session_id: Some(session_id.to_string()),
                workflow_mode: Some(crate::models::analytics::AnalyticsWorkflowMode::Plan),
                phase_id: Some("plan_clarification".to_string()),
                execution_scope: Some(crate::models::analytics::AnalyticsExecutionScope::DirectLlm),
                execution_id: Some(format!("plan:{}:clarification", session_id)),
                parent_execution_id: None,
                agent_role: Some("plan_clarification".to_string()),
                agent_name: Some(resolved_agent.display_label()),
                step_id: None,
                story_id: None,
                gate_id: None,
                attempt: Some((clarifications.len() + 1) as i64),
                request_sequence: Some(1),
                call_site: Some("plan_mode.clarification".to_string()),
                metadata_json: None,
            },
        )
    } else {
        base_provider
    };

    let question = crate::services::plan_mode::clarifier::generate_clarification_question(
        description,
        analysis,
        clarifications,
        plan_context_ref,
        lang_instruction,
        adapter.as_ref(),
        llm_provider,
    )
    .await
    .unwrap_or(None);

    Ok((resolved_agent, question))
}

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
        kernel_session_id,
        provider,
        model,
        base_url,
        agent_ref,
        agent_source,
        project_path,
        context_sources,
        conversation_context,
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
    let analytics_components =
        get_analytics_tracker_components(&app_handle, app_state.inner()).await;

    // Create initial session
    let mut session = PlanModeSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        kernel_session_id: normalized_kernel_session_id.clone(),
        locale: locale.clone(),
        description: description.clone(),
        phase: PlanModePhase::Analyzing,
        analysis: None,
        clarifications: vec![],
        current_question: None,
        plan: None,
        step_outputs: HashMap::new(),
        step_states: HashMap::new(),
        step_attempts: HashMap::new(),
        progress: None,
        execution_resume_payload: None,
        resolved_phase_agents: Default::default(),
        execution_agent_snapshot: None,
        retry_agent_snapshot: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let session_id = session.session_id.clone();
    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;

    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR)),
        result = async {
            let resolved_strategy = match resolve_plan_phase_agent(
                "plan_strategy",
                agent_ref.as_deref(),
                agent_source.as_deref(),
                provider,
                model,
                base_url.clone(),
                app_state.inner(),
                false,
            )
            .await {
                Ok(agent) => agent,
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
                    session.resolved_phase_agents.strategy = None;
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
                    if let Some(summary) = build_plan_analysis_summary_item(&session) {
                        publish_plan_handoff_summary(
                            kernel_state.inner(),
                            session.kernel_session_id.as_deref(),
                            summary,
                        )
                        .await;
                    }

                    return Ok(CommandResponse::ok(session));
                }
            };
            session.resolved_phase_agents.strategy = Some(resolved_strategy.clone());
            let provider_config =
                build_llm_provider_from_plan_phase_agent(&resolved_strategy, app_state.inner())
                    .await?;
            let base_provider =
                crate::services::task_mode::prd_generator::create_provider(provider_config.clone());
            let llm_provider = if let Some((tx, calc)) = analytics_components.clone() {
                crate::services::analytics::wrap_provider_with_tracking(
                    base_provider,
                    tx,
                    calc,
                    crate::models::analytics::AnalyticsAttribution {
                        project_id: None,
                        kernel_session_id: normalized_kernel_session_id.clone(),
                        mode_session_id: Some(session.session_id.clone()),
                        workflow_mode: Some(crate::models::analytics::AnalyticsWorkflowMode::Plan),
                        phase_id: Some("plan_strategy".to_string()),
                        execution_scope: Some(
                            crate::models::analytics::AnalyticsExecutionScope::DirectLlm,
                        ),
                        execution_id: Some(format!("plan:{}:strategy", session.session_id)),
                        parent_execution_id: None,
                        agent_role: Some("plan_strategy".to_string()),
                        agent_name: Some(resolved_strategy.display_label()),
                        step_id: None,
                        story_id: None,
                        gate_id: None,
                        attempt: Some(1),
                        request_sequence: Some(1),
                        call_site: Some("plan_mode.strategy".to_string()),
                        metadata_json: None,
                    },
                )
            } else {
                base_provider
            };

            let registry = state.adapter_registry.read().await;

            let locale_tag = normalize_locale(locale.as_deref());
            let lang_instruction = locale_instruction(locale_tag);
            let plan_context = build_plan_conversation_context(
                &app_state,
                &knowledge_state,
                None,
                kernel_state.inner(),
                project_path.as_deref(),
                None,
                normalized_kernel_session_id.as_deref(),
                conversation_context.as_deref(),
                context_sources.as_ref(),
                &description,
                InjectionPhase::Planning,
                Some(&provider_config),
                Some(&resolved_strategy),
                false,
                false,
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
                        session.current_question = None;
                        session.phase = PlanModePhase::Clarifying;
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
            if let Some(summary) = build_plan_analysis_summary_item(&session) {
                publish_plan_handoff_summary(
                    kernel_state.inner(),
                    session.kernel_session_id.as_deref(),
                    summary,
                )
                .await;
            }

            Ok(CommandResponse::ok(session))
        } => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;
    result
}

/// Generate the first clarification question for a plan session after analysis.
#[tauri::command]
pub async fn start_plan_clarification(
    request: StartPlanClarificationRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PlanModeSession>, String> {
    let StartPlanClarificationRequest {
        session_id,
        provider,
        model,
        base_url,
        agent_ref,
        agent_source,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    let (description, analysis, clarifications) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;
        if session.phase != PlanModePhase::Clarifying {
            return Ok(CommandResponse::err("Not in clarifying phase"));
        }
        if session.current_question.is_some() {
            return Ok(CommandResponse::ok(session.clone()));
        }
        (
            session.description.clone(),
            session
                .analysis
                .clone()
                .ok_or_else(|| "No analysis available".to_string())?,
            session.clarifications.clone(),
        )
    };

    let (operation_id, operation_token) = register_plan_operation_token(&state, &session_id).await;
    let question_result = tokio::select! {
        _ = operation_token.cancelled() => Err(PLAN_OPERATION_CANCELLED_ERROR.to_string()),
        result = generate_plan_clarification_question(
            state.inner(),
            app_state.inner(),
            knowledge_state.inner(),
            kernel_state.inner(),
            get_analytics_tracker_components(&app_handle, app_state.inner()).await,
            &session_id,
            &description,
            &analysis,
            &clarifications,
            provider,
            model,
            base_url,
            agent_ref,
            agent_source,
            project_path,
            context_sources,
            conversation_context,
            locale.clone(),
        ) => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;

    let (resolved_agent, question) = match question_result {
        Ok(payload) => payload,
        Err(e) if e == PLAN_OPERATION_CANCELLED_ERROR => {
            return Ok(CommandResponse::err(PLAN_OPERATION_CANCELLED_ERROR));
        }
        Err(e) => return Err(e),
    };

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;
    if session.phase != PlanModePhase::Clarifying {
        return Ok(CommandResponse::err("Not in clarifying phase"));
    }
    if locale.is_some() {
        session.locale = locale.clone();
    }
    session.resolved_phase_agents.clarification = Some(resolved_agent);
    match question {
        Some(question) => session.current_question = Some(question),
        None => session.phase = PlanModePhase::Planning,
    }
    let updated_session = session.clone();
    drop(sessions);
    persist_plan_session_best_effort(&state, &updated_session, "start_plan_clarification").await;
    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &updated_session,
        "plan_mode.start_plan_clarification",
    )
    .await;

    Ok(CommandResponse::ok(updated_session))
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
        agent_ref,
        agent_source,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    // Snapshot data needed for question generation.
    let (description, analysis, mut clarifications, current_question_text) = {
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
        result = generate_plan_clarification_question(
            state.inner(),
            app_state.inner(),
            knowledge_state.inner(),
            kernel_state.inner(),
            get_analytics_tracker_components(&app_handle, app_state.inner()).await,
            &session_id,
            &description,
            &analysis,
            &clarifications,
            provider,
            model,
            base_url,
            agent_ref,
            agent_source,
            project_path,
            context_sources,
            conversation_context,
            locale.clone(),
        ) => result,
    };
    clear_plan_operation_token(&state, &session_id, &operation_id).await;

    let (resolved_agent, next_question) = match next_question_result {
        Ok(payload) => payload,
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
    session.resolved_phase_agents.clarification = Some(resolved_agent);
    if locale.is_some() {
        session.locale = locale.clone();
    }
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
    if matches!(updated_session.phase, PlanModePhase::Planning) {
        if let Some(summary) = build_plan_clarification_summary_item(&updated_session) {
            publish_plan_handoff_summary(
                kernel_state.inner(),
                updated_session.kernel_session_id.as_deref(),
                summary,
            )
            .await;
        }
    }

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

        let (provider, model) =
            resolve_plan_provider_and_model(Some("openai".to_string()), None, &app_state).await;

        assert_eq!(provider, "openai");
        assert!(
            !model.trim().is_empty(),
            "resolved model should not be empty"
        );
    }

    #[tokio::test]
    async fn resolves_provider_and_model_when_both_are_missing() {
        let app_state = AppState::new();
        app_state
            .initialize()
            .await
            .expect("app state should initialize");

        let (provider, model) = resolve_plan_provider_and_model(None, None, &app_state).await;

        assert!(
            !provider.trim().is_empty(),
            "resolved provider should not be empty"
        );
        assert!(
            !model.trim().is_empty(),
            "resolved model should not be empty"
        );
    }
}
