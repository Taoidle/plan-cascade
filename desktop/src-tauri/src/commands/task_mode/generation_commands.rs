use super::*;

/// Generate a task PRD from the session description using an LLM provider.
///
/// Calls the configured LLM provider to decompose the task description into
/// stories with dependencies, priorities, and acceptance criteria.
/// Implements retry-with-repair per ADR-F002 for JSON parse failures.
///
/// # Parameters
/// - `session_id`: The active task mode session ID
/// - `provider`: LLM provider name (e.g., "anthropic", "openai", "ollama")
/// - `model`: Model identifier (e.g., "claude-3-5-sonnet-20241022")
/// - `apiKey`: Optional API key (falls back to OS keyring)
/// - `baseUrl`: Optional base URL override
#[tauri::command]
pub async fn generate_task_prd(
    request: GenerateTaskPrdRequest,
    state: tauri::State<'_, TaskModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<TaskPrd>, String> {
    let GenerateTaskPrdRequest {
        session_id,
        provider,
        model,
        api_key,
        base_url,
        compiled_spec,
        conversation_history,
        max_context_tokens,
        context_sources,
        project_path,
    } = request;

    // Validate and extract session
    let (description, status) = {
        let sessions = state.sessions.read().await;
        match sessions.get(&session_id) {
            Some(s) => (s.description.clone(), s.status.clone()),
            None => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        }
    };

    if status != TaskModeStatus::Initialized {
        return Ok(CommandResponse::err(format!(
            "Cannot generate PRD in {:?} status",
            status
        )));
    }

    // Update status to GeneratingPrd
    {
        let mut sessions = state.sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.status = TaskModeStatus::GeneratingPrd;
            let snapshot = s.clone();
            drop(sessions);
            persist_task_session_best_effort(&state, &snapshot, "generate_task_prd.status_generating").await;
            sync_kernel_task_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &snapshot,
                None,
                "task_mode.generate_task_prd.status_generating",
            )
            .await;
        } else {
            return Ok(CommandResponse::err(
                "Invalid session ID or no active session",
            ));
        }
    }
    let (operation_id, operation_token) = register_task_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(TASK_OPERATION_CANCELLED_ERROR)),
        result = async {
            // If compiled_spec is provided (from interview pipeline), convert directly
            if let Some(spec_value) = compiled_spec {
                match prd_generator::convert_compiled_prd_to_task_prd(spec_value) {
                    Ok(prd) => {
                        let mut updated_session: Option<TaskModeSession> = None;
                        let mut sessions = state.sessions.write().await;
                        if let Some(s) = sessions.get_mut(&session_id) {
                            s.status = TaskModeStatus::ReviewingPrd;
                            s.prd = Some(prd.clone());
                            updated_session = Some(s.clone());
                        } else {
                            return Ok(CommandResponse::err(
                                "Invalid session ID or no active session",
                            ));
                        }
                        drop(sessions);
                        if let Some(snapshot) = updated_session.as_ref() {
                            persist_task_session_best_effort(
                                &state,
                                snapshot,
                                "generate_task_prd.compiled_spec_reviewing",
                            )
                            .await;
                            sync_kernel_task_snapshot_and_emit(
                                &app_handle,
                                kernel_state.inner(),
                                snapshot,
                                None,
                                "task_mode.generate_task_prd.compiled_spec_reviewing",
                            )
                            .await;
                        }
                        return Ok(CommandResponse::ok(prd));
                    }
                    Err(e) => {
                        eprintln!(
                            "[generate_task_prd] compiled_spec conversion failed, falling back to LLM: {}",
                            e
                        );
                        // Fall through to LLM generation
                    }
                }
            }

            // Resolve provider/model: explicit param → database settings → hardcoded defaults
            let resolved_provider = match provider {
                Some(ref p) if !p.is_empty() => p.clone(),
                _ => {
                    match app_state
                        .with_database(|db| db.get_setting("llm_provider"))
                        .await
                    {
                        Ok(Some(p)) if !p.is_empty() => p,
                        _ => "anthropic".to_string(),
                    }
                }
            };
            let resolved_model = match model {
                Some(ref m) if !m.is_empty() => m.clone(),
                _ => {
                    match app_state
                        .with_database(|db| db.get_setting("llm_model"))
                        .await
                    {
                        Ok(Some(m)) if !m.is_empty() => m,
                        _ => match resolved_provider.as_str() {
                            "anthropic" => "claude-sonnet-4-20250514".to_string(),
                            "openai" => "gpt-4o".to_string(),
                            "deepseek" => "deepseek-chat".to_string(),
                            "ollama" => "qwen2.5-coder:14b".to_string(),
                            _ => "claude-sonnet-4-20250514".to_string(),
                        },
                    }
                }
            };

            // Resolve provider configuration
            let llm_provider = match resolve_llm_provider(
                &resolved_provider,
                &resolved_model,
                api_key,
                base_url,
                &app_state,
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    // Reset status back to Initialized on failure
                    let mut updated_session: Option<TaskModeSession> = None;
                    let mut sessions = state.sessions.write().await;
                    if let Some(s) = sessions.get_mut(&session_id) {
                        s.status = TaskModeStatus::Initialized;
                        updated_session = Some(s.clone());
                    }
                    drop(sessions);
                    if let Some(snapshot) = updated_session.as_ref() {
                        persist_task_session_best_effort(
                            &state,
                            snapshot,
                            "generate_task_prd.provider_resolution_failed",
                        )
                        .await;
                        sync_kernel_task_snapshot_and_emit(
                            &app_handle,
                            kernel_state.inner(),
                            snapshot,
                            None,
                            "task_mode.generate_task_prd.provider_resolution_failed",
                        )
                        .await;
                    }
                    return Ok(CommandResponse::err(e));
                }
            };

            // Call LLM for PRD generation with conversation history context
            let history = conversation_history.unwrap_or_default();
            let context_budget = max_context_tokens.unwrap_or(200_000);

            // Read exploration result from session for context injection
            let exploration_context_str = {
                let sessions = state.sessions.read().await;
                sessions
                    .get(&session_id)
                    .and_then(|s| s.exploration_result.as_ref())
                    .map(exploration::format_exploration_context)
            };

            // Query domain knowledge for PRD generation (only if user enabled sources)
            let project_path_str = project_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .unwrap_or(".")
                .to_string();
            let enriched = assemble_enriched_context_v2(
                app_state.inner(),
                knowledge_state.inner(),
                &project_path_str,
                &description,
                crate::services::skills::model::InjectionPhase::Planning,
                context_sources.as_ref(),
                "task",
                Some(session_id.as_str()),
                true,
            )
            .await;
            let combined_context = crate::services::task_mode::context_provider::merge_enriched_context(
                exploration_context_str.as_deref(),
                &enriched.knowledge_block,
                &enriched.memory_block,
            );
            let combined_context = match (
                combined_context,
                enriched.skills_block.trim().is_empty(),
            ) {
                (Some(base), false) => Some(format!("{}\n\n{}", base, enriched.skills_block)),
                (None, false) => Some(enriched.skills_block.clone()),
                (base, true) => base,
            };

            let prd = match prd_generator::generate_prd_with_llm(
                llm_provider,
                &description,
                &history,
                context_budget,
                combined_context.as_deref(),
            )
            .await
            {
                Ok(prd) => prd,
                Err(e) => {
                    // Reset status back to Initialized on failure
                    let mut updated_session: Option<TaskModeSession> = None;
                    let mut sessions = state.sessions.write().await;
                    if let Some(s) = sessions.get_mut(&session_id) {
                        s.status = TaskModeStatus::Initialized;
                        updated_session = Some(s.clone());
                    }
                    drop(sessions);
                    if let Some(snapshot) = updated_session.as_ref() {
                        persist_task_session_best_effort(
                            &state,
                            snapshot,
                            "generate_task_prd.llm_generation_failed",
                        )
                        .await;
                        sync_kernel_task_snapshot_and_emit(
                            &app_handle,
                            kernel_state.inner(),
                            snapshot,
                            None,
                            "task_mode.generate_task_prd.llm_generation_failed",
                        )
                        .await;
                    }
                    return Ok(CommandResponse::err(format!(
                        "PRD generation failed: {}",
                        e
                    )));
                }
            };

            // Update session with generated PRD
            {
                let mut updated_session: Option<TaskModeSession> = None;
                let mut sessions = state.sessions.write().await;
                if let Some(s) = sessions.get_mut(&session_id) {
                    s.status = TaskModeStatus::ReviewingPrd;
                    s.prd = Some(prd.clone());
                    updated_session = Some(s.clone());
                } else {
                    return Ok(CommandResponse::err(
                        "Invalid session ID or no active session",
                    ));
                }
                drop(sessions);
                if let Some(snapshot) = updated_session.as_ref() {
                    persist_task_session_best_effort(&state, snapshot, "generate_task_prd.reviewing_prd").await;
                    sync_kernel_task_snapshot_and_emit(
                        &app_handle,
                        kernel_state.inner(),
                        snapshot,
                        None,
                        "task_mode.generate_task_prd.reviewing_prd",
                    )
                    .await;
                }
            }

            Ok(CommandResponse::ok(prd))
        } => result,
    };
    clear_task_operation_token(&state, &session_id, &operation_id).await;

    if matches!(&result, Ok(resp) if !resp.success && resp.error.as_deref() == Some(TASK_OPERATION_CANCELLED_ERROR))
    {
        let mut updated_session: Option<TaskModeSession> = None;
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if session.status == TaskModeStatus::GeneratingPrd {
                session.status = TaskModeStatus::Initialized;
                updated_session = Some(session.clone());
            }
        }
        drop(sessions);
        if let Some(snapshot) = updated_session.as_ref() {
            persist_task_session_best_effort(&state, snapshot, "generate_task_prd.cancelled").await;
            sync_kernel_task_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                snapshot,
                None,
                "task_mode.generate_task_prd.cancelled",
            )
            .await;
        }
    }

    result
}

