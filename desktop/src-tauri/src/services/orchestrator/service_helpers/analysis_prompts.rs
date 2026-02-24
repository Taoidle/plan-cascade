use super::*;

pub(super) fn analysis_phase_system_prompt(phase: AnalysisPhase) -> &'static str {
    match phase {
        AnalysisPhase::StructureDiscovery => {
            "You are a repository structure investigator.\n\
             You must do all work directly with tools (Cwd, LS, Glob, Read, Grep).\n\
             Do not delegate to Task or any sub-agent.\n\n\
             Required workflow:\n\
             1) Call Cwd and LS on repository root.\n\
             2) Discover manifests/configs with Glob (json/toml/yaml/md).\n\
             3) Read only files that were discovered in step 2 (never assume a manifest exists).\n\
             4) Read likely entrypoints for each language stack found.\n\
             5) Provide only verified findings with concrete file paths.\n\n\
             Output sections:\n\
             - Repository Shape\n\
             - Runtime and Build Stack\n\
             - Entry Points (verified)\n\
             - Unknowns"
        }
        AnalysisPhase::ArchitectureTrace => {
            "You are an architecture tracing specialist.\n\
             You must do all work directly with tools (Read, Grep, Glob, LS).\n\
             Do not delegate to Task or any sub-agent.\n\n\
             Required workflow:\n\
             1) Use Grep to locate module boundaries, service layers, handlers, state stores.\n\
             2) Read concrete implementation files across major components.\n\
             3) Trace data flow and integration points with explicit file evidence.\n\
             4) Any uncertain statement must be marked unknown.\n\n\
             Output sections:\n\
             - Architecture Overview\n\
             - Component Map (with files)\n\
             - Data and Control Flow\n\
             - Risks and Unknowns"
        }
        AnalysisPhase::ConsistencyCheck => {
            "You are a consistency verifier.\n\
             You must verify claims against concrete file reads and grep evidence.\n\
             Do not delegate to Task or any sub-agent.\n\n\
             Required workflow:\n\
             1) Re-open high-impact files cited previously.\n\
             2) Re-run targeted grep for disputed/important claims.\n\
             3) Label each major claim as VERIFIED, UNVERIFIED, or CONTRADICTED.\n\n\
             Output sections:\n\
             - Verified Claims (with evidence)\n\
             - Unverified Claims\n\
             - Contradictions\n\
             - Additional Evidence Needed"
        }
    }
}

pub(super) fn analysis_phase_worker_prompt(phase: AnalysisPhase) -> String {
    let base = analysis_phase_system_prompt(phase);
    format!(
        "{base}\n\n\
         Worker-mode requirements:\n\
         - You are one layer within a multi-layer phase.\n\
         - Use targeted read-only tools, then produce a final written summary for this layer.\n\
         - Do NOT wait for other layers to satisfy global quotas.\n\
         - Stop once your layer objective is covered with concrete file evidence.\n\
         - Avoid repetitive LS/Glob loops; prefer Read/Grep on high-signal files.\n\
         - Keep tool usage compact: usually <= 8 tool calls for this layer unless blocked.\n\
         - If enough evidence is collected, immediately provide the summary instead of continuing exploration."
    )
}

pub(super) fn analysis_phase_system_prompt_with_quota(
    phase: AnalysisPhase,
    quota: &AnalysisToolQuota,
    previous_failures: &[String],
) -> String {
    let base = analysis_phase_system_prompt(phase);
    let required = if quota.required_tools.is_empty() {
        "(none)".to_string()
    } else {
        quota.required_tools.join(", ")
    };
    let previous = if previous_failures.is_empty() {
        "none".to_string()
    } else {
        previous_failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    };
    format!(
        "{base}\n\n\
         Hard requirements for this phase:\n\
         - Minimum total tool calls: {min_total}\n\
         - Minimum Read calls: {min_read}\n\
         - Minimum search calls (Grep/Glob): {min_search}\n\
         - Required tools that must appear: {required}\n\
         - Previous gate failures: {previous}\n\n\
         Tool hygiene requirements:\n\
         - Always provide required arguments for each tool.\n\
         - Prefer targeted paths; avoid broad workspace scans unless necessary.\n\
         - If a previous call failed due missing args, fix the call format before continuing.\n\n\
         If requirements were not met previously, DO NOT finish yet. \
         Continue with concrete tool calls until all requirements are satisfied.",
        min_total = quota.min_total_calls,
        min_read = quota.min_read_calls,
        min_search = quota.min_search_calls,
    )
}

