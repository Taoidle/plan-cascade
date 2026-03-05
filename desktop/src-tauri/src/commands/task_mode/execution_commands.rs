use super::*;

/// Approve a task PRD and trigger batch execution.
///
/// Validates the PRD structure, spawns execution as a background tokio task,
/// and returns immediately. Progress events are emitted via Tauri's
/// AppHandle::emit('task-mode-progress', payload) during execution.
#[tauri::command]
pub async fn approve_task_prd(
    app: tauri::AppHandle,
    request: ApproveTaskPrdRequest,
    state: tauri::State<'_, TaskModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    plugin_state: tauri::State<'_, crate::commands::plugins::PluginState>,
) -> Result<CommandResponse<bool>, String> {
    let ApproveTaskPrdRequest {
        session_id,
        prd,
        provider,
        model,
        base_url,
        execution_mode,
        workflow_config,
        global_default_agent,
        phase_configs,
        context_sources,
        project_path,
    } = request;

    let task_description = {
        let sessions = state.sessions.read().await;
        let session = match sessions.get(&session_id) {
            Some(s) => s,
            None => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        };
        if session.status != TaskModeStatus::ReviewingPrd {
            return Ok(CommandResponse::err(format!(
                "Cannot approve PRD in {:?} status",
                session.status
            )));
        }
        session.description.clone()
    };

    // Validate PRD
    if prd.stories.is_empty() {
        return Ok(CommandResponse::err("PRD must contain at least one story"));
    }

    // Calculate batches
    let stories: Vec<ExecutableStory> = prd
        .stories
        .iter()
        .map(|s| ExecutableStory {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            dependencies: s.dependencies.clone(),
            acceptance_criteria: s.acceptance_criteria.clone(),
            agent: None,
        })
        .collect();

    // Build execution config from workflow config overrides
    let mut config = ExecutionConfig::default();
    if let Some(ref wc) = workflow_config {
        if let Some(max_p) = wc.max_parallel {
            config.max_parallel = max_p;
        }
        config.skip_verification = wc.skip_verification;
        config.skip_review = wc.skip_review;
    }
    match crate::services::task_mode::calculate_batches(&stories, config.max_parallel) {
        Ok(batches) => {
            let mut approved_prd = prd;
            approved_prd.batches = batches;
            {
                let mut updated_session: Option<TaskModeSession> = None;
                let mut sessions = state.sessions.write().await;
                let session = match sessions.get_mut(&session_id) {
                    Some(s) => s,
                    None => {
                        return Ok(CommandResponse::err(
                            "Invalid session ID or no active session",
                        ))
                    }
                };
                session.prd = Some(approved_prd);
                session.status = TaskModeStatus::Executing;
                updated_session = Some(session.clone());
                drop(sessions);
                if let Some(snapshot) = updated_session.as_ref() {
                    persist_task_session_best_effort(
                        &state,
                        snapshot,
                        "approve_task_prd.status_executing",
                    )
                    .await;
                    sync_kernel_task_snapshot_and_emit(
                        &app,
                        kernel_state.inner(),
                        snapshot,
                        None,
                        "task_mode.approve_task_prd.status_executing",
                    )
                    .await;
                }
            }

            // Create cancellation token for this execution
            let cancellation_token = CancellationToken::new();
            {
                let mut tokens = state.cancellation_tokens.write().await;
                tokens.insert(session_id.clone(), cancellation_token.clone());
            }

            // Clear any previous execution result
            {
                let mut results = state.execution_results.write().await;
                results.remove(&session_id);
            }

            // Clone what we need for the spawned background task
            let sessions_arc = state.sessions.clone();
            let results_arc = state.execution_results.clone();
            let tokens_arc = state.cancellation_tokens.clone();
            let kernel_state_handle = kernel_state.inner().clone();
            let state_for_persist = state.inner().clone();
            let sid = session_id.clone();
            let app_handle = app.clone();
            let stories_for_exec = stories.clone();

            // Resolve LLM provider config if provider/model specified
            let provider_config: Option<crate::services::llm::types::ProviderConfig> =
                if let (Some(ref prov), Some(ref mdl)) = (&provider, &model) {
                    match resolve_provider_config(prov, mdl, None, base_url.clone(), &app_state)
                        .await
                    {
                        Ok(cfg) => Some(cfg),
                        Err(e) => {
                            eprintln!(
                                "[approve_task_prd] LLM provider config resolution failed: {}",
                                e
                            );
                            None
                        }
                    }
                } else {
                    None
                };

            // Determine execution mode:
            // - If explicitly specified, use that
            // - If LLM provider config available, default to Llm
            // - Otherwise default to Cli
            let mode = execution_mode.unwrap_or_else(|| {
                if provider_config.is_some() {
                    StoryExecutionMode::Llm
                } else {
                    StoryExecutionMode::Cli
                }
            });

            // Resolve database pool for OrchestratorService (if using LLM mode)
            let db_pool = if matches!(mode, StoryExecutionMode::Llm) {
                app_state
                    .with_database(|db| Ok(db.pool().clone()))
                    .await
                    .ok()
            } else {
                None
            };

            // Pre-compute domain context before tokio::spawn (needs Tauri State access)
            // Only query sources the user explicitly enabled via context_sources.
            let project_path_str = project_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .unwrap_or(".")
                .to_string();
            let enriched_ctx = assemble_enriched_context_v2(
                app_state.inner(),
                knowledge_state.inner(),
                &project_path_str,
                &task_description,
                crate::services::skills::model::InjectionPhase::Implementation,
                context_sources.as_ref(),
                "task",
                Some(session_id.as_str()),
                !matches!(mode, StoryExecutionMode::Llm),
            )
            .await;
            let knowledge_block = enriched_ctx.knowledge_block;
            let memory_block = enriched_ctx.memory_block;
            let skills_block = enriched_ctx.skills_block;
            let selected_skill_matches = enriched_ctx.selected_skills;

            // Pre-compute knowledge tool params for LLM mode (needs Tauri State access)
            let knowledge_tool_params: Option<KnowledgeToolParams> = if matches!(
                mode,
                StoryExecutionMode::Llm
            ) {
                if let Some(ref cs) = context_sources {
                    if cs.knowledge.as_ref().map_or(false, |k| k.enabled) {
                        crate::services::task_mode::context_provider::ensure_knowledge_initialized_public(
                                &knowledge_state, &app_state,
                            ).await;
                        if let Ok(pipeline) = knowledge_state.get_pipeline().await {
                            let pid = if cs.project_id.is_empty() {
                                "default".to_string()
                            } else {
                                cs.project_id.clone()
                            };
                            let collections = pipeline.list_collections(&pid).unwrap_or_default();
                            let language = crate::services::tools::system_prompt::detect_language(
                                &task_description,
                            );
                            let summaries: Vec<
                                    crate::services::tools::system_prompt::KnowledgeCollectionSummary,
                                > = collections
                                    .iter()
                                    .map(|c| {
                                        crate::services::tools::system_prompt::KnowledgeCollectionSummary {
                                            name: c.name.clone(),
                                            document_count: pipeline
                                                .list_documents(&c.id)
                                                .map(|d| d.len())
                                                .unwrap_or(0),
                                            chunk_count: c.chunk_count as usize,
                                        }
                                    })
                                    .collect();
                            let awareness =
                                    crate::services::tools::system_prompt::build_knowledge_awareness_section(
                                        &summaries, language,
                                    );
                            let k = cs.knowledge.as_ref().unwrap();
                            let col_filter = if k.selected_collections.is_empty() {
                                None
                            } else {
                                Some(k.selected_collections.clone())
                            };
                            let doc_filter = if k.selected_documents.is_empty() {
                                None
                            } else {
                                Some(k.selected_documents.clone())
                            };
                            Some(KnowledgeToolParams {
                                pipeline,
                                project_id: pid,
                                collection_filter: col_filter,
                                document_filter: doc_filter,
                                awareness_section: awareness,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let plugin_quality_gates = plugin_state.collect_quality_gates().await;
            config.plugin_quality_gates = plugin_quality_gates;

            // Spawn background tokio task for batch execution
            let exec_config = config;
            tokio::spawn(async move {
                let executor =
                    BatchExecutor::new(stories_for_exec, exec_config, cancellation_token);
                let resolver = match &phase_configs {
                    Some(configs) if !configs.is_empty() => AgentResolver::new(
                        build_agents_config_from_frontend(configs, global_default_agent.as_deref()),
                    ),
                    _ => AgentResolver::with_defaults(),
                };

                // Create emit callback that sends events via Tauri AppHandle
                let app_for_emit = app_handle.clone();
                let kernel_for_emit = kernel_state_handle.clone();
                let sid_for_emit = sid.clone();
                let completed_counter = Arc::new(AtomicU64::new(0));
                let failed_counter = Arc::new(AtomicU64::new(0));
                let current_story = Arc::new(Mutex::new(None::<String>));
                let emit = move |event: TaskModeProgressEvent| {
                    let _ = app_for_emit.emit(TASK_MODE_EVENT_CHANNEL, &event);

                    match event.event_type.as_str() {
                        "story_started" => {
                            if let Ok(mut story) = current_story.lock() {
                                *story = event.story_id.clone();
                            }
                        }
                        "story_completed" => {
                            completed_counter.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut story) = current_story.lock() {
                                *story = None;
                            }
                        }
                        "story_failed" => {
                            failed_counter.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut story) = current_story.lock() {
                                *story = None;
                            }
                        }
                        _ => {}
                    }

                    let phase = match event.event_type.as_str() {
                        "execution_completed" => "completed",
                        "execution_cancelled" => "cancelled",
                        "error" => "failed",
                        _ => "executing",
                    };
                    let status = match event.event_type.as_str() {
                        "execution_completed" => Some(WorkflowStatus::Completed),
                        "execution_cancelled" => Some(WorkflowStatus::Cancelled),
                        "error" => Some(WorkflowStatus::Failed),
                        _ => None,
                    };
                    let current_story_id = if event.event_type == "story_started" {
                        event.story_id.clone()
                    } else if event.event_type == "story_completed"
                        || event.event_type == "story_failed"
                    {
                        None
                    } else {
                        current_story.lock().ok().and_then(|story| story.clone())
                    };
                    let completed = completed_counter.load(Ordering::Relaxed);
                    let failed = failed_counter.load(Ordering::Relaxed);
                    let kernel_for_emit = kernel_for_emit.clone();
                    let app_for_emit = app_for_emit.clone();
                    let sid_for_emit = sid_for_emit.clone();
                    tokio::spawn(async move {
                        let kernel_session_ids = kernel_for_emit
                            .sync_task_snapshot_by_linked_session(
                                &sid_for_emit,
                                Some(phase.to_string()),
                                current_story_id,
                                Some(completed),
                                Some(failed),
                                status,
                            )
                            .await
                            .unwrap_or_default();
                        emit_kernel_updates(
                            &app_for_emit,
                            &kernel_for_emit,
                            &kernel_session_ids,
                            "task_mode.approve_task_prd.progress_event",
                        )
                        .await;
                    });
                };

                let project_path = std::path::PathBuf::from(project_path_str.clone());

                // Create story executor that delegates to the appropriate backend.
                // In CLI mode, spawns external CLI tools. In LLM mode, uses OrchestratorService.
                let story_executor = build_story_executor(
                    app_handle.clone(),
                    mode,
                    provider_config,
                    db_pool,
                    knowledge_block,
                    memory_block,
                    skills_block,
                    selected_skill_matches,
                    knowledge_tool_params,
                );

                let result = executor
                    .execute(&sid, &resolver, project_path, emit, story_executor)
                    .await;

                // Update session state based on result
                let mut kernel_snapshot: Option<TaskModeSession> = None;
                let mut sessions = sessions_arc.write().await;
                if let Some(session) = sessions.get_mut(&sid) {
                    match &result {
                        Ok(exec_result) => {
                            // Update progress
                            session.progress = Some(executor.get_progress().await);

                            if exec_result.cancelled {
                                session.status = TaskModeStatus::Cancelled;
                            } else if exec_result.success {
                                session.status = TaskModeStatus::Completed;
                            } else {
                                session.status = TaskModeStatus::Failed;
                            }

                            // Store the result
                            let mut results = results_arc.write().await;
                            results.insert(sid.clone(), exec_result.clone());
                            kernel_snapshot = Some(session.clone());
                        }
                        Err(_) => {
                            session.status = TaskModeStatus::Failed;
                            kernel_snapshot = Some(session.clone());
                        }
                    }
                }
                drop(sessions);
                if let Some(snapshot) = kernel_snapshot.as_ref() {
                    persist_task_session_best_effort(
                        &state_for_persist,
                        snapshot,
                        "approve_task_prd.execution_terminal",
                    )
                    .await;
                    sync_kernel_task_snapshot_and_emit(
                        &app_handle,
                        &kernel_state_handle,
                        snapshot,
                        None,
                        "task_mode.approve_task_prd.execution_terminal",
                    )
                    .await;
                }

                let mut tokens = tokens_arc.write().await;
                tokens.remove(&sid);
            });

            Ok(CommandResponse::ok(true))
        }
        Err(e) => Ok(CommandResponse::err(format!(
            "PRD validation failed: {}",
            e
        ))),
    }
}

/// Get the current task execution status.
#[tauri::command]
pub async fn get_task_execution_status(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<TaskExecutionStatus>, String> {
    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        _ => {
            return Ok(CommandResponse::err(
                "Invalid session ID or no active session",
            ))
        }
    };

    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: session.prd.as_ref().map(|p| p.batches.len()).unwrap_or(0),
        stories_completed: 0,
        stories_failed: 0,
        total_stories: session.prd.as_ref().map(|p| p.stories.len()).unwrap_or(0),
        story_statuses: HashMap::new(),
        current_phase: "idle".to_string(),
    });

    Ok(CommandResponse::ok(TaskExecutionStatus {
        session_id: session.session_id.clone(),
        status: session.status.clone(),
        current_batch: progress.current_batch,
        total_batches: progress.total_batches,
        story_statuses: progress.story_statuses,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
    }))
}

/// Cancel the current task execution.
///
/// Triggers the CancellationToken to gracefully stop the background
/// batch execution task.
#[tauri::command]
pub async fn cancel_task_execution(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<bool>, String> {
    let session_snapshot = {
        let sessions = state.sessions.read().await;
        let session = match sessions.get(&session_id) {
            Some(s) => s,
            _ => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        };
        if session.status != TaskModeStatus::Executing {
            return Ok(CommandResponse::err("No execution in progress to cancel"));
        }
        session.clone()
    };

    // Trigger the cancellation token
    let ct = state.cancellation_tokens.read().await;
    if let Some(token) = ct.get(&session_id) {
        token.cancel();
    } else {
        return Ok(CommandResponse::err("No execution in progress to cancel"));
    }

    sync_kernel_task_snapshot_and_emit(
        &app_handle,
        kernel_state.inner(),
        &session_snapshot,
        Some("executing"),
        "task_mode.cancel_task_execution.requested",
    )
    .await;

    // Note: The background task will update session.status to Cancelled
    // when it detects the cancellation token.
    Ok(CommandResponse::ok(true))
}

/// Cancel a running task pre-execution operation (explore/analysis/PRD generation/review).
#[tauri::command]
pub async fn cancel_task_operation(
    session_id: Option<String>,
    state: tauri::State<'_, TaskModeState>,
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
            "No task operation in progress to cancel",
        ))
    }
}