/// Explore the project codebase to gather context for PRD generation.
///
/// Runs project exploration based on flow level:
/// - `quick`: Skips exploration entirely (returns empty result)
/// - `standard`: Deterministic-only exploration (IndexStore project summary)
/// - `full`: Deterministic + LLM-assisted exploration via coordinator OrchestratorService
///
/// Exploration failure is non-blocking — returns a warning-level result and the workflow
/// continues to PRD generation.
#[tauri::command]
pub async fn explore_project(
    request: ExploreProjectRequest,
    state: tauri::State<'_, TaskModeState>,
    app_state: tauri::State<'_, AppState>,
    standalone_state: tauri::State<'_, crate::commands::standalone::StandaloneState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<ExplorationResult>, String> {
    let ExploreProjectRequest {
        session_id,
        flow_level,
        task_description,
        provider,
        model,
        api_key,
        base_url,
        locale,
        context_sources,
    } = request;

    // Validate session
    {
        let sessions = state.sessions.read().await;
        match sessions.get(&session_id) {
            Some(s) => {
                if s.status != TaskModeStatus::Initialized {
                    return Ok(CommandResponse::err(format!(
                        "Cannot explore in {:?} status",
                        s.status
                    )));
                }
            }
            _ => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        }
    }

    // Set status to Exploring
    {
        let mut sessions = state.sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.status = TaskModeStatus::Exploring;
            let snapshot = s.clone();
            drop(sessions);
            persist_task_session_best_effort(&state, &snapshot, "explore_project.status_exploring").await;
            sync_kernel_task_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                &snapshot,
                None,
                "task_mode.explore_project.status_exploring",
            )
            .await;
        } else {
            return Ok(CommandResponse::err(
                "Invalid session ID or no active session",
            ));
        }
    }
    let (operation_id, operation_token) = register_task_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(TASK_OPERATION_CANCELLED_ERROR)),
        result = async {

    let start = std::time::Instant::now();

    // Quick flow: skip exploration entirely
    if flow_level == "quick" {
        let result = ExplorationResult {
            tech_stack: exploration::TechStackSummary {
                languages: vec![],
                frameworks: vec![],
                build_tools: vec![],
                test_frameworks: vec![],
                package_manager: None,
            },
            key_files: vec![],
            components: vec![],
            patterns: vec![],
            llm_summary: None,
            summary_quality: SummaryQuality::Empty,
            summary_source: SummarySource::DeterministicOnly,
            summary_notes: None,
            duration_ms: 0,
            used_llm_exploration: false,
        };

        // Reset status
        {
            let mut updated_session: Option<TaskModeSession> = None;
            let mut sessions = state.sessions.write().await;
            if let Some(s) = sessions.get_mut(&session_id) {
                s.status = TaskModeStatus::Initialized;
                s.exploration_result = Some(result.clone());
                updated_session = Some(s.clone());
            } else {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ));
            }
            drop(sessions);
            if let Some(snapshot) = updated_session.as_ref() {
                persist_task_session_best_effort(&state, snapshot, "explore_project.quick_initialized").await;
                sync_kernel_task_snapshot_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    snapshot,
                    None,
                    "task_mode.explore_project.quick_initialized",
                )
                .await;
            }
        }
        return Ok(CommandResponse::ok(result));
    }

    // Resolve project path from standalone state
    let project_path = {
        let wd = standalone_state.working_directory.read().await;
        wd.clone()
    };

    // --- Deterministic exploration ---
    let deterministic_result = {
        // Try to get IndexStore from standalone_state
        let index_manager_guard = standalone_state.index_manager.read().await;
        if let Some(ref index_manager) = *index_manager_guard {
            let store = index_manager.index_store();
            let project_path_str = project_path.to_string_lossy();
            match store.get_project_summary(&project_path_str) {
                Ok(summary) => Some(exploration::deterministic_explore(&summary, &project_path)),
                Err(e) => {
                    eprintln!("[explore_project] IndexStore summary failed: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    let mut result = deterministic_result.unwrap_or_else(|| ExplorationResult {
        tech_stack: exploration::TechStackSummary {
            languages: vec![],
            frameworks: vec![],
            build_tools: vec![],
            test_frameworks: vec![],
            package_manager: None,
        },
        key_files: vec![],
        components: vec![],
        patterns: vec![],
        llm_summary: None,
        summary_quality: SummaryQuality::Empty,
        summary_source: SummarySource::DeterministicOnly,
        summary_notes: None,
        duration_ms: 0,
        used_llm_exploration: false,
    });

    // Emit progress event
    let _ = app_handle.emit(
        "exploration-progress",
        serde_json::json!({
            "sessionId": session_id,
            "phase": "deterministic_complete",
        }),
    );

    // --- LLM exploration (full flow only) ---
    if flow_level == "full" {
        // Resolve provider/model for the coordinator
        let resolved_provider = match provider {
            Some(ref p) if !p.is_empty() => p.clone(),
            _ => {
                match app_state
                    .with_database(|db| db.get_setting("llm_provider"))
                    .await
                {
                    Ok(Some(p)) if !p.is_empty() => p,
                    _ => "anthropic".to_string(),
                }
            }
        };
        let resolved_model = match model {
            Some(ref m) if !m.is_empty() => m.clone(),
            _ => {
                match app_state
                    .with_database(|db| db.get_setting("llm_model"))
                    .await
                {
                    Ok(Some(m)) if !m.is_empty() => m,
                    _ => match resolved_provider.as_str() {
                        "anthropic" => "claude-sonnet-4-20250514".to_string(),
                        "openai" => "gpt-4o".to_string(),
                        "deepseek" => "deepseek-chat".to_string(),
                        "ollama" => "qwen2.5-coder:14b".to_string(),
                        _ => "claude-sonnet-4-20250514".to_string(),
                    },
                }
            }
        };

        match resolve_provider_config(
            &resolved_provider,
            &resolved_model,
            api_key,
            base_url,
            &app_state,
        )
        .await
        {
            Ok(provider_config) => {
                let _ = app_handle.emit(
                    "exploration-progress",
                    serde_json::json!({
                        "sessionId": session_id,
                        "phase": "llm_exploration_started",
                    }),
                );

                // Create coordinator OrchestratorService
                let locale_tag = normalize_locale(locale.as_deref());
                let coordinator_prompt = exploration::build_coordinator_exploration_prompt(
                    &task_description,
                    &result,
                    Some(locale_tag),
                );

                let config = crate::services::orchestrator::OrchestratorConfig {
                    provider: provider_config,
                    system_prompt: Some(coordinator_prompt),
                    max_iterations: 14,
                    max_total_tokens: 200_000,
                    project_root: project_path.clone(),
                    analysis_artifacts_root: dirs::home_dir()
                        .unwrap_or_else(|| std::env::temp_dir())
                        .join(".plan-cascade")
                        .join("analysis-runs"),
                    streaming: true,
                    enable_compaction: true,
                    analysis_profile: Default::default(),
                    analysis_limits: Default::default(),
                    analysis_session_id: None,
                    project_id: None,
                    compaction_config: Default::default(),
                    task_type: Some("explore".to_string()),
                    sub_agent_depth: Some(0), // Allow spawning explore sub-agents
                };

                let (search_provider, search_api_key) = resolve_search_provider_for_tools();
                let mut coordinator = crate::services::orchestrator::OrchestratorService::new(config)
                    .with_search_provider(&search_provider, search_api_key);

                // Wire database pool for CodebaseSearch
                if let Ok(pool) = app_state.with_database(|db| Ok(db.pool().clone())).await {
                    coordinator = coordinator.with_database(pool);
                }

                // Wire embedding resources for semantic CodebaseSearch parity with standalone mode
                if let Some(ref manager) = *standalone_state.index_manager.read().await {
                    let project_path_str = project_path.to_string_lossy().to_string();
                    if let Some(emb_svc) = manager.get_embedding_service(&project_path_str).await {
                        coordinator = coordinator.with_embedding_service(emb_svc);
                    }
                    if let Some(emb_mgr) = manager.get_embedding_manager(&project_path_str).await {
                        coordinator = coordinator.with_embedding_manager(emb_mgr);
                    }
                    if let Some(hnsw) = manager.get_hnsw_index(&project_path_str).await {
                        coordinator = coordinator.with_hnsw_index(hnsw);
                    }
                }

                // Wire SearchKnowledge tool for on-demand knowledge base access
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
                            coordinator = coordinator.with_knowledge_tool(
                                pipeline, pid, col_filter, doc_filter, awareness,
                            );
                        }
                    }
                }

                // Create event channel (drain events in background)
                let (tx, mut rx) = tokio::sync::mpsc::channel::<
                    crate::services::streaming::UnifiedStreamEvent,
                >(256);
                let session_id_clone = session_id.clone();
                let app_handle_clone = app_handle.clone();
                tokio::spawn(async move {
                    while let Some(event) = rx.recv().await {
                        // Forward progress events to frontend
                        let _ = app_handle_clone.emit(
                            "exploration-progress",
                            serde_json::json!({
                                "sessionId": session_id_clone,
                                "phase": "llm_exploring",
                                "event": format!("{:?}", event),
                            }),
                        );
                    }
                });

                // Run the coordinator agentic loop
                let coordinator_result = coordinator
                    .execute(
                        format!(
                            "Explore this project's codebase to gather context for the following task:\n\n{}",
                            task_description
                        ),
                        tx,
                    )
                    .await;

                result.used_llm_exploration = true;

                let parsed_summary = coordinator_result
                    .response
                    .as_deref()
                    .and_then(exploration::parse_coordinator_summary);
                let short_or_incomplete = parsed_summary
                    .as_deref()
                    .map(exploration::is_summary_incomplete)
                    .unwrap_or(true);
                let has_error = coordinator_result.error.is_some() || !coordinator_result.success;

                if let Some(summary) = parsed_summary {
                    if has_error || short_or_incomplete {
                        let synthesized = exploration::synthesize_summary_from_deterministic(
                            &task_description,
                            &result,
                            Some(locale_tag),
                        );
                        result.llm_summary = synthesized.or(Some(summary));
                        result.summary_quality = SummaryQuality::Partial;
                        result.summary_source = SummarySource::FallbackSynthesized;
                        result.summary_notes = Some(
                            "LLM summary was partial or interrupted; supplemented with deterministic synthesis."
                                .to_string(),
                        );
                    } else {
                        result.llm_summary = Some(summary);
                        result.summary_quality = SummaryQuality::Complete;
                        result.summary_source = SummarySource::Llm;
                        result.summary_notes = None;
                    }
                } else if has_error {
                    result.llm_summary = exploration::synthesize_summary_from_deterministic(
                        &task_description,
                        &result,
                        Some(locale_tag),
                    );
                    if result.llm_summary.is_some() {
                        result.summary_quality = SummaryQuality::Partial;
                        result.summary_source = SummarySource::FallbackSynthesized;
                        result.summary_notes = Some(
                            "LLM exploration failed; used deterministic synthesized summary."
                                .to_string(),
                        );
                    } else {
                        result.summary_quality = SummaryQuality::Empty;
                        result.summary_source = SummarySource::DeterministicOnly;
                        result.summary_notes = Some(
                            "LLM exploration failed and no synthesized summary could be generated."
                                .to_string(),
                        );
                    }
                } else {
                    result.llm_summary = exploration::synthesize_summary_from_deterministic(
                        &task_description,
                        &result,
                        Some(locale_tag),
                    );
                    if result.llm_summary.is_some() {
                        result.summary_quality = SummaryQuality::Partial;
                        result.summary_source = SummarySource::FallbackSynthesized;
                        result.summary_notes = Some(
                            "LLM exploration returned no summary text; synthesized deterministic summary."
                                .to_string(),
                        );
                    }
                }

                if !coordinator_result.success {
                    eprintln!(
                        "[explore_project] LLM exploration failed: {:?}",
                        coordinator_result.error
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[explore_project] Provider resolution failed for LLM exploration: {}",
                    e
                );
                // Non-blocking: continue with deterministic-only result
            }
        }
    }

    // Update duration
    result.duration_ms = start.elapsed().as_millis() as u64;

    // Store result and reset status
    {
        let mut updated_session: Option<TaskModeSession> = None;
        let mut sessions = state.sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.status = TaskModeStatus::Initialized;
            s.exploration_result = Some(result.clone());
            updated_session = Some(s.clone());
        } else {
            return Ok(CommandResponse::err(
                "Invalid session ID or no active session",
            ));
        }
        drop(sessions);
        if let Some(snapshot) = updated_session.as_ref() {
            persist_task_session_best_effort(&state, snapshot, "explore_project.complete_initialized").await;
            sync_kernel_task_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                snapshot,
                None,
                "task_mode.explore_project.complete_initialized",
            )
            .await;
        }
    }

    let _ = app_handle.emit(
        "exploration-progress",
        serde_json::json!({
            "sessionId": session_id,
            "phase": "complete",
            "durationMs": result.duration_ms,
        }),
    );

    Ok(CommandResponse::ok(result))
        } => result,
    };
    clear_task_operation_token(&state, &session_id, &operation_id).await;

    if matches!(&result, Ok(resp) if !resp.success && resp.error.as_deref() == Some(TASK_OPERATION_CANCELLED_ERROR))
    {
        let mut updated_session: Option<TaskModeSession> = None;
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            if session.status == TaskModeStatus::Exploring {
                session.status = TaskModeStatus::Initialized;
                updated_session = Some(session.clone());
            }
        }
        drop(sessions);
        if let Some(snapshot) = updated_session.as_ref() {
            persist_task_session_best_effort(&state, snapshot, "explore_project.cancelled").await;
            sync_kernel_task_snapshot_and_emit(
                &app_handle,
                kernel_state.inner(),
                snapshot,
                None,
                "task_mode.explore_project.cancelled",
            )
            .await;
        }
    }

    result
}
