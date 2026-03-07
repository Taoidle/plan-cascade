use super::*;
use crate::services::workflow_kernel::{
    WorkflowMode, WorkflowModeTranscriptUpdatedEvent, WorkflowSessionCatalogUpdatedEvent,
    WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL, WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL,
};
use serde_json::{json, Value};

fn transcript_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

fn build_card_transcript_line(card_type: &str, data: Value, interactive: bool) -> Value {
    let timestamp = transcript_timestamp();
    let card_payload = json!({
        "cardType": card_type,
        "cardId": format!("{card_type}-{timestamp}"),
        "data": data,
        "interactive": interactive,
    });
    let content = serde_json::to_string(&card_payload).unwrap_or_else(|_| "{}".to_string());
    json!({
        "id": timestamp,
        "type": "card",
        "content": content,
        "timestamp": timestamp,
        "cardPayload": card_payload,
    })
}

fn build_plan_completion_transcript_line(session: &PlanModeSession) -> Option<Value> {
    let plan = session.plan.as_ref()?;
    let total_steps = plan.steps.len();
    let steps_completed = session
        .step_states
        .values()
        .filter(|state| matches!(state, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = session
        .step_states
        .values()
        .filter(|state| matches!(state, StepExecutionState::Failed { .. }))
        .count();
    let steps_cancelled = session
        .step_states
        .values()
        .filter(|state| matches!(state, StepExecutionState::Cancelled))
        .count();
    let steps_attempted = steps_completed + steps_failed + steps_cancelled;
    let total_duration_ms: u64 = session
        .step_states
        .values()
        .filter_map(|state| match state {
            StepExecutionState::Completed { duration_ms } => Some(*duration_ms),
            _ => None,
        })
        .sum();
    let step_summaries: HashMap<String, String> = plan
        .steps
        .iter()
        .map(|step| {
            let summary = session
                .step_outputs
                .get(&step.id)
                .map(|output| {
                    if output.summary.trim().is_empty() {
                        if output.full_content.trim().is_empty() {
                            output.content.clone()
                        } else {
                            output.full_content.clone()
                        }
                    } else {
                        output.summary.clone()
                    }
                })
                .unwrap_or_else(|| {
                    session
                        .step_states
                        .get(&step.id)
                        .map(|state| match state {
                            StepExecutionState::Completed { .. } => "Completed".to_string(),
                            StepExecutionState::Failed { reason } => reason.clone(),
                            StepExecutionState::Cancelled => "Cancelled".to_string(),
                            StepExecutionState::Running => "Running".to_string(),
                            StepExecutionState::Pending => "Pending".to_string(),
                        })
                        .unwrap_or_else(|| "No summary available".to_string())
                });
            (step.id.clone(), summary)
        })
        .collect();
    let failure_reasons: HashMap<String, String> = session
        .step_states
        .iter()
        .filter_map(|(step_id, state)| match state {
            StepExecutionState::Failed { reason } => Some((step_id.clone(), reason.clone())),
            _ => None,
        })
        .collect();
    let terminal_state = match session.phase {
        PlanModePhase::Completed => "completed",
        PlanModePhase::Cancelled => "cancelled",
        PlanModePhase::Failed => "failed",
        _ if steps_failed > 0 => "failed",
        _ if steps_completed == total_steps => "completed",
        _ => "failed",
    };
    let retry_stats = compute_retry_stats(&session.step_attempts, &session.step_states);
    Some(build_card_transcript_line(
        "plan_completion_card",
        json!({
            "success": terminal_state == "completed" && steps_failed == 0,
            "terminalState": terminal_state,
            "planTitle": plan.title,
            "totalSteps": total_steps,
            "stepsCompleted": steps_completed,
            "stepsFailed": steps_failed,
            "stepsCancelled": steps_cancelled,
            "stepsAttempted": steps_attempted,
            "stepsFailedBeforeCancel": if terminal_state == "cancelled" { steps_failed } else { 0 },
            "totalDurationMs": total_duration_ms,
            "stepSummaries": step_summaries,
            "failureReasons": failure_reasons,
            "cancelledBy": if terminal_state == "cancelled" { json!("user") } else { Value::Null },
            "highlights": [],
            "nextActions": if terminal_state == "completed" {
                vec!["Validate outputs and merge into the final result."]
            } else {
                vec!["Retry blocked steps and verify dependency outputs first."]
            },
            "retryStats": {
                "totalRetries": retry_stats.total_retries,
                "stepsRetried": retry_stats.steps_retried,
                "exhaustedFailures": retry_stats.exhausted_failures,
            },
        }),
        false,
    ))
}

async fn append_plan_transcript_lines_for_linked_sessions(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    plan_session_id: &str,
    lines: Vec<Value>,
    source: &str,
) {
    if lines.is_empty() {
        return;
    }

    let kernel_session_ids = kernel_state
        .linked_kernel_sessions_for_mode_session(WorkflowMode::Plan, plan_session_id)
        .await;
    if kernel_session_ids.is_empty() {
        return;
    }

    for kernel_session_id in &kernel_session_ids {
        if let Ok(transcript) = kernel_state
            .append_mode_transcript(kernel_session_id, WorkflowMode::Plan, lines.clone())
            .await
        {
            let _ = app.emit(
                WORKFLOW_MODE_TRANSCRIPT_UPDATED_CHANNEL,
                WorkflowModeTranscriptUpdatedEvent {
                    session_id: transcript.session_id,
                    mode: transcript.mode,
                    revision: transcript.revision,
                    appended_lines: lines.clone(),
                    replace_from_line_id: None,
                    lines: transcript.lines.clone(),
                    source: source.to_string(),
                },
            );
        }
    }

    if let Ok(catalog_state) = kernel_state.get_session_catalog_state().await {
        let _ = app.emit(
            WORKFLOW_SESSION_CATALOG_UPDATED_CHANNEL,
            WorkflowSessionCatalogUpdatedEvent {
                active_session_id: catalog_state.active_session_id,
                sessions: catalog_state.sessions,
                source: source.to_string(),
            },
        );
    }
}

fn build_plan_progress_from_checkpoint(
    current_batch: usize,
    total_batches: usize,
    step_states: &HashMap<String, StepExecutionState>,
) -> crate::services::plan_mode::types::PlanExecutionProgress {
    let total_steps = step_states.len();
    let steps_completed = step_states
        .values()
        .filter(|state| matches!(state, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = step_states
        .values()
        .filter(|state| matches!(state, StepExecutionState::Failed { .. }))
        .count();
    let progress_pct = if total_steps > 0 {
        (steps_completed as f64 / total_steps as f64) * 100.0
    } else {
        0.0
    };

    crate::services::plan_mode::types::PlanExecutionProgress {
        current_batch,
        total_batches,
        steps_completed,
        steps_failed,
        total_steps,
        progress_pct,
    }
}

fn compute_retry_stats(
    step_attempts: &HashMap<String, usize>,
    step_states: &HashMap<String, StepExecutionState>,
) -> PlanRetryStats {
    let mut total_retries = 0usize;
    let mut steps_retried = 0usize;
    for attempts in step_attempts.values() {
        if *attempts > 1 {
            steps_retried += 1;
            total_retries += attempts.saturating_sub(1);
        }
    }

    let exhausted_failures = step_states
        .iter()
        .filter(|(step_id, state)| {
            matches!(state, StepExecutionState::Failed { .. })
                && step_attempts
                    .get(step_id.as_str())
                    .map(|attempts| *attempts > 1)
                    .unwrap_or(false)
        })
        .count();

    PlanRetryStats {
        total_retries,
        steps_retried,
        exhausted_failures,
    }
}

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
        resolve_plan_provider_and_model(provider, model, app_state.inner()).await;
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
                kernel_state.inner(),
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
                    persist_plan_session_best_effort(&state, &updated_session, "generate_plan.reviewing_plan")
                        .await;

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
        mut plan,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    let max_parallel = plan.execution_config.normalized_max_parallel();
    plan.execution_config.max_parallel = max_parallel;
    let max_step_iterations = plan.execution_config.normalized_max_step_iterations();
    plan.execution_config.max_step_iterations = max_step_iterations;
    let retry_policy = plan.execution_config.normalized_retry_policy();
    plan.execution_config.retry = retry_policy.clone();
    plan.batches = crate::services::plan_mode::types::calculate_plan_batches_with_parallel(
        &plan.steps,
        max_parallel,
    );

    let resume_payload = serde_json::to_value(PlanExecutionResumePayload {
        provider: provider.clone(),
        model: model.clone(),
        base_url: base_url.clone(),
        project_path: project_path.clone(),
        context_sources: context_sources.clone(),
        conversation_context: conversation_context.clone(),
        locale: locale.clone(),
    })
    .ok();

    // Validate
    let (adapter_name, task_description, resume_state) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase != PlanModePhase::ReviewingPlan
            && session.phase != PlanModePhase::Executing
        {
            return Ok(CommandResponse::err("Not in reviewing phase"));
        }

        (
            plan.adapter_name.clone(),
            session.description.clone(),
            crate::services::plan_mode::step_executor::PlanExecutionResumeState {
                step_outputs: session.step_outputs.clone(),
                step_states: session.step_states.clone(),
                step_attempts: session.step_attempts.clone(),
            },
        )
    };

    let (resolved_provider, resolved_model) =
        resolve_plan_provider_and_model(provider, model, app_state.inner()).await;
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
        let was_resuming = session.phase == PlanModePhase::Executing;
        session.plan = Some(plan.clone());
        session.phase = PlanModePhase::Executing;
        session.execution_resume_payload = resume_payload.clone();
        if !was_resuming {
            session.step_attempts.clear();
        }
        session.clone()
    };
    persist_plan_session_best_effort(
        &state,
        &executing_session_snapshot,
        "approve_plan.executing",
    )
    .await;

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
    let state_for_persist = state.inner().clone();
    let sid = session_id.clone();
    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag).to_string();
    let execution_context_bundle = build_plan_conversation_context(
        &app_state,
        &knowledge_state,
        kernel_state.inner(),
        project_path.as_deref(),
        Some(session_id.as_str()),
        conversation_context.as_deref(),
        context_sources.as_ref(),
        &task_description,
        InjectionPhase::Implementation,
    )
    .await;
    let execution_context = if execution_context_bundle.rendered_context.is_empty() {
        None
    } else {
        Some(execution_context_bundle.rendered_context)
    };

    let resolved_project_root = match project_path.as_deref().map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => standalone_state.working_directory.read().await.clone(),
    };
    let resolved_project_path = resolved_project_root.to_string_lossy().to_string();

    let selected_skills =
        crate::services::task_mode::context_provider::hydrate_skill_matches_by_ids(
            app_state.inner(),
            &resolved_project_path,
            &execution_context_bundle.effective_skill_ids,
        )
        .await;

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
        selected_skills,
    };

    tokio::spawn(async move {
        let mut config = crate::services::plan_mode::step_executor::StepExecutionConfig::default();
        config.max_parallel = max_parallel;
        config.max_step_iterations = max_step_iterations;
        config.max_retry_attempts = if retry_policy.enabled {
            retry_policy.max_attempts
        } else {
            0
        };
        config.retry_backoff_ms = retry_policy.backoff_ms;
        config.fail_batch_on_exhausted = retry_policy.fail_batch_on_exhausted;

        let mut plan_mut = plan;
        let app_for_execute = app_handle.clone();

        let progress_callback: crate::services::plan_mode::step_executor::PlanExecutionProgressCallback = Arc::new({
            let sessions_arc = sessions_arc.clone();
            let state_for_persist = state_for_persist.clone();
            let kernel_state = app_handle.state::<WorkflowKernelState>().inner().clone();
            let app_for_progress = app_handle.clone();
            let sid = sid.clone();
            move |checkpoint| {
                let sessions_arc = sessions_arc.clone();
                let state_for_persist = state_for_persist.clone();
                let kernel_state = kernel_state.clone();
                let app_for_progress = app_for_progress.clone();
                let sid = sid.clone();
                Box::pin(async move {
                    let snapshot = {
                        let mut sessions = sessions_arc.write().await;
                        if let Some(session) = sessions.get_mut(&sid) {
                            session.phase = PlanModePhase::Executing;
                            session.step_outputs = checkpoint.step_outputs;
                            session.step_states = checkpoint.step_states;
                            session.step_attempts = checkpoint.step_attempts;
                            session.progress = Some(build_plan_progress_from_checkpoint(
                                checkpoint.current_batch,
                                checkpoint.total_batches,
                                &session.step_states,
                            ));
                            Some(session.clone())
                        } else {
                            None
                        }
                    };
                    if let Some(snapshot) = snapshot {
                        persist_plan_session_best_effort(
                            &state_for_persist,
                            &snapshot,
                            "approve_plan.progress_checkpoint",
                        )
                        .await;
                        sync_kernel_plan_snapshot_and_emit(
                            &app_for_progress,
                            &kernel_state,
                            &snapshot,
                            "plan_mode.approve_plan.progress_checkpoint",
                        )
                        .await;
                    }
                })
            }
        });

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
            Some(resume_state.clone()),
            Some(progress_callback),
        )
        .await;

        // Update session with results
        let mut updated_session_snapshot: Option<PlanModeSession> = None;
        let mut sessions = sessions_arc.write().await;
        if let Some(session) = sessions.get_mut(&sid) {
            match result {
                Ok((outputs, states, step_attempts)) => {
                    let failed = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Failed { .. }));
                    let cancelled = states
                        .values()
                        .any(|s| matches!(s, StepExecutionState::Cancelled));

                    session.step_outputs = outputs;
                    session.step_states = states;
                    session.step_attempts = step_attempts;
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
                    session.step_attempts.clear();
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
            persist_plan_session_best_effort(
                &state_for_persist,
                &updated_session,
                "approve_plan.completed",
            )
            .await;
            let kernel_state = app_handle.state::<WorkflowKernelState>();
            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &updated_session,
                "plan_mode.approve_plan.completed",
            )
            .await;
            if let Some(completion_line) = build_plan_completion_transcript_line(&updated_session) {
                append_plan_transcript_lines_for_linked_sessions(
                    &app_handle,
                    kernel_state.inner(),
                    &sid,
                    vec![completion_line],
                    "plan_mode.approve_plan.completed",
                )
                .await;
            }
        }

        // Clear cancellation token
        let mut tokens = tokens_arc.write().await;
        tokens.remove(&sid);
    });

    Ok(CommandResponse::ok(true))
}

