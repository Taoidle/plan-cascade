use super::*;
use crate::services::workflow_kernel::observability::{self, WorkflowFailureRecordInput};

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
            persist_task_session_best_effort(
                &state,
                &snapshot,
                "generate_task_prd.status_generating",
            )
            .await;
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

            // Root session context is resolved from kernel when the frontend omits history.
            let root_handoff = if conversation_history
                .as_ref()
                .is_some_and(|history| !history.is_empty())
            {
                None
            } else {
                super::handoff_context_for_task_session(kernel_state.inner(), &session_id).await
            };
            let history = conversation_history
                .filter(|history| !history.is_empty())
                .unwrap_or_else(|| {
                    root_handoff
                        .as_ref()
                        .map(super::conversation_history_from_task_handoff)
                        .unwrap_or_default()
                });
            let handoff_context = root_handoff
                .as_ref()
                .and_then(super::render_task_handoff_context);
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
            let combined_context = match (combined_context, handoff_context) {
                (Some(base), Some(handoff)) => Some(format!("{}\n\n{}", base, handoff)),
                (None, Some(handoff)) => Some(handoff),
                (base, None) => base,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackPrdStory {
    id: String,
    title: String,
    description: String,
    priority: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackPrdPayload {
    title: Option<String>,
    description: Option<String>,
    stories: Vec<FeedbackPrdStory>,
    #[serde(default)]
    warnings: Vec<String>,
}

fn normalize_priority(priority: &str) -> String {
    match priority.trim().to_ascii_lowercase().as_str() {
        "high" => "high".to_string(),
        "low" => "low".to_string(),
        _ => "medium".to_string(),
    }
}

fn detect_story_cycle(stories: &[TaskStory]) -> bool {
    use std::collections::{HashMap, HashSet};

    fn dfs(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> bool {
        if visited.contains(node) {
            return false;
        }
        if !visiting.insert(node.to_string()) {
            return true;
        }
        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if dfs(dep, graph, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(node);
        visited.insert(node.to_string());
        false
    }

    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for story in stories {
        graph.insert(story.id.clone(), story.dependencies.clone());
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for story in stories {
        if dfs(&story.id, &graph, &mut visiting, &mut visited) {
            return true;
        }
    }
    false
}

fn infer_max_parallel_from_prd(prd: &TaskPrd) -> usize {
    let inferred = prd
        .batches
        .iter()
        .map(|batch| batch.story_ids.len())
        .max()
        .unwrap_or(4);
    inferred.clamp(1, 8)
}

fn parse_feedback_prd_payload(
    raw: serde_json::Value,
    fallback_title: &str,
    fallback_description: &str,
    max_parallel: usize,
) -> Result<(TaskPrd, Vec<String>), String> {
    let mut normalized = raw;
    if let Some(stories) = normalized
        .get_mut("stories")
        .and_then(|value| value.as_array_mut())
    {
        for story in stories {
            if let Some(object) = story.as_object_mut() {
                if !object.contains_key("acceptanceCriteria") {
                    if let Some(value) = object.remove("acceptance_criteria") {
                        object.insert("acceptanceCriteria".to_string(), value);
                    }
                }
            }
        }
    }

    let payload: FeedbackPrdPayload = serde_json::from_value(normalized)
        .map_err(|error| format!("Failed to parse PRD feedback output: {error}"))?;
    if payload.stories.is_empty() {
        return Err("PRD feedback result contains no stories".to_string());
    }

    let mut seen_story_ids = std::collections::HashSet::new();
    let stories: Vec<TaskStory> = payload
        .stories
        .into_iter()
        .map(|story| {
            let story_id = story.id.trim().to_string();
            if story_id.is_empty() {
                return Err("Story id cannot be empty".to_string());
            }
            if !seen_story_ids.insert(story_id.clone()) {
                return Err(format!("Duplicate story id: {}", story_id));
            }

            if story
                .acceptance_criteria
                .iter()
                .all(|criterion| criterion.trim().is_empty())
            {
                return Err(format!(
                    "Story '{}' must include at least one acceptance criterion",
                    story_id
                ));
            }

            let dependencies: Vec<String> = story
                .dependencies
                .into_iter()
                .map(|dependency| dependency.trim().to_string())
                .filter(|dependency| !dependency.is_empty())
                .collect();
            if dependencies
                .iter()
                .any(|dependency| dependency == &story_id)
            {
                return Err(format!("Story '{}' cannot depend on itself", story_id));
            }

            Ok(TaskStory {
                id: story_id,
                title: story.title.trim().to_string(),
                description: story.description.trim().to_string(),
                priority: normalize_priority(&story.priority),
                dependencies,
                acceptance_criteria: story
                    .acceptance_criteria
                    .into_iter()
                    .map(|criterion| criterion.trim().to_string())
                    .filter(|criterion| !criterion.is_empty())
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let story_ids: std::collections::HashSet<String> =
        stories.iter().map(|story| story.id.clone()).collect();
    for story in &stories {
        for dependency in &story.dependencies {
            if !story_ids.contains(dependency) {
                return Err(format!(
                    "Story '{}' references missing dependency '{}'",
                    story.id, dependency
                ));
            }
        }
    }
    if detect_story_cycle(&stories) {
        return Err("PRD feedback output contains circular dependencies".to_string());
    }

    let executable: Vec<crate::services::task_mode::batch_executor::ExecutableStory> = stories
        .iter()
        .map(
            |story| crate::services::task_mode::batch_executor::ExecutableStory {
                id: story.id.clone(),
                title: story.title.clone(),
                description: story.description.clone(),
                dependencies: story.dependencies.clone(),
                acceptance_criteria: story.acceptance_criteria.clone(),
                agent: None,
            },
        )
        .collect();
    let batches = crate::services::task_mode::calculate_batches(&executable, max_parallel)
        .map_err(|error| format!("Failed to compute execution batches: {error}"))?;

    Ok((
        TaskPrd {
            title: payload
                .title
                .map(|title| title.trim().to_string())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| fallback_title.to_string()),
            description: payload
                .description
                .map(|description| description.trim().to_string())
                .filter(|description| !description.is_empty())
                .unwrap_or_else(|| fallback_description.to_string()),
            stories,
            batches,
        },
        payload.warnings,
    ))
}

fn build_prd_feedback_summary(
    previous: &TaskPrd,
    updated: &TaskPrd,
    warnings: Vec<String>,
) -> PrdFeedbackApplySummary {
    let previous_map: std::collections::HashMap<&str, &TaskStory> = previous
        .stories
        .iter()
        .map(|story| (story.id.as_str(), story))
        .collect();
    let updated_map: std::collections::HashMap<&str, &TaskStory> = updated
        .stories
        .iter()
        .map(|story| (story.id.as_str(), story))
        .collect();

    let mut added_story_ids: Vec<String> = updated_map
        .keys()
        .filter(|story_id| !previous_map.contains_key(*story_id))
        .map(|story_id| (*story_id).to_string())
        .collect();
    added_story_ids.sort();

    let mut removed_story_ids: Vec<String> = previous_map
        .keys()
        .filter(|story_id| !updated_map.contains_key(*story_id))
        .map(|story_id| (*story_id).to_string())
        .collect();
    removed_story_ids.sort();

    let mut updated_story_ids: Vec<String> = updated_map
        .iter()
        .filter_map(|(story_id, next_story)| {
            let previous_story = previous_map.get(story_id)?;
            if previous_story.title != next_story.title
                || previous_story.description != next_story.description
                || previous_story.priority != next_story.priority
                || previous_story.dependencies != next_story.dependencies
                || previous_story.acceptance_criteria != next_story.acceptance_criteria
            {
                Some((*story_id).to_string())
            } else {
                None
            }
        })
        .collect();
    updated_story_ids.sort();

    let previous_batch_by_story: std::collections::HashMap<&str, usize> = previous
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .story_ids
                .iter()
                .map(move |story_id| (story_id.as_str(), batch.index))
        })
        .collect();
    let updated_batch_by_story: std::collections::HashMap<&str, usize> = updated
        .batches
        .iter()
        .flat_map(|batch| {
            batch
                .story_ids
                .iter()
                .map(move |story_id| (story_id.as_str(), batch.index))
        })
        .collect();

    let mut batch_changes = Vec::new();
    if previous.batches.len() != updated.batches.len() {
        batch_changes.push(format!(
            "batch_count:{}->{}",
            previous.batches.len(),
            updated.batches.len()
        ));
    }
    for (story_id, next_batch) in &updated_batch_by_story {
        if let Some(previous_batch) = previous_batch_by_story.get(story_id) {
            if previous_batch != next_batch {
                batch_changes.push(format!(
                    "{}:batch_{}->batch_{}",
                    story_id,
                    previous_batch + 1,
                    next_batch + 1
                ));
            }
        }
    }

    PrdFeedbackApplySummary {
        added_story_ids,
        removed_story_ids,
        updated_story_ids,
        batch_changes,
        warnings,
    }
}

fn map_prd_feedback_error_code(message: &str) -> String {
    if message.contains("Feedback cannot be empty") {
        return "empty_feedback".to_string();
    }
    if message.contains("Invalid session ID") {
        return "missing_session".to_string();
    }
    if message.contains("Cannot apply PRD feedback in") {
        return "invalid_phase".to_string();
    }
    if message.contains(TASK_OPERATION_CANCELLED_ERROR) {
        return "operation_cancelled".to_string();
    }
    if message.contains("No PRD found in current session") {
        return "missing_prd".to_string();
    }
    "prd_feedback_apply_failed".to_string()
}

async fn record_prd_feedback_apply_observability(
    app_state: &AppState,
    record: WorkflowFailureRecordInput,
    success: bool,
) {
    let _ = app_state
        .with_database(|db| observability::record_prd_feedback_apply(db, &record, success))
        .await;
    if success {
        tracing::info!(
            event = "prd_feedback_apply_success",
            kernelSessionId = %record.kernel_session_id.clone().unwrap_or_default(),
            modeSessionId = %record.mode_session_id.clone().unwrap_or_default(),
            mode = %record.mode.clone().unwrap_or_default(),
            phase_before = %record.phase_before.clone().unwrap_or_default(),
            phase_after = %record.phase_after.clone().unwrap_or_default(),
            action = %record.action,
            errorCode = ""
        );
    } else {
        tracing::warn!(
            event = "prd_feedback_apply_failure",
            kernelSessionId = %record.kernel_session_id.clone().unwrap_or_default(),
            modeSessionId = %record.mode_session_id.clone().unwrap_or_default(),
            mode = %record.mode.clone().unwrap_or_default(),
            phase_before = %record.phase_before.clone().unwrap_or_default(),
            phase_after = %record.phase_after.clone().unwrap_or_default(),
            action = %record.action,
            errorCode = %record.error_code.clone().unwrap_or_default()
        );
    }
}

#[tauri::command]
pub async fn apply_task_prd_feedback(
    request: ApplyTaskPrdFeedbackRequest,
    state: tauri::State<'_, TaskModeState>,
    app_state: tauri::State<'_, AppState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<PrdFeedbackApplyResult>, String> {
    let ApplyTaskPrdFeedbackRequest {
        session_id,
        feedback,
        provider,
        model,
        api_key,
        base_url,
        conversation_history,
        max_context_tokens: _max_context_tokens,
        locale,
        context_sources,
        project_path,
    } = request;

    let base_observability_record = WorkflowFailureRecordInput {
        action: "apply_task_prd_feedback".to_string(),
        card: Some("prd_card".to_string()),
        mode: Some("task".to_string()),
        kernel_session_id: None,
        mode_session_id: Some(session_id.clone()),
        phase_before: Some("reviewing_prd".to_string()),
        phase_after: Some("reviewing_prd".to_string()),
        error_code: None,
        message: None,
        timestamp: None,
    };

    let normalized_feedback = feedback.trim();
    if normalized_feedback.is_empty() {
        record_prd_feedback_apply_observability(
            app_state.inner(),
            WorkflowFailureRecordInput {
                error_code: Some("empty_feedback".to_string()),
                message: Some("Feedback cannot be empty".to_string()),
                ..base_observability_record.clone()
            },
            false,
        )
        .await;
        return Ok(CommandResponse::err("Feedback cannot be empty"));
    }

    let current_prd = {
        let sessions = state.sessions.read().await;
        let session = match sessions.get(&session_id) {
            Some(session) => session,
            None => {
                record_prd_feedback_apply_observability(
                    app_state.inner(),
                    WorkflowFailureRecordInput {
                        error_code: Some("missing_session".to_string()),
                        message: Some("Invalid session ID or no active session".to_string()),
                        ..base_observability_record.clone()
                    },
                    false,
                )
                .await;
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ));
            }
        };
        if session.status != TaskModeStatus::ReviewingPrd {
            let message = format!("Cannot apply PRD feedback in {:?} status", session.status);
            record_prd_feedback_apply_observability(
                app_state.inner(),
                WorkflowFailureRecordInput {
                    error_code: Some("invalid_phase".to_string()),
                    message: Some(message.clone()),
                    ..base_observability_record.clone()
                },
                false,
            )
            .await;
            return Ok(CommandResponse::err(format!(
                "Cannot apply PRD feedback in {:?} status",
                session.status
            )));
        }
        match session.prd.clone() {
            Some(prd) => prd,
            None => {
                record_prd_feedback_apply_observability(
                    app_state.inner(),
                    WorkflowFailureRecordInput {
                        error_code: Some("missing_prd".to_string()),
                        message: Some("No PRD found in current session".to_string()),
                        ..base_observability_record.clone()
                    },
                    false,
                )
                .await;
                return Ok(CommandResponse::err("No PRD found in current session"));
            }
        }
    };

    sync_kernel_task_phase_by_linked_session_and_emit(
        &app_handle,
        kernel_state.inner(),
        &session_id,
        "reviewing_prd",
        "task_mode.apply_task_prd_feedback.started",
    )
    .await;

    let (operation_id, operation_token) = register_task_operation_token(&state, &session_id).await;
    let result: CommandResponse<PrdFeedbackApplyResult> = tokio::select! {
        _ = operation_token.cancelled() => CommandResponse::err(TASK_OPERATION_CANCELLED_ERROR),
        result = async {
            let resolved_provider = match provider {
                Some(ref provider) if !provider.is_empty() => provider.clone(),
                _ => match app_state.with_database(|db| db.get_setting("llm_provider")).await {
                    Ok(Some(provider)) if !provider.is_empty() => provider,
                    _ => "anthropic".to_string(),
                },
            };
            let resolved_model = match model {
                Some(ref model) if !model.is_empty() => model.clone(),
                _ => match app_state.with_database(|db| db.get_setting("llm_model")).await {
                    Ok(Some(model)) if !model.is_empty() => model,
                    _ => match resolved_provider.as_str() {
                        "anthropic" => "claude-sonnet-4-20250514".to_string(),
                        "openai" => "gpt-4o".to_string(),
                        "deepseek" => "deepseek-chat".to_string(),
                        "ollama" => "qwen2.5-coder:14b".to_string(),
                        _ => "claude-sonnet-4-20250514".to_string(),
                    },
                },
            };

            let llm_provider = match resolve_llm_provider(
                &resolved_provider,
                &resolved_model,
                api_key,
                base_url,
                &app_state,
            )
            .await
            {
                Ok(provider) => provider,
                Err(error) => return CommandResponse::err(error),
            };

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
                normalized_feedback,
                crate::services::skills::model::InjectionPhase::Planning,
                context_sources.as_ref(),
                "task",
                Some(session_id.as_str()),
                true,
            )
            .await;
            let enriched_context =
                crate::services::task_mode::context_provider::merge_enriched_context(
                    None,
                    &enriched.knowledge_block,
                    &enriched.memory_block,
                );

            use crate::services::llm::types::Message;
            use crate::services::persona::{PersonaRegistry, PersonaRole};

            let locale_tag = normalize_locale(locale.as_deref());
            let language_instruction = locale_instruction(locale_tag);
            let mut phase_instructions = format!(
                r#"You are updating a task PRD during review based on user feedback.

Rules:
1. Return the FULL updated PRD, not partial patches.
2. Keep story IDs stable whenever possible.
3. Ensure dependencies only reference existing story IDs.
4. Do not create self dependencies or circular dependencies.
5. Every story must include at least one acceptance criterion.
6. Use priority values: high|medium|low.

Current PRD JSON:
{}

User feedback:
{}

Output language:
{}
"#,
                serde_json::to_string_pretty(&current_prd).unwrap_or_else(|_| "{}".to_string()),
                normalized_feedback,
                language_instruction,
            );
            if !enriched.skills_block.trim().is_empty() {
                phase_instructions.push_str("\n\n");
                phase_instructions.push_str(&enriched.skills_block);
            }

            let root_handoff = if conversation_history
                .as_ref()
                .is_some_and(|history| !history.is_empty())
            {
                None
            } else {
                super::handoff_context_for_task_session(kernel_state.inner(), &session_id).await
            };
            let history = conversation_history
                .filter(|history| !history.is_empty())
                .unwrap_or_else(|| {
                    root_handoff
                        .as_ref()
                        .map(super::conversation_history_from_task_handoff)
                        .unwrap_or_default()
                });
            let handoff_context = root_handoff
                .as_ref()
                .and_then(super::render_task_handoff_context);
            let history_snippet = history
                .into_iter()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|turn| {
                    format!(
                        "user: {}\nassistant: {}",
                        turn.user.trim(),
                        turn.assistant.trim()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let user_prompt = match (
                history_snippet.is_empty(),
                handoff_context.as_deref().filter(|value| !value.trim().is_empty()),
            ) {
                (true, None) => format!(
                    "Apply this feedback to the PRD and return the full updated PRD JSON only:\n{}",
                    normalized_feedback
                ),
                (true, Some(handoff)) => format!(
                    "Root session handoff context:\n{}\n\nApply this feedback to the PRD and return the full updated PRD JSON only:\n{}",
                    handoff,
                    normalized_feedback
                ),
                (false, None) => format!(
                    "Recent conversation context:\n{}\n\nApply this feedback to the PRD and return the full updated PRD JSON only:\n{}",
                    history_snippet,
                    normalized_feedback
                ),
                (false, Some(handoff)) => format!(
                    "Recent conversation context:\n{}\n\nRoot session handoff context:\n{}\n\nApply this feedback to the PRD and return the full updated PRD JSON only:\n{}",
                    history_snippet,
                    handoff,
                    normalized_feedback
                ),
            };

            let target_schema = r#"{
  "title": "string",
  "description": "string",
  "stories": [{
    "id": "string",
    "title": "string",
    "description": "string",
    "priority": "high|medium|low",
    "dependencies": ["string"],
    "acceptanceCriteria": ["string"]
  }],
  "warnings": ["string"]
}"#;

            let persona = PersonaRegistry::get(PersonaRole::ProductManager);
            let formatter_output = match crate::services::persona::run_expert_formatter::<serde_json::Value>(
                llm_provider.clone(),
                None,
                &persona,
                &phase_instructions,
                enriched_context.as_deref(),
                Some(locale_tag),
                vec![Message::user(user_prompt)],
                target_schema,
                None,
            )
            .await
            {
                Ok(result) => result.structured_output,
                Err(error) => {
                    return CommandResponse::err(format!(
                        "PRD feedback apply failed: {}",
                        error
                    ))
                }
            };

            let max_parallel = infer_max_parallel_from_prd(&current_prd);
            let (updated_prd, warnings) = match parse_feedback_prd_payload(
                formatter_output,
                &current_prd.title,
                &current_prd.description,
                max_parallel,
            ) {
                Ok(parsed) => parsed,
                Err(error) => return CommandResponse::err(error),
            };

            let summary = build_prd_feedback_summary(&current_prd, &updated_prd, warnings);

            {
                let mut sessions = state.sessions.write().await;
                let session = match sessions.get_mut(&session_id) {
                    Some(session) => session,
                    None => return CommandResponse::err("Invalid session ID or no active session"),
                };
                if session.status != TaskModeStatus::ReviewingPrd {
                    return CommandResponse::err(format!(
                        "Cannot apply PRD feedback in {:?} status",
                        session.status
                    ));
                }
                session.prd = Some(updated_prd.clone());
                session.status = TaskModeStatus::ReviewingPrd;
                let snapshot = session.clone();
                drop(sessions);
                persist_task_session_best_effort(
                    &state,
                    &snapshot,
                    "apply_task_prd_feedback.reviewing_prd",
                )
                .await;
                sync_kernel_task_snapshot_and_emit(
                    &app_handle,
                    kernel_state.inner(),
                    &snapshot,
                    Some("reviewing_prd"),
                    "task_mode.apply_task_prd_feedback.reviewing_prd",
                )
                .await;
            }

            CommandResponse::ok(PrdFeedbackApplyResult {
                prd: updated_prd,
                summary,
            })
        } => result,
    };

    clear_task_operation_token(&state, &session_id, &operation_id).await;
    if let Some(session_snapshot) = state.get_session_snapshot(&session_id).await {
        sync_kernel_task_snapshot_and_emit(
            &app_handle,
            kernel_state.inner(),
            &session_snapshot,
            Some("reviewing_prd"),
            "task_mode.apply_task_prd_feedback.finished",
        )
        .await;
    }

    let observability_record = if result.success {
        WorkflowFailureRecordInput {
            error_code: None,
            message: None,
            ..base_observability_record.clone()
        }
    } else {
        let message = result
            .error
            .clone()
            .unwrap_or_else(|| "PRD feedback apply failed".to_string());
        WorkflowFailureRecordInput {
            error_code: Some(map_prd_feedback_error_code(&message)),
            message: Some(message),
            ..base_observability_record.clone()
        }
    };
    record_prd_feedback_apply_observability(
        app_state.inner(),
        observability_record,
        result.success,
    )
    .await;

    Ok(result)
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
            persist_task_session_best_effort(&state, &snapshot, "explore_project.status_exploring")
                .await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_prd() -> TaskPrd {
        TaskPrd {
            title: "PRD".to_string(),
            description: "desc".to_string(),
            stories: vec![
                TaskStory {
                    id: "story-001".to_string(),
                    title: "Base".to_string(),
                    description: "base".to_string(),
                    priority: "high".to_string(),
                    dependencies: vec![],
                    acceptance_criteria: vec!["done".to_string()],
                },
                TaskStory {
                    id: "story-002".to_string(),
                    title: "Feature".to_string(),
                    description: "feature".to_string(),
                    priority: "medium".to_string(),
                    dependencies: vec!["story-001".to_string()],
                    acceptance_criteria: vec!["done".to_string()],
                },
            ],
            batches: vec![
                crate::services::task_mode::batch_executor::ExecutionBatch {
                    index: 0,
                    story_ids: vec!["story-001".to_string()],
                },
                crate::services::task_mode::batch_executor::ExecutionBatch {
                    index: 1,
                    story_ids: vec!["story-002".to_string()],
                },
            ],
        }
    }

    #[test]
    fn parse_feedback_prd_payload_accepts_valid_output() {
        let raw = json!({
            "title": "Updated PRD",
            "description": "Updated desc",
            "stories": [
                {
                    "id": "story-001",
                    "title": "Base",
                    "description": "base",
                    "priority": "high",
                    "dependencies": [],
                    "acceptanceCriteria": ["done"]
                },
                {
                    "id": "story-002",
                    "title": "Feature",
                    "description": "feature",
                    "priority": "medium",
                    "dependencies": ["story-001"],
                    "acceptanceCriteria": ["done"]
                }
            ]
        });

        let (parsed, warnings) =
            parse_feedback_prd_payload(raw, "fallback", "fallback", 4).expect("parse payload");
        assert_eq!(parsed.title, "Updated PRD");
        assert_eq!(parsed.stories.len(), 2);
        assert_eq!(parsed.batches.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_feedback_prd_payload_rejects_missing_dependencies() {
        let raw = json!({
            "stories": [
                {
                    "id": "story-001",
                    "title": "Broken",
                    "description": "broken",
                    "priority": "medium",
                    "dependencies": ["story-999"],
                    "acceptanceCriteria": ["done"]
                }
            ]
        });

        let error = parse_feedback_prd_payload(raw, "fallback", "fallback", 4).unwrap_err();
        assert!(error.contains("missing dependency"));
    }

    #[test]
    fn build_prd_feedback_summary_reports_structural_changes() {
        let previous = sample_prd();
        let mut next = sample_prd();
        next.stories[1].title = "Feature v2".to_string();
        next.stories.push(TaskStory {
            id: "story-003".to_string(),
            title: "New story".to_string(),
            description: "new".to_string(),
            priority: "low".to_string(),
            dependencies: vec!["story-002".to_string()],
            acceptance_criteria: vec!["done".to_string()],
        });
        next.batches
            .push(crate::services::task_mode::batch_executor::ExecutionBatch {
                index: 2,
                story_ids: vec!["story-003".to_string()],
            });

        let summary = build_prd_feedback_summary(&previous, &next, vec!["note".to_string()]);
        assert_eq!(summary.added_story_ids, vec!["story-003".to_string()]);
        assert_eq!(summary.updated_story_ids, vec!["story-002".to_string()]);
        assert_eq!(summary.warnings, vec!["note".to_string()]);
        assert!(!summary.batch_changes.is_empty());
    }
}
