use super::*;

/// Run requirement analysis using the ProductManager persona.
///
/// Uses the Expert+Formatter pipeline to produce a structured analysis of
/// requirements from the task description, interview results, and exploration context.
/// The expert step produces natural language analysis; the formatter step
/// extracts structured key requirements, gaps, and scope.
#[tauri::command]
pub async fn run_requirement_analysis(
    request: RunRequirementAnalysisRequest,
    app_state: tauri::State<'_, AppState>,
    state: tauri::State<'_, TaskModeState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<RequirementAnalysisResult>, String> {
    let RunRequirementAnalysisRequest {
        session_id,
        task_description,
        interview_result,
        exploration_context,
        provider,
        model,
        api_key,
        base_url,
        locale,
        context_sources,
        project_path,
    } = request;

    use crate::services::persona::{PersonaRegistry, PersonaRole};

    // Validate session
    {
        let sessions = state.sessions.read().await;
        match sessions.get(&session_id) {
            Some(_) => {}
            _ => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        }
    }

    sync_kernel_task_phase_by_linked_session_and_emit(
        &app_handle,
        kernel_state.inner(),
        &session_id,
        "requirement_analysis",
        "task_mode.run_requirement_analysis.started",
    )
    .await;

    let (operation_id, operation_token) = register_task_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(TASK_OPERATION_CANCELLED_ERROR)),
        result = async {

    // Resolve provider/model
    let resolved_provider = match provider {
        Some(ref p) if !p.is_empty() => p.clone(),
        _ => match app_state
            .with_database(|db| db.get_setting("llm_provider"))
            .await
        {
            Ok(Some(p)) if !p.is_empty() => p,
            _ => "anthropic".to_string(),
        },
    };
    let resolved_model = match model {
        Some(ref m) if !m.is_empty() => m.clone(),
        _ => match app_state
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
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Query domain knowledge for requirement analysis (only if user enabled sources)
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
        &task_description,
        crate::services::skills::model::InjectionPhase::Planning,
        context_sources.as_ref(),
        "task",
        Some(session_id.as_str()),
        true,
    )
    .await;
    let knowledge_block = &enriched.knowledge_block;
    let memory_block = &enriched.memory_block;
    let skills_block = &enriched.skills_block;
    let skill_expertise = enriched.skill_expertise;

    let persona = PersonaRegistry::get(PersonaRole::ProductManager);
    let persona = if !skill_expertise.is_empty() {
        let mut p = persona.clone();
        p.expertise.extend(skill_expertise);
        p
    } else {
        persona
    };

    let locale_tag = normalize_locale(locale.as_deref());
    let language_instruction = locale_instruction(locale_tag);

    // Build phase instructions for the PM expert
    let mut phase_instructions = format!(
        r#"Analyze the following task and produce a thorough requirements analysis.

## Task Description
{task_description}
{}{}

## Output Language
{language_instruction}

## Your Analysis Should Cover
1. **Key Requirements**: List the essential functional and non-functional requirements
2. **Identified Gaps**: What's missing or ambiguous in the requirements?
3. **Suggested Scope**: Define a clear, achievable scope for this task
4. **Risk Assessment**: Any requirements that could be problematic
5. **Priority Ranking**: Which requirements are most critical

Be specific and actionable. Reference concrete technical details when available."#,
        interview_result
            .as_ref()
            .map(|i| format!("\n\n## Interview Results\n{}", i))
            .unwrap_or_default(),
        exploration_context
            .as_ref()
            .map(|e| format!("\n\n## Project Context\n{}", e))
            .unwrap_or_default(),
        language_instruction = language_instruction,
    );
    if !skills_block.is_empty() {
        phase_instructions.push_str("\n\n");
        phase_instructions.push_str(&skills_block);
    }

    let enriched_context = crate::services::task_mode::context_provider::merge_enriched_context(
        exploration_context.as_deref(),
        &knowledge_block,
        &memory_block,
    );

    let target_schema = r#"{
  "analysis": "string - Natural language analysis in markdown format",
  "key_requirements": ["string - Each key requirement as a clear statement"],
  "identified_gaps": ["string - Each gap or ambiguity found"],
  "suggested_scope": "string - Clear scope definition"
}"#;

    use crate::services::llm::types::Message;

    let user_messages = vec![Message::user(format!(
        "Analyze the requirements for: {}",
        task_description
    ))];

    match crate::services::persona::run_expert_formatter::<serde_json::Value>(
        llm_provider.clone(),
        None,
        &persona,
        &phase_instructions,
        enriched_context.as_deref(),
        Some(locale_tag),
        user_messages,
        target_schema,
        None,
    )
    .await
    {
        Ok(result) => {
            let structured = &result.structured_output;
            Ok(CommandResponse::ok(RequirementAnalysisResult {
                analysis: result.expert_analysis,
                key_requirements: structured
                    .get("key_requirements")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                identified_gaps: structured
                    .get("identified_gaps")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                suggested_scope: structured
                    .get("suggested_scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Not specified")
                    .to_string(),
                persona_role: "ProductManager".to_string(),
            }))
        }
        Err(e) => Ok(CommandResponse::err(format!(
            "Requirement analysis failed: {}",
            e
        ))),
    }
        } => result,
    };
    clear_task_operation_token(&state, &session_id, &operation_id).await;
    if let Some(session_snapshot) = state.get_session_snapshot(&session_id).await {
        sync_kernel_task_snapshot_and_emit(
            &app_handle,
            kernel_state.inner(),
            &session_snapshot,
            None,
            "task_mode.run_requirement_analysis.finished",
        )
        .await;
    }
    result
}

