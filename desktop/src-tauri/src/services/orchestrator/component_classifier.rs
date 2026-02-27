//! Component Classification
//!
//! Classifies indexed files into logical components using either LLM analysis
//! or a heuristic fallback (top-level directory names).  Results are cached in
//! the `component_mappings` SQLite table for incremental reuse.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::index_store::IndexStore;
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, ToolCallMode};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single prefix→component mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMapping {
    pub prefix: String,
    pub component: String,
    #[serde(default)]
    pub description: String,
}

/// Source of the classification result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassificationSource {
    Heuristic,
    Llm,
}

/// Result of a component classification run.
pub struct ClassificationResult {
    pub mappings: Vec<ComponentMapping>,
    pub source: ClassificationSource,
    pub files_updated: usize,
}

// ---------------------------------------------------------------------------
// Directory tree builder
// ---------------------------------------------------------------------------

/// Build a compact directory tree string from indexed file paths.
///
/// Groups files by directory prefix up to `max_depth` levels and formats as
/// an indented tree with file counts.
pub fn build_directory_tree(file_paths: &[String], max_depth: usize) -> String {
    let mut dir_counts: HashMap<String, usize> = HashMap::new();

    for path in file_paths {
        let parts: Vec<&str> = path.split('/').collect();
        // Count files at each directory depth level
        for depth in 0..max_depth.min(parts.len().saturating_sub(1)) {
            let dir = parts[..=depth].join("/");
            *dir_counts.entry(dir).or_insert(0) += 1;
        }
        // Count root-level files (no directory)
        if parts.len() == 1 {
            *dir_counts.entry(String::new()).or_insert(0) += 1;
        }
    }

    // Build a sorted tree
    let mut dirs: Vec<(String, usize)> = dir_counts.into_iter().collect();
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut output = String::new();
    let root_count = dirs.iter().find(|(d, _)| d.is_empty()).map(|(_, c)| *c).unwrap_or(0);
    if root_count > 0 {
        output.push_str(&format!("(root: {} files)\n", root_count));
    }

    for (dir, count) in &dirs {
        if dir.is_empty() {
            continue;
        }
        let depth = dir.chars().filter(|&c| c == '/').count();
        let indent = "  ".repeat(depth);
        let name = dir.rsplit('/').next().unwrap_or(dir);
        output.push_str(&format!("{}{}/  ({} files)\n", indent, name, count));
    }

    output
}

// ---------------------------------------------------------------------------
// Heuristic classification
// ---------------------------------------------------------------------------

/// Classify files by top-level directory (fallback when no LLM is available).
pub fn heuristic_classify(file_paths: &[String]) -> Vec<ComponentMapping> {
    let mut groups: HashMap<String, usize> = HashMap::new();

    for path in file_paths {
        let key = if let Some(slash_pos) = path.find('/') {
            &path[..slash_pos]
        } else {
            "" // root-level file
        };
        *groups.entry(key.to_string()).or_insert(0) += 1;
    }

    let mut mappings = Vec::new();
    let mut small_dirs: Vec<(String, usize)> = Vec::new();

    for (dir, count) in &groups {
        if dir.is_empty() {
            mappings.push(ComponentMapping {
                prefix: String::new(),
                component: "repo-root".to_string(),
                description: "Root-level project files".to_string(),
            });
        } else if *count < 3 {
            small_dirs.push((dir.clone(), *count));
        } else {
            let component = to_kebab_case(dir);
            mappings.push(ComponentMapping {
                prefix: format!("{}/", dir),
                component,
                description: String::new(),
            });
        }
    }

    // Merge small directories into "other"
    if !small_dirs.is_empty() {
        for (dir, _) in &small_dirs {
            mappings.push(ComponentMapping {
                prefix: format!("{}/", dir),
                component: "other".to_string(),
                description: "Small miscellaneous directories".to_string(),
            });
        }
    }

    // Sort by prefix length descending (longest match first)
    mappings.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));
    mappings
}

// ---------------------------------------------------------------------------
// LLM classification
// ---------------------------------------------------------------------------

const CLASSIFICATION_SYSTEM_PROMPT: &str = r#"You are a code architecture analyst. Given a project's directory tree, identify the logical components.

Return a JSON array of objects, each with:
- "prefix": the directory path prefix (e.g. "src/core/", "tests/") — use trailing slash for directories, empty string "" for root-level files
- "component": a short kebab-case name for the component (e.g. "api-server", "frontend", "shared-utils")
- "description": one-sentence description of what this component contains

Rules:
1. Group related directories into the same component when they serve the same purpose
2. Use meaningful domain names, not generic directory names
3. Merge very small directories (< 3 files) into nearby components or "other"
4. Keep the number of components between 3 and 15
5. Every file in the project should match at least one prefix
6. Return ONLY the JSON array, no markdown fences, no commentary"#;