/// Retry a single failed/cancelled plan step and keep existing plan outputs/states.
#[tauri::command]
pub async fn retry_plan_step(
    request: RetryPlanStepRequest,
    state: tauri::State<'_, PlanModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    standalone_state: tauri::State<'_, crate::commands::standalone::StandaloneState>,
    permission_state: tauri::State<'_, crate::commands::permissions::PermissionState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<bool>, String> {
    let RetryPlanStepRequest {
        session_id,
        step_id,
        provider,
        model,
        base_url,
        project_path,
        context_sources,
        conversation_context,
        locale,
    } = request;

    let normalized_step_id = step_id.trim().to_string();
    if normalized_step_id.is_empty() {
        return Ok(CommandResponse::err("Step id cannot be empty"));
    }

    {
        let tokens = state.cancellation_tokens.read().await;
        if tokens.contains_key(&session_id) {
            return Ok(CommandResponse::err(
                "Plan execution already in progress for this session",
            ));
        }
    }

    let (plan, adapter_name, task_description, step_outputs, step_states) = {
        let sessions = state.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;

        if session.phase == PlanModePhase::Executing {
            return Ok(CommandResponse::err(
                "Plan execution already in progress for this session",
            ));
        }

        let plan = session
            .plan
            .clone()
            .ok_or_else(|| "No plan available for retry".to_string())?;

        if !plan.steps.iter().any(|step| step.id == normalized_step_id) {
            return Ok(CommandResponse::err(format!(
                "Step not found in plan: {}",
                normalized_step_id
            )));
        }

        match session.step_states.get(&normalized_step_id) {
            Some(StepExecutionState::Failed { .. }) | Some(StepExecutionState::Cancelled) => {}
            Some(_) => {
                return Ok(CommandResponse::err(format!(
                    "Step '{}' is not retryable (must be failed/cancelled)",
                    normalized_step_id
                )))
            }
            None => {
                return Ok(CommandResponse::err(format!(
                    "Step '{}' has no execution state and cannot be retried",
                    normalized_step_id
                )))
            }
        }

        (
            plan.clone(),
            plan.adapter_name.clone(),
            session.description.clone(),
            session.step_outputs.clone(),
            session.step_states.clone(),
        )
    };

    let (resolved_provider, resolved_model) =
        resolve_plan_provider_and_model(provider, model, app_state.inner()).await;
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

    // Mark retry as executing immediately for kernel/UI convergence.
    let executing_session_snapshot = {
        let mut sessions = state.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| "No active plan mode session".to_string())?;
        session.phase = PlanModePhase::Executing;
        session
            .step_states
            .insert(normalized_step_id.clone(), StepExecutionState::Running);
        session.clone()
    };
    persist_plan_session_best_effort(
        &state,
        &executing_session_snapshot,
        "retry_plan_step.executing",
    )
    .await;

    sync_kernel_plan_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &executing_session_snapshot,
        "plan_mode.retry_plan_step.executing",
    )
    .await;

    let cancel_token = CancellationToken::new();
    {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.insert(session_id.clone(), cancel_token.clone());
    }

    let sessions_arc = state.sessions.clone();
    let tokens_arc = state.cancellation_tokens.clone();
    let state_for_persist = state.inner().clone();
    let sid = session_id.clone();
    let retry_step_id = normalized_step_id.clone();
    let locale_tag = normalize_locale(locale.as_deref());
    let lang_instruction = locale_instruction(locale_tag).to_string();
    let execution_context_bundle = build_plan_conversation_context(
        &app_state,
        &knowledge_state,
        kernel_state.inner(),
        project_path.as_deref(),
        Some(session_id.as_str()),
        conversation_context.as_deref(),
        context_sources.as_ref(),
        &task_description,
        InjectionPhase::Implementation,
    )
    .await;
    let execution_context = if execution_context_bundle.rendered_context.is_empty() {
        None
    } else {
        Some(execution_context_bundle.rendered_context)
    };

    let resolved_project_root = match project_path.as_deref().map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => standalone_state.working_directory.read().await.clone(),
    };
    let resolved_project_path = resolved_project_root.to_string_lossy().to_string();

    let selected_skills =
        crate::services::task_mode::context_provider::hydrate_skill_matches_by_ids(
            app_state.inner(),
            &resolved_project_path,
            &execution_context_bundle.effective_skill_ids,
        )
        .await;

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
        selected_skills,
    };

    tokio::spawn(async move {
        let mut config = crate::services::plan_mode::step_executor::StepExecutionConfig::default();
        let normalized_retry = plan.execution_config.normalized_retry_policy();
        config.max_parallel = plan.execution_config.normalized_max_parallel();
        config.max_step_iterations = plan.execution_config.normalized_max_step_iterations();
        config.max_retry_attempts = if normalized_retry.enabled {
            normalized_retry.max_attempts
        } else {
            0
        };
        config.retry_backoff_ms = normalized_retry.backoff_ms;
        config.fail_batch_on_exhausted = normalized_retry.fail_batch_on_exhausted;

        let app_for_retry = app_handle.clone();
        let result = crate::services::plan_mode::step_executor::retry_single_step(
            &sid,
            &plan,
            &retry_step_id,
            step_outputs,
            step_states,
            adapter,
            llm_provider,
            Some(step_runtime),
            config,
            execution_context,
            lang_instruction,
            Some(app_for_retry),
            cancel_token,
        )
        .await;

        let mut updated_session_snapshot: Option<PlanModeSession> = None;
        let mut sessions = sessions_arc.write().await;
        if let Some(session) = sessions.get_mut(&sid) {
            match result {
                Ok((outputs, states)) => {
                    let has_non_completed = states.values().any(|state| {
                        matches!(
                            state,
                            StepExecutionState::Failed { .. } | StepExecutionState::Cancelled
                        )
                    });

                    session.step_outputs = outputs;
                    session.step_states = states;
                    let attempts = session
                        .step_attempts
                        .entry(retry_step_id.clone())
                        .or_insert(0);
                    *attempts = attempts.saturating_add(1);
                    session.plan = Some(plan.clone());
                    if has_non_completed {
                        session.phase = PlanModePhase::Failed;
                    } else {
                        session.phase = PlanModePhase::Completed;
                    }
                }
                Err(error) => {
                    session.phase = PlanModePhase::Failed;
                    let attempts = session
                        .step_attempts
                        .entry(retry_step_id.clone())
                        .or_insert(0);
                    *attempts = attempts.saturating_add(1);
                    session.step_states.insert(
                        retry_step_id.clone(),
                        StepExecutionState::Failed {
                            reason: format!("{error}"),
                        },
                    );
                }
            }
            updated_session_snapshot = Some(session.clone());
        }
        drop(sessions);

        if let Some(updated_session) = updated_session_snapshot {
            persist_plan_session_best_effort(
                &state_for_persist,
                &updated_session,
                "retry_plan_step.completed",
            )
            .await;
            let kernel_state = app_handle.state::<WorkflowKernelState>();
            sync_kernel_plan_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &updated_session,
                "plan_mode.retry_plan_step.completed",
            )
            .await;
            if let Some(completion_line) = build_plan_completion_transcript_line(&updated_session) {
                append_plan_transcript_lines_for_linked_sessions(
                    &app_handle,
                    kernel_state.inner(),
                    &sid,
                    vec![completion_line],
                    "plan_mode.retry_plan_step.completed",
                )
                .await;
            }
        }

        let mut tokens = tokens_arc.write().await;
        tokens.remove(&sid);
    });

    Ok(CommandResponse::ok(true))
}