/// Run architecture review using the SoftwareArchitect persona.
///
/// Reviews the approved PRD from an architectural perspective.
/// The architect identifies concerns, suggests improvements, and may
/// propose PRD modifications. Returns an interactive result that
/// the frontend can display for user approval.
#[tauri::command]
pub async fn run_architecture_review(
    request: RunArchitectureReviewRequest,
    app_state: tauri::State<'_, AppState>,
    state: tauri::State<'_, TaskModeState>,
    knowledge_state: tauri::State<'_, crate::commands::knowledge::KnowledgeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResponse<ArchitectureReviewResult>, String> {
    let RunArchitectureReviewRequest {
        session_id,
        prd_json,
        exploration_context,
        provider,
        model,
        api_key,
        base_url,
        locale,
        context_sources,
        project_path,
    } = request;

    use crate::services::persona::{PersonaRegistry, PersonaRole};

    // Validate session
    {
        let sessions = state.sessions.read().await;
        match sessions.get(&session_id) {
            Some(_) => {}
            _ => {
                return Ok(CommandResponse::err(
                    "Invalid session ID or no active session",
                ))
            }
        }
    }

    sync_kernel_task_phase_by_linked_session_and_emit(
        &app_handle,
        kernel_state.inner(),
        &session_id,
        "architecture_review",
        "task_mode.run_architecture_review.started",
    )
    .await;

    let (operation_id, operation_token) = register_task_operation_token(&state, &session_id).await;
    let result = tokio::select! {
        _ = operation_token.cancelled() => Ok(CommandResponse::err(TASK_OPERATION_CANCELLED_ERROR)),
        result = async {

    // Resolve provider/model
    let resolved_provider = match provider {
        Some(ref p) if !p.is_empty() => p.clone(),
        _ => match app_state
            .with_database(|db| db.get_setting("llm_provider"))
            .await
        {
            Ok(Some(p)) if !p.is_empty() => p,
            _ => "anthropic".to_string(),
        },
    };
    let resolved_model = match model {
        Some(ref m) if !m.is_empty() => m.clone(),
        _ => match app_state
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
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    // Query domain knowledge for architecture review (only if user enabled sources)
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
        &prd_json,
        crate::services::skills::model::InjectionPhase::Planning,
        context_sources.as_ref(),
        "task",
        Some(session_id.as_str()),
        true,
    )
    .await;
    let knowledge_block = &enriched.knowledge_block;
    let memory_block = &enriched.memory_block;
    let skills_block = &enriched.skills_block;
    let skill_expertise = enriched.skill_expertise;

    let persona = PersonaRegistry::get(PersonaRole::SoftwareArchitect);
    let persona = if !skill_expertise.is_empty() {
        let mut p = persona.clone();
        p.expertise.extend(skill_expertise);
        p
    } else {
        persona
    };

    let locale_tag = normalize_locale(locale.as_deref());
    let language_instruction = locale_instruction(locale_tag);

    // Build phase instructions for the architect expert
    let mut phase_instructions = format!(
        r#"Review the following PRD (Product Requirements Document) from an architectural perspective.

## PRD
{prd_json}
{}

## Your Review Should Cover
1. **Architectural Concerns**: Identify potential issues with the proposed stories
   - Severity levels: "high" (blocking), "medium" (should address), "low" (nice to have)
2. **Improvement Suggestions**: General architectural improvements
3. **PRD Modifications**: Specific changes to stories if needed
   - Types: "update_story", "add_story", "remove_story", "split_story", "merge_story"
   - Provide structured payload details so frontend can apply patches directly.
4. **Overall Assessment**: Whether the PRD is architecturally sound

Consider: scalability, maintainability, separation of concerns, error handling,
testing strategy, dependency management, and integration patterns.

## Output Language
{language_instruction}"#,
        exploration_context
            .as_ref()
            .map(|e| format!("\n\n## Project Context\n{}", e))
            .unwrap_or_default(),
        language_instruction = language_instruction,
    );
    if !skills_block.is_empty() {
        phase_instructions.push_str("\n\n");
        phase_instructions.push_str(&skills_block);
    }

    let enriched_context = crate::services::task_mode::context_provider::merge_enriched_context(
        exploration_context.as_deref(),
        &knowledge_block,
        &memory_block,
    );

    let target_schema = r#"{
  "analysis": "string - Natural language architecture analysis in markdown",
  "concerns": [{"severity": "high|medium|low", "description": "string"}],
  "suggestions": ["string - Each improvement suggestion"],
  "prd_modifications": [{
    "operation_id": "string - Stable unique ID for this modification",
    "type": "update_story|add_story|remove_story|split_story|merge_story",
    "target_story_id": "string|null - story ID to modify/remove/split/merge (null for add)",
    "preview": "string - one-line summary for UI",
    "reason": "string - why this change is needed",
    "confidence": "number - 0 to 1",
    "payload": {
      "title": "string|null",
      "description": "string|null",
      "priority": "high|medium|low|null",
      "dependencies": ["string"],
      "acceptance_criteria": ["string"],
      "story": {
        "id": "string|null",
        "title": "string",
        "description": "string",
        "priority": "high|medium|low",
        "dependencies": ["string"],
        "acceptance_criteria": ["string"]
      },
      "stories": [{
        "id": "string|null",
        "title": "string",
        "description": "string",
        "priority": "high|medium|low",
        "dependencies": ["string"],
        "acceptance_criteria": ["string"]
      }],
      "dependency_remap": {"story_id": ["replacement_story_id"]}
    }
  }],
  "approved": "boolean - Whether the PRD is architecturally sound as-is"
}"#;

    use crate::services::llm::types::Message;

    let user_messages = vec![Message::user(format!(
        "Review this PRD from an architectural perspective:\n\n{}",
        prd_json
    ))];

    match crate::services::persona::run_expert_formatter::<serde_json::Value>(
        llm_provider.clone(),
        None,
        &persona,
        &phase_instructions,
        enriched_context.as_deref(),
        Some(locale_tag),
        user_messages,
        target_schema,
        None,
    )
    .await
    {
        Ok(result) => {
            let structured = &result.structured_output;
            Ok(CommandResponse::ok(ArchitectureReviewResult {
                analysis: result.expert_analysis,
                concerns: structured
                    .get("concerns")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                Some(ReviewConcern {
                                    severity: v
                                        .get("severity")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("medium")
                                        .to_string(),
                                    description: v
                                        .get("description")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                suggestions: structured
                    .get("suggestions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                prd_modifications: parse_prd_modifications(structured),
                approved: structured
                    .get("approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                persona_role: "SoftwareArchitect".to_string(),
            }))
        }
        Err(e) => Ok(CommandResponse::err(format!(
            "Architecture review failed: {}",
            e
        ))),
    }
        } => result,
    };
    clear_task_operation_token(&state, &session_id, &operation_id).await;
    if let Some(session_snapshot) = state.get_session_snapshot(&session_id).await {
        sync_kernel_task_snapshot_and_emit(
            &app_handle,
            kernel_state.inner(),
            &session_snapshot,
            None,
            "task_mode.run_architecture_review.finished",
        )
        .await;
    }
    result
}
