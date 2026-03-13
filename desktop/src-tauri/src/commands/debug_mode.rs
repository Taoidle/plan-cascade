use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::commands::workflow::{emit_kernel_update_for_session, emit_session_catalog_update};
use crate::models::CommandResponse;
use crate::services::debug_mode::{
    build_debug_capability_snapshot, capability_profile_for_environment, DebugArtifactContent,
    DebugArtifactDescriptor, DebugCapabilitySnapshot, DebugEnvironment, DebugEvidenceRef,
    DebugExecutionReport, DebugHypothesis, DebugLifecyclePhase, DebugModeSession,
    DebugPatchOperation, DebugPendingApproval, DebugProgressPayload, DebugState, DebugSeverity,
    EnterDebugModeRequest, FixProposal, RootCauseReport, VerificationCheck, VerificationReport,
};
use crate::commands::permissions::PermissionState;
use crate::services::orchestrator::permissions::PermissionLevel;
use crate::services::workflow_kernel::{
    ModeQualitySnapshot, WorkflowKernelState, WorkflowMode, WorkflowStatus,
};
use crate::utils::paths::ensure_plan_cascade_dir;
use tauri::Emitter;

#[derive(Clone)]
pub struct DebugModeState {
    sessions: Arc<RwLock<HashMap<String, DebugModeSession>>>,
    storage_root: Arc<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugModeSessionRecord {
    version: u32,
    session: DebugModeSession,
}

const DEBUG_MODE_SESSION_VERSION: u32 = 1;

impl DebugModeState {
    pub fn new() -> Self {
        Self::new_with_storage_dir(resolve_debug_mode_storage_root())
    }

    pub fn new_with_storage_dir(storage_root: PathBuf) -> Self {
        let sessions_dir = storage_root.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            storage_root: Arc::new(storage_root),
        }
    }

    pub async fn get_session_snapshot(&self, session_id: &str) -> Option<DebugModeSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    pub async fn get_or_load_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<DebugModeSession>, String> {
        if let Some(snapshot) = self.get_session_snapshot(session_id).await {
            return Ok(Some(snapshot));
        }

        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read(&path)
            .map_err(|error| format!("Failed to read persisted debug session '{session_id}': {error}"))?;
        let record: DebugModeSessionRecord = serde_json::from_slice(&raw)
            .map_err(|error| format!("Persisted debug session '{session_id}' is corrupted: {error}"))?;
        if record.version != DEBUG_MODE_SESSION_VERSION {
            return Err(format!(
                "Unsupported debug session record version {} for '{}'",
                record.version, session_id
            ));
        }

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), record.session.clone());
        }

        Ok(Some(record.session))
    }

    pub async fn store_session_snapshot(&self, session: DebugModeSession) -> Result<(), String> {
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.session_id.clone(), session.clone());
        }
        let encoded = serde_json::to_vec_pretty(&DebugModeSessionRecord {
            version: DEBUG_MODE_SESSION_VERSION,
            session: session.clone(),
        })
        .map_err(|error| format!("Failed to encode debug session '{}': {error}", session.session_id))?;
        fs::write(self.session_file_path(&session.session_id), encoded)
            .map_err(|error| format!("Failed to persist debug session '{}': {error}", session.session_id))
    }

    pub async fn delete_session_snapshot(&self, session_id: &str) -> Result<(), String> {
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }
        let path = self.session_file_path(session_id);
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("Failed to delete debug session '{session_id}': {error}"))?;
        }
        Ok(())
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.storage_root
            .join("sessions")
            .join(format!("{session_id}.json"))
    }

    fn artifact_dir_path(&self, session_id: &str) -> PathBuf {
        self.storage_root.join("artifacts").join(session_id)
    }
}

fn resolve_debug_mode_storage_root() -> PathBuf {
    if let Ok(root) = ensure_plan_cascade_dir() {
        let path = root.join("debug-mode");
        let _ = fs::create_dir_all(&path);
        return path;
    }

    let fallback = std::env::temp_dir().join("plan-cascade-debug-mode");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn sanitize_artifact_slug(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "artifact".to_string()
    } else {
        slug
    }
}

fn debug_environment_as_str(environment: DebugEnvironment) -> &'static str {
    match environment {
        DebugEnvironment::Dev => "dev",
        DebugEnvironment::Staging => "staging",
        DebugEnvironment::Prod => "prod",
    }
}

fn debug_severity_as_str(severity: DebugSeverity) -> &'static str {
    match severity {
        DebugSeverity::Low => "low",
        DebugSeverity::Medium => "medium",
        DebugSeverity::High => "high",
        DebugSeverity::Critical => "critical",
    }
}

fn infer_environment(description: &str) -> DebugEnvironment {
    let normalized = description.to_ascii_lowercase();
    if normalized.contains("prod") || normalized.contains("production") {
        DebugEnvironment::Prod
    } else if normalized.contains("stage") {
        DebugEnvironment::Staging
    } else {
        DebugEnvironment::Dev
    }
}

fn extract_first_url(description: &str) -> Option<String> {
    let regex = regex::Regex::new(r#"https?://[^\s)>"]+"#).ok()?;
    regex.find(description).map(|match_| match_.as_str().to_string())
}

fn build_session_title(description: &str) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return "New debug case".to_string();
    }
    trimmed.chars().take(80).collect()
}

fn infer_severity(description: &str) -> DebugSeverity {
    let normalized = description.to_ascii_lowercase();
    if normalized.contains("critical")
        || normalized.contains("outage")
        || normalized.contains("sev1")
        || normalized.contains("p0")
    {
        DebugSeverity::Critical
    } else if normalized.contains("high")
        || normalized.contains("sev2")
        || normalized.contains("p1")
        || normalized.contains("blocked")
    {
        DebugSeverity::High
    } else if normalized.contains("low")
        || normalized.contains("minor")
        || normalized.contains("cosmetic")
    {
        DebugSeverity::Low
    } else {
        DebugSeverity::Medium
    }
}

