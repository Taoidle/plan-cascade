//! Skill Commands
//!
//! Tauri commands for the universal skill system.
//! Provides 10 commands for listing, searching, detecting, toggling,
//! creating, deleting, and managing skills.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

use crate::models::response::CommandResponse;
use crate::services::skills::config::{
    load_skills_config, resolve_source_path_for_project, save_skills_config, SourceDefinition,
};
use crate::services::skills::discovery::discover_all_skills;
use crate::services::skills::generator::SkillGeneratorStore;
use crate::services::skills::index::{build_index, compute_index_stats};
use crate::services::skills::model::{
    GeneratedSkill, GeneratedSkillRecord, InjectionPhase, MatchReason, SelectionPolicy,
    SkillDocument, SkillIndex, SkillIndexStats, SkillMatch, SkillReviewStatus, SkillSource,
    SkillSummary, SkillToolPolicyMode, SkillsOverview,
};
use crate::services::skills::select::{lexical_score_skills, select_skills_for_session};
use crate::services::task_mode::context_provider::{
    resolve_effective_skills, SkillSelectionMode,
};
use crate::state::AppState;
use crate::utils::configure_background_process;
use crate::utils::paths::ensure_plan_cascade_dir;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EffectiveSkillPreviewV2 {
    pub effective_skill_ids: Vec<String>,
    pub selected_skills: Vec<SkillSummary>,
    pub blocked_tools: Vec<String>,
    pub selection_reason: String,
    pub selection_origin: String,
    pub hierarchy_matches: Vec<String>,
    pub why_not_selected: Vec<crate::services::task_mode::context_provider::NonSelectedSkillDiagnostic>,
    pub skills_block: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillCommandInvocation {
    pub skill_id: String,
    pub skill_name: String,
    pub session_id: Option<String>,
    pub pinned: bool,
    pub selection_origin: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillSourceInfo {
    pub name: String,
    pub source_type: String,
    pub path: Option<String>,
    pub repository: Option<String>,
    pub url: Option<String>,
    pub enabled: bool,
    pub installed: bool,
    pub skill_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillSourceMutationResult {
    pub source: SkillSourceInfo,
    pub files_deleted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeneratedSkillExportV2 {
    pub schema_version: u32,
    pub export_type: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub body: String,
    pub review_status: SkillReviewStatus,
    pub review_notes: Option<String>,
    pub source_session_ids: Vec<String>,
}

/// List all skills from all sources for a project.
///
/// Returns: builtin + external + user + project-local + generated.
/// Each entry includes source, priority, enabled state, and detection status.
#[tauri::command]
pub async fn list_skills(
    project_path: String,
    source_filter: Option<String>,
    include_disabled: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillSummary>>, String> {
    let include_disabled = include_disabled.unwrap_or(true);

    // Build unified index from file-based + generated skills.
    let index = match build_unified_skill_index_for_project(&project_path, include_disabled, &state)
        .await
    {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let mut summaries = index.summaries();
    apply_summary_filters(&mut summaries, source_filter.as_deref(), include_disabled);

    Ok(CommandResponse::ok(summaries))
}

#[tauri::command]
pub async fn list_skills_v2(
    project_path: String,
    source_filter: Option<String>,
    include_disabled: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillSummary>>, String> {
    list_skills(project_path, source_filter, include_disabled, state).await
}

/// Get full skill content by ID.
#[tauri::command]
pub async fn get_skill(
    project_path: String,
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String> {
    // Check file-based skills
    let index = match build_skill_index_for_project(&project_path, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    if let Some(skill) = index.get_by_id(&id) {
        return Ok(CommandResponse::ok(skill.clone()));
    }

    // Check generated skills
    let generated_record = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.get_generated_skill(&id)
        })
        .await;

    match generated_record {
        Ok(Some(record)) => {
            return Ok(CommandResponse::ok(generated_record_to_document(record)));
        }
        Ok(None) => {}
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    }

    Ok(CommandResponse::err(format!("Skill not found: {}", id)))
}

#[tauri::command]
pub async fn get_skill_detail_v2(
    project_path: String,
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String> {
    get_skill(project_path, id, state).await
}

/// Search skills by query (lexical matching).
#[tauri::command]
pub async fn search_skills(
    project_path: String,
    query: String,
    top_k: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillMatch>>, String> {
    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let mut results = lexical_score_skills(&index, &query, &InjectionPhase::Always);

    let limit = top_k.unwrap_or(10);
    results.truncate(limit);

    Ok(CommandResponse::ok(results))
}

/// Run auto-detection to find applicable skills for a project.
#[tauri::command]
pub async fn detect_applicable_skills(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillMatch>>, String> {
    let index = match build_skill_index_for_project(&project_path, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let policy = SelectionPolicy::default();
    let project_root = Path::new(&project_path);

    // Run detection with empty user message (pure auto-detection)
    let matches = select_skills_for_session(
        &index,
        project_root,
        "",
        &InjectionPhase::Implementation,
        &policy,
    );

    // Filter to only auto-detected
    let auto_detected: Vec<SkillMatch> = matches
        .into_iter()
        .filter(|m| matches!(m.match_reason, MatchReason::AutoDetected))
        .collect();

    Ok(CommandResponse::ok(auto_detected))
}

/// Toggle a file-based skill's enabled state.
/// Persists disabled state in the settings table with key `skill_disabled:{id}`.
/// Only disabled skills are stored (enabled is the default).
#[tauri::command]
pub async fn toggle_skill(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    let result = state
        .with_database(|db| {
            if enabled {
                // Remove the disabled marker (enabled is default)
                db.delete_setting(&format!("skill_disabled:{}", id))
            } else {
                // Store disabled marker
                db.set_setting(&format!("skill_disabled:{}", id), "1")
            }
        })
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new project-local skill file in .skills/ directory.
#[tauri::command]
pub async fn create_skill_file(
    project_path: String,
    name: String,
    description: String,
    tags: Vec<String>,
    body: String,
) -> Result<CommandResponse<String>, String> {
    let skills_dir = Path::new(&project_path).join(".skills");

    // Ensure .skills/ directory exists
    if let Err(e) = std::fs::create_dir_all(&skills_dir) {
        return Ok(CommandResponse::err(format!(
            "Failed to create .skills/ directory: {}",
            e
        )));
    }

    // Sanitize name for filename
    let filename = name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();

    let file_path = skills_dir.join(format!("{}.md", filename));

    if file_path.exists() {
        return Ok(CommandResponse::err(format!(
            "Skill file already exists: {}",
            file_path.display()
        )));
    }

    // Build SKILL.md content with frontmatter
    let tags_str = if tags.is_empty() {
        String::new()
    } else {
        format!(
            "tags: [{}]\n",
            tags.iter()
                .map(|t| format!("{}", t))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let content = format!(
        "---\nname: {}\ndescription: {}\n{}---\n\n{}",
        name, description, tags_str, body
    );

    if let Err(e) = std::fs::write(&file_path, &content) {
        return Ok(CommandResponse::err(format!(
            "Failed to write skill file: {}",
            e
        )));
    }

    Ok(CommandResponse::ok(file_path.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn save_skill_v2(
    project_path: String,
    name: String,
    description: String,
    tags: Vec<String>,
    body: String,
) -> Result<CommandResponse<String>, String> {
    create_skill_file(project_path, name, description, tags, body).await
}

/// Delete a skill. For file-based skills, deletes the file.
/// For generated skills, deletes the database row.
#[tauri::command]
pub async fn delete_skill(
    id: String,
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    // Try to find it as a file-based skill
    let index = match build_skill_index_for_project(&project_path, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    if let Some(skill) = index.get_by_id(&id) {
        // Only allow deleting project-local skills
        if matches!(skill.source, SkillSource::ProjectLocal) {
            if let Err(e) = std::fs::remove_file(&skill.path) {
                return Ok(CommandResponse::err(format!(
                    "Failed to delete skill file: {}",
                    e
                )));
            }
            return Ok(CommandResponse::ok(()));
        } else {
            return Ok(CommandResponse::err(
                "Cannot delete non-project-local skills".to_string(),
            ));
        }
    }

    // Try to delete as generated skill
    let result = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.delete_generated_skill(&id)
        })
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(format!(
            "Skill not found: {} ({})",
            id, e
        ))),
    }
}

/// Toggle a generated skill's enabled state (persisted in DB).
#[tauri::command]
pub async fn toggle_generated_skill(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    let enabled_int: i32 = if enabled { 1 } else { 0 };

    let result = state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let rows = conn.execute(
                "UPDATE skill_library SET enabled = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![enabled_int, id],
            )?;
            if rows == 0 {
                return Err(crate::utils::error::AppError::not_found(format!(
                    "Generated skill not found: {}",
                    id
                )));
            }
            Ok(())
        })
        .await;

    match result {
        Ok(()) => Ok(CommandResponse::ok(())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Refresh the skill index (re-scan all sources).
#[tauri::command]
pub async fn refresh_skill_index(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillIndexStats>, String> {
    let index = match build_skill_index_for_project(&project_path, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let stats = compute_index_stats(&index);
    Ok(CommandResponse::ok(stats))
}

/// Get skills config overview (sources, counts, detection results).
#[tauri::command]
pub async fn get_skills_overview(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillsOverview>, String> {
    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let stats = compute_index_stats(&index);

    // Get detected skills
    let project_root = Path::new(&project_path);
    let policy = SelectionPolicy::default();
    let detected_matches = select_skills_for_session(
        &index,
        project_root,
        "",
        &InjectionPhase::Implementation,
        &policy,
    );
    let detected_skills: Vec<SkillSummary> =
        detected_matches.into_iter().map(|m| m.skill).collect();

    // Collect source names
    let sources: Vec<String> = index
        .skills()
        .iter()
        .filter_map(|s| match &s.source {
            SkillSource::External { source_name } => Some(source_name.clone()),
            _ => None,
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    Ok(CommandResponse::ok(SkillsOverview {
        stats,
        detected_skills,
        sources,
    }))
}

#[tauri::command]
pub async fn preview_effective_skills_v2(
    project_path: String,
    query: String,
    phase: Option<String>,
    selected_skill_ids: Option<Vec<String>>,
    selection_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<EffectiveSkillPreviewV2>, String> {
    let selected_skill_ids = selected_skill_ids.unwrap_or_default();
    let selection_mode = match selection_mode.as_deref() {
        Some("explicit") => SkillSelectionMode::Explicit,
        _ => {
            if selected_skill_ids.is_empty() {
                SkillSelectionMode::Auto
            } else {
                SkillSelectionMode::Explicit
            }
        }
    };

    let phase = match phase.as_deref() {
        Some("planning") => InjectionPhase::Planning,
        Some("retry") => InjectionPhase::Retry,
        Some("always") => InjectionPhase::Always,
        _ => InjectionPhase::Implementation,
    };

    let effective = resolve_effective_skills(
        &state,
        &project_path,
        &query,
        phase,
        &selected_skill_ids,
        &[],
        selection_mode,
        None,
        true,
        None,
    )
    .await;

    Ok(CommandResponse::ok(EffectiveSkillPreviewV2 {
        effective_skill_ids: effective
            .matches
            .iter()
            .map(|skill| skill.skill.id.clone())
            .collect(),
        selected_skills: effective.matches.iter().map(|skill| skill.skill.clone()).collect(),
        blocked_tools: effective.blocked_tools,
        selection_reason: effective.selection_reason,
        selection_origin: effective.selection_origin,
        hierarchy_matches: effective.hierarchy_matches,
        why_not_selected: effective.why_not_selected,
        skills_block: effective.skills_block,
    }))
}

#[tauri::command]
pub async fn invoke_skill_command_v2(
    project_path: String,
    skill_id: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillCommandInvocation>, String> {
    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let Some(skill) = index.get_by_id(&skill_id) else {
        return Ok(CommandResponse::err(format!("Skill not found: {}", skill_id)));
    };
    if !skill.user_invocable {
        return Ok(CommandResponse::err(format!(
            "Skill '{}' is not user-invocable",
            skill.name
        )));
    }
    if matches!(
        skill.review_status,
        Some(SkillReviewStatus::PendingReview | SkillReviewStatus::Rejected | SkillReviewStatus::Archived)
    ) {
        return Ok(CommandResponse::err(format!(
            "Skill '{}' is not approved for invocation",
            skill.name
        )));
    }

    if matches!(skill.source, SkillSource::Generated) {
        let _ = state
            .with_database(|db| {
                let store = SkillGeneratorStore::new(Arc::new(db.clone()));
                store.increment_usage(&skill_id)
            })
            .await;
    }

    Ok(CommandResponse::ok(SkillCommandInvocation {
        skill_id: skill.id.clone(),
        skill_name: skill.name.clone(),
        session_id,
        pinned: true,
        selection_origin: "command_invoked".to_string(),
    }))
}

#[tauri::command]
pub async fn review_generated_skill_v2(
    id: String,
    decision: String,
    review_notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String> {
    let review_status = match decision.trim().to_ascii_lowercase().as_str() {
        "approve" | "approved" => SkillReviewStatus::Approved,
        "reject" | "rejected" => SkillReviewStatus::Rejected,
        "archive" | "archived" => SkillReviewStatus::Archived,
        "restore" | "pending_review" | "pending" => SkillReviewStatus::PendingReview,
        other => {
            return Ok(CommandResponse::err(format!(
                "Unsupported generated skill review decision: {}",
                other
            )))
        }
    };

    let reviewed = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.review_generated_skill(&id, review_status, review_notes.as_deref())
        })
        .await;

    match reviewed {
        Ok(record) => Ok(CommandResponse::ok(generated_record_to_document(record))),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn update_generated_skill_v2(
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    body: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String> {
    let updated = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.update_generated_skill(&id, &name, &description, &tags, &body)
        })
        .await;

    match updated {
        Ok(record) => Ok(CommandResponse::ok(generated_record_to_document(record))),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn export_generated_skill_v2(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<String>, String> {
    let exported = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.get_generated_skill(&id)
        })
        .await;

    let Some(record) = exported.map_err(|e| e.to_string())? else {
        return Ok(CommandResponse::err(format!(
            "Generated skill not found: {}",
            id
        )));
    };

    let payload = GeneratedSkillExportV2 {
        schema_version: 1,
        export_type: "generated_skill".to_string(),
        name: record.name,
        description: record.description,
        tags: record.tags,
        body: record.body,
        review_status: record.review_status,
        review_notes: record.review_notes,
        source_session_ids: record.source_session_ids,
    };

    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize generated skill export: {}", e))?;
    Ok(CommandResponse::ok(json))
}

#[tauri::command]
pub async fn import_generated_skill_v2(
    project_path: String,
    json: String,
    conflict_policy: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String> {
    let payload: GeneratedSkillExportV2 = serde_json::from_str(&json)
        .map_err(|e| format!("Invalid generated skill import payload: {}", e))?;
    if payload.export_type != "generated_skill" {
        return Ok(CommandResponse::err(format!(
            "Unsupported generated skill export type: {}",
            payload.export_type
        )));
    }

    let normalized_policy = conflict_policy
        .unwrap_or_else(|| "rename".to_string())
        .trim()
        .to_ascii_lowercase();

    let imported = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            let existing = store
                .list_generated_skills(&project_path, true)?
                .into_iter()
                .find(|record| record.name.eq_ignore_ascii_case(payload.name.trim()));

            if let Some(existing) = existing {
                match normalized_policy.as_str() {
                    "skip" => {
                        return Err(crate::utils::error::AppError::validation(format!(
                            "Generated skill '{}' already exists",
                            payload.name
                        )))
                    }
                    "replace" => {
                        let updated = store.update_generated_skill(
                            &existing.id,
                            payload.name.trim(),
                            payload.description.trim(),
                            &payload.tags,
                            &payload.body,
                        )?;
                        let reviewed = store.review_generated_skill(
                            &updated.id,
                            SkillReviewStatus::PendingReview,
                            payload.review_notes.as_deref(),
                        )?;
                        return Ok(reviewed);
                    }
                    _ => {}
                }
            }

            let base_name = payload.name.trim();
            let final_name = if normalized_policy == "rename" {
                let existing_names = store
                    .list_generated_skills(&project_path, true)?
                    .into_iter()
                    .map(|record| record.name)
                    .collect::<std::collections::HashSet<_>>();
                dedupe_generated_skill_name(base_name, &existing_names)
            } else {
                base_name.to_string()
            };
            let generated = GeneratedSkill {
                name: final_name,
                description: payload.description.trim().to_string(),
                tags: payload.tags.clone(),
                body: payload.body.clone(),
                source_session_ids: payload.source_session_ids.clone(),
            };
            let saved = store.save_generated_skill(&project_path, &generated)?;
            store.review_generated_skill(
                &saved.id,
                SkillReviewStatus::PendingReview,
                payload.review_notes.as_deref(),
            )
        })
        .await;

    match imported {
        Ok(record) => Ok(CommandResponse::ok(generated_record_to_document(record))),
        Err(error) => Ok(CommandResponse::err(error.to_string())),
    }
}

#[tauri::command]
pub async fn list_skill_sources_v2(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillSourceInfo>>, String> {
    let project_root = Path::new(&project_path);
    let config_path = project_root.join("external-skills.json");
    let config = match load_skills_config(&config_path) {
        Ok(cfg) => cfg,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };
    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();

    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(error) => return Ok(CommandResponse::err(error)),
    };

    let sources = config
        .sources
        .iter()
        .map(|(name, source)| {
            source_info_from_definition(name, source, project_root, plan_cascade_dir.as_deref(), &index)
        })
        .collect();

    Ok(CommandResponse::ok(sources))
}

#[tauri::command]
pub async fn install_skill_source_v2(
    project_path: String,
    source: String,
    name: Option<String>,
) -> Result<CommandResponse<SkillSourceInfo>, String> {
    let normalized_source = source.trim();
    if normalized_source.is_empty() {
        return Ok(CommandResponse::err("Skill source is required".to_string()));
    }

    let project_root = Path::new(&project_path);
    let config_path = project_root.join("external-skills.json");
    let mut config = load_skills_config(&config_path).unwrap_or_default();

    let source_name = name
        .as_deref()
        .map(sanitize_skill_source_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| derive_skill_source_name(normalized_source));
    if config.sources.contains_key(&source_name) {
        return Ok(CommandResponse::err(format!(
            "Skill source '{}' already exists",
            source_name
        )));
    }

    let installed = install_skill_source(normalized_source, &source_name, project_root).await;
    let (source_type, path, repository, url) = match installed {
        Ok(result) => result,
        Err(error) => return Ok(CommandResponse::err(error)),
    };

    config.sources.insert(
        source_name.clone(),
        SourceDefinition {
            source_type: source_type.clone(),
            path: Some(path.clone()),
            repository: repository.clone(),
            url: url.clone(),
            enabled: true,
        },
    );
    if let Err(error) = save_skills_config(&config_path, &config) {
        return Ok(CommandResponse::err(error.to_string()));
    }

    Ok(CommandResponse::ok(SkillSourceInfo {
        name: source_name,
        source_type,
        path: Some(path),
        repository,
        url,
        enabled: true,
        installed: true,
        skill_count: 0,
    }))
}

#[tauri::command]
pub async fn set_skill_source_enabled_v2(
    project_path: String,
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillSourceInfo>, String> {
    let project_root = Path::new(&project_path);
    let config_path = project_root.join("external-skills.json");
    let mut config = match load_skills_config(&config_path) {
        Ok(cfg) => cfg,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };
    let Some(source) = config.sources.get_mut(&name) else {
        return Ok(CommandResponse::err(format!(
            "Skill source '{}' not found",
            name
        )));
    };
    source.enabled = enabled;
    if let Err(error) = save_skills_config(&config_path, &config) {
        return Ok(CommandResponse::err(error.to_string()));
    }

    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let source = config.sources.get(&name).expect("source exists");
    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();
    Ok(CommandResponse::ok(source_info_from_definition(
        &name,
        source,
        project_root,
        plan_cascade_dir.as_deref(),
        &index,
    )))
}

#[tauri::command]
pub async fn refresh_skill_source_v2(
    project_path: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillSourceInfo>, String> {
    let project_root = Path::new(&project_path);
    let config_path = project_root.join("external-skills.json");
    let config = match load_skills_config(&config_path) {
        Ok(cfg) => cfg,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };
    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();
    let Some(source) = config.sources.get(&name) else {
        return Ok(CommandResponse::err(format!(
            "Skill source '{}' not found",
            name
        )));
    };

    let source_root = resolve_skill_source_root(source, project_root, plan_cascade_dir.as_deref())
        .ok_or_else(|| format!("Skill source '{}' has no resolvable path", name))?;

    match source.source_type.as_str() {
        "git" => {
            if !source_root.exists() {
                let Some(repository) = source.repository.as_deref() else {
                    return Ok(CommandResponse::err(format!(
                        "Git source '{}' is missing repository metadata",
                        name
                    )));
                };
                clone_skill_source(repository, &source_root).await?;
            } else {
                pull_skill_source(&source_root).await?;
            }
        }
        "url" => {
            let Some(url) = source.url.as_deref() else {
                return Ok(CommandResponse::err(format!(
                    "URL source '{}' is missing URL metadata",
                    name
                )));
            };
            download_skill_source(url, &source_root).await?;
        }
        _ => {
            if !source_root.exists() {
                return Ok(CommandResponse::err(format!(
                    "Local skill source '{}' is missing: {}",
                    name,
                    source_root.display()
                )));
            }
        }
    }

    let index = match build_unified_skill_index_for_project(&project_path, true, &state).await {
        Ok(idx) => idx,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    Ok(CommandResponse::ok(source_info_from_definition(
        &name,
        source,
        project_root,
        plan_cascade_dir.as_deref(),
        &index,
    )))
}

#[tauri::command]
pub async fn remove_skill_source_v2(
    project_path: String,
    name: String,
    delete_installed_copy: Option<bool>,
) -> Result<CommandResponse<SkillSourceMutationResult>, String> {
    let project_root = Path::new(&project_path);
    let config_path = project_root.join("external-skills.json");
    let mut config = match load_skills_config(&config_path) {
        Ok(cfg) => cfg,
        Err(error) => return Ok(CommandResponse::err(error.to_string())),
    };

    let Some(source) = config.sources.remove(&name) else {
        return Ok(CommandResponse::err(format!(
            "Skill source '{}' not found",
            name
        )));
    };
    if let Err(error) = save_skills_config(&config_path, &config) {
        return Ok(CommandResponse::err(error.to_string()));
    }

    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();
    let source_info = source_info_from_definition(
        &name,
        &source,
        project_root,
        plan_cascade_dir.as_deref(),
        &SkillIndex::new(vec![]),
    );
    let mut files_deleted = false;
    if delete_installed_copy.unwrap_or(true) {
        if let Some(path) = resolve_skill_source_root(&source, project_root, plan_cascade_dir.as_deref()) {
            if should_delete_managed_skill_source(&path, &source.source_type) {
                let deletion = if path.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if deletion.is_ok() {
                    files_deleted = true;
                }
            }
        }
    }

    Ok(CommandResponse::ok(SkillSourceMutationResult {
        source: source_info,
        files_deleted,
    }))
}

// --- Internal helper functions ---

fn dedupe_generated_skill_name(
    base_name: &str,
    existing_names: &std::collections::HashSet<String>,
) -> String {
    if !existing_names.contains(base_name) {
        return base_name.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{} (Imported {})", base_name, suffix);
        if !existing_names.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn sanitize_skill_source_name(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn derive_skill_source_name(source: &str) -> String {
    let trimmed = source.trim().trim_end_matches(".git");
    let tail = trimmed
        .rsplit(['/', ':'])
        .find(|segment| !segment.is_empty())
        .unwrap_or("skill-source");
    let sanitized = sanitize_skill_source_name(tail);
    if sanitized.is_empty() {
        "skill-source".to_string()
    } else {
        sanitized
    }
}

fn normalize_git_source(source: &str) -> String {
    if let Some(repo) = source.strip_prefix("github:") {
        return format!("https://github.com/{}.git", repo.trim().trim_end_matches(".git"));
    }
    if source.starts_with("https://github.com/") && !source.ends_with(".git") {
        return format!("{}.git", source.trim_end_matches('/'));
    }
    source.to_string()
}

async fn install_skill_source(
    source: &str,
    source_name: &str,
    project_root: &Path,
) -> Result<(String, String, Option<String>, Option<String>), String> {
    let candidate_path = PathBuf::from(source);
    if candidate_path.exists() {
        let path = if candidate_path.is_absolute() {
            candidate_path
        } else {
            project_root.join(candidate_path)
        };
        return Ok((
            "local".to_string(),
            path.to_string_lossy().to_string(),
            None,
            None,
        ));
    }

    let install_root = ensure_plan_cascade_dir()
        .map_err(|e| e.to_string())?
        .join("skill-sources");
    std::fs::create_dir_all(&install_root).map_err(|e| {
        format!(
            "Failed to create skill source install directory {}: {}",
            install_root.display(),
            e
        )
    })?;
    let target_dir = install_root.join(source_name);
    if target_dir.exists() {
        return Err(format!(
            "Skill source install directory already exists: {}",
            target_dir.display()
        ));
    }

    if source.starts_with("http://") || source.starts_with("https://") {
        if source.contains("github.com/") && !source.contains("/raw/") && !source.ends_with(".md")
        {
            let git_url = normalize_git_source(source);
            clone_skill_source(&git_url, &target_dir).await?;
            return Ok((
                "git".to_string(),
                target_dir.to_string_lossy().to_string(),
                Some(git_url),
                None,
            ));
        }

        download_skill_source(source, &target_dir).await?;
        return Ok((
            "url".to_string(),
            target_dir.to_string_lossy().to_string(),
            None,
            Some(source.to_string()),
        ));
    }

    if source.starts_with("git@")
        || source.starts_with("ssh://")
        || source.ends_with(".git")
        || source.starts_with("github:")
    {
        let git_url = normalize_git_source(source);
        clone_skill_source(&git_url, &target_dir).await?;
        return Ok((
            "git".to_string(),
            target_dir.to_string_lossy().to_string(),
            Some(git_url),
            None,
        ));
    }

    Err(format!("Unsupported skill source: {}", source))
}

async fn clone_skill_source(git_url: &str, target_dir: &Path) -> Result<(), String> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| "Invalid target directory for skill source".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create parent directory {}: {}", parent.display(), e))?;

    let mut clone_cmd = tokio::process::Command::new("git");
    clone_cmd.args([
        "clone",
        "--depth",
        "1",
        "--filter=blob:none",
        git_url,
        target_dir.to_str().unwrap_or("skill-source"),
    ]);
    configure_background_process(&mut clone_cmd);
    let output = clone_cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute git clone: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("git clone failed for {}: {}", git_url, stderr.trim()))
}

async fn pull_skill_source(target_dir: &Path) -> Result<(), String> {
    let mut pull_cmd = tokio::process::Command::new("git");
    pull_cmd.args([
        "-C",
        target_dir.to_str().unwrap_or("."),
        "pull",
        "--ff-only",
    ]);
    configure_background_process(&mut pull_cmd);
    let output = pull_cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute git pull: {}", e))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "git pull failed for {}: {}",
        target_dir.display(),
        stderr.trim()
    ))
}

async fn download_skill_source(url: &str, target_dir: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("plan-cascade-desktop")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download skill source: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download skill source {}: HTTP {}",
            url,
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read downloaded skill source: {}", e))?;

    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create target directory {}: {}", target_dir.display(), e))?;
    let filename = url
        .split('/')
        .next_back()
        .filter(|name| name.ends_with(".md"))
        .unwrap_or("SKILL.md");
    std::fs::write(target_dir.join(filename), &bytes)
        .map_err(|e| format!("Failed to persist downloaded skill source: {}", e))?;
    Ok(())
}

fn resolve_skill_source_root(
    source: &SourceDefinition,
    project_root: &Path,
    plan_cascade_dir: Option<&Path>,
) -> Option<PathBuf> {
    resolve_source_path_for_project(source, project_root, plan_cascade_dir)
}

fn should_delete_managed_skill_source(path: &Path, source_type: &str) -> bool {
    if source_type == "local" {
        return false;
    }
    let Ok(base_dir) = crate::utils::paths::plan_cascade_dir() else {
        return false;
    };
    let managed_root = base_dir.join("skill-sources");
    path.starts_with(&managed_root)
}

fn source_info_from_definition(
    name: &str,
    source: &SourceDefinition,
    project_root: &Path,
    plan_cascade_dir: Option<&Path>,
    index: &SkillIndex,
) -> SkillSourceInfo {
    let skill_count = index
        .skills()
        .iter()
        .filter(|skill| matches!(&skill.source, SkillSource::External { source_name } if source_name == name))
        .count();
    let installed = resolve_skill_source_root(source, project_root, plan_cascade_dir)
        .map(|path| path.exists())
        .unwrap_or(false);

    SkillSourceInfo {
        name: name.to_string(),
        source_type: source.source_type.clone(),
        path: source.path.clone(),
        repository: source.repository.clone(),
        url: source.url.clone(),
        enabled: source.enabled,
        installed,
        skill_count,
    }
}

/// Build a SkillIndex for a project by scanning all sources.
/// Applies persisted disabled state from the settings table.
async fn build_skill_index_for_project(
    project_path: &str,
    state: &State<'_, AppState>,
) -> Result<SkillIndex, String> {
    let project_root = Path::new(project_path);

    // Load config (try project-local, then global)
    let config_path = project_root.join("external-skills.json");
    let config = load_skills_config(&config_path).unwrap_or_default();
    let plan_cascade_dir = crate::utils::paths::plan_cascade_dir().ok();

    // Discover skills from all sources
    let discovered = discover_all_skills(project_root, &config, plan_cascade_dir.as_deref())
        .map_err(|e| format!("Discovery failed: {}", e))?;

    // Build the index
    let index = build_index(discovered).map_err(|e| format!("Index build failed: {}", e))?;

    // Apply persisted disabled state from the database
    let disabled_ids: std::collections::HashSet<String> = state
        .with_database(|db| {
            let rows = db.get_settings_by_prefix("skill_disabled:")?;
            Ok(rows
                .into_iter()
                .map(|(key, _)| {
                    key.strip_prefix("skill_disabled:")
                        .unwrap_or(&key)
                        .to_string()
                })
                .collect())
        })
        .await
        .unwrap_or_default();

    if disabled_ids.is_empty() {
        return Ok(index);
    }

    // Rebuild index with disabled skills marked
    let mut docs = index.skills().to_vec();
    for doc in &mut docs {
        if disabled_ids.contains(&doc.id) {
            doc.enabled = false;
        }
    }
    Ok(SkillIndex::new(docs))
}

/// Build a unified SkillIndex for a project by combining file-based and generated skills.
async fn build_unified_skill_index_for_project(
    project_path: &str,
    include_disabled: bool,
    state: &State<'_, AppState>,
) -> Result<SkillIndex, String> {
    let base_index = build_skill_index_for_project(project_path, state).await?;
    let mut docs = base_index.skills().to_vec();

    let generated_docs = get_generated_documents(project_path, include_disabled, state).await?;
    docs.extend(generated_docs);

    Ok(SkillIndex::new(docs))
}

/// Load generated skills from the database and convert them into SkillDocument records.
async fn get_generated_documents(
    project_path: &str,
    include_disabled: bool,
    state: &State<'_, AppState>,
) -> Result<Vec<SkillDocument>, String> {
    let path = project_path.to_string();
    let records: Vec<GeneratedSkillRecord> = state
        .with_database(|db| {
            let store = SkillGeneratorStore::new(Arc::new(db.clone()));
            store.list_generated_skills(&path, include_disabled)
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(records
        .into_iter()
        .map(generated_record_to_document)
        .collect())
}

fn generated_record_to_document(record: GeneratedSkillRecord) -> SkillDocument {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("usage_count".to_string(), record.usage_count.to_string());
    metadata.insert(
        "success_rate".to_string(),
        format!("{:.2}", record.success_rate),
    );
    metadata.insert(
        "source_session_ids".to_string(),
        record.source_session_ids.join(", "),
    );
    metadata.insert("created_at".to_string(), record.created_at.clone());
    metadata.insert("updated_at".to_string(), record.updated_at.clone());
    SkillDocument {
        id: record.id.clone(),
        name: record.name,
        description: record.description,
        version: None,
        tags: record.tags,
        body: record.body,
        path: std::path::PathBuf::from(format!("generated://{}", record.id)),
        hash: format!("generated-{}", record.id),
        last_modified: None,
        user_invocable: false,
        tool_policy_mode: SkillToolPolicyMode::Advisory,
        allowed_tools: vec![],
        license: None,
        metadata,
        hooks: None,
        source: SkillSource::Generated,
        priority: 0,
        detect: None,
        inject_into: vec![InjectionPhase::Always],
        enabled: record.enabled,
        review_status: Some(record.review_status),
        review_notes: record.review_notes,
        reviewed_at: record.reviewed_at,
    }
}

fn apply_summary_filters(
    summaries: &mut Vec<SkillSummary>,
    source_filter: Option<&str>,
    include_disabled: bool,
) {
    if let Some(filter) = source_filter {
        summaries.retain(|s| match filter {
            "builtin" => matches!(s.source, SkillSource::Builtin),
            "external" => matches!(s.source, SkillSource::External { .. }),
            "user" => matches!(s.source, SkillSource::User),
            "project" | "project_local" => matches!(s.source, SkillSource::ProjectLocal),
            "generated" => matches!(s.source, SkillSource::Generated),
            _ => true,
        });
    }

    if !include_disabled {
        summaries.retain(|s| s.enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_create_skill_file_basic() {
        let dir = TempDir::new().unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(create_skill_file(
            dir.path().to_str().unwrap().to_string(),
            "my-skill".to_string(),
            "A test skill".to_string(),
            vec!["test".to_string()],
            "# My Skill\n\nDo the thing.".to_string(),
        ));

        let response = result.unwrap();
        assert!(response.success, "Error: {:?}", response.error);

        // Verify file was created
        let skills_dir = dir.path().join(".skills");
        assert!(skills_dir.exists());

        let skill_file = skills_dir.join("my-skill.md");
        assert!(skill_file.exists());

        let content = fs::read_to_string(&skill_file).unwrap();
        assert!(content.contains("name: my-skill"));
        assert!(content.contains("description: A test skill"));
        assert!(content.contains("tags: [test]"));
        assert!(content.contains("# My Skill"));
    }

    #[test]
    fn test_create_skill_file_duplicate_fails() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();
        fs::write(skills_dir.join("existing.md"), "# Existing").unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(create_skill_file(
            dir.path().to_str().unwrap().to_string(),
            "existing".to_string(),
            "Test".to_string(),
            vec![],
            "# New".to_string(),
        ));

        let response = result.unwrap();
        assert!(!response.success);
        assert!(response.error.as_ref().unwrap().contains("already exists"));
    }

    #[test]
    fn test_build_skill_index_for_project_empty() {
        let dir = TempDir::new().unwrap();

        // We can't easily test the async function with State, but we can test the core logic
        let config = crate::services::skills::config::SkillsConfig::default();
        let discovered = discover_all_skills(dir.path(), &config, None).unwrap();
        let index = build_index(discovered).unwrap();
        assert!(index.is_empty());
    }

    #[test]
    fn test_build_skill_index_with_local_skills() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".skills");
        fs::create_dir(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("test.md"),
            "---\nname: test-skill\ndescription: A test\n---\n# Test body",
        )
        .unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Project Guide").unwrap();

        let config = crate::services::skills::config::SkillsConfig::default();
        let discovered = discover_all_skills(dir.path(), &config, None).unwrap();
        let index = build_index(discovered).unwrap();

        assert_eq!(index.len(), 2);
        let stats = compute_index_stats(&index);
        assert_eq!(stats.project_local_count, 2);
    }

    #[test]
    fn test_apply_summary_filters_respects_source() {
        let mut summaries = vec![
            SkillSummary {
                id: "builtin-1".to_string(),
                name: "builtin".to_string(),
                description: "builtin".to_string(),
                version: None,
                tags: vec![],
                tool_policy_mode: SkillToolPolicyMode::Advisory,
                allowed_tools: vec![],
                source: SkillSource::Builtin,
                priority: 10,
                enabled: true,
                detected: false,
                user_invocable: false,
                has_hooks: false,
                inject_into: vec![InjectionPhase::Always],
                path: std::path::PathBuf::from("/tmp/builtin"),
                review_status: None,
                review_notes: None,
                reviewed_at: None,
            },
            SkillSummary {
                id: "generated-1".to_string(),
                name: "generated".to_string(),
                description: "generated".to_string(),
                version: None,
                tags: vec![],
                tool_policy_mode: SkillToolPolicyMode::Advisory,
                allowed_tools: vec![],
                source: SkillSource::Generated,
                priority: 0,
                enabled: true,
                detected: false,
                user_invocable: false,
                has_hooks: false,
                inject_into: vec![InjectionPhase::Always],
                path: std::path::PathBuf::from("generated://generated-1"),
                review_status: None,
                review_notes: None,
                reviewed_at: None,
            },
        ];

        apply_summary_filters(&mut summaries, Some("builtin"), true);
        assert_eq!(summaries.len(), 1);
        assert!(matches!(summaries[0].source, SkillSource::Builtin));
    }

    #[test]
    fn test_generated_record_to_document_has_generated_source() {
        let record = GeneratedSkillRecord {
            id: "gen-1".to_string(),
            project_path: "/tmp/project".to_string(),
            name: "generated-skill".to_string(),
            description: "desc".to_string(),
            tags: vec!["generated".to_string()],
            body: "body".to_string(),
            source_type: "generated".to_string(),
            source_session_ids: vec!["session-1".to_string()],
            usage_count: 0,
            success_rate: 1.0,
            keywords: vec![],
            enabled: true,
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        };

        let doc = generated_record_to_document(record);
        assert!(matches!(doc.source, SkillSource::Generated));
        assert_eq!(doc.name, "generated-skill");
        assert!(doc.path.to_string_lossy().starts_with("generated://"));
    }
}
