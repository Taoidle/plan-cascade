use super::*;

pub(super) fn tool_output_for_model_context(
    tool_name: &str,
    result: &crate::services::tools::executor::ToolResult,
    analysis_phase: Option<&str>,
) -> String {
    let raw = result.to_content();
    if analysis_phase.is_none() {
        return raw;
    }

    let line_limit = if tool_name == "Read" {
        ANALYSIS_TOOL_RESULT_MAX_LINES
    } else {
        ANALYSIS_TOOL_RESULT_MAX_LINES / 3
    };
    let mut lines = raw.lines().take(line_limit).collect::<Vec<_>>().join("\n");
    if lines.len() > ANALYSIS_TOOL_RESULT_MAX_CHARS {
        lines = truncate_for_log(&lines, ANALYSIS_TOOL_RESULT_MAX_CHARS);
    }
    if raw.len() > lines.len() {
        format!(
            "{}\n\n[tool output truncated for analysis context: {} -> {} chars]",
            lines,
            raw.len(),
            lines.len()
        )
    } else {
        lines
    }
}

/// Truncate tool output for the messages vector during regular (non-analysis) execution.
///
/// This applies bounded truncation so that large tool results do not bloat the LLM
/// context window. The frontend ToolResult event still receives the full content;
/// only the messages vec (what the LLM sees) is truncated.
pub(super) fn truncate_tool_output_for_context(tool_name: &str, content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }

    let (max_lines, max_chars) = match tool_name {
        "Read" => (REGULAR_READ_MAX_LINES, REGULAR_READ_MAX_CHARS),
        "Grep" => (REGULAR_GREP_MAX_LINES, REGULAR_GREP_MAX_CHARS),
        "LS" | "Glob" => (REGULAR_LS_MAX_LINES, REGULAR_LS_MAX_CHARS),
        "Bash" => (REGULAR_BASH_MAX_LINES, REGULAR_BASH_MAX_CHARS),
        _ => (REGULAR_BASH_MAX_LINES, REGULAR_BASH_MAX_CHARS),
    };

    let original_len = content.len();
    let original_line_count = content.lines().count();

    // If under both limits, pass through unchanged
    if original_line_count <= max_lines && original_len <= max_chars {
        return content.to_string();
    }

    // Truncate by line count first
    let mut truncated: String = content
        .lines()
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n");

    // Then truncate by char limit if still over
    if truncated.len() > max_chars {
        truncated = truncate_for_log(&truncated, max_chars);
    }

    let truncated_len = truncated.len();
    format!(
        "{}\n\n[truncated for context: {} -> {} chars, {} -> {} lines]",
        truncated, original_len, truncated_len, original_line_count, max_lines
    )
}

pub(super) fn trim_line_reference_suffix(path: &str) -> String {
    let mut normalized = path.to_string();

    if let Some(idx) = normalized.find("#L").or_else(|| normalized.find("#l")) {
        normalized.truncate(idx);
    }

    if let Some(idx) = normalized.rfind(':') {
        let is_drive_prefix = idx == 1
            && normalized
                .as_bytes()
                .first()
                .map(|b| b.is_ascii_alphabetic())
                .unwrap_or(false);
        if !is_drive_prefix {
            let suffix = &normalized[idx + 1..];
            let looks_like_line_ref = !suffix.is_empty()
                && suffix
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == ':' || c == '-');
            let prefix = &normalized[..idx];
            let looks_like_path = prefix.contains('/') || prefix.contains('\\');
            if looks_like_line_ref && looks_like_path {
                normalized.truncate(idx);
            }
        }
    }

    normalized
}

pub(super) fn normalize_candidate_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return None;
    }

    let normalized = trim_line_reference_suffix(
        &trimmed
            .trim_matches(|c: char| "\"'`[](){}<>,".contains(c))
            .replace('\\', "/"),
    )
    .trim_start_matches("./")
    .trim_end_matches('/')
    .to_string();

    if normalized.is_empty() || normalized == "." || normalized == ".." {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn extract_primary_path_from_arguments(arguments: &serde_json::Value) -> Option<String> {
    const PRIMARY_KEYS: &[&str] = &["file_path", "path", "notebook_path", "working_dir", "cwd"];

    for key in PRIMARY_KEYS {
        if let Some(value) = arguments.get(*key).and_then(|v| v.as_str()) {
            if let Some(path) = normalize_candidate_path(value) {
                return Some(path);
            }
        }
    }
    None
}

pub(super) fn extract_all_paths_from_arguments(arguments: &serde_json::Value) -> Vec<String> {
    const PATH_KEYS: &[&str] = &["file_path", "path", "notebook_path", "working_dir", "cwd"];
    let mut found = Vec::<String>::new();

    fn walk(value: &serde_json::Value, found: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, inner) in map {
                    if PATH_KEYS.contains(&key.as_str()) {
                        if let Some(s) = inner.as_str() {
                            if let Some(path) = normalize_candidate_path(s) {
                                found.push(path);
                            }
                        }
                    }
                    walk(inner, found);
                }
            }
            serde_json::Value::Array(items) => {
                for inner in items {
                    walk(inner, found);
                }
            }
            _ => {}
        }
    }

    walk(arguments, &mut found);
    found.sort();
    found.dedup();
    found
}