pub(super) fn evaluate_analysis_quota(
    capture: &PhaseCapture,
    quota: &AnalysisToolQuota,
) -> Vec<String> {
    let mut failures = Vec::new();

    if capture.tool_calls < quota.min_total_calls {
        failures.push(format!(
            "tool_calls {} < required {}",
            capture.tool_calls, quota.min_total_calls
        ));
    }
    if capture.read_calls < quota.min_read_calls {
        failures.push(format!(
            "read_calls {} < required {}",
            capture.read_calls, quota.min_read_calls
        ));
    }

    let has_core_evidence = capture.read_calls >= quota.min_read_calls
        && capture.tool_calls >= quota.min_total_calls.saturating_sub(1)
        && !capture.observed_paths.is_empty();
    let search_calls = capture.search_calls();
    if search_calls < quota.min_search_calls && !has_core_evidence {
        failures.push(format!(
            "search_calls {} < required {}",
            search_calls, quota.min_search_calls
        ));
    }

    for required in &quota.required_tools {
        if capture.tool_call_count(required) == 0 {
            failures.push(format!("required tool '{}' not used", required));
        }
    }

    failures
}

pub(super) fn build_phase_summary_from_evidence(
    phase: AnalysisPhase,
    capture: &PhaseCapture,
) -> String {
    let mut summary_lines = Vec::new();
    summary_lines.push(format!("### {} (Evidence Fallback)", phase.title()));
    summary_lines.push(format!(
        "- Captured tool evidence: tool_calls={}, read_calls={}, grep_calls={}, glob_calls={}, ls_calls={}, cwd_calls={}",
        capture.tool_calls,
        capture.read_calls,
        capture.grep_calls,
        capture.glob_calls,
        capture.ls_calls,
        capture.cwd_calls
    ));

    let observed = join_sorted_paths(&capture.observed_paths, 15);
    summary_lines.push("- Observed paths:".to_string());
    if observed == "(none)" {
        summary_lines.push("  - (none)".to_string());
    } else {
        for line in observed.lines() {
            summary_lines.push(format!("  - {}", line));
        }
    }

    summary_lines.push("- Evidence highlights:".to_string());
    if capture.evidence_lines.is_empty() {
        summary_lines.push("  - No per-tool evidence lines captured.".to_string());
    } else {
        for item in capture.evidence_lines.iter().take(10) {
            summary_lines.push(format!("  - {}", truncate_for_log(item, 220)));
        }
    }

    summary_lines.join("\n")
}

pub(super) fn condense_phase_summary_for_context(summary: &str, max_chars: usize) -> String {
    let mut kept = Vec::<String>::new();
    for raw in summary.lines() {
        let line = raw.trim_end();
        let compact = line.trim();
        if compact.is_empty() {
            continue;
        }
        if compact.starts_with("- Read files:")
            || compact.starts_with("Read files:")
            || compact.contains("  - Read files:")
            || compact.starts_with("- Evidence highlights:")
            || compact.starts_with("- Captured tool evidence:")
            || compact.starts_with("- Observed paths:")
            || compact.starts_with("Observed paths:")
            || compact.contains("Chunk summaries merged")
            || compact.contains("Evidence Fallback")
            || compact.starts_with("### ")
            || compact.starts_with("- [")
        {
            continue;
        }
        if compact.starts_with("- D:/")
            || compact.starts_with("- C:/")
            || compact.starts_with("- /")
            || compact.starts_with("D:/")
            || compact.starts_with("C:/")
            || compact.starts_with("/")
        {
            continue;
        }
        kept.push(line.to_string());
        if kept.len() >= 24 {
            break;
        }
    }

    if kept.is_empty() {
        return truncate_for_log(summary, max_chars.max(300));
    }
    truncate_for_log(&kept.join("\n"), max_chars.max(300))
}