/// Classify files using an LLM provider.
pub async fn llm_classify(
    provider: &dyn LlmProvider,
    file_paths: &[String],
    project_name: &str,
) -> Result<Vec<ComponentMapping>, String> {
    let tree = build_directory_tree(file_paths, 3);

    let user_message = format!(
        "Project: {}\nTotal files: {}\n\nDirectory structure:\n{}",
        project_name,
        file_paths.len(),
        tree
    );

    let messages = vec![Message::user(&user_message)];
    let options = LlmRequestOptions {
        tool_call_mode: ToolCallMode::None,
        temperature_override: Some(0.1),
        ..Default::default()
    };

    // First attempt with timeout
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(45),
        provider.send_message(
            messages.clone(),
            Some(CLASSIFICATION_SYSTEM_PROMPT.to_string()),
            vec![],
            options.clone(),
        ),
    )
    .await
    .map_err(|_| "LLM component classification timed out after 45s".to_string())?
    .map_err(|e| format!("LLM component classification failed: {}", e))?;

    let response_text = response
        .content
        .as_deref()
        .or(response.thinking.as_deref())
        .unwrap_or("")
        .to_string();

    match parse_classification_response(&response_text) {
        Ok(mappings) => Ok(mappings),
        Err(first_error) => {
            // ADR-F002: Retry with repair prompt
            let repair_message = format!(
                "Your previous response could not be parsed as valid JSON.\n\n\
                 Parse error: {}\n\n\
                 Your previous response was:\n{}\n\n\
                 Please respond with ONLY a valid JSON array of objects, each with \
                 \"prefix\", \"component\", and \"description\" fields. \
                 No markdown fences, no explanatory text. Just the raw JSON array \
                 starting with [ and ending with ].",
                first_error, response_text
            );

            let mut retry_messages = messages;
            retry_messages.push(Message::assistant(&response_text));
            retry_messages.push(Message::user(&repair_message));

            let retry_response = tokio::time::timeout(
                std::time::Duration::from_secs(45),
                provider.send_message(
                    retry_messages,
                    Some(CLASSIFICATION_SYSTEM_PROMPT.to_string()),
                    vec![],
                    options,
                ),
            )
            .await
            .map_err(|_| "LLM classification retry timed out".to_string())?
            .map_err(|e| format!("LLM classification retry failed: {}", e))?;

            let retry_text = retry_response
                .content
                .as_deref()
                .or(retry_response.thinking.as_deref())
                .unwrap_or("");

            parse_classification_response(retry_text).map_err(|e| {
                format!(
                    "Failed to parse LLM classification after retry. First: {}. Retry: {}",
                    first_error, e
                )
            })
        }
    }
}

/// Parse the LLM response text into component mappings.
fn parse_classification_response(text: &str) -> Result<Vec<ComponentMapping>, String> {
    let trimmed = text.trim();

    // Strip markdown fences if present
    let json_str = if trimmed.starts_with("```") {
        let inner = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        inner
    } else {
        trimmed
    };

    // Find the JSON array boundaries
    let start = json_str.find('[').ok_or("No '[' found in response")?;
    let end = json_str.rfind(']').ok_or("No ']' found in response")?;

    if end <= start {
        return Err("Invalid JSON array boundaries".to_string());
    }

    let array_str = &json_str[start..=end];
    let mappings: Vec<ComponentMapping> =
        serde_json::from_str(array_str).map_err(|e| format!("JSON parse error: {}", e))?;

    if mappings.is_empty() {
        return Err("Empty mappings array".to_string());
    }

    Ok(mappings)
}

// ---------------------------------------------------------------------------
// Lookup (for incremental indexing)
// ---------------------------------------------------------------------------