/// Get the execution report after completion.
///
/// Returns the final `BatchExecutionResult` populated by the background task.
#[tauri::command]
pub async fn get_task_execution_report(
    session_id: String,
    state: tauri::State<'_, TaskModeState>,
) -> Result<CommandResponse<ExecutionReport>, String> {
    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        _ => {
            return Ok(CommandResponse::err(
                "Invalid session ID or no active session",
            ))
        }
    };

    if !matches!(
        session.status,
        TaskModeStatus::Completed | TaskModeStatus::Failed | TaskModeStatus::Cancelled
    ) {
        return Ok(CommandResponse::err("Execution has not finished yet"));
    }

    // Build a story title lookup from the PRD (if available)
    let story_title_map: HashMap<String, String> = session
        .prd
        .as_ref()
        .map(|prd| {
            prd.stories
                .iter()
                .map(|s| (s.id.clone(), s.title.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Build a story-to-batch-index lookup from the PRD batches
    let story_batch_map: HashMap<String, usize> = session
        .prd
        .as_ref()
        .map(|prd| {
            prd.batches
                .iter()
                .flat_map(|b| b.story_ids.iter().map(move |sid| (sid.clone(), b.index)))
                .collect()
        })
        .unwrap_or_default();

    // Try to get the real execution result
    let exec_results = state.execution_results.read().await;
    if let Some(result) = exec_results.get(&session_id) {
        let agent_assignments: HashMap<String, String> = result
            .agent_assignments
            .iter()
            .map(|(id, a)| (id.clone(), a.agent_name.clone()))
            .collect();

        // --- Build timeline entries ---
        let mut timeline = Vec::new();
        // Estimate start_offset_ms per batch: sum durations of prior batches.
        // First, collect max duration per batch from completed stories.
        let mut batch_max_durations: HashMap<usize, u64> = HashMap::new();
        for (story_id, state) in &result.story_results {
            let batch_idx = story_batch_map.get(story_id).copied().unwrap_or(0);
            if let StoryExecutionState::Completed { duration_ms, .. } = state {
                let entry = batch_max_durations.entry(batch_idx).or_insert(0);
                if *duration_ms > *entry {
                    *entry = *duration_ms;
                }
            }
        }
        // Compute cumulative start offsets per batch index
        let max_batch_idx = story_batch_map.values().copied().max().unwrap_or(0);
        let mut batch_start_offsets: Vec<u64> = vec![0; max_batch_idx + 1];
        for i in 1..=max_batch_idx {
            batch_start_offsets[i] = batch_start_offsets[i - 1]
                + batch_max_durations.get(&(i - 1)).copied().unwrap_or(0);
        }

        for (story_id, story_state) in &result.story_results {
            let batch_idx = story_batch_map.get(story_id).copied().unwrap_or(0);
            let start_offset_ms = batch_start_offsets.get(batch_idx).copied().unwrap_or(0);
            let story_title = story_title_map
                .get(story_id)
                .cloned()
                .unwrap_or_else(|| story_id.clone());

            match story_state {
                StoryExecutionState::Completed {
                    agent,
                    duration_ms,
                    gate_result,
                } => {
                    let gate_summary = gate_result.as_ref().map(|pr| {
                        if pr.passed {
                            "passed".to_string()
                        } else {
                            format!(
                                "failed ({})",
                                pr.short_circuit_phase
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "validation".to_string())
                            )
                        }
                    });
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: agent.clone(),
                        duration_ms: *duration_ms,
                        start_offset_ms,
                        status: "completed".to_string(),
                        gate_result: gate_summary,
                    });
                }
                StoryExecutionState::Failed { last_agent, .. } => {
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: last_agent.clone(),
                        duration_ms: 0,
                        start_offset_ms,
                        status: "failed".to_string(),
                        gate_result: None,
                    });
                }
                StoryExecutionState::Cancelled => {
                    timeline.push(TimelineEntry {
                        story_id: story_id.clone(),
                        story_title,
                        batch_index: batch_idx,
                        agent: agent_assignments.get(story_id).cloned().unwrap_or_default(),
                        duration_ms: 0,
                        start_offset_ms,
                        status: "cancelled".to_string(),
                        gate_result: None,
                    });
                }
                _ => {} // Pending/Running shouldn't appear in final results
            }
        }
        // Sort timeline by batch_index then story_id for deterministic output
        timeline.sort_by(|a, b| {
            a.batch_index
                .cmp(&b.batch_index)
                .then_with(|| a.story_id.cmp(&b.story_id))
        });

        // --- Build agent performance ---
        // Tracks: (assigned, completed, durations_vec)
        let mut agent_stats: HashMap<String, (usize, usize, Vec<u64>)> = HashMap::new();
        for (story_id, assignment) in &result.agent_assignments {
            let entry =
                agent_stats
                    .entry(assignment.agent_name.clone())
                    .or_insert((0, 0, Vec::new()));
            entry.0 += 1; // assigned
            if let Some(story_state) = result.story_results.get(story_id) {
                if let StoryExecutionState::Completed { duration_ms, .. } = story_state {
                    entry.1 += 1; // completed
                    entry.2.push(*duration_ms);
                }
            }
        }
        let agent_performance: Vec<AgentPerformanceEntry> = agent_stats
            .into_iter()
            .map(|(agent_name, (assigned, completed, durations))| {
                let success_rate = if assigned > 0 {
                    completed as f64 / assigned as f64
                } else {
                    0.0
                };
                let average_duration_ms = if !durations.is_empty() {
                    durations.iter().sum::<u64>() / durations.len() as u64
                } else {
                    0
                };
                AgentPerformanceEntry {
                    agent_name,
                    stories_assigned: assigned,
                    stories_completed: completed,
                    success_rate,
                    average_duration_ms,
                    average_quality_score: None, // populated below if quality scores exist
                }
            })
            .collect();

        // --- Build quality scores ---
        let quality_dimensions = [
            "correctness",
            "readability",
            "maintainability",
            "test_coverage",
            "security",
        ];
        let mut quality_scores = Vec::new();
        for (story_id, story_state) in &result.story_results {
            if let StoryExecutionState::Completed {
                gate_result: Some(pipeline_result),
                ..
            } = story_state
            {
                // Extract quality dimension scores from gate results.
                // Each gate that passed gets 100, failed gets 0. We map gate IDs
                // to quality dimensions where possible, and generate default
                // dimension scores based on overall pass/fail.
                let gate_results: Vec<_> = pipeline_result
                    .phase_results
                    .iter()
                    .flat_map(|pr| pr.gate_results.iter())
                    .collect();

                for dim in &quality_dimensions {
                    let score =
                        compute_quality_dimension_score(dim, &gate_results, pipeline_result.passed);
                    quality_scores.push(QualityDimensionScore {
                        story_id: story_id.clone(),
                        dimension: dim.to_string(),
                        score,
                        max_score: 100.0,
                    });
                }
            }
        }

        // Compute average quality score per agent
        let mut agent_performance = agent_performance;
        for entry in &mut agent_performance {
            let agent_story_scores: Vec<f64> = quality_scores
                .iter()
                .filter(|qs| {
                    result
                        .agent_assignments
                        .get(&qs.story_id)
                        .map(|a| a.agent_name == entry.agent_name)
                        .unwrap_or(false)
                })
                .map(|qs| qs.score)
                .collect();
            if !agent_story_scores.is_empty() {
                let avg = agent_story_scores.iter().sum::<f64>() / agent_story_scores.len() as f64;
                entry.average_quality_score = Some(avg);
            }
        }

        return Ok(CommandResponse::ok(ExecutionReport {
            session_id: session.session_id.clone(),
            total_stories: result.total_stories,
            stories_completed: result.completed,
            stories_failed: result.failed,
            total_duration_ms: result.total_duration_ms,
            agent_assignments,
            success: result.success,
            quality_scores,
            timeline,
            agent_performance,
        }));
    }

    // Fallback to progress-based report (no BatchExecutionResult available)
    let progress = session.progress.clone().unwrap_or(BatchExecutionProgress {
        current_batch: 0,
        total_batches: 0,
        stories_completed: 0,
        stories_failed: 0,
        total_stories: 0,
        story_statuses: HashMap::new(),
        current_phase: "complete".to_string(),
    });

    Ok(CommandResponse::ok(ExecutionReport {
        session_id: session.session_id.clone(),
        total_stories: progress.total_stories,
        stories_completed: progress.stories_completed,
        stories_failed: progress.stories_failed,
        total_duration_ms: 0,
        agent_assignments: HashMap::new(),
        success: session.status == TaskModeStatus::Completed,
        quality_scores: Vec::new(),
        timeline: Vec::new(),
        agent_performance: Vec::new(),
    }))
}