pub(super) fn build_synthesis_phase_block(
    phase_summaries: &[String],
    max_chars: usize,
    max_lines_per_phase: usize,
) -> String {
    if phase_summaries.is_empty() {
        return "No phase summaries were produced.".to_string();
    }

    let mut blocks = Vec::<String>::new();
    for summary in phase_summaries.iter().take(6) {
        let condensed = condense_phase_summary_for_context(summary, max_chars / 2);
        let trimmed_lines = condensed
            .lines()
            .take(max_lines_per_phase.max(2))
            .collect::<Vec<_>>()
            .join("\n");
        blocks.push(trimmed_lines);
    }
    truncate_for_log(&blocks.join("\n\n"), max_chars.max(300))
}

pub(super) fn build_synthesis_chunk_block(
    chunk_summaries: &[ChunkSummaryRecord],
    max_chars: usize,
) -> String {
    if chunk_summaries.is_empty() {
        return "No chunk summaries were produced.".to_string();
    }

    let mut component_counts = BTreeMap::<String, usize>::new();
    for record in chunk_summaries {
        *component_counts
            .entry(record.component.clone())
            .or_insert(0) += 1;
    }

    let mut ranked_components = component_counts.into_iter().collect::<Vec<_>>();
    ranked_components.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut lines = Vec::<String>::new();
    lines.push(format!(
        "- Chunk coverage: {} summaries across {} components",
        chunk_summaries.len(),
        ranked_components.len()
    ));
    for (component, count) in ranked_components.iter().take(8) {
        lines.push(format!("  - {}: {} chunks", component, count));
    }
    lines.push("- Sample chunk findings:".to_string());
    for record in chunk_summaries.iter().take(10) {
        lines.push(format!(
            "  - {} [{}]: {}",
            record.chunk_id,
            record.component,
            truncate_for_log(&record.summary, 140)
        ));
    }

    truncate_for_log(&lines.join("\n"), max_chars.max(300))
}