pub(super) fn summarize_tool_activity(
    tool_name: &str,
    arguments: Option<&serde_json::Value>,
    primary_path: Option<&str>,
) -> String {
    match tool_name {
        "Read" => format!(
            "Read {}",
            primary_path.unwrap_or("an unspecified file path")
        ),
        "LS" => format!(
            "Listed directory {}",
            primary_path.unwrap_or("at current working directory")
        ),
        "Glob" => {
            let pattern = arguments
                .and_then(|v| v.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            format!(
                "Glob pattern '{}' under {}",
                pattern,
                primary_path.unwrap_or("working directory")
            )
        }
        "Grep" => {
            let pattern = arguments
                .and_then(|v| v.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("(missing pattern)");
            format!(
                "Grep pattern '{}' under {}",
                pattern,
                primary_path.unwrap_or("working directory")
            )
        }
        "Cwd" => "Resolved working directory".to_string(),
        _ => format!(
            "{} called{}",
            tool_name,
            primary_path
                .map(|p| format!(" on {}", p))
                .unwrap_or_else(String::new)
        ),
    }
}

pub(super) fn select_local_seed_files(inventory: &FileInventory) -> Vec<String> {
    let preferred_paths = [
        "src/plan_cascade/cli/main.py",
        "src/plan_cascade/core/orchestrator.py",
        "src/plan_cascade/backends/factory.py",
        "src/plan_cascade/state/state_manager.py",
        "mcp_server/server.py",
        "desktop/src-tauri/src/main.rs",
        "desktop/src/App.tsx",
        "desktop/package.json",
        "desktop/src-tauri/Cargo.toml",
        "pyproject.toml",
    ];

    let mut selected = Vec::<String>::new();
    for preferred in preferred_paths {
        if inventory.items.iter().any(|i| i.path == preferred) {
            selected.push(preferred.to_string());
        }
    }

    let component_order = [
        "python-core",
        "mcp-server",
        "desktop-rust",
        "desktop-web",
        "python-tests",
        "rust-tests",
        "frontend-tests",
    ];
    for component in component_order {
        if let Some(item) = inventory.items.iter().find(|i| i.component == component) {
            selected.push(item.path.clone());
        }
    }

    selected.sort();
    selected.dedup();
    selected
}

pub(super) fn related_test_candidates(
    selected_paths: &[String],
    inventory_items: &[FileInventoryItem],
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for target in selected_paths {
        let normalized = target.replace('\\', "/");
        let stem = std::path::Path::new(&normalized)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if stem.is_empty() {
            continue;
        }
        let normalized_lower = normalized.to_ascii_lowercase();

        for item in inventory_items {
            if !item.is_test {
                continue;
            }
            let test_lower = item.path.to_ascii_lowercase();
            let likely_related = test_lower.contains(&stem)
                || normalized_lower
                    .split('/')
                    .next()
                    .map(|root| test_lower.contains(root))
                    .unwrap_or(false);
            if likely_related && seen.insert(item.path.clone()) {
                candidates.push(item.path.clone());
            }
        }
    }

    candidates.sort();
    candidates
}

pub(super) fn summarize_file_head(path: &std::path::Path, max_lines: usize) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > 400_000 {
        return Some("large file (head skipped)".to_string());
    }
    let content = std::fs::read_to_string(path).ok()?;
    if content.is_empty() {
        return Some("empty file".to_string());
    }
    let mut lines = Vec::<String>::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(trimmed.to_string());
        if lines.len() >= max_lines.max(1) {
            break;
        }
    }
    if lines.is_empty() {
        Some("no non-empty lines in head".to_string())
    } else {
        Some(lines.join(" | "))
    }
}