/// Look up the component for a file path using cached prefix→component mappings.
///
/// Uses longest-prefix match: mappings should be sorted by prefix length
/// descending.  Falls back to `"other"` if no prefix matches.
pub fn lookup_component(mappings: &[ComponentMapping], file_path: &str) -> String {
    for m in mappings {
        if m.prefix.is_empty() {
            // Root-level: matches files with no directory separator
            if !file_path.contains('/') {
                return m.component.clone();
            }
        } else if file_path.starts_with(&m.prefix) {
            return m.component.clone();
        }
    }
    "other".to_string()
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Run component classification for a project.
///
/// 1. Load all file paths from the index
/// 2. Try LLM classification (if a provider is given)
/// 3. Fall back to heuristic on failure or if no provider
/// 4. Store mappings in SQLite
/// 5. Batch-update all file_index rows
pub async fn classify_components(
    index_store: &IndexStore,
    project_path: &str,
    provider: Option<&Arc<dyn LlmProvider>>,
) -> ClassificationResult {
    // Load file paths
    let file_paths = match index_store.get_all_file_paths(project_path) {
        Ok(paths) => paths,
        Err(e) => {
            warn!(error = %e, "component classifier: failed to load file paths");
            return ClassificationResult {
                mappings: Vec::new(),
                source: ClassificationSource::Heuristic,
                files_updated: 0,
            };
        }
    };

    if file_paths.is_empty() {
        return ClassificationResult {
            mappings: Vec::new(),
            source: ClassificationSource::Heuristic,
            files_updated: 0,
        };
    }

    // Try LLM classification
    let (mappings, source) = if let Some(llm) = provider {
        let project_name = project_path
            .rsplit('/')
            .next()
            .unwrap_or(project_path);

        match llm_classify(llm.as_ref(), &file_paths, project_name).await {
            Ok(mut m) => {
                // Ensure sorted by prefix length descending
                m.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));
                info!(
                    mappings = m.len(),
                    "component classifier: LLM classification succeeded"
                );
                (m, ClassificationSource::Llm)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "component classifier: LLM classification failed, falling back to heuristic"
                );
                (heuristic_classify(&file_paths), ClassificationSource::Heuristic)
            }
        }
    } else {
        info!("component classifier: no LLM provider, using heuristic");
        (heuristic_classify(&file_paths), ClassificationSource::Heuristic)
    };

    // Store mappings
    let source_str = match source {
        ClassificationSource::Llm => "llm",
        ClassificationSource::Heuristic => "heuristic",
    };

    let db_mappings: Vec<(String, String, String)> = mappings
        .iter()
        .map(|m| (m.prefix.clone(), m.component.clone(), m.description.clone()))
        .collect();

    if let Err(e) = index_store.upsert_component_mappings(project_path, &db_mappings, source_str) {
        warn!(error = %e, "component classifier: failed to store mappings");
    }

    // Batch-update file_index rows
    let update_pairs: Vec<(String, String)> = mappings
        .iter()
        .map(|m| (m.prefix.clone(), m.component.clone()))
        .collect();

    let files_updated = match index_store.batch_update_components(project_path, &update_pairs) {
        Ok(n) => n,
        Err(e) => {
            warn!(error = %e, "component classifier: batch update failed");
            0
        }
    };

    ClassificationResult {
        mappings,
        source,
        files_updated,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a string to kebab-case (lowercase, replace non-alphanumeric with hyphens).
fn to_kebab_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_was_sep = false;

    for c in s.chars() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && !result.is_empty() && !prev_was_sep {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_sep = false;
        } else {
            if !result.is_empty() && !prev_was_sep {
                result.push('-');
                prev_was_sep = true;
            }
        }
    }

    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_classify() {
        let paths = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/utils.rs".to_string(),
            "tests/test_main.rs".to_string(),
            "tests/test_lib.rs".to_string(),
            "tests/test_utils.rs".to_string(),
            "README.md".to_string(),
        ];

        let mappings = heuristic_classify(&paths);
        assert!(!mappings.is_empty());

        // Root-level file should have "repo-root"
        let root = mappings.iter().find(|m| m.component == "repo-root");
        assert!(root.is_some());

        // "src" and "tests" should be components
        let src = mappings.iter().find(|m| m.prefix == "src/");
        assert!(src.is_some());
    }

    #[test]
    fn test_lookup_component() {
        let mappings = vec![
            ComponentMapping {
                prefix: "src/services/llm/".to_string(),
                component: "llm-providers".to_string(),
                description: String::new(),
            },
            ComponentMapping {
                prefix: "src/services/".to_string(),
                component: "backend-services".to_string(),
                description: String::new(),
            },
            ComponentMapping {
                prefix: "src/".to_string(),
                component: "source".to_string(),
                description: String::new(),
            },
            ComponentMapping {
                prefix: String::new(),
                component: "repo-root".to_string(),
                description: String::new(),
            },
        ];

        assert_eq!(
            lookup_component(&mappings, "src/services/llm/openai.rs"),
            "llm-providers"
        );
        assert_eq!(
            lookup_component(&mappings, "src/services/auth.rs"),
            "backend-services"
        );
        assert_eq!(lookup_component(&mappings, "src/main.rs"), "source");
        assert_eq!(lookup_component(&mappings, "README.md"), "repo-root");
        assert_eq!(lookup_component(&mappings, "unknown/deep/file.rs"), "other");
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("src_tauri"), "src-tauri");
        assert_eq!(to_kebab_case("MyComponent"), "my-component");
        assert_eq!(to_kebab_case("simple"), "simple");
        assert_eq!(to_kebab_case("src-tauri"), "src-tauri");
    }

    #[test]
    fn test_build_directory_tree() {
        let paths = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/core/engine.rs".to_string(),
            "tests/test_main.rs".to_string(),
            "README.md".to_string(),
        ];

        let tree = build_directory_tree(&paths, 3);
        assert!(tree.contains("src/"));
        assert!(tree.contains("tests/"));
        assert!(tree.contains("root:"));
    }

    #[test]
    fn test_parse_classification_response() {
        let input = r#"[
            {"prefix": "src/", "component": "source", "description": "Main source code"},
            {"prefix": "tests/", "component": "tests", "description": "Test suite"}
        ]"#;

        let result = parse_classification_response(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].component, "source");
    }

    #[test]
    fn test_parse_classification_response_with_fences() {
        let input = "```json\n[{\"prefix\": \"src/\", \"component\": \"source\", \"description\": \"\"}]\n```";
        let result = parse_classification_response(input).unwrap();
        assert_eq!(result.len(), 1);
    }
}