fn split_structured_lines(input: &str) -> Vec<String> {
    input.lines()
        .map(str::trim)
        .map(|line| line.trim_start_matches(|ch: char| {
            matches!(ch, '-' | '*' | '•' | '1'..='9' | '.' | ')' | ' ')
        }))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn merge_unique_strings(target: &mut Vec<String>, incoming: impl IntoIterator<Item = String>) {
    for item in incoming {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !target.iter().any(|existing| existing.eq_ignore_ascii_case(trimmed)) {
            target.push(trimmed.to_string());
        }
    }
}

fn merge_repro_steps(state: &mut DebugState, input: &str) {
    merge_unique_strings(&mut state.repro_steps, split_structured_lines(input));
}

fn infer_affected_surface_from_text(text: &str, target_url: Option<&str>) -> Vec<String> {
    let normalized = text.to_ascii_lowercase();
    let mut result = Vec::new();
    if normalized.contains("frontend")
        || normalized.contains("react")
        || normalized.contains("browser")
        || normalized.contains("console")
    {
        result.push("frontend".to_string());
    }
    if normalized.contains("api")
        || normalized.contains("request")
        || normalized.contains("network")
        || normalized.contains("http")
    {
        result.push("api".to_string());
    }
    if normalized.contains("redis") || normalized.contains("cache") {
        result.push("cache".to_string());
    }
    if normalized.contains("db")
        || normalized.contains("database")
        || normalized.contains("sql")
        || normalized.contains("query")
    {
        result.push("database".to_string());
    }
    if let Some(url) = target_url {
        result.push(url.to_string());
    }
    result
}

fn default_fix_proposal(summary: &str) -> FixProposal {
    FixProposal {
        summary: "Apply the smallest fix that addresses the confirmed root cause.".to_string(),
        change_scope: vec!["code".to_string(), "verification".to_string()],
        risk_level: DebugSeverity::Medium,
        files_or_systems_touched: vec!["frontend".to_string()],
        manual_approvals_required: vec!["patch_review".to_string()],
        verification_plan: vec![
            "Re-run the reproduction path".to_string(),
            "Check browser/network/log output".to_string(),
        ],
        patch_preview_ref: Some(format!("patch-preview:{summary}")),
        patch_operations: vec![],
    }
}

fn metadata_string(metadata: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn metadata_u64(metadata: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<u64> {
    metadata.get(key).and_then(|value| value.as_u64())
}

fn metadata_string_array(
    metadata: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Vec<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn latest_evidence_by_kind<'a>(
    session: &'a DebugModeSession,
    kind: &str,
    stage: Option<&str>,
) -> Option<&'a DebugEvidenceRef> {
    session.state.evidence_refs.iter().rev().find(|entry| {
        if entry.kind != kind {
            return false;
        }
        match stage {
            Some(expected) => metadata_string(&entry.metadata, "stage")
                .map(|value| value == expected)
                .unwrap_or(false),
            None => true,
        }
    })
}

fn trim_matching_wrappers(value: &str) -> String {
    let trimmed = value.trim();
    let wrappers = [('`', '`'), ('"', '"'), ('\'', '\''), ('“', '”')];
    for (start, end) in wrappers {
        if trimmed.starts_with(start) && trimmed.ends_with(end) && trimmed.len() >= 2 {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn normalize_candidate_project_path(candidate: &str) -> Option<String> {
    let normalized = candidate
        .split('?')
        .next()
        .unwrap_or(candidate)
        .split('#')
        .next()
        .unwrap_or(candidate)
        .replace('\\', "/");
    let normalized = normalized.trim().trim_matches('`').trim_matches('"').trim_matches('\'');
    if normalized.is_empty() {
        return None;
    }
    let path_patterns = ["src/", "app/", "pages/", "components/", "lib/", "server/", "api/", "tests/", "test/"];
    for pattern in path_patterns {
        if let Some(index) = normalized.find(pattern) {
            return Some(normalized[index..].to_string());
        }
    }
    let looks_like_file = regex::Regex::new(r"(?i)[\w./-]+\.(ts|tsx|js|jsx|vue|json|rs|py|go|java|kt|swift|css|scss|md)$")
        .ok()
        .map(|pattern| pattern.is_match(normalized))
        .unwrap_or(false);
    if looks_like_file {
        return Some(normalized.trim_start_matches('/').to_string());
    }
    None
}

fn collect_project_files_recursive(dir: &Path, acc: &mut Vec<PathBuf>, limit: usize) {
    if acc.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        if path.is_dir() {
            if matches!(name, ".git" | "node_modules" | "dist" | "build" | "target" | ".next" | ".turbo") {
                continue;
            }
            collect_project_files_recursive(&path, acc, limit);
            if acc.len() >= limit {
                return;
            }
        } else if path.is_file() {
            acc.push(path);
            if acc.len() >= limit {
                return;
            }
        }
    }
}

fn resolve_patch_target_files(session: &DebugModeSession, project_root: &Path) -> Vec<String> {
    let mut raw_candidates = Vec::new();
    if let Some(root_cause) = session.state.selected_root_cause.as_ref() {
        merge_unique_strings(&mut raw_candidates, root_cause.impact_scope.clone());
    }
    if let Some(fix_proposal) = session.state.fix_proposal.as_ref() {
        merge_unique_strings(&mut raw_candidates, fix_proposal.files_or_systems_touched.clone());
    }
    merge_unique_strings(&mut raw_candidates, session.state.affected_surface.iter().cloned());
    for evidence in session
        .state
        .evidence_refs
        .iter()
        .filter(|entry| entry.kind == "source_mapping")
    {
        merge_unique_strings(
            &mut raw_candidates,
            metadata_string_array(&evidence.metadata, "candidateFiles"),
        );
        merge_unique_strings(
            &mut raw_candidates,
            metadata_string_array(&evidence.metadata, "resolvedSources"),
        );
        merge_unique_strings(
            &mut raw_candidates,
            metadata_string_array(&evidence.metadata, "bundleScripts"),
        );
        merge_unique_strings(
            &mut raw_candidates,
            split_structured_lines(&evidence.summary),
        );
    }

    let mut resolved = Vec::new();
    let mut basename_candidates = Vec::new();
    for candidate in raw_candidates {
        let Some(normalized) = normalize_candidate_project_path(&candidate) else {
            continue;
        };
        let candidate_path = Path::new(&normalized);
        let direct = project_root.join(candidate_path);
        if direct.exists() && direct.is_file() {
            if let Ok(relative) = direct.strip_prefix(project_root) {
                let relative_string = relative.to_string_lossy().replace('\\', "/");
                merge_unique_strings(&mut resolved, [relative_string]);
            }
            continue;
        }
        if let Some(name) = candidate_path.file_name().and_then(|value| value.to_str()) {
            basename_candidates.push(name.to_string());
        }
    }

    if resolved.is_empty() && !basename_candidates.is_empty() {
        let mut project_files = Vec::new();
        collect_project_files_recursive(project_root, &mut project_files, 400);
        for basename in basename_candidates {
            for path in &project_files {
                if path.file_name().and_then(|value| value.to_str()) == Some(basename.as_str()) {
                    if let Ok(relative) = path.strip_prefix(project_root) {
                        let relative_string = relative.to_string_lossy().replace('\\', "/");
                        merge_unique_strings(&mut resolved, [relative_string]);
                    }
                }
            }
        }
    }

    resolved.truncate(4);
    resolved
}

fn extract_replacement_instructions(session: &DebugModeSession) -> Vec<(String, String)> {
    let mut instructions = Vec::new();
    let corpus_parts = vec![
        session.state.symptom_summary.clone(),
        session.state.actual_behavior.clone().unwrap_or_default(),
        session.state.expected_behavior.clone().unwrap_or_default(),
        session.state.recent_changes.clone().unwrap_or_default(),
        session
            .state
            .selected_root_cause
            .as_ref()
            .map(|value| value.recommended_direction.clone())
            .unwrap_or_default(),
    ];

    let patterns = [
        r#"(?i)replace\s+(`[^`]+`|"[^"]+"|'[^']+')\s+with\s+(`[^`]+`|"[^"]+"|'[^']+')"#,
        r#"(?i)change\s+(`[^`]+`|"[^"]+"|'[^']+')\s+to\s+(`[^`]+`|"[^"]+"|'[^']+')"#,
        r#"(?i)use\s+(`[^`]+`|"[^"]+"|'[^']+')\s+instead of\s+(`[^`]+`|"[^"]+"|'[^']+')"#,
        r#"(?i)将\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)\s*替换为\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)"#,
        r#"(?i)把\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)\s*改成\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)"#,
        r#"(?i)使用\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)\s*而不是\s*(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)"#,
    ];

    for part in corpus_parts {
        for pattern in patterns {
            let Ok(regex) = regex::Regex::new(pattern) else {
                continue;
            };
            for capture in regex.captures_iter(&part) {
                let Some(first) = capture.get(1) else {
                    continue;
                };
                let Some(second) = capture.get(2) else {
                    continue;
                };
                let (old_value, new_value) = if pattern.contains("instead of") || pattern.contains("而不是") {
                    (trim_matching_wrappers(second.as_str()), trim_matching_wrappers(first.as_str()))
                } else {
                    (trim_matching_wrappers(first.as_str()), trim_matching_wrappers(second.as_str()))
                };
                if !old_value.is_empty() && old_value != new_value {
                    instructions.push((old_value, new_value));
                }
            }
        }
    }

    let quoted_pattern = regex::Regex::new(r#"(`[^`]+`|"[^"]+"|'[^']+'|“[^”]+”)"#).ok();
    let actual_literals = quoted_pattern
        .as_ref()
        .map(|pattern| {
            pattern
                .captures_iter(session.state.actual_behavior.as_deref().unwrap_or_default())
                .filter_map(|capture| capture.get(1))
                .map(|value| trim_matching_wrappers(value.as_str()))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expected_literals = quoted_pattern
        .as_ref()
        .map(|pattern| {
            pattern
                .captures_iter(session.state.expected_behavior.as_deref().unwrap_or_default())
                .filter_map(|capture| capture.get(1))
                .map(|value| trim_matching_wrappers(value.as_str()))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if actual_literals.len() == 1 && expected_literals.len() == 1 && actual_literals[0] != expected_literals[0] {
        instructions.push((actual_literals[0].clone(), expected_literals[0].clone()));
    }

    let mut unique = Vec::new();
    for (old_value, new_value) in instructions {
        if !unique
            .iter()
            .any(|(existing_old, existing_new)| existing_old == &old_value && existing_new == &new_value)
        {
            unique.push((old_value, new_value));
        }
    }
    unique.truncate(3);
    unique
}

fn patch_inference_corpus(session: &DebugModeSession) -> Vec<String> {
    let mut corpus = vec![
        session.state.symptom_summary.clone(),
        session.state.actual_behavior.clone().unwrap_or_default(),
        session.state.expected_behavior.clone().unwrap_or_default(),
        session.state.recent_changes.clone().unwrap_or_default(),
        session
            .state
            .selected_root_cause
            .as_ref()
            .map(|value| value.conclusion.clone())
            .unwrap_or_default(),
        session
            .state
            .selected_root_cause
            .as_ref()
            .map(|value| value.recommended_direction.clone())
            .unwrap_or_default(),
    ];

    corpus.extend(
        session
            .state
            .evidence_refs
            .iter()
            .filter(|entry| matches!(entry.kind.as_str(), "console" | "source_mapping" | "network"))
            .flat_map(|entry| [entry.title.clone(), entry.summary.clone(), entry.source.clone()]),
    );
    corpus
}

fn collect_runtime_guard_methods(session: &DebugModeSession) -> Vec<String> {
    let mut methods: Vec<String> = Vec::new();
    let property_patterns = [
        r#"(?i)cannot read properties of (?:undefined|null) \(reading ['"`]([A-Za-z_][\w$]*)['"`]\)"#,
        r#"(?i)cannot read property ['"`]([A-Za-z_][\w$]*)['"`] of (?:undefined|null)"#,
        r#"(?i)(?:undefined|null) is not an object \(evaluating ['"`][^'"`]+\.([A-Za-z_][\w$]*)['"`]\)"#,
        r#"(?i)(?:guard|fallback|optional chaining|nullish coalescing)[^.\n]*\b(map|filter|find|reduce|forEach|some|every|trim|toLowerCase|toUpperCase|split|replace|match)\b"#,
    ];

    for part in patch_inference_corpus(session) {
        for pattern in property_patterns {
            let Ok(regex) = regex::Regex::new(pattern) else {
                continue;
            };
            for capture in regex.captures_iter(&part) {
                let Some(method) = capture.get(1) else {
                    continue;
                };
                let method = method.as_str().trim();
                if method.is_empty() {
                    continue;
                }
                let already_present = methods
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(method));
                if !already_present {
                    methods.push(method.to_string());
                }
            }
        }
    }

    methods.truncate(4);
    methods
}

fn collect_runtime_guard_properties(session: &DebugModeSession) -> Vec<String> {
    let mut properties: Vec<String> = Vec::new();
    let property_patterns = [
        r#"(?i)cannot read properties of (?:undefined|null) \(reading ['"`](length)['"`]\)"#,
        r#"(?i)cannot read property ['"`](length)['"`] of (?:undefined|null)"#,
        r#"(?i)(?:undefined|null) is not an object \(evaluating ['"`][^'"`]+\.(length)['"`]\)"#,
        r#"(?i)(?:guard|fallback|optional chaining|nullish coalescing)[^.\n]*\b(length)\b"#,
    ];

    for part in patch_inference_corpus(session) {
        for pattern in property_patterns {
            let Ok(regex) = regex::Regex::new(pattern) else {
                continue;
            };
            for capture in regex.captures_iter(&part) {
                let Some(property) = capture.get(1) else {
                    continue;
                };
                let property = property.as_str().trim();
                if property.is_empty() {
                    continue;
                }
                let already_present = properties
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(property));
                if !already_present {
                    properties.push(property.to_string());
                }
            }
        }
    }

    properties.truncate(2);
    properties
}

fn tree_sitter_language_for_patch_file(file_path: &str) -> Option<tree_sitter::Language> {
    let lower = file_path.to_ascii_lowercase();
    if lower.ends_with(".tsx") || lower.ends_with(".jsx") || lower.ends_with(".js") || lower.ends_with(".mjs") {
        Some(tree_sitter_typescript::LANGUAGE_TSX.into())
    } else if lower.ends_with(".ts") || lower.ends_with(".cts") || lower.ends_with(".mts") {
        Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
    } else {
        None
    }
}

fn should_skip_guard_expression(expression: &str) -> bool {
    expression.contains("?.")
        || expression.contains("??")
        || expression.contains("||")
        || expression.starts_with("String(")
        || expression.starts_with("Array.isArray(")
}

fn collect_ast_member_guard_candidates(
    node: tree_sitter::Node,
    source: &[u8],
    method_fallbacks: &std::collections::HashMap<String, &'static str>,
    property_fallbacks: &std::collections::HashMap<String, &'static str>,
    results: &mut Vec<(String, String, String, String)>,
) {
    if node.kind() == "member_expression" {
        let object = node.child_by_field_name("object");
        let property = node.child_by_field_name("property");
        if let (Some(object), Some(property)) = (object, property) {
            let Ok(object_text) = object.utf8_text(source) else {
                return;
            };
            let Ok(property_text) = property.utf8_text(source) else {
                return;
            };
            let Ok(member_text) = node.utf8_text(source) else {
                return;
            };
            let object_text = object_text.trim();
            let property_text = property_text.trim();
            let member_text = member_text.trim();
            if object_text.is_empty() || property_text.is_empty() || member_text.is_empty() {
                return;
            }
            if should_skip_guard_expression(object_text) || member_text.contains("?.") {
                return;
            }

            if let Some(parent) = node.parent() {
                if parent.kind() == "call_expression"
                    && parent
                        .child_by_field_name("function")
                        .map(|function| function.id() == node.id())
                        .unwrap_or(false)
                {
                    if let Some(fallback_literal) = method_fallbacks.get(property_text) {
                        results.push((
                            property_text.to_string(),
                            member_text.to_string(),
                            format!("({object_text} ?? {fallback_literal}).{property_text}"),
                            object_text.to_string(),
                        ));
                    }
                }
            }

            if let Some(fallback_suffix) = property_fallbacks.get(property_text) {
                results.push((
                    property_text.to_string(),
                    member_text.to_string(),
                    format!("({object_text}{fallback_suffix})"),
                    object_text.to_string(),
                ));
            }
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_ast_member_guard_candidates(child, source, method_fallbacks, property_fallbacks, results);
        }
    }
}

fn generate_ast_runtime_guard_patch_operations(
    project_root: &Path,
    target_files: &[String],
    methods: &[String],
    properties: &[String],
) -> Vec<DebugPatchOperation> {
    if target_files.is_empty() || (methods.is_empty() && properties.is_empty()) {
        return Vec::new();
    }

    let collection_methods = ["map", "filter", "find", "reduce", "forEach", "some", "every"];
    let string_methods = ["trim", "toLowerCase", "toUpperCase", "split", "replace", "match", "includes", "slice"];

    let method_fallbacks = methods
        .iter()
        .filter_map(|method| {
            let fallback = if collection_methods.iter().any(|candidate| candidate.eq_ignore_ascii_case(method)) {
                Some("[]")
            } else if string_methods.iter().any(|candidate| candidate.eq_ignore_ascii_case(method)) {
                Some("''")
            } else {
                None
            }?;
            Some((method.to_ascii_lowercase(), fallback))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let property_fallbacks = properties
        .iter()
        .filter_map(|property| match property.to_ascii_lowercase().as_str() {
            "length" => Some((property.to_ascii_lowercase(), "?.length ?? 0")),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut operations = Vec::new();
    for property_or_method in method_fallbacks.keys().chain(property_fallbacks.keys()) {
        let mut matches = Vec::new();
        for target_file in target_files {
            let Some(language) = tree_sitter_language_for_patch_file(target_file) else {
                continue;
            };
            let full_path = project_root.join(target_file);
            let Ok(file_contents) = fs::read_to_string(&full_path) else {
                continue;
            };

            let mut parser = tree_sitter::Parser::new();
            if parser.set_language(&language).is_err() {
                continue;
            }
            let Some(tree) = parser.parse(&file_contents, None) else {
                continue;
            };

            let mut ast_candidates = Vec::new();
            collect_ast_member_guard_candidates(
                tree.root_node(),
                file_contents.as_bytes(),
                &method_fallbacks,
                &property_fallbacks,
                &mut ast_candidates,
            );

            for (captured_property, find_text, replace_text, expression) in ast_candidates {
                if !captured_property.eq_ignore_ascii_case(property_or_method) {
                    continue;
                }
                if find_text == replace_text {
                    continue;
                }
                matches.push((target_file.clone(), captured_property, find_text, replace_text, expression));
            }
        }

        if matches.len() != 1 {
            continue;
        }
        let (target_file, property, find_text, replace_text, expression) = matches.remove(0);
        let fallback_literal = method_fallbacks
            .get(&property.to_ascii_lowercase())
            .copied()
            .or_else(|| match property.to_ascii_lowercase().as_str() {
                "length" => Some("[]"),
                _ => None,
            });

        if let Some(fallback_literal) = fallback_literal {
            if !expression.contains('.') && !expression.contains('[') {
                let full_path = project_root.join(&target_file);
                if let Ok(file_contents) = fs::read_to_string(&full_path) {
                    if let Some((binding_find, binding_replace)) =
                        infer_binding_default_patch(&file_contents, &expression, fallback_literal)
                    {
                        operations.push(DebugPatchOperation {
                            id: format!("auto-ast-binding-{}", operations.len() + 1),
                            kind: "replace_text".to_string(),
                            file_path: target_file.clone(),
                            description: format!(
                                "Inject a default binding for {expression} before it reaches {property} in {target_file}"
                            ),
                            find_text: Some(binding_find),
                            replace_text: Some(binding_replace),
                            content: None,
                            create_if_missing: false,
                            expected_occurrences: Some(1),
                        });
                        continue;
                    }
                }
            }
        }

        operations.push(DebugPatchOperation {
            id: format!("auto-ast-guard-{}", operations.len() + 1),
            kind: "replace_text".to_string(),
            file_path: target_file.clone(),
            description: format!(
                "AST guard for {expression}.{property} in {target_file}"
            ),
            find_text: Some(find_text),
            replace_text: Some(replace_text),
            content: None,
            create_if_missing: false,
            expected_occurrences: Some(1),
        });
    }

    operations
}

fn infer_binding_default_patch(
    file_contents: &str,
    identifier: &str,
    fallback_literal: &str,
) -> Option<(String, String)> {
    if identifier.trim().is_empty() || identifier.contains('.') || identifier.contains('[') {
        return None;
    }

    let escaped_identifier = regex::escape(identifier);
    let patterns = [
        r#"(?s)(?P<full>(?:const|let|var)\s+\{(?P<body>[^}]*)\}\s*=\s*[^;]+;)"#,
        r#"(?s)(?P<full>function\s+[A-Za-z_$][\w$]*\s*\(\s*\{(?P<body>[^}]*)\}(?P<suffix>\s*:[^)]+)?\s*\))"#,
        r#"(?s)(?P<full>\(\s*\{(?P<body>[^}]*)\}(?P<suffix>\s*:[^)]+)?\s*\)\s*=>)"#,
    ];

    let target_pattern = regex::Regex::new(&format!(r#"(?m)(?<![\w$]){escaped_identifier}(?!\s*(?:=|:)|[\w$])"#)).ok()?;
    let already_defaulted_pattern =
        regex::Regex::new(&format!(r#"(?m)(?<![\w$]){escaped_identifier}\s*="#)).ok()?;

    let mut matches = Vec::new();
    for pattern in patterns {
        let Ok(regex) = regex::Regex::new(pattern) else {
            continue;
        };
        for capture in regex.captures_iter(file_contents) {
            let Some(full) = capture.name("full") else {
                continue;
            };
            let Some(body) = capture.name("body") else {
                continue;
            };
            let body_text = body.as_str();
            if already_defaulted_pattern.is_match(body_text) {
                continue;
            }
            let Some(target_match) = target_pattern.find(body_text) else {
                continue;
            };

            let mut replaced_body = String::new();
            replaced_body.push_str(&body_text[..target_match.start()]);
            replaced_body.push_str(&format!("{identifier} = {fallback_literal}"));
            replaced_body.push_str(&body_text[target_match.end()..]);

            let full_text = full.as_str().to_string();
            let body_start_in_full = body.start() - full.start();
            let body_end_in_full = body.end() - full.start();
            let mut replaced_full = String::new();
            replaced_full.push_str(&full_text[..body_start_in_full]);
            replaced_full.push_str(&replaced_body);
            replaced_full.push_str(&full_text[body_end_in_full..]);
            matches.push((full_text, replaced_full));
        }
    }

    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}

fn generate_runtime_guard_patch_operations(
    session: &DebugModeSession,
    project_root: &Path,
    target_files: &[String],
) -> Vec<DebugPatchOperation> {
    let collection_methods = ["map", "filter", "find", "reduce", "forEach", "some", "every"];
    let string_methods = ["trim", "toLowerCase", "toUpperCase", "split", "replace", "match"];
    let runtime_methods = collect_runtime_guard_methods(session);
    if target_files.is_empty() || runtime_methods.is_empty() {
        return Vec::new();
    }

    let mut operations = Vec::new();
    for method in runtime_methods {
        let fallback_literal = if collection_methods.iter().any(|candidate| candidate.eq_ignore_ascii_case(&method)) {
            "[]"
        } else if string_methods
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&method))
        {
            "''"
        } else {
            continue;
        };

        let pattern = format!(
            r#"(?m)(?P<full>(?P<expr>[A-Za-z_$][\w$.\[\]'"]*)\.{}\s*\()"#,
            regex::escape(&method)
        );
        let Ok(regex) = regex::Regex::new(&pattern) else {
            continue;
        };

        let mut matches = Vec::new();
        for target_file in target_files {
            let Ok(file_contents) = fs::read_to_string(project_root.join(target_file)) else {
                continue;
            };
            for capture in regex.captures_iter(&file_contents) {
                let Some(full_match) = capture.name("full") else {
                    continue;
                };
                let Some(expr_match) = capture.name("expr") else {
                    continue;
                };
                let expression = expr_match.as_str().trim();
                if expression.contains("?.")
                    || expression.contains("??")
                    || expression.contains("||")
                    || expression.starts_with("String(")
                    || expression.starts_with("Array.isArray(")
                {
                    continue;
                }
                let matched_text = full_match.as_str().to_string();
                let suffix = &matched_text[expression.len()..];
                let replacement = format!("({expression} ?? {fallback_literal}){suffix}");
                if matched_text == replacement {
                    continue;
                }
                matches.push((target_file.clone(), matched_text, replacement, expression.to_string()));
            }
        }

        if matches.len() != 1 {
            continue;
        }
        let (target_file, matched_text, replacement, expression) = matches.remove(0);
        operations.push(DebugPatchOperation {
            id: format!("auto-guard-{}", operations.len() + 1),
            kind: "replace_text".to_string(),
            file_path: target_file.clone(),
            description: format!(
                "Guard {expression}.{method} with a nullish fallback before invoking it in {target_file}"
            ),
            find_text: Some(matched_text),
            replace_text: Some(replacement),
            content: None,
            create_if_missing: false,
            expected_occurrences: Some(1),
        });
    }

    operations
}

fn generate_runtime_property_guard_patch_operations(
    session: &DebugModeSession,
    project_root: &Path,
    target_files: &[String],
) -> Vec<DebugPatchOperation> {
    let properties = collect_runtime_guard_properties(session);
    if target_files.is_empty() || properties.is_empty() {
        return Vec::new();
    }

    let mut operations = Vec::new();
    for property in properties {
        let replacement_suffix = match property.as_str() {
            "length" => "?.length ?? 0",
            _ => continue,
        };

        let pattern = format!(
            r#"(?m)(?P<full>(?P<expr>[A-Za-z_$][\w$.\[\]'"]*)\.{}\b)(?!\s*\()"#,
            regex::escape(&property)
        );
        let Ok(regex) = regex::Regex::new(&pattern) else {
            continue;
        };

        let mut matches = Vec::new();
        for target_file in target_files {
            let Ok(file_contents) = fs::read_to_string(project_root.join(target_file)) else {
                continue;
            };
            for capture in regex.captures_iter(&file_contents) {
                let Some(full_match) = capture.name("full") else {
                    continue;
                };
                let Some(expr_match) = capture.name("expr") else {
                    continue;
                };
                let expression = expr_match.as_str().trim();
                if expression.contains("?.")
                    || expression.contains("??")
                    || expression.contains("||")
                    || expression.starts_with("String(")
                    || expression.starts_with("Array.isArray(")
                {
                    continue;
                }
                let matched_text = full_match.as_str().to_string();
                let replacement = format!("({expression}{replacement_suffix})");
                if matched_text == replacement {
                    continue;
                }
                matches.push((target_file.clone(), matched_text, replacement, expression.to_string()));
            }
        }

        if matches.len() != 1 {
            continue;
        }
        let (target_file, matched_text, replacement, expression) = matches.remove(0);
        operations.push(DebugPatchOperation {
            id: format!("auto-property-guard-{}", operations.len() + 1),
            kind: "replace_text".to_string(),
            file_path: target_file.clone(),
            description: format!(
                "Guard {expression}.{property} with optional chaining and a nullish fallback in {target_file}"
            ),
            find_text: Some(matched_text),
            replace_text: Some(replacement),
            content: None,
            create_if_missing: false,
            expected_occurrences: Some(1),
        });
    }

    operations
}

fn generate_patch_operations_for_session(session: &DebugModeSession) -> Vec<DebugPatchOperation> {
    let Some(project_root) = normalize_optional_project_path(session.project_path.as_deref())
        .or_else(|| normalize_optional_project_path(session.state.project_path.as_deref()))
        .and_then(|value| PathBuf::from(value).canonicalize().ok())
    else {
        return Vec::new();
    };

    let target_files = resolve_patch_target_files(session, &project_root);
    let replacements = extract_replacement_instructions(session);
    let runtime_methods = collect_runtime_guard_methods(session);
    let runtime_properties = collect_runtime_guard_properties(session);
    if target_files.is_empty() {
        return Vec::new();
    }

    let mut operations = Vec::new();
    for (index, (old_value, new_value)) in replacements.into_iter().enumerate() {
        let mut matching_files = Vec::new();
        for target_file in &target_files {
            let Ok(file_contents) = fs::read_to_string(project_root.join(target_file)) else {
                continue;
            };
            let occurrences = file_contents.matches(&old_value).count();
            if occurrences > 0 {
                matching_files.push((target_file.clone(), occurrences));
            }
        }
        if matching_files.len() != 1 {
            continue;
        }
        let (target_file, occurrences) = matching_files.remove(0);
        operations.push(DebugPatchOperation {
            id: format!("auto-replace-{}", index + 1),
            kind: "replace_text".to_string(),
            file_path: target_file.clone(),
            description: format!("Replace '{}' with '{}' in {}", old_value, new_value, target_file),
            find_text: Some(old_value),
            replace_text: Some(new_value),
            content: None,
            create_if_missing: false,
            expected_occurrences: Some(occurrences),
        });
    }

    for operation in generate_ast_runtime_guard_patch_operations(
        &project_root,
        &target_files,
        &runtime_methods,
        &runtime_properties,
    ) {
        let duplicate = operations.iter().any(|existing| {
            existing.kind == operation.kind
                && existing.file_path == operation.file_path
                && existing.find_text == operation.find_text
                && existing.replace_text == operation.replace_text
        });
        if !duplicate {
            operations.push(operation);
        }
    }

    for operation in generate_runtime_guard_patch_operations(session, &project_root, &target_files) {
        let duplicate = operations.iter().any(|existing| {
            existing.kind == operation.kind
                && existing.file_path == operation.file_path
                && existing.find_text == operation.find_text
                && existing.replace_text == operation.replace_text
        });
        if !duplicate {
            operations.push(operation);
        }
    }

    for operation in generate_runtime_property_guard_patch_operations(session, &project_root, &target_files) {
        let duplicate = operations.iter().any(|existing| {
            existing.kind == operation.kind
                && existing.file_path == operation.file_path
                && existing.find_text == operation.find_text
                && existing.replace_text == operation.replace_text
        });
        if !duplicate {
            operations.push(operation);
        }
    }

    operations.truncate(5);
    operations
}

fn ensure_debug_artifact_dir(state: &DebugModeState, session_id: &str) -> Result<PathBuf, String> {
    let dir = state.artifact_dir_path(session_id);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create debug artifact directory for '{session_id}': {error}"))?;
    Ok(dir)
}

fn sanitize_debug_artifact_contents(
    session_id: &str,
    file_name: &str,
    contents: &[u8],
) -> Result<Vec<u8>, String> {
    let Ok(text) = std::str::from_utf8(contents) else {
        return Ok(contents.to_vec());
    };

    let registry = crate::services::guardrail::shared_guardrail_registry();
    let runtime = crate::services::guardrail::GuardrailRuntimeContext {
        session_id: Some(session_id.to_string()),
        execution_id: Some(session_id.to_string()),
        tool_name: None,
        content_kind: None,
    };
    let future = async {
        registry
            .read()
            .await
            .validate_all(text, crate::services::guardrail::Direction::Artifact, &runtime)
            .await
    };
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tauri::async_runtime::block_on(future),
    };

    match result {
        crate::services::guardrail::GuardrailResult::Pass
        | crate::services::guardrail::GuardrailResult::Warn { .. } => Ok(contents.to_vec()),
        crate::services::guardrail::GuardrailResult::Redact {
            redacted_content, ..
        } => Ok(redacted_content.into_bytes()),
        crate::services::guardrail::GuardrailResult::Block { reason } => Err(format!(
            "Debug artifact '{}' blocked by guardrail: {}",
            file_name, reason
        )),
    }
}

fn write_debug_artifact_file(
    state: &DebugModeState,
    session_id: &str,
    file_name: &str,
    contents: impl AsRef<[u8]>,
) -> Result<String, String> {
    let dir = ensure_debug_artifact_dir(state, session_id)?;
    let path = dir.join(file_name);
    let sanitized = sanitize_debug_artifact_contents(session_id, file_name, contents.as_ref())?;
    fs::write(&path, sanitized)
        .map_err(|error| format!("Failed to write debug artifact '{file_name}' for '{session_id}': {error}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppliedPatchOperationRecord {
    id: String,
    kind: String,
    file_path: String,
    description: String,
    bytes_written: usize,
    backup_artifact_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PatchApplicationReport {
    case_id: Option<String>,
    session_id: String,
    project_path: String,
    applied_at: String,
    operation_count: usize,
    operations: Vec<AppliedPatchOperationRecord>,
}

fn normalize_optional_project_path(value: Option<&str>) -> Option<String> {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resolve_debug_project_root(
    session: &DebugModeSession,
    request_project_path: Option<&str>,
) -> Result<PathBuf, String> {
    let raw = normalize_optional_project_path(request_project_path)
        .or_else(|| normalize_optional_project_path(session.project_path.as_deref()))
        .or_else(|| normalize_optional_project_path(session.state.project_path.as_deref()))
        .ok_or_else(|| "Debug patching requires a project path.".to_string())?;
    let root = PathBuf::from(&raw);
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("Invalid debug project path '{raw}': {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("Debug project path '{raw}' is not a directory."));
    }
    Ok(canonical)
}

fn resolve_debug_project_file_path(
    project_root: &Path,
    file_path: &str,
    create_if_missing: bool,
) -> Result<PathBuf, String> {
    let relative = file_path.trim();
    if relative.is_empty() {
        return Err("Patch operation file path cannot be empty.".to_string());
    }
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(format!("Patch operation path must be relative: {relative}"));
    }
    if relative_path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("Patch operation path cannot escape project root: {relative}"));
    }

    let candidate = project_root.join(relative_path);
    let canonical_guard = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|error| format!("Invalid patch target '{relative}': {error}"))?
    } else if create_if_missing {
        let mut existing_ancestor = candidate.parent().map(PathBuf::from);
        loop {
            let Some(current) = existing_ancestor.clone() else {
                break project_root.to_path_buf();
            };
            if current.exists() {
                break current
                    .canonicalize()
                    .map_err(|error| format!("Patch target parent for '{relative}' is invalid: {error}"))?;
            }
            existing_ancestor = current.parent().map(PathBuf::from);
        }
    } else {
        return Err(format!("Patch target does not exist: {relative}"));
    };

    if !canonical_guard.starts_with(project_root) {
        return Err(format!("Patch target escapes project root: {relative}"));
    }
    Ok(candidate)
}

fn infer_debug_artifact_content_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("md") => "text/markdown".to_string(),
        Some("json") => "application/json".to_string(),
        Some("txt") | Some("log") => "text/plain".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

fn infer_debug_artifact_kind(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name.contains("patch-preview") {
        "patch_preview".to_string()
    } else if file_name.contains("patch-application") {
        "patch_application".to_string()
    } else if file_name.contains("verification-report") {
        "verification_report".to_string()
    } else if file_name.contains("incident-summary") {
        "incident_summary".to_string()
    } else if file_name.contains("backup-") {
        "patch_backup".to_string()
    } else {
        "artifact".to_string()
    }
}

fn infer_debug_artifact_description(kind: &str, file_name: &str) -> String {
    match kind {
        "patch_preview" => format!("Patch preview artifact ({file_name})"),
        "patch_application" => format!("Patch application report ({file_name})"),
        "verification_report" => format!("Verification report ({file_name})"),
        "incident_summary" => format!("Incident summary ({file_name})"),
        "patch_backup" => format!("Backup captured before patching ({file_name})"),
        _ => format!("Debug artifact ({file_name})"),
    }
}

fn build_debug_artifact_descriptor(path: &Path) -> Result<DebugArtifactDescriptor, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect debug artifact '{}': {error}", path.display()))?;
    let modified = metadata.modified().ok().map(chrono::DateTime::<chrono::Utc>::from);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let kind = infer_debug_artifact_kind(path);
    Ok(DebugArtifactDescriptor {
        path: path.to_string_lossy().to_string(),
        file_name: file_name.clone(),
        kind: kind.clone(),
        content_type: infer_debug_artifact_content_type(path),
        size_bytes: metadata.len(),
        updated_at: modified
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(now_rfc3339),
        description: infer_debug_artifact_description(&kind, &file_name),
    })
}

fn collect_debug_artifact_paths(state: &DebugModeState, session: &DebugModeSession) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut push_path = |path_str: &str| {
        let path = PathBuf::from(path_str);
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    };

    if let Some(proposal) = session.state.fix_proposal.as_ref() {
        if let Some(path) = proposal.patch_preview_ref.as_deref() {
            push_path(path);
        }
    }
    if let Some(report) = session.state.verification_report.as_ref() {
        for artifact in &report.artifacts {
            if artifact.starts_with('/') {
                push_path(artifact);
            }
        }
    }
    for evidence in &session.state.evidence_refs {
        if let Some(path) = evidence.metadata.get("artifactPath").and_then(|value| value.as_str()) {
            push_path(path);
        }
    }

    let dir = state.artifact_dir_path(&session.session_id);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
    }
    paths.sort_by(|left, right| right.cmp(left));
    paths
}

fn apply_debug_patch_operations(
    state: &DebugModeState,
    session: &DebugModeSession,
    project_root: &Path,
    operations: &[DebugPatchOperation],
    file_change_tracker: Option<&std::sync::Arc<std::sync::Mutex<crate::services::file_change_tracker::FileChangeTracker>>>,
    file_change_turn_index: Option<u32>,
) -> Result<(PatchApplicationReport, String), String> {
    if operations.is_empty() {
        return Err("No executable patch operations were attached to this fix proposal.".to_string());
    }

    let mut applied_operations = Vec::new();
    for (index, operation) in operations.iter().enumerate() {
        let target_path = resolve_debug_project_file_path(
            project_root,
            &operation.file_path,
            operation.create_if_missing || operation.kind == "write_file",
        )?;
        let previous_contents = if target_path.exists() {
            Some(
                fs::read_to_string(&target_path).map_err(|error| {
                    format!(
                        "Failed to read patch target '{}': {error}",
                        target_path.to_string_lossy()
                    )
                })?,
            )
        } else {
            None
        };

        let next_contents = match operation.kind.as_str() {
            "replace_text" => {
                let current = previous_contents
                    .clone()
                    .ok_or_else(|| format!("Patch target '{}' does not exist.", operation.file_path))?;
                let find_text = operation
                    .find_text
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| format!("Patch operation '{}' is missing findText.", operation.id))?;
                let replace_text = operation.replace_text.as_deref().unwrap_or_default();
                let occurrences = current.matches(find_text).count();
                if occurrences == 0 {
                    return Err(format!(
                        "Patch operation '{}' could not find the expected text in '{}'.",
                        operation.id, operation.file_path
                    ));
                }
                if let Some(expected) = operation.expected_occurrences {
                    if occurrences != expected {
                        return Err(format!(
                            "Patch operation '{}' expected {} occurrences in '{}' but found {}.",
                            operation.id, expected, operation.file_path, occurrences
                        ));
                    }
                }
                current.replacen(find_text, replace_text, operation.expected_occurrences.unwrap_or(occurrences))
            }
            "write_file" => operation
                .content
                .clone()
                .ok_or_else(|| format!("Patch operation '{}' is missing content.", operation.id))?,
            other => {
                return Err(format!(
                    "Unsupported patch operation kind '{}' for '{}'.",
                    other, operation.file_path
                ))
            }
        };

        let backup_artifact_path = if let Some(original) = previous_contents.as_ref() {
            let backup_name = format!(
                "backup-{:02}-{}.txt",
                index + 1,
                sanitize_artifact_slug(&operation.file_path.replace('/', "-"))
            );
            Some(write_debug_artifact_file(
                state,
                &session.session_id,
                &backup_name,
                original.as_bytes(),
            )?)
        } else {
            None
        };

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create directories for '{}': {error}",
                    target_path.to_string_lossy()
                )
            })?;
        }
        fs::write(&target_path, next_contents.as_bytes()).map_err(|error| {
            format!(
                "Failed to write patch target '{}': {error}",
                target_path.to_string_lossy()
            )
        })?;

        if let Some(tracker) = file_change_tracker {
            if let Ok(mut tracker_guard) = tracker.lock() {
                let before_hash = previous_contents
                    .as_ref()
                    .and_then(|value| tracker_guard.store_content(value.as_bytes()).ok());
                let after_hash = tracker_guard.store_content(next_contents.as_bytes()).ok();
                let metadata = crate::services::file_change_tracker::FileChangeMetadata {
                    source_mode: Some(crate::services::file_change_tracker::FileChangeSourceMode::Debug),
                    actor_kind: Some(crate::services::file_change_tracker::FileChangeActorKind::DebugPatch),
                    actor_id: Some(operation.id.clone()),
                    actor_label: Some("Debug Patch".to_string()),
                    sub_agent_depth: None,
                    origin_session_id: session.kernel_session_id.clone().or_else(|| Some(session.session_id.clone())),
                };
                let turn_index = file_change_turn_index.unwrap_or_else(|| tracker_guard.turn_index());
                tracker_guard.record_change_at_with_metadata(
                    turn_index,
                    &format!("debug-patch-{}-{}", operation.id, index),
                    "DebugPatch",
                    &operation.file_path,
                    before_hash,
                    after_hash.as_deref(),
                    &format!("{} {}", operation.id, operation.kind),
                    Some(&metadata),
                );
            }
        }

        applied_operations.push(AppliedPatchOperationRecord {
            id: if operation.id.trim().is_empty() {
                format!("patch-op-{}", index + 1)
            } else {
                operation.id.clone()
            },
            kind: operation.kind.clone(),
            file_path: operation.file_path.clone(),
            description: operation.description.clone(),
            bytes_written: next_contents.len(),
            backup_artifact_path,
        });
    }

    let report = PatchApplicationReport {
        case_id: session.state.case_id.clone(),
        session_id: session.session_id.clone(),
        project_path: project_root.to_string_lossy().to_string(),
        applied_at: now_rfc3339(),
        operation_count: applied_operations.len(),
        operations: applied_operations,
    };
    let file_name = format!(
        "patch-application-{}.json",
        sanitize_artifact_slug(
            session
                .state
                .case_id
                .clone()
                .unwrap_or_else(|| session.session_id.clone())
                .as_str()
        )
    );
    let encoded = serde_json::to_vec_pretty(&report).map_err(|error| {
        format!(
            "Failed to encode patch application report for '{}': {error}",
            session.session_id
        )
    })?;
    let artifact_path = write_debug_artifact_file(state, &session.session_id, &file_name, encoded)?;
    Ok((report, artifact_path))
}

fn build_patch_preview_markdown(session: &DebugModeSession, proposal: &FixProposal) -> String {
    let root_cause = session
        .state
        .selected_root_cause
        .as_ref()
        .map(|root| root.conclusion.as_str())
        .unwrap_or("Root cause is still being refined.");
    let affected_surface = if session.state.affected_surface.is_empty() {
        "- reported surface".to_string()
    } else {
        session
            .state
            .affected_surface
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let verification_plan = if proposal.verification_plan.is_empty() {
        "- Re-run the reported failure path".to_string()
    } else {
        proposal
            .verification_plan
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let files = if proposal.files_or_systems_touched.is_empty() {
        "- reported surface".to_string()
    } else {
        proposal
            .files_or_systems_touched
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let patch_operations = if proposal.patch_operations.is_empty() {
        "- No executable patch operations are attached yet.".to_string()
    } else {
        proposal
            .patch_operations
            .iter()
            .map(|operation| {
                let mut line = format!("- [{}] {} ({})", operation.kind, operation.file_path, operation.description);
                if let Some(find_text) = operation.find_text.as_deref() {
                    line.push_str(&format!("\n  find: `{}`", find_text.replace('`', "\\`")));
                }
                if let Some(replace_text) = operation.replace_text.as_deref() {
                    line.push_str(&format!("\n  replace: `{}`", replace_text.replace('`', "\\`")));
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "# Debug Patch Preview\n\n\
Case: {case_id}\n\
Title: {title}\n\
Environment: {environment}\n\
Severity: {severity}\n\n\
## Symptom\n{symptom}\n\n\
## Root Cause\n{root_cause}\n\n\
## Proposed Fix\n{summary}\n\n\
## Files or Systems Touched\n{files}\n\n\
## Patch Operations\n{patch_operations}\n\n\
## Affected Surface\n{affected_surface}\n\n\
## Verification Plan\n{verification_plan}\n",
        case_id = session
            .state
            .case_id
            .clone()
            .unwrap_or_else(|| session.session_id.clone()),
        title = session
            .state
            .title
            .clone()
            .unwrap_or_else(|| "Debug case".to_string()),
        environment = debug_environment_as_str(session.state.environment),
        severity = debug_severity_as_str(session.state.severity),
        symptom = session.state.symptom_summary,
        root_cause = root_cause,
        summary = proposal.summary,
        files = files,
        patch_operations = patch_operations,
        affected_surface = affected_surface,
        verification_plan = verification_plan,
    )
}

fn persist_fix_proposal_artifact(
    state: &DebugModeState,
    session: &DebugModeSession,
    proposal: &mut FixProposal,
) -> Result<Option<String>, String> {
    let slug = sanitize_artifact_slug(
        proposal
            .summary
            .split_whitespace()
            .take(6)
            .collect::<Vec<_>>()
            .join("-")
            .as_str(),
    );
    let file_name = format!("patch-preview-{slug}.md");
    let path = write_debug_artifact_file(
        state,
        &session.session_id,
        &file_name,
        build_patch_preview_markdown(session, proposal),
    )?;
    proposal.patch_preview_ref = Some(path.clone());
    Ok(Some(path))
}

fn persist_incident_summary_artifact(
    state: &DebugModeState,
    session: &DebugModeSession,
) -> Result<String, String> {
    let verification_summary = session
        .state
        .verification_report
        .as_ref()
        .map(|report| report.summary.as_str())
        .unwrap_or("Verification has not completed.");
    let root_cause = session
        .state
        .selected_root_cause
        .as_ref()
        .map(|root| root.conclusion.as_str())
        .unwrap_or("Root cause still under investigation.");
    let contents = format!(
        "# Debug Incident Summary\n\n\
Case: {case_id}\n\
Title: {title}\n\
Phase: {phase}\n\
Environment: {environment}\n\
Severity: {severity}\n\n\
## Symptom\n{symptom}\n\n\
## Root Cause\n{root_cause}\n\n\
## Verification\n{verification_summary}\n",
        case_id = session
            .state
            .case_id
            .clone()
            .unwrap_or_else(|| session.session_id.clone()),
        title = session
            .state
            .title
            .clone()
            .unwrap_or_else(|| "Debug case".to_string()),
        phase = session.state.phase,
        environment = debug_environment_as_str(session.state.environment),
        severity = debug_severity_as_str(session.state.severity),
        symptom = session.state.symptom_summary,
        root_cause = root_cause,
        verification_summary = verification_summary,
    );
    write_debug_artifact_file(state, &session.session_id, "incident-summary.md", contents)
}

fn persist_verification_report_artifact(
    state: &DebugModeState,
    session: &DebugModeSession,
    report: &VerificationReport,
) -> Result<String, String> {
    let encoded = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("Failed to encode verification report for '{}': {error}", session.session_id))?;
    write_debug_artifact_file(state, &session.session_id, "verification-report.json", encoded)
}

fn materialize_debug_artifacts(
    state: &DebugModeState,
    session: &mut DebugModeSession,
) -> Result<(), String> {
    let session_snapshot = session.clone();
    if let Some(proposal) = session.state.fix_proposal.as_mut() {
        let patch_path = persist_fix_proposal_artifact(state, &session_snapshot, proposal)?;
        if let Some(path) = patch_path {
            let has_patch_evidence = session.state.evidence_refs.iter().any(|entry| {
                entry.kind == "patch_preview"
                    && entry
                        .metadata
                        .get("artifactPath")
                        .and_then(|value| value.as_str())
                        .map(|existing| existing == path)
                        .unwrap_or(false)
            });
            if !has_patch_evidence {
                let mut metadata = serde_json::Map::new();
                metadata.insert("artifactPath".to_string(), serde_json::Value::String(path.clone()));
                session.state.evidence_refs.push(DebugEvidenceRef {
                    id: uuid::Uuid::new_v4().to_string(),
                    kind: "patch_preview".to_string(),
                    title: "Patch preview generated".to_string(),
                    summary: format!("Patch preview saved to {path}"),
                    source: "system".to_string(),
                    created_at: now_rfc3339(),
                    metadata,
                });
            }
        }
    }

    let verification_snapshot = session.clone();
    if let Some(report) = session.state.verification_report.as_mut() {
        let report_path = persist_verification_report_artifact(state, &verification_snapshot, report)?;
        if !report.artifacts.iter().any(|artifact| artifact == &report_path) {
            report.artifacts.push(report_path.clone());
        }
        let incident_path = persist_incident_summary_artifact(state, &verification_snapshot)?;
        if !report.artifacts.iter().any(|artifact| artifact == &incident_path) {
            report.artifacts.push(incident_path.clone());
        }
        let has_verification_evidence = session.state.evidence_refs.iter().any(|entry| {
            entry.kind == "verification_artifact"
                && entry
                    .metadata
                    .get("artifactPath")
                    .and_then(|value| value.as_str())
                    .map(|existing| existing == report_path)
                    .unwrap_or(false)
        });
        if !has_verification_evidence {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "artifactPath".to_string(),
                serde_json::Value::String(report_path.clone()),
            );
            session.state.evidence_refs.push(DebugEvidenceRef {
                id: uuid::Uuid::new_v4().to_string(),
                kind: "verification_artifact".to_string(),
                title: "Verification report generated".to_string(),
                summary: format!("Verification report saved to {report_path}"),
                source: "system".to_string(),
                created_at: now_rfc3339(),
                metadata,
            });
        }
    } else if session.state.selected_root_cause.is_some() || session.state.fix_proposal.is_some() {
        let incident_path = persist_incident_summary_artifact(state, session)?;
        let has_summary_evidence = session.state.evidence_refs.iter().any(|entry| {
            entry.kind == "incident_summary"
                && entry
                    .metadata
                    .get("artifactPath")
                    .and_then(|value| value.as_str())
                    .map(|existing| existing == incident_path)
                    .unwrap_or(false)
        });
        if !has_summary_evidence {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "artifactPath".to_string(),
                serde_json::Value::String(incident_path.clone()),
            );
            session.state.evidence_refs.push(DebugEvidenceRef {
                id: uuid::Uuid::new_v4().to_string(),
                kind: "incident_summary".to_string(),
                title: "Incident summary generated".to_string(),
                summary: format!("Incident summary saved to {incident_path}"),
                source: "system".to_string(),
                created_at: now_rfc3339(),
                metadata,
            });
        }
    }

    Ok(())
}

fn build_verification_report(session: &DebugModeSession) -> VerificationReport {
    let patch_preview_ref = session
        .state
        .fix_proposal
        .as_ref()
        .and_then(|proposal| proposal.patch_preview_ref.clone());
    let patch_application_artifact = session
        .state
        .evidence_refs
        .iter()
        .rev()
        .find(|entry| entry.kind == "patch")
        .and_then(|entry| metadata_string(&entry.metadata, "artifactPath"));
    let repro_label = if !session.state.repro_steps.is_empty() {
        format!("Reproduction path: {}", session.state.repro_steps.join(" -> "))
    } else if let Some(target) = session.state.target_url_or_entry.clone() {
        format!("Target path: {target}")
    } else {
        "Reported failure path".to_string()
    };

    let verification_console = latest_evidence_by_kind(session, "console", Some("verification"));
    let baseline_console = latest_evidence_by_kind(session, "console", None);
    let verification_network = latest_evidence_by_kind(session, "network", Some("verification"));
    let baseline_network = latest_evidence_by_kind(session, "network", None);
    let latest_logs = latest_evidence_by_kind(session, "logs", None);
    let latest_test = latest_evidence_by_kind(session, "manual", Some("verification"))
        .filter(|entry| entry.source.contains("test"));

    let mut checks = Vec::new();

    checks.push(VerificationCheck {
        id: "patch".to_string(),
        label: "Patch preview".to_string(),
        status: if patch_application_artifact.is_some() || patch_preview_ref.is_some() {
            "passed"
        } else {
            "skipped"
        }
        .to_string(),
        details: patch_application_artifact
            .clone()
            .map(|path| format!("Patch application artifact saved at {path}."))
            .or_else(|| {
                patch_preview_ref
                    .clone()
                    .map(|path| format!("Patch preview artifact saved at {path}."))
            })
            .or_else(|| Some("No patch preview artifact is available yet.".to_string())),
    });

    checks.push(VerificationCheck {
        id: "repro".to_string(),
        label: repro_label,
        status: if !session.state.repro_steps.is_empty() || session.state.target_url_or_entry.is_some() {
            "passed"
        } else {
            "skipped"
        }
        .to_string(),
        details: if !session.state.repro_steps.is_empty() || session.state.target_url_or_entry.is_some() {
            Some("Verification kept the original failure path in scope.".to_string())
        } else {
            Some("No stable reproduction path was attached for post-patch verification.".to_string())
        },
    });

    checks.push(if let Some(entry) = verification_console {
        let blocking_count = metadata_u64(&entry.metadata, "blockingEntryCount").unwrap_or(0);
        let entry_count = metadata_u64(&entry.metadata, "entryCount").unwrap_or(0);
        let current_url = metadata_string(&entry.metadata, "currentUrl").unwrap_or_else(|| entry.summary.clone());
        VerificationCheck {
            id: "console".to_string(),
            label: "Browser console recheck".to_string(),
            status: if blocking_count > 0 { "failed" } else { "passed" }.to_string(),
            details: Some(format!(
                "Verification capture from {current_url} recorded {entry_count} console entries and {blocking_count} blocking entries."
            )),
        }
    } else if baseline_console.is_some() {
        VerificationCheck {
            id: "console".to_string(),
            label: "Browser console recheck".to_string(),
            status: "skipped".to_string(),
            details: Some("Only baseline console evidence is available. Re-run verification capture after patching to confirm the console is clean.".to_string()),
        }
    } else {
        VerificationCheck {
            id: "console".to_string(),
            label: "Browser console recheck".to_string(),
            status: "skipped".to_string(),
            details: Some("No browser console evidence was attached.".to_string()),
        }
    });

    checks.push(if let Some(entry) = verification_network {
        let failed_count = metadata_u64(&entry.metadata, "failedEventCount").unwrap_or(0);
        let total_count = metadata_u64(&entry.metadata, "totalEventCount").unwrap_or(0);
        let current_url = metadata_string(&entry.metadata, "currentUrl").unwrap_or_else(|| entry.summary.clone());
        VerificationCheck {
            id: "network".to_string(),
            label: "Network recheck".to_string(),
            status: if failed_count > 0 { "failed" } else { "passed" }.to_string(),
            details: Some(format!(
                "Verification capture from {current_url} recorded {total_count} network events with {failed_count} failures."
            )),
        }
    } else if baseline_network.is_some() {
        VerificationCheck {
            id: "network".to_string(),
            label: "Network recheck".to_string(),
            status: "skipped".to_string(),
            details: Some("Only baseline network evidence is available. Re-run verification capture after patching to confirm request health.".to_string()),
        }
    } else {
        VerificationCheck {
            id: "network".to_string(),
            label: "Network recheck".to_string(),
            status: "skipped".to_string(),
            details: Some("No network evidence was attached.".to_string()),
        }
    });

    checks.push(if let Some(entry) = latest_logs {
        VerificationCheck {
            id: "logs".to_string(),
            label: "Logs / service evidence".to_string(),
            status: "passed".to_string(),
            details: Some(format!("Latest log evidence: {}", entry.title)),
        }
    } else {
        VerificationCheck {
            id: "logs".to_string(),
            label: "Logs / service evidence".to_string(),
            status: "skipped".to_string(),
            details: Some("No log or service-side verification evidence was attached.".to_string()),
        }
    });

    if let Some(entry) = latest_test {
        checks.push(VerificationCheck {
            id: "tests".to_string(),
            label: "Targeted automated verification".to_string(),
            status: "passed".to_string(),
            details: Some(format!("Verification used test evidence: {}", entry.title)),
        });
    }

    let failed_count = checks.iter().filter(|check| check.status == "failed").count();
    let passed_count = checks.iter().filter(|check| check.status == "passed").count();
    let skipped_count = checks.iter().filter(|check| check.status == "skipped").count();

    let mut residual_risks = Vec::new();
    if verification_console.is_none() && session.state.target_url_or_entry.is_some() {
        residual_risks.push("Browser console was not re-captured after patch approval.".to_string());
    }
    if verification_network.is_none() && session.state.target_url_or_entry.is_some() {
        residual_risks.push("Network traces were not re-captured after patch approval.".to_string());
    }
    if latest_logs.is_none() {
        residual_risks.push("No service-side log evidence was attached during verification.".to_string());
    }
    if failed_count > 0 {
        residual_risks.push("One or more verification checks still report blocking signals.".to_string());
    }
    if matches!(session.state.environment, DebugEnvironment::Prod) {
        residual_risks.push("Production rollout remains manual because mutate actions stay blocked.".to_string());
    }
    if residual_risks.is_empty() {
        residual_risks.push("Monitor adjacent surfaces for related regressions.".to_string());
    }

    let summary = if failed_count > 0 {
        format!(
            "Verification completed with {failed_count} failing checks, {passed_count} passing checks, and {skipped_count} skipped checks."
        )
    } else {
        format!(
            "Verification completed with {passed_count} passing checks and {skipped_count} skipped checks."
        )
    };

    let mut artifacts = Vec::new();
    if let Some(path) = patch_application_artifact.or(patch_preview_ref) {
        artifacts.push(path);
    }
    artifacts.push(format!(
        "verification:{}",
        session
            .state
            .case_id
            .clone()
            .unwrap_or_else(|| "debug".to_string())
    ));

    VerificationReport {
        summary,
        checks,
        residual_risks,
        artifacts,
    }
}

fn evidence_kind_from_source(source: &str) -> String {
    let normalized = source.to_ascii_lowercase();
    if normalized.contains("console") {
        "console".to_string()
    } else if normalized.contains("network") {
        "network".to_string()
    } else if normalized.contains("source_mapping") || normalized.contains("sourcemap") {
        "source_mapping".to_string()
    } else if normalized.contains("browser") {
        "browser".to_string()
    } else if normalized.contains("log") {
        "logs".to_string()
    } else if normalized.contains("trace") {
        "trace".to_string()
    } else if normalized.contains("db") || normalized.contains("database") {
        "database".to_string()
    } else if normalized.contains("cache") || normalized.contains("redis") {
        "cache".to_string()
    } else if normalized.contains("user") {
        "clarification".to_string()
    } else {
        "manual".to_string()
    }
}

fn evidence_summary_corpus(session: &DebugModeSession) -> String {
    let mut chunks = vec![
        session.state.symptom_summary.clone(),
        session.state.actual_behavior.clone().unwrap_or_default(),
        session.state.expected_behavior.clone().unwrap_or_default(),
        session.state.recent_changes.clone().unwrap_or_default(),
        session.state.target_url_or_entry.clone().unwrap_or_default(),
    ];
    chunks.extend(
        session
            .state
            .evidence_refs
            .iter()
            .flat_map(|entry| [entry.title.clone(), entry.summary.clone(), entry.source.clone()]),
    );
    chunks.join("\n").to_ascii_lowercase()
}

fn has_any_token(haystack: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|token| haystack.contains(token))
}

fn top_supporting_evidence_ids(session: &DebugModeSession, limit: usize) -> Vec<String> {
    session
        .state
        .evidence_refs
        .iter()
        .rev()
        .take(limit)
        .map(|entry| entry.id.clone())
        .collect()
}

fn derive_debug_hypotheses(session: &DebugModeSession) -> Vec<DebugHypothesis> {
    let corpus = evidence_summary_corpus(session);
    let evidence_count = session.state.evidence_refs.len();
    let supporting_ids = top_supporting_evidence_ids(session, 4);
    let mut hypotheses = Vec::new();

    if session.state.target_url_or_entry.is_some()
        || has_any_token(&corpus, &["frontend", "browser", "console", "hydration", "render"])
    {
        hypotheses.push(DebugHypothesis {
            id: uuid::Uuid::new_v4().to_string(),
            statement: "The failure is likely caused by a client-side regression on the affected route or component boundary.".to_string(),
            confidence: (0.52 + (evidence_count as f64 * 0.06)).min(0.88),
            supporting_evidence_ids: supporting_ids.clone(),
            contradicting_evidence_ids: Vec::new(),
            next_checks: vec![
                "Compare the failing route or component against recent frontend changes.".to_string(),
                "Validate browser console, network, and source-map hints on the affected page.".to_string(),
            ],
            status: if evidence_count >= 3 {
                "testing".to_string()
            } else {
                "candidate".to_string()
            },
        });
    }

    if has_any_token(&corpus, &["fetch", "request", "api", "network", "500", "404", "timeout"]) {
        hypotheses.push(DebugHypothesis {
            id: uuid::Uuid::new_v4().to_string(),
            statement: "The incident may be driven by an upstream API or network failure rather than a purely local UI defect.".to_string(),
            confidence: (0.5 + (evidence_count as f64 * 0.05)).min(0.84),
            supporting_evidence_ids: supporting_ids.clone(),
            contradicting_evidence_ids: Vec::new(),
            next_checks: vec![
                "Inspect failed requests, status codes, and backend logs for the same path.".to_string(),
                "Verify whether the issue reproduces with stable test data or mocked responses.".to_string(),
            ],
            status: "candidate".to_string(),
        });
    }

    if has_any_token(
        &corpus,
        &["redis", "cache", "db", "database", "query", "sql", "migration"],
    ) {
        hypotheses.push(DebugHypothesis {
            id: uuid::Uuid::new_v4().to_string(),
            statement: "The evidence suggests a data, cache, or persistence-layer inconsistency behind the visible symptom.".to_string(),
            confidence: (0.48 + (evidence_count as f64 * 0.05)).min(0.8),
            supporting_evidence_ids: supporting_ids.clone(),
            contradicting_evidence_ids: Vec::new(),
            next_checks: vec![
                "Check the affected records, cache keys, or derived state for the failing scenario.".to_string(),
                "Compare current data shape against recent migrations or rollout changes.".to_string(),
            ],
            status: "candidate".to_string(),
        });
    }

    if hypotheses.is_empty() {
        hypotheses.push(DebugHypothesis {
            id: uuid::Uuid::new_v4().to_string(),
            statement: "The issue appears to be a localized regression near the reported surface, but more evidence is needed to isolate the exact failure path.".to_string(),
            confidence: (0.42 + (evidence_count as f64 * 0.04)).min(0.72),
            supporting_evidence_ids: supporting_ids,
            contradicting_evidence_ids: Vec::new(),
            next_checks: vec![
                "Collect reproduction steps, logs, or browser evidence from the failing path.".to_string(),
                "Narrow the regression to one component, service, or dependency boundary.".to_string(),
            ],
            status: "candidate".to_string(),
        });
    }

    hypotheses.truncate(3);
    hypotheses
}

fn derive_root_cause(session: &DebugModeSession) -> Option<RootCauseReport> {
    let best = session
        .state
        .active_hypotheses
        .iter()
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))?;
    let evidence_count = session.state.evidence_refs.len();
    if evidence_count < 3 {
        return None;
    }

    let mut contradictions = Vec::new();
    if session.state.repro_steps.is_empty() && session.state.target_url_or_entry.is_none() {
        contradictions.push("Reproduction detail is still incomplete.".to_string());
    }

    let impact_scope = if session.state.affected_surface.is_empty() {
        infer_affected_surface_from_text(
            &session.state.symptom_summary,
            session.state.target_url_or_entry.as_deref(),
        )
    } else {
        session.state.affected_surface.clone()
    };

    Some(RootCauseReport {
        conclusion: format!(
            "Current evidence most strongly supports this root cause: {}",
            best.statement
        ),
        supporting_evidence_ids: best.supporting_evidence_ids.clone(),
        contradictions,
        confidence: (best.confidence + 0.08).min(0.93),
        impact_scope: if impact_scope.is_empty() {
            vec!["reported surface".to_string()]
        } else {
            impact_scope
        },
        recommended_direction: best
            .next_checks
            .first()
            .cloned()
            .unwrap_or_else(|| "Prepare the smallest fix that addresses the confirmed boundary.".to_string()),
    })
}

fn derive_fix_proposal_for_session(session: &DebugModeSession, root_cause: &RootCauseReport) -> FixProposal {
    let mut proposal = default_fix_proposal(&session.state.symptom_summary);
    proposal.summary = format!(
        "Prepare a minimal fix for the confirmed boundary: {}",
        root_cause.recommended_direction
    );
    proposal.change_scope = vec![
        "minimal patch".to_string(),
        "targeted verification".to_string(),
        match session.state.environment {
            DebugEnvironment::Dev => "dev",
            DebugEnvironment::Staging => "staging",
            DebugEnvironment::Prod => "prod",
        }
        .to_string(),
    ];
    proposal.files_or_systems_touched = if root_cause.impact_scope.is_empty() {
        vec!["reported surface".to_string()]
    } else {
        root_cause.impact_scope.clone()
    };
    proposal.risk_level = match session.state.environment {
        DebugEnvironment::Prod => DebugSeverity::High,
        _ => session.state.severity,
    };
    proposal.verification_plan = vec![
        if !session.state.repro_steps.is_empty() {
            format!("Re-run reproduction steps: {}", session.state.repro_steps.join(" -> "))
        } else if let Some(target) = session.state.target_url_or_entry.clone() {
            format!("Re-run the affected target: {}", target)
        } else {
            "Re-run the reported failure path".to_string()
        },
        "Check browser console, network traces, and service logs for the same scenario.".to_string(),
        "Run the most targeted automated verification available for the changed surface.".to_string(),
    ];
    proposal.manual_approvals_required = if matches!(session.state.environment, DebugEnvironment::Prod) {
        vec!["patch_review".to_string(), "prod_mutation_blocked".to_string()]
    } else {
        vec!["patch_review".to_string()]
    };
    proposal.patch_preview_ref = Some(format!(
        "patch-preview:{}:{}",
        session
            .state
            .case_id
            .clone()
            .unwrap_or_else(|| "debug".to_string()),
        session.updated_at
    ));
    proposal.patch_operations = generate_patch_operations_for_session(session);
    proposal
}

fn recompute_debug_analysis(session: &mut DebugModeSession) {
    let evidence_count = session.state.evidence_refs.len();
    let has_repro_context =
        !session.state.repro_steps.is_empty() || session.state.target_url_or_entry.is_some();

    if session.state.symptom_summary.trim().is_empty() {
        session.state.phase = DebugLifecyclePhase::Clarifying.as_str().to_string();
        session.state.pending_prompt = Some(
            "Describe the symptom, expected behavior, and affected path so Debug can start analyzing."
                .to_string(),
        );
        session.state.active_hypotheses.clear();
        session.state.selected_root_cause = None;
        session.state.fix_proposal = None;
        session.state.pending_approval = None;
        session.state.verification_report = None;
        return;
    }

    if evidence_count == 0 {
        session.state.phase = DebugLifecyclePhase::Clarifying.as_str().to_string();
        session.state.pending_prompt = Some(
            "Provide reproduction steps, logs, browser URL, or another concrete signal to continue."
                .to_string(),
        );
        session.state.active_hypotheses.clear();
        session.state.selected_root_cause = None;
        session.state.fix_proposal = None;
        session.state.pending_approval = None;
        session.state.verification_report = None;
        return;
    }

    if !has_repro_context || evidence_count < 2 {
        session.state.phase = DebugLifecyclePhase::GatheringSignal.as_str().to_string();
        session.state.pending_prompt = Some(
            "Gather one more concrete signal such as browser output, logs, stack traces, or a stable reproduction path."
                .to_string(),
        );
        session.state.active_hypotheses = derive_debug_hypotheses(session);
        session.state.selected_root_cause = None;
        session.state.fix_proposal = None;
        session.state.pending_approval = None;
        session.state.verification_report = None;
        return;
    }

    session.state.active_hypotheses = derive_debug_hypotheses(session);
    session.state.phase = DebugLifecyclePhase::Hypothesizing.as_str().to_string();
    session.state.pending_prompt = Some(
        "Review the leading hypotheses or attach one more signal to isolate the root cause.".to_string(),
    );
    session.state.selected_root_cause = None;
    session.state.fix_proposal = None;
    session.state.pending_approval = None;
    session.state.verification_report = None;

    if let Some(root_cause) = derive_root_cause(session) {
        session.state.selected_root_cause = Some(root_cause.clone());
        session.state.phase = DebugLifecyclePhase::IdentifyingRootCause.as_str().to_string();
        session.state.pending_prompt = Some(
            "Root cause is ready for review. Continue to generate the smallest safe fix proposal."
                .to_string(),
        );

        if evidence_count >= 4 || session.state.target_url_or_entry.is_some() {
            session.state.fix_proposal = Some(derive_fix_proposal_for_session(session, &root_cause));
            session.state.pending_approval = Some(DebugPendingApproval {
                kind: "patch_review".to_string(),
                title: "Patch review required".to_string(),
                description: "Review the proposed fix before applying any code or system changes.".to_string(),
                required_actions: vec!["approve_patch".to_string(), "reject_patch".to_string()],
            });
            session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
            session.state.pending_prompt = None;
        }
    }
}

async fn sync_debug_kernel_snapshot(
    kernel_state: &WorkflowKernelState,
    session: &DebugModeSession,
    status: Option<WorkflowStatus>,
) -> Result<Vec<String>, String> {
    kernel_state
        .sync_debug_snapshot_by_linked_session(&session.session_id, session.state.clone(), status)
        .await
}

async fn emit_debug_progress(
    app: &tauri::AppHandle,
    session: &DebugModeSession,
    card_type: Option<&str>,
    message: Option<&str>,
) -> Result<(), String> {
    app.emit(
        "debug-progress",
        DebugProgressPayload {
            session_id: session.session_id.clone(),
            phase: session.state.phase.clone(),
            card_type: card_type.map(ToOwned::to_owned),
            message: message.map(ToOwned::to_owned),
            data: None,
        },
    )
    .map_err(|error| format!("Failed to emit debug progress: {error}"))
}

async fn emit_kernel_updates_for_linked_sessions(
    app: &tauri::AppHandle,
    kernel_state: &WorkflowKernelState,
    linked_kernel_sessions: &[String],
    source: &str,
) {
    for kernel_session_id in linked_kernel_sessions {
        let _ = emit_kernel_update_for_session(app, kernel_state, kernel_session_id, source).await;
    }
    let _ = emit_session_catalog_update(app, kernel_state, source).await;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitDebugClarificationRequest {
    pub session_id: String,
    pub answer: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveDebugPatchRequest {
    pub session_id: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub project_path: Option<String>,
    pub context_sources: Option<crate::services::task_mode::context_provider::ContextSourceConfig>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectDebugPatchRequest {
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryDebugPhaseRequest {
    pub session_id: String,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachDebugEvidenceRequest {
    pub session_id: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedDebugFixProposalRequest {
    pub session_id: String,
    pub proposal: FixProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteDebugArtifactRequest {
    pub session_id: String,
    pub file_name: String,
    pub content: String,
}

#[tauri::command]
pub async fn enter_debug_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    _kernel_state: tauri::State<'_, WorkflowKernelState>,
    permission_state: tauri::State<'_, PermissionState>,
    request: EnterDebugModeRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let created_at = now_rfc3339();
    let environment = request
        .environment
        .unwrap_or_else(|| infer_environment(&request.description));
    let capability_profile = capability_profile_for_environment(environment);
    let project_path = normalize_optional_project_path(request.project_path.as_deref());

    let session = DebugModeSession {
        session_id: session_id.clone(),
        kernel_session_id: request.kernel_session_id.clone(),
        project_path: project_path.clone(),
        state: DebugState {
            case_id: Some(format!("dbg-{}", &session_id[..8])),
            phase: DebugLifecyclePhase::Clarifying.as_str().to_string(),
            severity: infer_severity(&request.description),
            environment,
            symptom_summary: request.description.clone(),
            title: Some(build_session_title(&request.description)),
            project_path,
            expected_behavior: None,
            actual_behavior: Some(request.description.clone()),
            repro_steps: Vec::new(),
            affected_surface: infer_affected_surface_from_text(
                &request.description,
                extract_first_url(&request.description).as_deref(),
            ),
            recent_changes: None,
            target_url_or_entry: extract_first_url(&request.description),
            evidence_refs: Vec::new(),
            active_hypotheses: Vec::new(),
            selected_root_cause: None,
            fix_proposal: None,
            pending_approval: None,
            verification_report: None,
            pending_prompt: None,
            capability_profile,
            tool_block_reason: if matches!(environment, DebugEnvironment::Prod) {
                Some("prod_observe_only".to_string())
            } else {
                None
            },
            background_status: Some("idle".to_string()),
            last_checkpoint_id: None,
            entry_handoff: Default::default(),
            quality: Some(ModeQualitySnapshot::for_mode(WorkflowMode::Debug)),
        },
        created_at: created_at.clone(),
        updated_at: created_at,
    };

    let mut session = session;
    recompute_debug_analysis(&mut session);
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }

    permission_state
        .gate
        .set_session_level(&session_id, PermissionLevel::Permissive)
        .await;
    permission_state
        .gate
        .set_debug_capability_profile(&session_id, Some(capability_profile))
        .await;

    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    let _ = emit_debug_progress(&app, &session, Some("debug_intake_card"), Some("debug_started")).await;
    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn get_debug_capability_snapshot(
    state: tauri::State<'_, DebugModeState>,
    permission_state: tauri::State<'_, PermissionState>,
    session_id: String,
) -> Result<CommandResponse<DebugCapabilitySnapshot>, String> {
    let profile = if let Some(capabilities) = permission_state
        .gate
        .get_debug_runtime_capabilities(&session_id)
        .await
    {
        capabilities.profile
    } else {
        match state.get_or_load_session_snapshot(&session_id).await {
            Ok(Some(session)) => session.state.capability_profile,
            Ok(None) => return Ok(CommandResponse::err("Debug session not found")),
            Err(error) => return Ok(CommandResponse::err(error)),
        }
    };

    Ok(CommandResponse::ok(build_debug_capability_snapshot(profile)))
}

#[tauri::command]
pub async fn submit_debug_clarification(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    request: SubmitDebugClarificationRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    let evidence_id = uuid::Uuid::new_v4().to_string();
    session.state.evidence_refs.push(DebugEvidenceRef {
        id: evidence_id.clone(),
        kind: "clarification".to_string(),
        title: "Additional evidence".to_string(),
        summary: request.answer.clone(),
        source: "user".to_string(),
        created_at: now_rfc3339(),
        metadata: serde_json::Map::new(),
    });
    merge_repro_steps(&mut session.state, &request.answer);
    merge_unique_strings(
        &mut session.state.affected_surface,
        infer_affected_surface_from_text(&request.answer, session.state.target_url_or_entry.as_deref()),
    );
    recompute_debug_analysis(&mut session);
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }

    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }

    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "submit_debug_clarification").await;
    let phase_card = match session.state.phase.as_str() {
        "patch_review" => Some("patch_review_card"),
        "identifying_root_cause" => Some("root_cause_card"),
        "hypothesizing" | "gathering_signal" => Some("hypothesis_card"),
        _ => Some("debug_intake_card"),
    };
    let _ = emit_debug_progress(&app, &session, phase_card, Some("debug_analysis_updated")).await;

    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn approve_debug_patch(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    file_changes_state: tauri::State<'_, crate::commands::file_changes::FileChangesState>,
    request: ApproveDebugPatchRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    if let Some(project_path) = normalize_optional_project_path(request.project_path.as_deref()) {
        session.project_path = Some(project_path.clone());
        session.state.project_path = Some(project_path);
    }

    if matches!(session.state.environment, DebugEnvironment::Prod) {
        session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
        session.state.pending_prompt = Some(
            "Production debug sessions are observe-only. Export the patch proposal for manual rollout instead of applying it here."
                .to_string(),
        );
        session.updated_at = now_rfc3339();
        if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
            return Ok(CommandResponse::err(error));
        }
        if let Err(error) = state.store_session_snapshot(session.clone()).await {
            return Ok(CommandResponse::err(error));
        }
        let linked_sessions =
            match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
                Ok(value) => value,
                Err(error) => return Ok(CommandResponse::err(error)),
            };
        emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "approve_debug_patch_prod_blocked").await;
        let _ = emit_debug_progress(&app, &session, Some("patch_review_card"), Some("prod_patch_blocked")).await;
        return Ok(CommandResponse::err(
            "Production debug sessions are observe-only; patch application is blocked".to_string(),
        ));
    }

    let Some(fix_proposal) = session.state.fix_proposal.clone() else {
        return Ok(CommandResponse::err("No debug fix proposal is available for patch approval"));
    };
    if fix_proposal.patch_operations.is_empty() {
        session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
        session.state.pending_prompt = Some(
            "The current fix proposal has no executable patch operations. Seed a structured patch plan before approving it."
                .to_string(),
        );
        session.updated_at = now_rfc3339();
        if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
            return Ok(CommandResponse::err(error));
        }
        if let Err(error) = state.store_session_snapshot(session.clone()).await {
            return Ok(CommandResponse::err(error));
        }
        let linked_sessions =
            match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
                Ok(value) => value,
                Err(error) => return Ok(CommandResponse::err(error)),
            };
        emit_kernel_updates_for_linked_sessions(
            &app,
            kernel_state.inner(),
            &linked_sessions,
            "approve_debug_patch_missing_operations",
        )
        .await;
        let _ = emit_debug_progress(&app, &session, Some("patch_review_card"), Some("patch_operations_required")).await;
        return Ok(CommandResponse::err(
            "Debug patch approval requires executable patch operations".to_string(),
        ));
    }

    let project_root = match resolve_debug_project_root(&session, request.project_path.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
            session.state.pending_prompt = Some(error.clone());
            session.updated_at = now_rfc3339();
            if let Err(materialize_error) = materialize_debug_artifacts(state.inner(), &mut session) {
                return Ok(CommandResponse::err(materialize_error));
            }
            if let Err(store_error) = state.store_session_snapshot(session.clone()).await {
                return Ok(CommandResponse::err(store_error));
            }
            let linked_sessions = match sync_debug_kernel_snapshot(
                kernel_state.inner(),
                &session,
                Some(WorkflowStatus::Active),
            )
            .await
            {
                Ok(value) => value,
                Err(sync_error) => return Ok(CommandResponse::err(sync_error)),
            };
            emit_kernel_updates_for_linked_sessions(
                &app,
                kernel_state.inner(),
                &linked_sessions,
                "approve_debug_patch_missing_project",
            )
            .await;
            let _ = emit_debug_progress(&app, &session, Some("patch_review_card"), Some("patch_project_required")).await;
            return Ok(CommandResponse::err(error));
        }
    };

    session.state.phase = DebugLifecyclePhase::Patching.as_str().to_string();
    session.state.pending_prompt = Some(
        "Applying the approved patch and preparing verification artifacts.".to_string(),
    );
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "approve_debug_patch_patching").await;
    let _ = emit_debug_progress(&app, &session, Some("patch_review_card"), Some("debug_patching")).await;

    let tracker_session_id = session
        .kernel_session_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| session.session_id.clone());
    let file_change_tracker = file_changes_state
        .get_or_create(&tracker_session_id, &project_root.to_string_lossy())
        .await;
    let file_change_turn_index = if let Ok(mut tracker) = file_change_tracker.lock() {
        tracker.set_app_handle(app.clone());
        let next = tracker.turn_index().saturating_add(1);
        tracker.set_turn_index(next);
        Some(next)
    } else {
        None
    };

    let (patch_report, patch_report_artifact_path) = match apply_debug_patch_operations(
        state.inner(),
        &session,
        &project_root,
        &fix_proposal.patch_operations,
        Some(&file_change_tracker),
        file_change_turn_index,
    ) {
        Ok(value) => value,
        Err(error) => {
            session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
            session.state.pending_prompt = Some(error.clone());
            session.state.evidence_refs.push(DebugEvidenceRef {
                id: uuid::Uuid::new_v4().to_string(),
                kind: "patch_application_failed".to_string(),
                title: "Patch application failed".to_string(),
                summary: error.clone(),
                source: "system".to_string(),
                created_at: now_rfc3339(),
                metadata: serde_json::Map::new(),
            });
            session.updated_at = now_rfc3339();
            if let Err(materialize_error) = materialize_debug_artifacts(state.inner(), &mut session) {
                return Ok(CommandResponse::err(materialize_error));
            }
            if let Err(store_error) = state.store_session_snapshot(session.clone()).await {
                return Ok(CommandResponse::err(store_error));
            }
            let linked_sessions = match sync_debug_kernel_snapshot(
                kernel_state.inner(),
                &session,
                Some(WorkflowStatus::Active),
            )
            .await
            {
                Ok(value) => value,
                Err(sync_error) => return Ok(CommandResponse::err(sync_error)),
            };
            emit_kernel_updates_for_linked_sessions(
                &app,
                kernel_state.inner(),
                &linked_sessions,
                "approve_debug_patch_apply_failed",
            )
            .await;
            let _ = emit_debug_progress(&app, &session, Some("patch_review_card"), Some("debug_patch_apply_failed")).await;
            return Ok(CommandResponse::err(error));
        }
    };

    session.state.phase = DebugLifecyclePhase::Verifying.as_str().to_string();
    session.state.pending_approval = None;
    session.state.pending_prompt = Some(
        "Running verification against the approved patch and collecting final artifacts.".to_string(),
    );
    let mut patch_metadata = serde_json::Map::new();
    patch_metadata.insert(
        "artifactPath".to_string(),
        serde_json::Value::String(patch_report_artifact_path.clone()),
    );
    patch_metadata.insert(
        "operationCount".to_string(),
        serde_json::Value::Number(serde_json::Number::from(patch_report.operation_count as u64)),
    );
    patch_metadata.insert(
        "projectPath".to_string(),
        serde_json::Value::String(patch_report.project_path.clone()),
    );
    session.state.evidence_refs.push(DebugEvidenceRef {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "patch".to_string(),
        title: "Patch applied".to_string(),
        summary: session
            .state
            .fix_proposal
            .as_ref()
            .map(|proposal| proposal.summary.clone())
            .unwrap_or_else(|| "Approved patch applied.".to_string()),
        source: "system".to_string(),
        created_at: now_rfc3339(),
        metadata: patch_metadata,
    });
    session.state.evidence_refs.push(DebugEvidenceRef {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "verification".to_string(),
        title: "Verification completed".to_string(),
        summary: format!(
            "Applied {} structured patch operations and generated verification artifacts.",
            patch_report.operation_count
        ),
        source: "system".to_string(),
        created_at: now_rfc3339(),
        metadata: serde_json::Map::new(),
    });
    session.state.verification_report = Some(build_verification_report(&session));
    if let Some(report) = session.state.verification_report.as_mut() {
        if !report.artifacts.iter().any(|artifact| artifact == &patch_report_artifact_path) {
            report.artifacts.push(patch_report_artifact_path.clone());
        }
    }
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "approve_debug_patch_verifying").await;
    let _ = emit_debug_progress(&app, &session, Some("verification_card"), Some("debug_verifying")).await;

    session.state.phase = DebugLifecyclePhase::Completed.as_str().to_string();
    session.state.pending_prompt = None;
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }

    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }

    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Completed)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "approve_debug_patch").await;
    let _ = emit_debug_progress(&app, &session, Some("verification_card"), Some("debug_completed")).await;

    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn reject_debug_patch(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    request: RejectDebugPatchRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    session.state.pending_approval = None;
    session.state.fix_proposal = None;
    session.state.verification_report = None;
    session.state.evidence_refs.push(DebugEvidenceRef {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "review_feedback".to_string(),
        title: "Patch rejected".to_string(),
        summary: request.reason.clone(),
        source: "user".to_string(),
        created_at: now_rfc3339(),
        metadata: serde_json::Map::new(),
    });
    recompute_debug_analysis(&mut session);
    session.state.phase = if session.state.selected_root_cause.is_some() {
        DebugLifecyclePhase::Hypothesizing.as_str().to_string()
    } else {
        DebugLifecyclePhase::GatheringSignal.as_str().to_string()
    };
    session.state.pending_prompt = Some(format!(
        "Patch was rejected ({}). Add another signal or refine the preferred fix direction.",
        request.reason
    ));
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }

    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }

    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "reject_debug_patch").await;
    let _ = emit_debug_progress(&app, &session, Some("hypothesis_card"), Some("patch_rejected")).await;

    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn retry_debug_phase(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    request: RetryDebugPhaseRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    session.state.phase = request.phase.clone();
    if request.phase == DebugLifecyclePhase::GatheringSignal.as_str() {
        session.state.pending_prompt = Some(
            "Collect one more signal such as browser output, logs, or a stable repro path."
                .to_string(),
        );
    } else if request.phase == DebugLifecyclePhase::Hypothesizing.as_str() {
        session.state.pending_prompt = Some(
            "Review the leading hypotheses or attach evidence that rules one out.".to_string(),
        );
    } else if request.phase == DebugLifecyclePhase::IdentifyingRootCause.as_str() {
        session.state.pending_prompt = Some(
            "Root cause is isolated. Confirm it or continue gathering evidence.".to_string(),
        );
    } else if request.phase == DebugLifecyclePhase::Verifying.as_str() {
        session.state.verification_report = Some(build_verification_report(&session));
        session.state.pending_prompt = Some(
            "Verification artifacts were refreshed from the current approved patch.".to_string(),
        );
    }
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }

    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "retry_debug_phase").await;
    let _ = emit_debug_progress(&app, &session, None, Some("debug_phase_retried")).await;
    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn attach_debug_evidence(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    request: AttachDebugEvidenceRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    session.state.evidence_refs.push(DebugEvidenceRef {
        id: uuid::Uuid::new_v4().to_string(),
        kind: evidence_kind_from_source(&request.source),
        title: request.title.clone(),
        summary: request.summary.clone(),
        source: request.source.clone(),
        created_at: now_rfc3339(),
        metadata: request.metadata.unwrap_or_default(),
    });
    merge_repro_steps(&mut session.state, &request.summary);
    merge_unique_strings(
        &mut session.state.affected_surface,
        infer_affected_surface_from_text(&request.summary, session.state.target_url_or_entry.as_deref()),
    );
    recompute_debug_analysis(&mut session);
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "attach_debug_evidence").await;
    let _ = emit_debug_progress(&app, &session, Some("evidence_card"), Some("debug_evidence_attached")).await;
    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn seed_debug_fix_proposal(
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    request: SeedDebugFixProposalRequest,
) -> Result<CommandResponse<DebugModeSession>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&request.session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    session.state.fix_proposal = Some(request.proposal);
    session.state.pending_approval = Some(DebugPendingApproval {
        kind: "patch_review".to_string(),
        title: "Patch review required".to_string(),
        description: "Review the proposed fix before applying any code or system changes.".to_string(),
        required_actions: vec!["approve_patch".to_string(), "reject_patch".to_string()],
    });
    session.state.phase = DebugLifecyclePhase::PatchReview.as_str().to_string();
    session.state.pending_prompt = None;
    session.updated_at = now_rfc3339();
    if let Err(error) = materialize_debug_artifacts(state.inner(), &mut session) {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    if let Err(error) =
        sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Active)).await
    {
        return Ok(CommandResponse::err(error));
    }
    Ok(CommandResponse::ok(session))
}