pub(super) fn looks_like_test_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with("tests/")
        || normalized.starts_with("desktop/src-tauri/tests/")
        || normalized.starts_with("desktop/src/components/__tests__/")
        || normalized.ends_with("_test.py")
        || normalized.ends_with(".test.ts")
        || normalized.ends_with(".test.tsx")
        || normalized.ends_with(".spec.ts")
        || normalized.ends_with(".spec.tsx")
}

pub(super) fn extract_path_candidates_from_text(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for token in text.split_whitespace() {
        let candidate = token.trim_matches(|c: char| "\"'`[](){}<>,;:".contains(c));
        if !(candidate.contains('/') || candidate.contains('\\')) {
            continue;
        }
        if !is_plausible_path_text(candidate) {
            continue;
        }
        if let Some(path) = normalize_candidate_path(candidate) {
            paths.push(path);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn is_plausible_path_text(candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.len() < 2 || candidate.len() > 260 {
        return false;
    }
    if candidate.starts_with("http://") || candidate.starts_with("https://") {
        return false;
    }

    // Filter common code/regex/template fragments that contain '/' but are not paths.
    if candidate.starts_with("!/") || candidate.starts_with("/^") {
        return false;
    }
    if candidate.contains("${")
        || candidate.contains("`)")
        || candidate.contains(".test(")
        || candidate.contains(".match(")
    {
        return false;
    }
    if candidate.contains('*')
        || candidate.contains('?')
        || candidate.contains('|')
        || candidate.contains('^')
        || candidate.contains('!')
        || candidate.contains("...")
    {
        return false;
    }

    // Keep path-like strings conservative: letters/digits plus common path symbols.
    if candidate
        .chars()
        .any(|c| !(c.is_alphanumeric() || "/\\._-:+@~#".contains(c)))
    {
        return false;
    }

    candidate
        .split(['/', '\\'])
        .any(|segment| segment.chars().any(|c| c.is_alphanumeric()))
}

pub(super) fn is_observed_path(candidate: &str, observed: &HashSet<String>) -> bool {
    let normalized = match normalize_candidate_path(candidate) {
        Some(path) => path,
        None => return true,
    };
    observed.iter().any(|known| {
        known == &normalized
            || known.ends_with(&normalized)
            || known.starts_with(&normalized)
            || normalized.ends_with(known)
            || normalized.starts_with(known)
    })
}

pub(super) fn observed_root_segments(observed: &HashSet<String>) -> HashSet<String> {
    let mut roots = HashSet::new();
    for item in observed {
        if let Some(normalized) = normalize_candidate_path(item) {
            if let Some(first) = normalized.split('/').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() && trimmed != "." && trimmed != ".." {
                    roots.insert(trimmed.to_ascii_lowercase());
                }
            }
        }
    }
    roots
}

pub(super) fn is_concrete_path_reference(candidate: &str, observed_roots: &HashSet<String>) -> bool {
    let normalized = match normalize_candidate_path(candidate) {
        Some(path) => path,
        None => return false,
    };

    if normalized.starts_with('/')
        || normalized.starts_with("./")
        || normalized.starts_with("../")
        || normalized.starts_with("\\\\")
    {
        return true;
    }

    // Windows drive letter paths like C:/...
    if normalized.len() >= 2 {
        let bytes = normalized.as_bytes();
        if bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            return true;
        }
    }

    let segments = normalized
        .split('/')
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return false;
    }

    // Filter documentation labels like "Desktop/CLI" that are not file-system paths.
    if segments.len() == 2
        && segments
            .iter()
            .all(|seg| seg.chars().all(|c| c.is_ascii_alphabetic()))
        && segments
            .iter()
            .any(|seg| seg.chars().any(|c| c.is_ascii_uppercase()))
    {
        return false;
    }

    // Filter ALL-CAPS slash-delimited labels like VERIFIED/UNVERIFIED/CONTRADICTED.
    let uppercase_label = segments
        .iter()
        .map(|seg| seg.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_'))
        .all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        });
    if uppercase_label {
        return false;
    }

    if segments
        .last()
        .map(|seg| seg.contains('.'))
        .unwrap_or(false)
    {
        return true;
    }

    if segments.iter().any(|seg| seg.starts_with('.')) {
        return true;
    }

    segments
        .first()
        .map(|first| observed_roots.contains(&first.to_ascii_lowercase()))
        .unwrap_or(false)
}

pub(super) fn find_unverified_paths(text: &str, observed: &HashSet<String>) -> Vec<String> {
    let observed_roots = observed_root_segments(observed);
    extract_path_candidates_from_text(text)
        .into_iter()
        .filter(|path| is_concrete_path_reference(path, &observed_roots))
        .filter(|path| !is_observed_path(path, observed))
        .collect()
}