pub(super) fn build_synthesis_evidence_block(
    evidence_lines: &[String],
    max_lines: usize,
    max_line_chars: usize,
) -> String {
    if evidence_lines.is_empty() {
        return "- No tool evidence captured.".to_string();
    }

    evidence_lines
        .iter()
        .take(max_lines.max(1))
        .map(|line| truncate_for_log(line, max_line_chars.max(80)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn should_rewrite_synthesis_output(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let line_count = text.lines().count();
    let marker_hits = [
        "evidence fallback",
        "captured tool evidence",
        "chunk summaries merged",
        "tool_calls=",
        "read_calls=",
        "observed paths:",
        "### structure discovery",
        "### architecture trace",
        "### consistency check",
        "[analysis:",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count();

    marker_hits >= 2 || line_count > 220 || text.len() > 16_000
}

pub(super) fn build_synthesis_rewrite_prompt(user_request: &str, draft: &str) -> String {
    let is_chinese = user_request
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let clipped = truncate_for_log(draft, 40_000);

    if is_chinese {
        return format!(
            "Rewrite the draft below into a user-facing final project analysis.\n\
             Respond in Chinese.\n\
             Requirements:\n\
             1) Keep factual content, but remove raw tool logs, phase fallback dumps, chunk lists, and tool_calls/read_calls counters.\n\
             2) Keep clear structure; suggested sections: Project Snapshot, Architecture, Verified Facts, Risks, Unknowns.\n\
             3) Do not invent paths; mark uncertain items as Unknown.\n\
             4) Keep it concise but complete, within about 120 lines.\n\n\
             User request:\n{}\n\n\
             Draft:\n{}",
            user_request, clipped
        );
    }

    format!(
        "Rewrite the draft below into a user-facing final project analysis.\n\
         Requirements:\n\
         1) Keep factual content, but remove raw tool logs, phase fallback dumps, chunk lists, and tool_calls/read_calls counters.\n\
         2) Keep clear structure; suggested sections: Project Snapshot, Architecture, Verified Facts, Risks, Unknowns.\n\
         3) Do not invent paths; mark uncertain items as Unknown.\n\
         4) Keep it concise but complete, within about 120 lines.\n\n\
         User request:\n{}\n\n\
         Draft:\n{}",
        user_request, clipped
    )
}

pub(super) fn sample_paths_with_prefix(
    paths: &HashSet<String>,
    prefix: &str,
    limit: usize,
) -> Vec<String> {
    let normalized_prefix = prefix
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    let mut items = paths
        .iter()
        .filter_map(|p| normalize_candidate_path(p))
        .filter(|p| p.starts_with(&normalized_prefix))
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items.into_iter().take(limit.max(1)).collect()
}

pub(super) fn observed_root_buckets(
    paths: &HashSet<String>,
    root_limit: usize,
    sample_limit: usize,
) -> Vec<(String, usize, Vec<String>)> {
    let mut buckets = BTreeMap::<String, Vec<String>>::new();
    for raw in paths {
        let Some(normalized) = normalize_candidate_path(raw) else {
            continue;
        };
        let mut segments = normalized
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != ".");
        let first = match segments.next() {
            Some(value) => value,
            None => continue,
        };
        let root = if first.ends_with(':') {
            segments.next().unwrap_or(first)
        } else {
            first
        };
        if root.is_empty() {
            continue;
        }
        buckets
            .entry(root.to_string())
            .or_default()
            .push(normalized.clone());
    }

    let mut ranked = buckets.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(root_limit.max(1))
        .map(|(root, mut items)| {
            items.sort();
            items.dedup();
            let count = items.len();
            let samples = items
                .into_iter()
                .take(sample_limit.max(1))
                .collect::<Vec<_>>();
            (root, count, samples)
        })
        .collect()
}

pub(super) fn sanitize_warning_for_report(warning: &str, project_root: &std::path::Path) -> String {
    let root = project_root.to_string_lossy().replace('\\', "/");
    warning
        .replace('\\', "/")
        .replace(&root, "<project_root>")
        .replace("<project_root>//", "<project_root>/")
}

pub(super) fn build_deterministic_analysis_fallback_report(
    request: &str,
    project_root: &std::path::Path,
    ledger: &AnalysisLedger,
    coverage_report: &AnalysisCoverageReport,
    targets: EffectiveAnalysisTargets,
    synthesis_error: Option<&str>,
) -> String {
    let project_name = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("project");

    let mut lines = Vec::<String>::new();
    lines.push("Project Analysis".to_string());
    lines.push(String::new());
    lines.push("Project Snapshot".to_string());
    lines.push(format!("- Request: {}", truncate_for_log(request, 120)));
    lines.push(format!("- Repository: {}", project_name));
    lines.push(format!(
        "- Evidence: indexed_files={}, observed_paths={}, sampled_read_files={}, test_files_total={}, test_files_read={}",
        coverage_report.inventory_total_files,
        ledger.observed_paths.len(),
        coverage_report.sampled_read_files,
        coverage_report.test_files_total,
        coverage_report.test_files_read
    ));
    lines.push(format!(
        "- Coverage: observed={:.2}% (target {:.2}%), read-depth={:.2}% (target {:.2}%), tests={:.2}% (target {:.2}%)",
        coverage_report.coverage_ratio * 100.0,
        targets.coverage_ratio * 100.0,
        coverage_report.sampled_read_ratio * 100.0,
        targets.sampled_read_ratio * 100.0,
        coverage_report.test_coverage_ratio * 100.0,
        targets.test_coverage_ratio * 100.0
    ));

    if let Some(inventory) = ledger.inventory.as_ref() {
        let mut language_counts = BTreeMap::<String, usize>::new();
        let mut component_counts = BTreeMap::<String, usize>::new();
        for item in &inventory.items {
            *language_counts.entry(item.language.clone()).or_insert(0) += 1;
            *component_counts.entry(item.component.clone()).or_insert(0) += 1;
        }
        let mut top_languages = language_counts.into_iter().collect::<Vec<_>>();
        top_languages.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut top_components = component_counts.into_iter().collect::<Vec<_>>();
        top_components.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        if !top_languages.is_empty() {
            lines.push(format!(
                "- Indexed languages: {}",
                top_languages
                    .iter()
                    .take(6)
                    .map(|(lang, count)| format!("{} ({})", lang, count))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !top_components.is_empty() {
            lines.push(format!(
                "- Indexed components: {}",
                top_components
                    .iter()
                    .take(8)
                    .map(|(component, count)| format!("{} ({})", component, count))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    lines.push(String::new());
    lines.push("Architecture".to_string());
    let architecture_scopes = [
        ("Python core/CLI", "src/plan_cascade/"),
        ("MCP server", "mcp_server/"),
        ("Desktop Rust backend", "desktop/src-tauri/src/"),
        ("Desktop web frontend", "desktop/src/"),
        ("Tests", "tests/"),
    ];
    let mut architecture_hits = 0usize;
    for (label, prefix) in architecture_scopes {
        let samples = sample_paths_with_prefix(&ledger.observed_paths, prefix, 3);
        if !samples.is_empty() {
            architecture_hits += 1;
            lines.push(format!("- {}: {}", label, samples.join(", ")));
        }
    }
    if architecture_hits == 0 {
        let root_buckets = observed_root_buckets(&ledger.observed_paths, 6, 2);
        if root_buckets.is_empty() {
            lines.push("- No major component boundaries were confidently observed.".to_string());
        } else {
            lines.push("- Dominant repository roots from observed evidence:".to_string());
            for (root, count, samples) in root_buckets {
                let sample_text = if samples.is_empty() {
                    "(no sample)".to_string()
                } else {
                    samples.join(", ")
                };
                lines.push(format!(
                    "- {} ({} files observed): {}",
                    root, count, sample_text
                ));
            }
        }
    }

    lines.push(String::new());
    lines.push("Verified Facts".to_string());
    let key_candidates = [
        "README.md",
        "pyproject.toml",
        "desktop/package.json",
        "desktop/src-tauri/Cargo.toml",
        "src/plan_cascade/cli/main.py",
        "src/plan_cascade/core/orchestrator.py",
        "mcp_server/server.py",
        "tests/test_orchestrator.py",
        "desktop/src-tauri/tests/integration/mod.rs",
    ];
    let mut fact_count = 0usize;
    for candidate in key_candidates {
        if is_observed_path(candidate, &ledger.observed_paths) {
            fact_count += 1;
            lines.push(format!("- Observed: {}", candidate));
        }
    }
    let dynamic_key_candidates = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "README.md",
        "README_zh.md",
        "CMakeLists.txt",
        "Makefile",
    ];
    for candidate in dynamic_key_candidates {
        if fact_count >= 12 {
            break;
        }
        if is_observed_path(candidate, &ledger.observed_paths)
            && !lines.iter().any(|line| line.ends_with(candidate))
        {
            fact_count += 1;
            lines.push(format!("- Observed: {}", candidate));
        }
    }
    if fact_count == 0 {
        let mut sampled = ledger.read_paths.iter().cloned().collect::<Vec<_>>();
        sampled.sort();
        sampled.dedup();
        if sampled.is_empty() {
            lines.push("- No high-confidence key files were read.".to_string());
        } else {
            lines.push(format!(
                "- Representative read files: {}",
                sampled.into_iter().take(10).collect::<Vec<_>>().join(", ")
            ));
        }
    }

    lines.push(String::new());
    lines.push("Risks".to_string());
    let mut risk_count = 0usize;
    if let Some(err) = synthesis_error {
        risk_count += 1;
        lines.push(format!(
            "- Synthesis model call failed; deterministic fallback report used: {}",
            truncate_for_log(err, 180)
        ));
    }
    if coverage_report.coverage_ratio < targets.coverage_ratio {
        risk_count += 1;
        lines.push(format!(
            "- Observed coverage below target: {:.2}% < {:.2}%",
            coverage_report.coverage_ratio * 100.0,
            targets.coverage_ratio * 100.0
        ));
    }
    if coverage_report.sampled_read_ratio < targets.sampled_read_ratio {
        risk_count += 1;
        lines.push(format!(
            "- Read-depth below target: {:.2}% < {:.2}%",
            coverage_report.sampled_read_ratio * 100.0,
            targets.sampled_read_ratio * 100.0
        ));
    }
    if coverage_report.test_coverage_ratio < targets.test_coverage_ratio {
        risk_count += 1;
        lines.push(format!(
            "- Test coverage below target: {:.2}% < {:.2}%",
            coverage_report.test_coverage_ratio * 100.0,
            targets.test_coverage_ratio * 100.0
        ));
    }
    for warning in ledger.warnings.iter().take(4) {
        risk_count += 1;
        let sanitized = sanitize_warning_for_report(warning, project_root);
        lines.push(format!("- {}", truncate_for_log(&sanitized, 200)));
    }
    if risk_count == 0 {
        lines.push(
            "- No high-confidence structural risks were detected from collected evidence."
                .to_string(),
        );
    }

    lines.push(String::new());
    lines.push("Unknowns".to_string());
    let mut unknown_count = 0usize;
    let has_test_evidence = coverage_report.test_files_total > 0
        || sample_paths_with_prefix(&ledger.observed_paths, "tests/", 1)
            .first()
            .is_some()
        || sample_paths_with_prefix(&ledger.observed_paths, "test/", 1)
            .first()
            .is_some()
        || ledger
            .observed_paths
            .iter()
            .any(|path| looks_like_test_path(path));
    if !has_test_evidence {
        unknown_count += 1;
        lines.push("- Test implementation details were not sufficiently sampled.".to_string());
    }

    if coverage_report.sampled_read_ratio < 0.60 {
        unknown_count += 1;
        lines.push(
            "- Deep module-level logic may need additional targeted reads for full confidence."
                .to_string(),
        );
    }

    let has_budget_warning = ledger.warnings.iter().any(|warning| {
        let lower = warning.to_ascii_lowercase();
        lower.contains("maximum iterations")
            || lower.contains("token budget")
            || lower.contains("rate limited")
    });
    if has_budget_warning {
        unknown_count += 1;
        lines.push(
            "- Some areas may require a rerun after resolving provider/runtime limits.".to_string(),
        );
    }
    if unknown_count == 0 {
        lines.push("- Detailed business logic per module is not expanded in this concise fallback; raw evidence is available in analysis artifacts.".to_string());
    }

    lines.join("\n")
}

pub(super) fn join_sorted_paths(paths: &HashSet<String>, limit: usize) -> String {
    if paths.is_empty() {
        return "(none)".to_string();
    }
    let mut items: Vec<String> = paths.iter().cloned().collect();
    items.sort();
    items.into_iter().take(limit).collect::<Vec<_>>().join("\n")
}

pub(super) fn build_test_evidence_block(
    inventory: &FileInventory,
    observed_paths: &HashSet<String>,
    read_paths: &HashSet<String>,
) -> String {
    let mut indexed_tests = inventory
        .items
        .iter()
        .filter(|item| item.is_test)
        .map(|item| item.path.clone())
        .collect::<Vec<_>>();
    indexed_tests.sort();

    let mut observed_tests = indexed_tests
        .iter()
        .filter(|path| is_observed_path(path, observed_paths))
        .cloned()
        .collect::<Vec<_>>();
    observed_tests.sort();
    observed_tests.dedup();

    let mut read_tests = indexed_tests
        .iter()
        .filter(|path| read_paths.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    read_tests.sort();
    read_tests.dedup();

    let sample = |items: &[String], limit: usize| -> String {
        if items.is_empty() {
            "(none)".to_string()
        } else {
            items
                .iter()
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        }
    };

    format!(
        "- indexed_test_files={}\n- observed_test_files={}\n- read_test_files={}\n- sample_observed_tests={}\n- sample_read_tests={}",
        indexed_tests.len(),
        observed_tests.len(),
        read_tests.len(),
        sample(&observed_tests, 12),
        sample(&read_tests, 12),
    )
}