#[tauri::command]
pub async fn fetch_debug_report(
    state: tauri::State<'_, DebugModeState>,
    session_id: String,
) -> Result<CommandResponse<DebugExecutionReport>, String> {
    let Some(session) = (match state.get_or_load_session_snapshot(&session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    let report = DebugExecutionReport {
        case_id: session.state.case_id.clone(),
        summary: session
            .state
            .verification_report
            .as_ref()
            .map(|report| report.summary.clone())
            .or_else(|| session.state.fix_proposal.as_ref().map(|proposal| proposal.summary.clone()))
            .unwrap_or_else(|| session.state.symptom_summary.clone()),
        root_cause_conclusion: session
            .state
            .selected_root_cause
            .as_ref()
            .map(|root| root.conclusion.clone()),
        fix_applied: session
            .state
            .evidence_refs
            .iter()
            .any(|entry| entry.kind == "patch"),
        verification: session.state.verification_report.clone(),
        residual_risks: session
            .state
            .verification_report
            .as_ref()
            .map(|report| report.residual_risks.clone())
            .unwrap_or_default(),
    };
    Ok(CommandResponse::ok(report))
}

#[tauri::command]
pub async fn list_debug_artifacts(
    state: tauri::State<'_, DebugModeState>,
    session_id: String,
) -> Result<CommandResponse<Vec<DebugArtifactDescriptor>>, String> {
    let Some(session) = (match state.get_or_load_session_snapshot(&session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    let descriptors = collect_debug_artifact_paths(state.inner(), &session)
        .into_iter()
        .filter(|path| path.is_file())
        .filter_map(|path| build_debug_artifact_descriptor(&path).ok())
        .collect::<Vec<_>>();
    Ok(CommandResponse::ok(descriptors))
}

#[tauri::command]
pub async fn load_debug_artifact(
    state: tauri::State<'_, DebugModeState>,
    session_id: String,
    artifact_path: String,
) -> Result<CommandResponse<DebugArtifactContent>, String> {
    let Some(session) = (match state.get_or_load_session_snapshot(&session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    let resolved_artifact = PathBuf::from(artifact_path.trim());
    let known_artifacts = collect_debug_artifact_paths(state.inner(), &session);
    if !known_artifacts.iter().any(|candidate| candidate == &resolved_artifact) {
        return Ok(CommandResponse::err(
            "Debug artifact is not registered for this session".to_string(),
        ));
    }
    let artifact_dir = state.artifact_dir_path(&session.session_id);
    if !resolved_artifact.starts_with(&artifact_dir) {
        return Ok(CommandResponse::err(
            "Debug artifact path is outside the session artifact directory".to_string(),
        ));
    }
    let descriptor = match build_debug_artifact_descriptor(&resolved_artifact) {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    };
    let data = match fs::read(&resolved_artifact) {
        Ok(value) => value,
        Err(error) => {
            return Ok(CommandResponse::err(format!(
                "Failed to read debug artifact '{}': {error}",
                resolved_artifact.display()
            )))
        }
    };
    Ok(CommandResponse::ok(DebugArtifactContent { artifact: descriptor, data }))
}

#[tauri::command]
pub async fn write_debug_artifact(
    state: tauri::State<'_, DebugModeState>,
    session_id: String,
    request: WriteDebugArtifactRequest,
) -> Result<CommandResponse<DebugArtifactDescriptor>, String> {
    let Some(session) = (match state.get_or_load_session_snapshot(&session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    if session.session_id != request.session_id {
        return Ok(CommandResponse::err(
            "Debug artifact request does not match the active session".to_string(),
        ));
    }

    let base_name = Path::new(request.file_name.trim())
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("debug-artifact");
    let extension = Path::new(request.file_name.trim())
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("json");
    let artifact_name = format!("{}.{}", sanitize_artifact_slug(base_name), extension);
    let artifact_path = write_debug_artifact_file(state.inner(), &session.session_id, &artifact_name, request.content)?;
    let descriptor = build_debug_artifact_descriptor(Path::new(&artifact_path))?;
    Ok(CommandResponse::ok(descriptor))
}

#[tauri::command]
pub async fn get_debug_session_snapshot(
    state: tauri::State<'_, DebugModeState>,
    session_id: String,
) -> Result<CommandResponse<DebugModeSession>, String> {
    Ok(match state.get_or_load_session_snapshot(&session_id).await {
        Ok(Some(session)) => CommandResponse::ok(session),
        Ok(None) => CommandResponse::err("Debug session not found"),
        Err(error) => CommandResponse::err(error),
    })
}

#[tauri::command]
pub async fn cancel_debug_operation(
    app: tauri::AppHandle,
    state: tauri::State<'_, DebugModeState>,
    kernel_state: tauri::State<'_, WorkflowKernelState>,
    session_id: String,
) -> Result<CommandResponse<bool>, String> {
    let Some(mut session) = (match state.get_or_load_session_snapshot(&session_id).await {
        Ok(value) => value,
        Err(error) => return Ok(CommandResponse::err(error)),
    }) else {
        return Ok(CommandResponse::err("Debug session not found"));
    };

    session.state.phase = DebugLifecyclePhase::Cancelled.as_str().to_string();
    session.updated_at = now_rfc3339();
    if let Err(error) = state.store_session_snapshot(session.clone()).await {
        return Ok(CommandResponse::err(error));
    }
    let linked_sessions =
        match sync_debug_kernel_snapshot(kernel_state.inner(), &session, Some(WorkflowStatus::Cancelled)).await {
            Ok(value) => value,
            Err(error) => return Ok(CommandResponse::err(error)),
        };
    emit_kernel_updates_for_linked_sessions(&app, kernel_state.inner(), &linked_sessions, "cancel_debug_operation").await;
    Ok(CommandResponse::ok(true))
}

#[tauri::command]
pub async fn exit_debug_mode(
    state: tauri::State<'_, DebugModeState>,
    permission_state: tauri::State<'_, PermissionState>,
    session_id: String,
) -> Result<CommandResponse<bool>, String> {
    Ok(match state.delete_session_snapshot(&session_id).await {
        Ok(()) => {
            permission_state.gate.cleanup_session(&session_id).await;
            CommandResponse::ok(true)
        }
        Err(error) => CommandResponse::err(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_session() -> DebugModeSession {
        DebugModeSession {
            session_id: "debug-session".to_string(),
            kernel_session_id: None,
            project_path: Some("/tmp/project".to_string()),
            state: DebugState {
                case_id: Some("dbg-test".to_string()),
                phase: DebugLifecyclePhase::Verifying.as_str().to_string(),
                severity: DebugSeverity::High,
                environment: DebugEnvironment::Staging,
                symptom_summary: "Checkout fails after clicking pay.".to_string(),
                title: Some("Checkout failure".to_string()),
                project_path: Some("/tmp/project".to_string()),
                expected_behavior: None,
                actual_behavior: None,
                repro_steps: vec!["Open checkout".to_string(), "Click pay".to_string()],
                affected_surface: vec!["checkout".to_string()],
                recent_changes: None,
                target_url_or_entry: Some("https://example.test/checkout".to_string()),
                evidence_refs: vec![],
                active_hypotheses: vec![],
                selected_root_cause: None,
                fix_proposal: Some(FixProposal {
                    summary: "Guard the checkout intent before submit.".to_string(),
                    change_scope: vec!["frontend".to_string()],
                    risk_level: DebugSeverity::Medium,
                    files_or_systems_touched: vec!["src/checkout.tsx".to_string()],
                    manual_approvals_required: vec!["patch_review".to_string()],
                    verification_plan: vec!["Retry checkout".to_string()],
                    patch_preview_ref: Some("/tmp/patch-preview.md".to_string()),
                    patch_operations: vec![],
                }),
                pending_approval: None,
                verification_report: None,
                pending_prompt: None,
                capability_profile: crate::services::debug_mode::DebugCapabilityProfile::StagingLimited,
                tool_block_reason: None,
                background_status: None,
                last_checkpoint_id: None,
                entry_handoff: Default::default(),
                quality: Some(ModeQualitySnapshot::for_mode(WorkflowMode::Debug)),
            },
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        }
    }

    #[test]
    fn verification_report_prefers_verification_stage_browser_evidence() {
        let mut session = build_test_session();

        let mut console_metadata = serde_json::Map::new();
        console_metadata.insert("stage".to_string(), serde_json::Value::String("verification".to_string()));
        console_metadata.insert(
            "currentUrl".to_string(),
            serde_json::Value::String("https://example.test/checkout".to_string()),
        );
        console_metadata.insert("entryCount".to_string(), serde_json::Value::from(3));
        console_metadata.insert("blockingEntryCount".to_string(), serde_json::Value::from(0));
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-1".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "No blocking console errors".to_string(),
            source: "builtin_browser:console:verification".to_string(),
            created_at: now_rfc3339(),
            metadata: console_metadata,
        });

        let mut network_metadata = serde_json::Map::new();
        network_metadata.insert("stage".to_string(), serde_json::Value::String("verification".to_string()));
        network_metadata.insert(
            "currentUrl".to_string(),
            serde_json::Value::String("https://example.test/checkout".to_string()),
        );
        network_metadata.insert("totalEventCount".to_string(), serde_json::Value::from(8));
        network_metadata.insert("failedEventCount".to_string(), serde_json::Value::from(0));
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "network-1".to_string(),
            kind: "network".to_string(),
            title: "Browser network".to_string(),
            summary: "Healthy network events".to_string(),
            source: "builtin_browser:network:verification".to_string(),
            created_at: now_rfc3339(),
            metadata: network_metadata,
        });

        let report = build_verification_report(&session);

        assert!(report.summary.contains("passing checks"));
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "console" && check.status == "passed"));
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "network" && check.status == "passed"));
        assert!(!report
            .residual_risks
            .iter()
            .any(|risk| risk.contains("Browser console was not re-captured")));
    }

    #[test]
    fn apply_debug_patch_operations_writes_project_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        let target_file = project_root.join("src/checkout.tsx");
        fs::write(&target_file, "const label = 'Pay now';\n").expect("seed file");
        let canonical_project_root = fs::canonicalize(&project_root).expect("canonical project root");

        let storage = tempfile::tempdir().expect("storage");
        let state = DebugModeState::new_with_storage_dir(storage.path().to_path_buf());
        let mut session = build_test_session();
        session.kernel_session_id = Some("root-session".to_string());
        session.project_path = Some(canonical_project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        let tracker_data_dir = storage.path().join("tracker");
        let tracker = std::sync::Arc::new(std::sync::Mutex::new(
            crate::services::file_change_tracker::FileChangeTracker::new_with_data_dir(
                "root-session",
                &canonical_project_root,
                &tracker_data_dir,
            ),
        ));
        if let Ok(mut guard) = tracker.lock() {
            guard.set_turn_index(4);
        }

        let operations = vec![DebugPatchOperation {
            id: "replace-checkout-copy".to_string(),
            kind: "replace_text".to_string(),
            file_path: "src/checkout.tsx".to_string(),
            description: "Swap the checkout label".to_string(),
            find_text: Some("Pay now".to_string()),
            replace_text: Some("Confirm payment".to_string()),
            content: None,
            create_if_missing: false,
            expected_occurrences: Some(1),
        }];

        let (report, artifact_path) =
            apply_debug_patch_operations(
                &state,
                &session,
                &canonical_project_root,
                &operations,
                Some(&tracker),
                Some(4),
            )
                .expect("apply patch");

        let updated = fs::read_to_string(&target_file).expect("read updated file");
        assert!(updated.contains("Confirm payment"));
        assert_eq!(report.operation_count, 1);
        assert!(PathBuf::from(&artifact_path).exists());
        assert_eq!(report.operations[0].file_path, "src/checkout.tsx");
        assert!(report.operations[0].backup_artifact_path.is_some());
        let guard = tracker.lock().expect("lock tracker");
        let turns = guard.get_changes_by_turn();
        assert_eq!(turns.len(), 1);
        let change = &turns[0].changes[0];
        assert_eq!(
            change.source_mode,
            Some(crate::services::file_change_tracker::FileChangeSourceMode::Debug)
        );
        assert_eq!(
            change.actor_kind,
            Some(crate::services::file_change_tracker::FileChangeActorKind::DebugPatch)
        );
        assert_eq!(change.origin_session_id.as_deref(), Some("root-session"));
    }

    #[test]
    fn derive_fix_proposal_generates_patch_operations_from_explicit_replacement_instruction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/checkout.tsx"),
            "const checkoutPath = '/api/legacy-checkout';\n",
        )
        .expect("seed file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.symptom_summary =
            "Replace `/api/legacy-checkout` with `/api/checkout` in the failing checkout route.".to_string();
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "The checkout route still calls the legacy endpoint.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.82,
            impact_scope: vec!["src/checkout.tsx".to_string()],
            recommended_direction: "Replace the legacy checkout endpoint with the current one.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-1".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/checkout.tsx".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/checkout.tsx".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);

        assert_eq!(proposal.patch_operations.len(), 1);
        assert_eq!(proposal.patch_operations[0].file_path, "src/checkout.tsx");
        assert_eq!(
            proposal.patch_operations[0].find_text.as_deref(),
            Some("/api/legacy-checkout")
        );
        assert_eq!(
            proposal.patch_operations[0].replace_text.as_deref(),
            Some("/api/checkout")
        );
    }

    #[test]
    fn derive_fix_proposal_selects_unique_matching_file_from_multiple_candidates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/checkout.tsx"),
            "const endpoint = '/api/legacy-checkout';\n",
        )
        .expect("seed checkout file");
        fs::write(
            project_root.join("src/summary.tsx"),
            "const endpoint = '/api/summary';\n",
        )
        .expect("seed summary file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.symptom_summary =
            "Replace `/api/legacy-checkout` with `/api/checkout` in checkout.".to_string();
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "Checkout still points at the legacy endpoint.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.8,
            impact_scope: vec!["src/checkout.tsx".to_string(), "src/summary.tsx".to_string()],
            recommended_direction: "Replace the legacy endpoint.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-2".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/checkout.tsx\nsrc/summary.tsx".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![
                        serde_json::Value::String("src/checkout.tsx".to_string()),
                        serde_json::Value::String("src/summary.tsx".to_string()),
                    ]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);

        assert_eq!(proposal.patch_operations.len(), 1);
        assert_eq!(proposal.patch_operations[0].file_path, "src/checkout.tsx");
    }

    #[test]
    fn derive_fix_proposal_uses_actual_expected_literals_for_replacement() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/banner.tsx"),
            "const label = '旧文案';\n",
        )
        .expect("seed banner file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.actual_behavior = Some("The banner shows '旧文案'.".to_string());
        session.state.expected_behavior = Some("The banner should show '新文案'.".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "The banner copy was not updated.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.76,
            impact_scope: vec!["src/banner.tsx".to_string()],
            recommended_direction: "Update the rendered banner label.".to_string(),
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);

        assert_eq!(proposal.patch_operations.len(), 1);
        assert_eq!(proposal.patch_operations[0].file_path, "src/banner.tsx");
        assert_eq!(proposal.patch_operations[0].find_text.as_deref(), Some("旧文案"));
        assert_eq!(proposal.patch_operations[0].replace_text.as_deref(), Some("新文案"));
    }

    #[test]
    fn derive_fix_proposal_generates_collection_guard_patch_operation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/orders.tsx"),
            "export function Orders({ items }) {\n  return items.map((item) => item.id);\n}\n",
        )
        .expect("seed orders file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.actual_behavior =
            Some("Console shows TypeError: Cannot read properties of undefined (reading 'map').".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "Orders renders before items is initialized.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.81,
            impact_scope: vec!["src/orders.tsx".to_string()],
            recommended_direction: "Guard the items.map call with a safe fallback.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-map".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "TypeError: Cannot read properties of undefined (reading 'map')".to_string(),
            source: "builtin_browser:console:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: serde_json::Map::new(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-orders".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/orders.tsx".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/orders.tsx".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);
        let operation = proposal
            .patch_operations
            .iter()
            .find(|operation| operation.description.contains("Guard items.map"))
            .expect("collection guard operation");

        assert_eq!(operation.file_path, "src/orders.tsx");
        assert_eq!(operation.find_text.as_deref(), Some("items.map("));
        assert_eq!(operation.replace_text.as_deref(), Some("(items ?? []).map("));
    }

    #[test]
    fn derive_fix_proposal_generates_ast_guard_for_multiline_member_call() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/orders.tsx"),
            "export function Orders({ items }) {\n  return items\n    .map((item) => item.id)\n    .join(', ');\n}\n",
        )
        .expect("seed orders file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.actual_behavior =
            Some("Console shows TypeError: Cannot read properties of undefined (reading 'map').".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "Orders renders before items is initialized.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.81,
            impact_scope: vec!["src/orders.tsx".to_string()],
            recommended_direction: "Guard the items.map call with a safe fallback.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-map-multiline".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "TypeError: Cannot read properties of undefined (reading 'map')".to_string(),
            source: "builtin_browser:console:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: serde_json::Map::new(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-orders-multiline".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/orders.tsx".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/orders.tsx".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);
        let operation = proposal
            .patch_operations
            .iter()
            .find(|operation| operation.description.contains("AST guard"))
            .expect("ast guard operation");

        assert_eq!(operation.file_path, "src/orders.tsx");
        assert_eq!(operation.find_text.as_deref(), Some("items\n    .map"));
        assert_eq!(operation.replace_text.as_deref(), Some("(items ?? []).map"));
    }

    #[test]
    fn derive_fix_proposal_prefers_binding_default_injection_when_destructure_is_available() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/orders.tsx"),
            "export function Orders(props: { items?: Array<{ id: string }> }) {\n  const { items } = props;\n  return items.map((item) => item.id);\n}\n",
        )
        .expect("seed orders file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.actual_behavior =
            Some("Console shows TypeError: Cannot read properties of undefined (reading 'map').".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "Orders renders before items is initialized.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.82,
            impact_scope: vec!["src/orders.tsx".to_string()],
            recommended_direction: "Guard the items.map call with a safe fallback.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-map-destructure".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "TypeError: Cannot read properties of undefined (reading 'map')".to_string(),
            source: "builtin_browser:console:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: serde_json::Map::new(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-orders-destructure".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/orders.tsx".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/orders.tsx".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);
        let operation = proposal
            .patch_operations
            .iter()
            .find(|operation| operation.description.contains("Inject a default binding"))
            .expect("binding default operation");

        assert_eq!(operation.file_path, "src/orders.tsx");
        assert_eq!(
            operation.find_text.as_deref(),
            Some("const { items } = props;")
        );
        assert_eq!(
            operation.replace_text.as_deref(),
            Some("const { items = [] } = props;")
        );
    }

    #[test]
    fn derive_fix_proposal_generates_string_guard_patch_operation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/profile.ts"),
            "export const normalizedName = customerName.trim().toLowerCase();\n",
        )
        .expect("seed profile file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.actual_behavior =
            Some("Profile crashes with Cannot read properties of undefined (reading 'trim').".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "The profile normalizer assumes customerName is always present.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.79,
            impact_scope: vec!["src/profile.ts".to_string()],
            recommended_direction: "Add a fallback before trimming the customerName string.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-trim".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "TypeError: Cannot read properties of undefined (reading 'trim')".to_string(),
            source: "builtin_browser:console:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: serde_json::Map::new(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-profile".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/profile.ts".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/profile.ts".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);
        let operation = proposal
            .patch_operations
            .iter()
            .find(|operation| operation.description.contains("Guard customerName.trim"))
            .expect("string guard operation");

        assert_eq!(operation.file_path, "src/profile.ts");
        assert_eq!(operation.find_text.as_deref(), Some("customerName.trim("));
        assert_eq!(operation.replace_text.as_deref(), Some("(customerName ?? '').trim("));
    }

    #[test]
    fn derive_fix_proposal_generates_length_property_guard_patch_operation_without_replacement_instruction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join("src")).expect("create project dirs");
        fs::write(
            project_root.join("src/results.ts"),
            "export const shouldShowResults = results.length > 0;\n",
        )
        .expect("seed results file");

        let mut session = build_test_session();
        session.project_path = Some(project_root.to_string_lossy().to_string());
        session.state.project_path = session.project_path.clone();
        session.state.symptom_summary = "Results page crashes when results is undefined.".to_string();
        session.state.actual_behavior =
            Some("TypeError: Cannot read properties of undefined (reading 'length')".to_string());
        session.state.expected_behavior = Some("The results page should stay empty instead of crashing.".to_string());
        session.state.selected_root_cause = Some(RootCauseReport {
            conclusion: "The results visibility check assumes results is always defined.".to_string(),
            supporting_evidence_ids: vec![],
            contradictions: vec![],
            confidence: 0.8,
            impact_scope: vec!["src/results.ts".to_string()],
            recommended_direction: "Guard the results length check with optional chaining and a fallback.".to_string(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "console-length".to_string(),
            kind: "console".to_string(),
            title: "Browser console".to_string(),
            summary: "TypeError: Cannot read properties of undefined (reading 'length')".to_string(),
            source: "builtin_browser:console:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: serde_json::Map::new(),
        });
        session.state.evidence_refs.push(DebugEvidenceRef {
            id: "source-map-results".to_string(),
            kind: "source_mapping".to_string(),
            title: "Browser source mapping".to_string(),
            summary: "src/results.ts".to_string(),
            source: "builtin_browser:source_mapping:baseline".to_string(),
            created_at: now_rfc3339(),
            metadata: {
                let mut metadata = serde_json::Map::new();
                metadata.insert(
                    "candidateFiles".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String("src/results.ts".to_string())]),
                );
                metadata
            },
        });

        let root_cause = session.state.selected_root_cause.clone().expect("root cause");
        let proposal = derive_fix_proposal_for_session(&session, &root_cause);
        let operation = proposal
            .patch_operations
            .iter()
            .find(|operation| operation.description.contains("results.length"))
            .expect("length guard operation");

        assert_eq!(operation.file_path, "src/results.ts");
        assert_eq!(operation.find_text.as_deref(), Some("results.length"));
        assert_eq!(operation.replace_text.as_deref(), Some("(results?.length ?? 0)"));
    }
}
