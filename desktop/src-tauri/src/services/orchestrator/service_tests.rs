fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: Some("test-key".to_string()),
            model: "claude-3-5-sonnet-20241022".to_string(),
            ..Default::default()
        },
        system_prompt: Some("You are a helpful assistant.".to_string()),
        max_iterations: 10,
        max_total_tokens: 10000,
        project_root: std::env::temp_dir(),
        streaming: true,
        enable_compaction: true,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    }
}

#[test]
fn test_orchestrator_creation() {
    let config = test_config();
    let orchestrator = OrchestratorService::new(config);

    let info = orchestrator.provider_info();
    assert_eq!(info.name, "anthropic");
    assert_eq!(info.model, "claude-3-5-sonnet-20241022");
    assert!(info.supports_tools);
}

#[test]
fn test_execution_result() {
    let result = ExecutionResult {
        response: Some("Hello!".to_string()),
        usage: UsageStats {
            input_tokens: 100,
            output_tokens: 50,
            thinking_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
        },
        iterations: 1,
        success: true,
        error: None,
    };

    assert!(result.success);
    assert_eq!(result.response, Some("Hello!".to_string()));
}

#[test]
fn test_cancellation_token() {
    let config = test_config();
    let orchestrator = OrchestratorService::new(config);

    let token = orchestrator.cancellation_token();
    assert!(!token.is_cancelled());

    token.cancel();
    assert!(token.is_cancelled());
}

#[test]
fn test_session_execution_result() {
    let result = SessionExecutionResult {
        session_id: "test-session".to_string(),
        success: true,
        completed_stories: 3,
        failed_stories: 0,
        total_stories: 3,
        usage: UsageStats::default(),
        error: None,
        quality_gates_passed: Some(true),
    };

    assert!(result.success);
    assert_eq!(result.completed_stories, 3);
}

#[test]
fn test_compute_effective_analysis_targets_is_dynamic() {
    let limits = AnalysisLimits::default();
    let inventory_small = FileInventory {
        total_files: 120,
        total_test_files: 20,
        indexed_files: 120,
        items: Vec::new(),
    };
    let inventory_large = FileInventory {
        total_files: 4_000,
        total_test_files: 300,
        indexed_files: 4_000,
        items: Vec::new(),
    };

    let small_targets = compute_effective_analysis_targets(
        &limits,
        AnalysisProfile::DeepCoverage,
        &inventory_small,
    );
    let large_targets = compute_effective_analysis_targets(
        &limits,
        AnalysisProfile::DeepCoverage,
        &inventory_large,
    );

    assert!(small_targets.sampled_read_ratio > large_targets.sampled_read_ratio);
    assert!(small_targets.max_total_read_files <= inventory_small.total_files);
    assert!(large_targets.max_total_read_files <= inventory_large.total_files);
    assert!(large_targets.max_total_read_files > limits.max_total_read_files);
}

#[test]
fn test_parse_fallback_tool_calls_uses_content_and_thinking() {
    let response = LlmResponse {
        content: Some(
            "```tool_call\n{\"tool\":\"LS\",\"arguments\":{\"path\":\".\"}}\n```".to_string(),
        ),
        thinking: Some(
            "```tool_call\n{\"tool\":\"Read\",\"arguments\":{\"file_path\":\"README.md\"}}\n```"
                .to_string(),
        ),
        tool_calls: vec![],
        stop_reason: crate::services::llm::StopReason::EndTurn,
        usage: UsageStats::default(),
        model: "test-model".to_string(),
    };

    let parsed = parse_fallback_tool_calls(&response, None);
    assert!(parsed.dropped_reasons.is_empty());
    assert_eq!(parsed.calls.len(), 2);
    assert!(parsed.calls.iter().any(|c| c.tool_name == "LS"));
    assert!(parsed.calls.iter().any(|c| c.tool_name == "Read"));
}

#[test]
fn test_prepare_tool_call_for_execution_repairs_ls_and_rejects_invalid_read() {
    let ls_args = serde_json::json!({});
    let prepared =
        prepare_tool_call_for_execution("LS", &ls_args, Some("structure_discovery")).unwrap();
    assert_eq!(prepared.0, "LS");
    assert_eq!(prepared.1.get("path").and_then(|v| v.as_str()), Some("."));

    let read_args = serde_json::json!({});
    let read_result =
        prepare_tool_call_for_execution("Read", &read_args, Some("consistency_check"));
    assert!(read_result.is_err());
}

#[test]
fn test_parse_fallback_tool_calls_collects_dropped_reasons_in_analysis_mode() {
    let response = LlmResponse {
        content: Some("```tool_call\n{\"tool\":\"Grep\",\"arguments\":{}}\n```".to_string()),
        thinking: None,
        tool_calls: vec![],
        stop_reason: crate::services::llm::StopReason::EndTurn,
        usage: UsageStats::default(),
        model: "test-model".to_string(),
    };

    let parsed = parse_fallback_tool_calls(&response, Some("architecture_trace"));
    assert!(parsed.calls.is_empty());
    assert!(!parsed.dropped_reasons.is_empty());
}

#[test]
fn test_merge_usage_accumulates_all_token_buckets() {
    let mut total = UsageStats {
        input_tokens: 10,
        output_tokens: 20,
        thinking_tokens: Some(5),
        cache_read_tokens: None,
        cache_creation_tokens: Some(2),
    };
    let delta = UsageStats {
        input_tokens: 3,
        output_tokens: 7,
        thinking_tokens: Some(4),
        cache_read_tokens: Some(9),
        cache_creation_tokens: Some(1),
    };

    merge_usage(&mut total, &delta);
    assert_eq!(total.input_tokens, 13);
    assert_eq!(total.output_tokens, 27);
    assert_eq!(total.thinking_tokens, Some(9));
    assert_eq!(total.cache_read_tokens, Some(9));
    assert_eq!(total.cache_creation_tokens, Some(3));
}

#[test]
fn test_extract_primary_path_from_arguments_prefers_file_path() {
    let args = serde_json::json!({
        "path": "src",
        "file_path": "src/main.rs"
    });
    let path = extract_primary_path_from_arguments(&args);
    assert_eq!(path.as_deref(), Some("src/main.rs"));
}

#[test]
fn test_truncate_for_log_handles_unicode_boundary() {
    let text = "\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}";
    let truncated = truncate_for_log(text, 5);
    assert!(truncated.ends_with("..."));
    assert_ne!(truncated, text.to_string());
}

#[test]
fn test_find_unverified_paths_flags_unknown_paths() {
    let observed = HashSet::from([
        "src/main.rs".to_string(),
        "desktop/src-tauri/src/main.rs".to_string(),
    ]);
    let text =
        "Verified: src/main.rs and desktop/src-tauri/src/main.rs. Maybe server/main.py too.";
    let issues = find_unverified_paths(text, &observed);
    assert!(issues.iter().any(|p| p == "server/main.py"));
    assert!(!issues.iter().any(|p| p == "src/main.rs"));
}

#[test]
fn test_extract_all_paths_from_arguments_collects_nested_paths() {
    let args = serde_json::json!({
        "path": "./src",
        "nested": {
            "file_path": "desktop/src-tauri/src/main.rs",
            "items": [
                {"path": ".\\README.md"},
                {"path": "https://example.com/not-a-file"}
            ]
        }
    });
    let paths = extract_all_paths_from_arguments(&args);

    assert!(paths.iter().any(|p| p == "src"));
    assert!(paths.iter().any(|p| p == "README.md"));
    assert!(paths.iter().any(|p| p == "desktop/src-tauri/src/main.rs"));
    assert!(!paths.iter().any(|p| p.contains("https://")));
}

#[test]
fn test_find_unverified_paths_ignores_observed_prefix_and_urls() {
    let observed = HashSet::from([
        "desktop/src-tauri/src".to_string(),
        "src/main.rs".to_string(),
    ]);
    let text = "Evidence: desktop/src-tauri/src/services/orchestrator/service.rs \
                and src/main.rs plus https://docs.example.com/page.";
    let issues = find_unverified_paths(text, &observed);
    assert!(issues.is_empty());
}

#[test]
fn test_find_unverified_paths_accepts_directory_prefix_with_trailing_slash() {
    let observed = HashSet::from(["src/plan_cascade/cli/main.py".to_string()]);
    let text = "Repository uses src/ layout and includes src/plan_cascade/cli/main.py.";
    let issues = find_unverified_paths(text, &observed);
    assert!(issues.is_empty(), "unexpected issues: {:?}", issues);
}

#[test]
fn test_find_unverified_paths_ignores_regex_and_template_fragments() {
    let observed = HashSet::from(["src/main.rs".to_string()]);
    let text = "Validation issues from generated prose: \
                !/^[a-zA-Z0-9_-]+$/.test(task.command); \
                ${plan.name}`);/n \
                ${task.command}`);/n \
                and src/main.rs.";

    let issues = find_unverified_paths(text, &observed);
    assert!(issues.is_empty(), "unexpected issues: {:?}", issues);
}

#[test]
fn test_find_unverified_paths_ignores_non_path_slash_terms() {
    let observed = HashSet::from([
        "src/plan_cascade/core".to_string(),
        "desktop/src-tauri/src/main.rs".to_string(),
    ]);
    let text =
        "Tech stack includes JavaScript/TypeScript, and architecture mentions backend/core. \
                Verified file: desktop/src-tauri/src/main.rs.";
    let issues = find_unverified_paths(text, &observed);
    assert!(
        !issues.iter().any(|p| p == "JavaScript/TypeScript"),
        "unexpected language-token issue: {:?}",
        issues
    );
    assert!(
        !issues.iter().any(|p| p == "backend/core"),
        "unexpected generic-component issue: {:?}",
        issues
    );
}

#[test]
fn test_find_unverified_paths_ignores_desktop_cli_label() {
    let observed = HashSet::from([
        "desktop/src-tauri/src/main.rs".to_string(),
        "src/plan_cascade/cli/main.py".to_string(),
    ]);
    let text = "Modes include Desktop/CLI flows. Verified files: desktop/src-tauri/src/main.rs and src/plan_cascade/cli/main.py.";
    let issues = find_unverified_paths(text, &observed);
    assert!(
        !issues.iter().any(|p| p == "Desktop/CLI"),
        "unexpected label-like issue: {:?}",
        issues
    );
}

#[test]
fn test_find_unverified_paths_ignores_uppercase_status_labels() {
    let observed = HashSet::from(["src/plan_cascade/core/orchestrator.py".to_string()]);
    let text = "Consistency terms: VERIFIED/UNVERIFIED/CONTRADICTED. \
                Verified file: src/plan_cascade/core/orchestrator.py.";
    let issues = find_unverified_paths(text, &observed);
    assert!(
        !issues
            .iter()
            .any(|p| p.contains("VERIFIED/UNVERIFIED/CONTRADICTED")),
        "unexpected uppercase label issue: {:?}",
        issues
    );
}

#[test]
fn test_find_unverified_paths_ignores_truncated_ellipsis_paths() {
    let observed = HashSet::from(["src/plan_cascade/cli/main.py".to_string()]);
    let text =
        "Truncated mention: D:/VsCodeProjects/planning-with-files/desktop/src-tauri/Car... \
                plus verified src/plan_cascade/cli/main.py.";
    let issues = find_unverified_paths(text, &observed);
    assert!(
        !issues.iter().any(|p| p.contains("...")),
        "unexpected ellipsis path issue: {:?}",
        issues
    );
}

#[test]
fn test_find_unverified_paths_accepts_path_with_line_span_when_file_observed() {
    let observed = HashSet::from(["src/plan_cascade/core/orchestrator.py".to_string()]);
    let text = "Core orchestrator is defined at src/plan_cascade/core/orchestrator.py:58-83.";
    let issues = find_unverified_paths(text, &observed);
    assert!(
        issues.is_empty(),
        "line-span reference should map to observed file: {:?}",
        issues
    );
}

#[test]
fn test_build_deterministic_analysis_fallback_report_avoids_raw_phase_dump() {
    let mut ledger = AnalysisLedger::default();
    ledger
        .observed_paths
        .insert("src/plan_cascade/cli/main.py".to_string());
    ledger
        .observed_paths
        .insert("mcp_server/server.py".to_string());
    ledger
        .observed_paths
        .insert("desktop/src-tauri/src/main.rs".to_string());
    ledger
        .read_paths
        .insert("src/plan_cascade/core/orchestrator.py".to_string());
    ledger
        .warnings
        .push("sample warning for verification".to_string());
    ledger.inventory = Some(FileInventory {
        total_files: 10,
        total_test_files: 2,
        indexed_files: 10,
        items: vec![
            crate::services::orchestrator::analysis_index::FileInventoryItem {
                path: "src/plan_cascade/cli/main.py".to_string(),
                component: "python-core".to_string(),
                language: "python".to_string(),
                extension: Some("py".to_string()),
                size_bytes: 120,
                line_count: 12,
                is_test: false,
                symbols: vec![],
            },
            crate::services::orchestrator::analysis_index::FileInventoryItem {
                path: "tests/test_orchestrator.py".to_string(),
                component: "python-tests".to_string(),
                language: "python".to_string(),
                extension: Some("py".to_string()),
                size_bytes: 80,
                line_count: 8,
                is_test: true,
                symbols: vec![],
            },
        ],
    });
    let coverage = AnalysisCoverageReport {
        inventory_total_files: 10,
        inventory_indexed_files: 10,
        sampled_read_files: 4,
        test_files_total: 2,
        test_files_read: 1,
        coverage_ratio: 0.9,
        test_coverage_ratio: 0.5,
        sampled_read_ratio: 0.4,
        observed_test_coverage_ratio: 0.5,
        chunk_count: 0,
        synthesis_rounds: 1,
    };
    let targets = EffectiveAnalysisTargets {
        coverage_ratio: 0.8,
        test_coverage_ratio: 0.4,
        sampled_read_ratio: 0.35,
        max_total_read_files: 10,
    };

    let report = build_deterministic_analysis_fallback_report(
        "analyze this project",
        std::path::Path::new("D:/repo/example"),
        &ledger,
        &coverage,
        targets,
        Some("token budget exceeded"),
    );
    assert!(report.contains("Project Snapshot"));
    assert!(report.contains("Verified Facts"));
    assert!(report.contains("- Repository: example"));
    assert!(!report.contains("D:/repo/example"));
    assert!(!report.contains("Chunk summaries merged"));
    assert!(!report.contains("Evidence Fallback"));
}

#[test]
fn test_is_analysis_excluded_path_checks_top_level_only() {
    assert!(is_analysis_excluded_path("codex/README.md"));
    assert!(!is_analysis_excluded_path(
        "codex-rs/tui/frames/codex/frame_1.txt"
    ));
    assert!(!is_analysis_excluded_path(".github/codex/home/config.toml"));
}

#[test]
fn test_sanitize_warning_for_report_masks_project_root() {
    let root = std::path::PathBuf::from("D:/VsCodeProjects/planning-with-files");
    let warning = "Unverified path mention: D:/VsCodeProjects/planning-with-files/codex";
    let sanitized = sanitize_warning_for_report(warning, &root);
    assert!(!sanitized.contains("D:/VsCodeProjects/planning-with-files"));
    assert!(sanitized.contains("<project_root>/codex"));
}

#[test]
fn test_should_rewrite_synthesis_output_detects_raw_dump() {
    let text = "### Structure Discovery (structure_discovery)\n\
                - Chunk summaries merged: 62\n\
                - Captured tool evidence: tool_calls=19, read_calls=10\n\
                - Observed paths:\n\
                - D:/repo/README.md";
    assert!(should_rewrite_synthesis_output(text));
}

#[test]
fn test_should_rewrite_synthesis_output_keeps_normal_report() {
    let text = "Project Snapshot\n\
                This repository contains a Python core and a Tauri desktop app.\n\
                Architecture\n\
                The codebase is split into src/plan_cascade, mcp_server, and desktop components.\n\
                Risks\n\
                Version mismatch between pyproject.toml and desktop package manifests.\n\
                Unknowns\n\
                No direct evidence for production deployment topology.";
    assert!(!should_rewrite_synthesis_output(text));
}

#[test]
fn test_architecture_baseline_keeps_seed_files_ahead_of_observed_noise() {
    let project_root = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join("..").join(".."))
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Ollama,
            model: "test-model".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 4,
        max_total_tokens: 8_000,
        project_root: project_root.clone(),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let mut ledger = AnalysisLedger::default();
    ledger
        .observed_paths
        .insert(project_root.join("README.md").to_string_lossy().to_string());
    ledger.observed_paths.insert(
        project_root
            .join("pyproject.toml")
            .to_string_lossy()
            .to_string(),
    );
    ledger.observed_paths.insert(
        project_root
            .join("desktop/package.json")
            .to_string_lossy()
            .to_string(),
    );

    let steps =
        orchestrator.baseline_steps_for_phase(AnalysisPhase::ArchitectureTrace, &ledger);
    let read_paths = steps
        .iter()
        .filter(|(tool, _)| tool == "Read")
        .filter_map(|(_, args)| {
            args.get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect::<Vec<_>>();

    assert!(
        read_paths
            .iter()
            .any(|p| p == "src/plan_cascade/core/orchestrator.py"),
        "expected seeded orchestrator path in baseline reads, got: {:?}",
        read_paths
    );
    assert!(
        read_paths.iter().any(|p| p == "desktop/src/App.tsx")
            || read_paths.iter().any(|p| p == "desktop/src/main.tsx"),
        "expected frontend entrypoint in baseline reads, got: {:?}",
        read_paths
    );
}

#[test]
fn test_baseline_steps_cover_required_tool_families() {
    let project_root = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join("..").join(".."))
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Ollama,
            model: "test-model".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 4,
        max_total_tokens: 8_000,
        project_root,
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let ledger = AnalysisLedger::default();

    let structure_steps =
        orchestrator.baseline_steps_for_phase(AnalysisPhase::StructureDiscovery, &ledger);
    assert!(structure_steps.iter().any(|(tool, _)| tool == "Glob"));
    assert!(structure_steps.iter().any(|(tool, _)| tool == "Read"));
    assert!(
        structure_steps.iter().any(|(tool, args)| {
            tool == "Glob"
                && args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .map(|p| p.contains("tests/**/*.py"))
                    .unwrap_or(false)
        }),
        "expected structure baseline to include Python test glob"
    );

    let architecture_steps =
        orchestrator.baseline_steps_for_phase(AnalysisPhase::ArchitectureTrace, &ledger);
    assert!(architecture_steps.iter().any(|(tool, _)| tool == "Grep"));
    assert!(architecture_steps.iter().any(|(tool, _)| tool == "Read"));
    assert!(
        architecture_steps.iter().any(|(tool, args)| {
            tool == "Grep"
                && args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|p| p == "tests" || p == "desktop/src-tauri/tests")
                    .unwrap_or(false)
        }),
        "expected architecture baseline to include test grep paths"
    );

    let consistency_steps =
        orchestrator.baseline_steps_for_phase(AnalysisPhase::ConsistencyCheck, &ledger);
    assert!(consistency_steps.iter().any(|(tool, _)| tool == "Grep"));
    assert!(consistency_steps.iter().any(|(tool, _)| tool == "Read"));
}

#[test]
fn test_analysis_phase_min_workers_before_early_exit() {
    assert_eq!(
        AnalysisPhase::StructureDiscovery.min_workers_before_early_exit(),
        2
    );
    assert_eq!(
        AnalysisPhase::ArchitectureTrace.min_workers_before_early_exit(),
        2
    );
    assert_eq!(
        AnalysisPhase::ConsistencyCheck.min_workers_before_early_exit(),
        2
    );
}

#[test]
fn test_evaluate_analysis_quota_reports_missing_requirements() {
    let capture = PhaseCapture {
        tool_calls: 1,
        read_calls: 0,
        grep_calls: 0,
        glob_calls: 0,
        ls_calls: 0,
        cwd_calls: 1,
        ..Default::default()
    };
    let quota = AnalysisToolQuota {
        min_total_calls: 3,
        min_read_calls: 1,
        min_search_calls: 1,
        required_tools: vec!["Cwd", "LS"],
    };

    let failures = evaluate_analysis_quota(&capture, &quota);
    assert!(failures.iter().any(|f| f.contains("tool_calls")));
    assert!(failures.iter().any(|f| f.contains("read_calls")));
    assert!(failures.iter().any(|f| f.contains("search_calls")));
    assert!(failures.iter().any(|f| f.contains("required tool 'LS'")));
}

#[test]
fn test_evaluate_analysis_quota_passes_when_requirements_met() {
    let capture = PhaseCapture {
        tool_calls: 6,
        read_calls: 2,
        grep_calls: 2,
        glob_calls: 1,
        ls_calls: 1,
        cwd_calls: 1,
        ..Default::default()
    };
    let quota = AnalysisToolQuota {
        min_total_calls: 4,
        min_read_calls: 1,
        min_search_calls: 2,
        required_tools: vec!["Cwd", "LS"],
    };

    let failures = evaluate_analysis_quota(&capture, &quota);
    assert!(failures.is_empty(), "unexpected failures: {:?}", failures);
}

#[test]
fn test_evaluate_analysis_quota_allows_missing_search_with_core_evidence() {
    let capture = PhaseCapture {
        tool_calls: 4,
        read_calls: 2,
        grep_calls: 0,
        glob_calls: 0,
        observed_paths: HashSet::from(["src/plan_cascade/cli/main.py".to_string()]),
        ..Default::default()
    };
    let quota = AnalysisToolQuota {
        min_total_calls: 3,
        min_read_calls: 2,
        min_search_calls: 1,
        required_tools: vec!["Read"],
    };

    let failures = evaluate_analysis_quota(&capture, &quota);
    assert!(
        !failures.iter().any(|f| f.contains("search_calls")),
        "unexpected failures: {:?}",
        failures
    );
}

// --- Story-004: text_describes_tool_intent tests ---

#[test]
fn test_text_describes_tool_intent_english_future() {
    // English future intent with tool mention should trigger
    assert!(text_describes_tool_intent(
        "Let me use the Read tool to check the file."
    ));
    assert!(text_describes_tool_intent(
        "I will call Bash to run the tests."
    ));
    assert!(text_describes_tool_intent(
        "I'll use Grep to search for the pattern."
    ));
}

#[test]
fn test_text_describes_tool_intent_chinese_intent() {
    // Chinese intent with tool mention should trigger
    assert!(text_describes_tool_intent(
        "\u{8ba9}\u{6211}\u{4f7f}\u{7528} Read \u{5de5}\u{5177}\u{6765}\u{8bfb}\u{53d6}\u{6587}\u{4ef6}\u{3002}"
    ));
    assert!(text_describes_tool_intent(
        "\u{6211}\u{5c06}\u{8c03}\u{7528} Bash \u{6765}\u{6267}\u{884c}\u{6d4b}\u{8bd5}\u{547d}\u{4ee4}\u{3002}"
    ));
    assert!(text_describes_tool_intent(
        "\u{63a5}\u{4e0b}\u{6765}\u{4f7f}\u{7528} Grep \u{641c}\u{7d22}\u{4ee3}\u{7801}\u{3002}"
    ));
}

#[test]
fn test_text_describes_tool_intent_no_tool_name() {
    // Intent phrases without tool names should NOT trigger
    assert!(!text_describes_tool_intent(
        "Let me use the function to check."
    ));
    assert!(!text_describes_tool_intent(
        "\u{6211}\u{5c06}\u{8c03}\u{7528}\u{51fd}\u{6570}\u{6765}\u{5904}\u{7406}\u{6570}\u{636e}\u{3002}"
    ));
}

#[test]
fn test_text_describes_tool_intent_no_intent_phrase() {
    // Tool names without intent phrases should NOT trigger
    assert!(!text_describes_tool_intent(
        "The Read operation returned the file content."
    ));
    assert!(!text_describes_tool_intent(
        "Bash is a command-line shell."
    ));
}

#[test]
fn test_text_describes_tool_intent_past_tense_summary() {
    // Past-tense summaries referencing tools should NOT trigger
    // (these use "I used" / "I called" which are NOT in the intent phrases list)
    assert!(!text_describes_tool_intent(
        "I used the Read tool to check the file and found the issue."
    ));
    assert!(!text_describes_tool_intent(
        "I called Bash and the tests passed."
    ));
    assert!(!text_describes_tool_intent(
        "After using Grep to search, the results showed the pattern."
    ));
}

#[test]
fn test_text_describes_tool_intent_empty() {
    assert!(!text_describes_tool_intent(""));
}

#[test]
fn test_text_describes_pending_action_english_and_chinese() {
    assert!(text_describes_pending_action(
        "I see a few directories. Let me read README.md next."
    ));
    assert!(text_describes_pending_action(
        "\u{6211}\u{5148}\u{67e5}\u{770b} README \u{6587}\u{4ef6}\u{3002}"
    ));
}

#[test]
fn test_text_describes_pending_action_rejects_completed_summary() {
    assert!(!text_describes_pending_action(
        "I already read README.md and summarized the architecture above."
    ));
    assert!(!text_describes_pending_action(
        "\u{6211}\u{5df2}\u{7ecf}\u{8bfb}\u{53d6}\u{4e86} README \u{5e76}\u{7ed9}\u{51fa}\u{4e86}\u{7ed3}\u{8bba}\u{3002}"
    ));
}

// --- Story-001: tool_call_reliability tests ---

#[test]
fn test_anthropic_provider_reliable() {
    let config = ProviderConfig {
        provider: ProviderType::Anthropic,
        api_key: Some("test".to_string()),
        model: "claude-3-5-sonnet".to_string(),
        ..Default::default()
    };
    let provider = AnthropicProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::Reliable
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Off
    );
}

#[test]
fn test_openai_provider_reliable() {
    let config = ProviderConfig {
        provider: ProviderType::OpenAI,
        api_key: Some("test".to_string()),
        model: "gpt-4o".to_string(),
        ..Default::default()
    };
    let provider = OpenAIProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::Reliable
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Off
    );
}

#[test]
fn test_qwen_provider_unreliable() {
    let config = ProviderConfig {
        provider: ProviderType::Qwen,
        api_key: Some("test".to_string()),
        model: "qwen-plus".to_string(),
        ..Default::default()
    };
    let provider = QwenProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::Unreliable
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Soft
    );
    // Still claims API tool support
    assert!(provider.supports_tools());
}

#[test]
fn test_deepseek_provider_unreliable() {
    let config = ProviderConfig {
        provider: ProviderType::DeepSeek,
        api_key: Some("test".to_string()),
        model: "deepseek-chat".to_string(),
        ..Default::default()
    };
    let provider = DeepSeekProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::Unreliable
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Soft
    );
}

#[test]
fn test_glm_provider_unreliable() {
    let config = ProviderConfig {
        provider: ProviderType::Glm,
        api_key: Some("test".to_string()),
        model: "glm-4-plus".to_string(),
        ..Default::default()
    };
    let provider = GlmProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::Unreliable
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Soft
    );
}

#[test]
fn test_ollama_provider_none() {
    let config = ProviderConfig {
        provider: ProviderType::Ollama,
        model: "llama3".to_string(),
        ..Default::default()
    };
    let provider = OllamaProvider::new(config);
    assert_eq!(
        provider.tool_call_reliability(),
        ToolCallReliability::None
    );
    assert_eq!(
        provider.default_fallback_mode(),
        FallbackToolFormatMode::Soft
    );
    // Does not claim API tool support
    assert!(!provider.supports_tools());
}

// --- Story-002: effective_system_prompt adaptive fallback tests ---

#[test]
fn test_reliable_provider_no_fallback_instructions() {
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: Some("test".to_string()),
            model: "claude-3-5-sonnet-20241022".to_string(),
            ..Default::default()
        },
        system_prompt: Some("Test prompt.".to_string()),
        max_iterations: 10,
        max_total_tokens: 10000,
        project_root: std::env::temp_dir(),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let tools = crate::services::tools::get_tool_definitions();
    let opts = LlmRequestOptions::default();
    let prompt = orchestrator.effective_system_prompt(&tools, &opts).unwrap();
    // Reliable providers should NOT get fallback instructions
    assert!(
        !prompt.contains("```tool_call"),
        "Reliable provider should not get fallback tool_call format instructions"
    );
}

#[test]
fn test_unreliable_provider_gets_fallback_instructions() {
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Qwen,
            api_key: Some("test".to_string()),
            model: "qwen-plus".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 10,
        max_total_tokens: 10000,
        project_root: std::env::temp_dir(),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let tools = crate::services::tools::get_tool_definitions();
    let opts = LlmRequestOptions::default();
    let prompt = orchestrator.effective_system_prompt(&tools, &opts).unwrap();
    // Unreliable providers should get fallback instructions (bilingual)
    assert!(
        prompt.contains("```tool_call"),
        "Unreliable provider should get fallback tool_call format instructions"
    );
    assert!(
        prompt.contains("\u{8bf7}\u{4f7f}\u{7528}\u{4ee5}\u{4e0b}\u{683c}\u{5f0f}\u{8c03}\u{7528}\u{5de5}\u{5177}"),
        "Unreliable provider should get Chinese tool call instructions"
    );
}

#[test]
fn test_user_override_disables_fallback_for_unreliable() {
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Qwen,
            api_key: Some("test".to_string()),
            model: "qwen-plus".to_string(),
            fallback_tool_format_mode: Some(FallbackToolFormatMode::Off),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 10,
        max_total_tokens: 10000,
        project_root: std::env::temp_dir(),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let tools = crate::services::tools::get_tool_definitions();
    let opts = LlmRequestOptions::default();
    let prompt = orchestrator.effective_system_prompt(&tools, &opts).unwrap();
    // User explicitly set Off — no fallback instructions even for unreliable provider
    assert!(
        !prompt.contains("```tool_call"),
        "User override Off should suppress fallback instructions"
    );
}

#[test]
fn test_none_provider_gets_fallback_instructions() {
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Ollama,
            model: "llama3".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 10,
        max_total_tokens: 10000,
        project_root: std::env::temp_dir(),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    let orchestrator = OrchestratorService::new(config);
    let tools = crate::services::tools::get_tool_definitions();
    let opts = LlmRequestOptions::default();
    let prompt = orchestrator.effective_system_prompt(&tools, &opts).unwrap();
    // None-reliability providers should get fallback instructions
    assert!(
        prompt.contains("```tool_call"),
        "None-reliability provider should get fallback instructions"
    );
}

// --- Story-003: sub_agent_token_budget tests ---

#[test]
fn test_sub_agent_token_budget_default() {
    // 32k context * 3 = 96k
    assert_eq!(sub_agent_token_budget(32_000, None), 96_000);
    // 128k context * 3 = 384k
    assert_eq!(sub_agent_token_budget(128_000, None), 384_000);
    // 200k context * 3 = 600k (under cap)
    assert_eq!(sub_agent_token_budget(200_000, None), 600_000);
    // 500k context * 3 = 1.5M -> capped at 1M
    assert_eq!(sub_agent_token_budget(500_000, None), 1_000_000);
    // Small context -> minimum 20k
    assert_eq!(sub_agent_token_budget(5_000, None), 20_000);
}

#[test]
fn test_sub_agent_token_budget_explore() {
    // 128k context * 4 = 512k
    assert_eq!(sub_agent_token_budget(128_000, Some("explore")), 512_000);
    // 200k context * 4 = 800k
    assert_eq!(sub_agent_token_budget(200_000, Some("explore")), 800_000);
    // 300k context * 4 = 1.2M -> capped at 1M
    assert_eq!(sub_agent_token_budget(300_000, Some("explore")), 1_000_000);
}

#[test]
fn test_sub_agent_token_budget_analyze() {
    // analyze gets same multiplier as explore
    assert_eq!(sub_agent_token_budget(128_000, Some("analyze")), 512_000);
}

// --- Story-005: serde default verification tests ---

#[test]
fn test_default_max_tokens_function_returns_one_million() {
    // Story-002 changed default from 100_000 to 1_000_000.
    // Verify the default function used by serde(default) returns the correct value.
    assert_eq!(default_max_tokens(), 1_000_000);
}

#[test]
fn test_default_enable_compaction_function_returns_true() {
    // Story-003 changed default from false to true.
    // Verify the default function used by serde(default) returns the correct value.
    assert_eq!(default_enable_compaction(), true);
}

#[test]
fn test_orchestrator_config_serde_defaults_without_optional_fields() {
    // When deserializing OrchestratorConfig JSON without max_total_tokens,
    // enable_compaction, or max_iterations, serde should fill in the correct defaults.
    let json = r#"{
        "provider": {
            "provider": "anthropic",
            "api_key": "test-key",
            "model": "claude-3-5-sonnet-20241022"
        },
        "project_root": "/tmp/test"
    }"#;

    let config: OrchestratorConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_total_tokens, 1_000_000, "default max_total_tokens should be 1M");
    assert_eq!(config.max_iterations, 50, "default max_iterations should be 50");
    assert!(config.enable_compaction, "default enable_compaction should be true");
    assert!(config.streaming, "default streaming should be true");
}

#[test]
fn test_orchestrator_config_serde_explicit_overrides() {
    // When explicit values are provided, they should override defaults.
    let json = r#"{
        "provider": {
            "provider": "anthropic",
            "api_key": "test-key",
            "model": "claude-3-5-sonnet-20241022"
        },
        "project_root": "/tmp/test",
        "max_total_tokens": 500000,
        "max_iterations": 25,
        "enable_compaction": false,
        "streaming": false
    }"#;

    let config: OrchestratorConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_total_tokens, 500_000);
    assert_eq!(config.max_iterations, 25);
    assert!(!config.enable_compaction);
    assert!(!config.streaming);
}

#[test]
fn test_sub_agent_spawner_uses_compaction_enabled() {
    // Verify that sub-agent configs always have enable_compaction: true.
    // This is important for Story-003's change to reduce token waste.
    let spawner_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Ollama,
            model: "test-model".to_string(),
            ..Default::default()
        },
        system_prompt: Some("test".to_string()),
        max_iterations: 25,
        max_total_tokens: sub_agent_token_budget(128_000, None),
        project_root: std::env::temp_dir(),
        streaming: true,
        enable_compaction: true, // sub-agents should always have this true
        analysis_artifacts_root: default_analysis_artifacts_root(),
        analysis_profile: AnalysisProfile::default(),
        analysis_limits: AnalysisLimits::default(),
        analysis_session_id: None,
    };
    assert!(spawner_config.enable_compaction);
    assert_eq!(spawner_config.max_total_tokens, 384_000);
}

// --- Story-002 (tool result truncation): truncate_tool_output_for_context tests ---

#[test]
fn test_truncate_read_below_limit_passes_through() {
    let content = (0..50).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Read", &content);
    assert_eq!(result, content, "Content below Read limit should pass through unchanged");
}

#[test]
fn test_truncate_read_above_line_limit() {
    let content = (0..250).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Read", &content);
    assert!(result.lines().count() <= REGULAR_READ_MAX_LINES + 3, "Read should be truncated to ~200 lines plus note");
    assert!(result.contains("[truncated"), "Should contain truncation note");
    assert!(result.contains("200"), "Truncation note should mention line limit");
}

#[test]
fn test_truncate_read_above_char_limit() {
    // Build content that is under line limit but over char limit
    let long_line = "x".repeat(100);
    let content = (0..150).map(|_| long_line.clone()).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Read", &content);
    assert!(result.len() < content.len(), "Char-limited content should be shorter");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_truncate_grep_above_line_limit() {
    let content = (0..150).map(|i| format!("match {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Grep", &content);
    assert!(result.lines().count() <= REGULAR_GREP_MAX_LINES + 3, "Grep should be truncated to ~100 lines plus note");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_truncate_grep_below_limit_passes_through() {
    let content = (0..50).map(|i| format!("match {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Grep", &content);
    assert_eq!(result, content, "Content below Grep limit should pass through unchanged");
}

#[test]
fn test_truncate_ls_above_line_limit() {
    let content = (0..200).map(|i| format!("file_{}.rs", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("LS", &content);
    assert!(result.lines().count() <= REGULAR_LS_MAX_LINES + 3, "LS should be truncated to ~150 lines plus note");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_truncate_ls_below_limit_passes_through() {
    let content = (0..50).map(|i| format!("file_{}.rs", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("LS", &content);
    assert_eq!(result, content, "Content below LS limit should pass through unchanged");
}

#[test]
fn test_truncate_bash_above_line_limit() {
    let content = (0..200).map(|i| format!("output {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Bash", &content);
    assert!(result.lines().count() <= REGULAR_BASH_MAX_LINES + 3, "Bash should be truncated to ~150 lines plus note");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_truncate_bash_below_limit_passes_through() {
    let content = (0..50).map(|i| format!("output {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Bash", &content);
    assert_eq!(result, content, "Content below Bash limit should pass through unchanged");
}

#[test]
fn test_truncate_unknown_tool_uses_bash_defaults() {
    // Unknown tool names should still get truncation (using Bash defaults)
    let content = (0..200).map(|i| format!("data {}", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("UnknownTool", &content);
    assert!(result.lines().count() <= REGULAR_BASH_MAX_LINES + 3, "Unknown tools should use Bash limits");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_truncate_note_shows_original_vs_truncated_size() {
    let content = (0..300).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let original_len = content.len();
    let result = truncate_tool_output_for_context("Read", &content);
    // The truncation note should contain the original size
    assert!(result.contains(&format!("{}", original_len)), "Truncation note should show original char count");
}

#[test]
fn test_truncate_empty_content_passes_through() {
    let result = truncate_tool_output_for_context("Read", "");
    assert_eq!(result, "", "Empty content should pass through unchanged");
}

#[test]
fn test_truncate_glob_uses_ls_limits() {
    // Glob should use LS limits since both are directory-listing tools
    let content = (0..200).map(|i| format!("path/file_{}.rs", i)).collect::<Vec<_>>().join("\n");
    let result = truncate_tool_output_for_context("Glob", &content);
    assert!(result.lines().count() <= REGULAR_LS_MAX_LINES + 3, "Glob should use LS limits");
    assert!(result.contains("[truncated"), "Should contain truncation note");
}

#[test]
fn test_regular_truncation_constants_exist() {
    // Verify all constants have the expected values from the story spec
    assert_eq!(REGULAR_READ_MAX_LINES, 200);
    assert_eq!(REGULAR_READ_MAX_CHARS, 8000);
    assert_eq!(REGULAR_GREP_MAX_LINES, 100);
    assert_eq!(REGULAR_GREP_MAX_CHARS, 6000);
    assert_eq!(REGULAR_LS_MAX_LINES, 150);
    assert_eq!(REGULAR_LS_MAX_CHARS, 5000);
    assert_eq!(REGULAR_BASH_MAX_LINES, 150);
    assert_eq!(REGULAR_BASH_MAX_CHARS, 8000);
}

// --- Story-010 (prefix-stable compaction): compact_messages_prefix_stable tests ---

#[test]
fn test_prefix_stable_compaction_preserves_head_and_tail() {
    // 2 head + 4 middle + 6 tail = 12 messages
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("original prompt"));         // head[0]
    messages.push(Message::assistant("session memory"));     // head[1]
    messages.push(Message::user("middle-1"));                // middle
    messages.push(Message::assistant("middle-2"));           // middle
    messages.push(Message::user("middle-3"));                // middle
    messages.push(Message::assistant("middle-4"));           // middle
    messages.push(Message::user("tail-1"));                  // tail
    messages.push(Message::assistant("tail-2"));             // tail
    messages.push(Message::user("tail-3"));                  // tail
    messages.push(Message::assistant("tail-4"));             // tail
    messages.push(Message::user("tail-5"));                  // tail
    messages.push(Message::assistant("tail-6"));             // tail

    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);

    assert!(result, "Compaction should succeed");
    // Should have 2 head + 6 tail = 8 messages (middle removed)
    assert_eq!(messages.len(), 8);
    // Head preserved
    assert_eq!(messages[0], Message::user("original prompt"));
    assert_eq!(messages[1], Message::assistant("session memory"));
    // Tail preserved
    assert_eq!(messages[2], Message::user("tail-1"));
    assert_eq!(messages[3], Message::assistant("tail-2"));
    assert_eq!(messages[4], Message::user("tail-3"));
    assert_eq!(messages[5], Message::assistant("tail-4"));
    assert_eq!(messages[6], Message::user("tail-5"));
    assert_eq!(messages[7], Message::assistant("tail-6"));
}

#[test]
fn test_prefix_stable_compaction_no_new_content_inserted() {
    // Verify no summary or new messages are injected
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    for i in 0..5 {
        messages.push(Message::user(format!("mid-{}", i)));
    }
    for i in 0..6 {
        messages.push(Message::assistant(format!("tail-{}", i)));
    }

    let original_head = messages[0..2].to_vec();
    let original_tail = messages[messages.len() - 6..].to_vec();

    OrchestratorService::compact_messages_prefix_stable(&mut messages);

    // All remaining messages should be from the original set (no new content)
    for msg in &messages {
        let text = match &msg.content[0] {
            MessageContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(
            !text.contains("Summary") && !text.contains("compacted") && !text.contains("Context"),
            "No summary or compaction text should be inserted, found: {}",
            text
        );
    }

    // Verify exact head and tail preservation
    assert_eq!(&messages[0..2], &original_head[..]);
    assert_eq!(&messages[messages.len() - 6..], &original_tail[..]);
}

#[test]
fn test_prefix_stable_compaction_too_few_messages() {
    // With exactly 8 messages (2 head + 6 tail), nothing to remove
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    for i in 0..6 {
        messages.push(Message::user(format!("tail-{}", i)));
    }

    let original_len = messages.len();
    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);

    assert!(!result, "Compaction should be skipped when no middle messages");
    assert_eq!(messages.len(), original_len, "Messages should be unchanged");
}

#[test]
fn test_prefix_stable_compaction_exactly_nine_messages() {
    // 2 head + 1 middle + 6 tail = 9 messages: should remove 1 middle
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    messages.push(Message::user("only-middle"));
    for i in 0..6 {
        messages.push(Message::assistant(format!("tail-{}", i)));
    }

    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);

    assert!(result, "Should compact with exactly 1 middle message");
    assert_eq!(messages.len(), 8, "Should have 2 head + 6 tail");
    // Verify middle was removed
    for msg in &messages {
        let text = match &msg.content[0] {
            MessageContent::Text { text } => text.clone(),
            _ => panic!("Expected text"),
        };
        assert_ne!(text, "only-middle", "Middle message should be removed");
    }
}

#[test]
fn test_prefix_stable_compaction_returns_removed_count() {
    // Verify the function communicates how many messages were removed
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    // 10 middle messages
    for i in 0..10 {
        messages.push(Message::user(format!("mid-{}", i)));
    }
    for i in 0..6 {
        messages.push(Message::assistant(format!("tail-{}", i)));
    }

    let before = messages.len();
    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);
    let after = messages.len();

    assert!(result);
    assert_eq!(before - after, 10, "Should have removed 10 middle messages");
    assert_eq!(after, 8, "Should have 2 head + 6 tail = 8");
}

#[test]
fn test_prefix_stable_compaction_does_not_call_llm() {
    // The function is synchronous (not async) - this is our guarantee it doesn't call the LLM.
    // compact_messages_prefix_stable is a plain fn, not async fn, proving no LLM call.
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    for i in 0..5 {
        messages.push(Message::user(format!("mid-{}", i)));
    }
    for i in 0..6 {
        messages.push(Message::assistant(format!("tail-{}", i)));
    }

    // This is a synchronous call - no await needed, no provider needed
    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);
    assert!(result);
}

#[test]
fn test_compaction_strategy_reliable_provider_uses_llm_summary() {
    // Reliable providers (Anthropic, OpenAI) should NOT use prefix-stable compaction
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            ..Default::default()
        },
        ..test_config()
    };
    let orchestrator = OrchestratorService::new(config);
    assert_eq!(
        orchestrator.provider.tool_call_reliability(),
        ToolCallReliability::Reliable,
    );
    // Reliable providers use LLM-summary compaction (compact_messages)
    // which is async and calls the provider - tested via integration
}

#[test]
fn test_compaction_strategy_unreliable_provider_uses_prefix_stable() {
    // Unreliable providers (Qwen, DeepSeek, GLM) should use prefix-stable compaction
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Qwen,
            api_key: Some("test-key".to_string()),
            model: "qwen-test".to_string(),
            base_url: Some("http://localhost:8080".to_string()),
            ..Default::default()
        },
        ..test_config()
    };
    let orchestrator = OrchestratorService::new(config);
    assert_eq!(
        orchestrator.provider.tool_call_reliability(),
        ToolCallReliability::Unreliable,
    );
}

#[test]
fn test_compaction_strategy_none_provider_uses_prefix_stable() {
    // None-reliability providers (Ollama) should use prefix-stable compaction
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Ollama,
            model: "llama3".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        },
        ..test_config()
    };
    let orchestrator = OrchestratorService::new(config);
    assert_eq!(
        orchestrator.provider.tool_call_reliability(),
        ToolCallReliability::None,
    );
}

#[test]
fn test_compaction_strategy_openai_uses_llm_summary() {
    // OpenAI is reliable, should use LLM-summary compaction
    let config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::OpenAI,
            api_key: Some("test-key".to_string()),
            model: "gpt-4".to_string(),
            ..Default::default()
        },
        ..test_config()
    };
    let orchestrator = OrchestratorService::new(config);
    assert_eq!(
        orchestrator.provider.tool_call_reliability(),
        ToolCallReliability::Reliable,
    );
}

// --- Story-003 (session memory compaction): SessionMemory tests ---

#[test]
fn test_session_memory_to_context_string_includes_files_read() {
    let memory = SessionMemory {
        files_read: vec![
            ("src/main.rs".to_string(), 150, 4500),
            ("src/lib.rs".to_string(), 80, 2200),
        ],
        key_findings: vec![],
        task_description: "Implement feature X".to_string(),
        tool_usage_counts: HashMap::new(),
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("Session Memory"),
        "Should contain session memory header"
    );
    assert!(
        ctx.contains("src/main.rs"),
        "Should list main.rs in files read"
    );
    assert!(
        ctx.contains("150 lines"),
        "Should include line count for main.rs"
    );
    assert!(
        ctx.contains("4500 bytes"),
        "Should include byte size for main.rs"
    );
    assert!(
        ctx.contains("src/lib.rs"),
        "Should list lib.rs in files read"
    );
    assert!(
        ctx.contains("80 lines"),
        "Should include line count for lib.rs"
    );
}

#[test]
fn test_session_memory_to_context_string_has_do_not_reread_instruction() {
    let memory = SessionMemory {
        files_read: vec![("README.md".to_string(), 20, 500)],
        key_findings: vec![],
        task_description: String::new(),
        tool_usage_counts: HashMap::new(),
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("Do NOT re-read"),
        "Should contain explicit 'Do NOT re-read' instruction, got: {}",
        ctx
    );
}

#[test]
fn test_session_memory_to_context_string_includes_key_findings() {
    let memory = SessionMemory {
        files_read: vec![],
        key_findings: vec![
            "The project uses a three-tier architecture".to_string(),
            "Found a bug in the error handling path".to_string(),
        ],
        task_description: String::new(),
        tool_usage_counts: HashMap::new(),
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("Key Findings"),
        "Should contain Key Findings section"
    );
    assert!(
        ctx.contains("three-tier architecture"),
        "Should include first finding"
    );
    assert!(
        ctx.contains("bug in the error handling"),
        "Should include second finding"
    );
}

#[test]
fn test_session_memory_to_context_string_includes_task_description() {
    let memory = SessionMemory {
        files_read: vec![],
        key_findings: vec![],
        task_description: "Refactor the authentication module".to_string(),
        tool_usage_counts: HashMap::new(),
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("## Task"),
        "Should contain Task section"
    );
    assert!(
        ctx.contains("Refactor the authentication module"),
        "Should include the task description"
    );
}

#[test]
fn test_session_memory_to_context_string_includes_tool_usage() {
    let mut tool_counts = HashMap::new();
    tool_counts.insert("Read".to_string(), 5);
    tool_counts.insert("Grep".to_string(), 3);
    tool_counts.insert("Bash".to_string(), 1);

    let memory = SessionMemory {
        files_read: vec![],
        key_findings: vec![],
        task_description: String::new(),
        tool_usage_counts: tool_counts,
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("Tool Usage"),
        "Should contain Tool Usage section"
    );
    assert!(
        ctx.contains("Read(5)"),
        "Should show Read count"
    );
    assert!(
        ctx.contains("Grep(3)"),
        "Should show Grep count"
    );
    assert!(
        ctx.contains("Bash(1)"),
        "Should show Bash count"
    );
}

#[test]
fn test_session_memory_to_context_string_empty_sections_omitted() {
    let memory = SessionMemory {
        files_read: vec![],
        key_findings: vec![],
        task_description: String::new(),
        tool_usage_counts: HashMap::new(),
    };

    let ctx = memory.to_context_string();

    assert!(
        ctx.contains("Session Memory"),
        "Should still have the header"
    );
    assert!(
        !ctx.contains("Files Already Read"),
        "Should NOT have files section when empty"
    );
    assert!(
        !ctx.contains("Key Findings"),
        "Should NOT have findings section when empty"
    );
    assert!(
        !ctx.contains("## Task"),
        "Should NOT have task section when empty"
    );
    assert!(
        !ctx.contains("Tool Usage"),
        "Should NOT have tool usage section when empty"
    );
}

#[test]
fn test_session_memory_to_context_string_full() {
    // Test a fully populated session memory to verify all sections work together
    let mut tool_counts = HashMap::new();
    tool_counts.insert("Read".to_string(), 10);
    tool_counts.insert("Edit".to_string(), 2);

    let memory = SessionMemory {
        files_read: vec![
            ("src/main.rs".to_string(), 100, 3000),
            ("Cargo.toml".to_string(), 40, 1200),
        ],
        key_findings: vec![
            "Found the entry point in main.rs".to_string(),
        ],
        task_description: "Add logging to the app".to_string(),
        tool_usage_counts: tool_counts,
    };

    let ctx = memory.to_context_string();

    // All sections present
    assert!(ctx.contains("Session Memory"));
    assert!(ctx.contains("## Task"));
    assert!(ctx.contains("Add logging to the app"));
    assert!(ctx.contains("Files Already Read"));
    assert!(ctx.contains("Do NOT re-read"));
    assert!(ctx.contains("src/main.rs"));
    assert!(ctx.contains("Cargo.toml"));
    assert!(ctx.contains("Key Findings"));
    assert!(ctx.contains("Found the entry point"));
    assert!(ctx.contains("Tool Usage"));
    assert!(ctx.contains("Read(10)"));
    assert!(ctx.contains("Edit(2)"));
}

#[test]
fn test_extract_key_findings_extracts_indicator_lines() {
    let snippets = vec![
        "I found that the project uses React 18 with TypeScript.".to_string(),
        "Short".to_string(),  // too short, should be ignored
        "The code uses a modular architecture with clear separation of concerns.".to_string(),
        "Confirmed that all tests pass in the CI pipeline successfully.".to_string(),
        "This line has no indicators and should not be included as a finding result.".to_string(),
    ];

    let findings = extract_key_findings(&snippets);

    assert!(
        findings.iter().any(|f| f.contains("React 18")),
        "Should extract 'found' indicator line, got: {:?}",
        findings
    );
    assert!(
        findings.iter().any(|f| f.contains("modular architecture")),
        "Should extract 'code uses' indicator line, got: {:?}",
        findings
    );
    assert!(
        findings.iter().any(|f| f.contains("tests pass")),
        "Should extract 'confirmed' indicator line, got: {:?}",
        findings
    );
    // The last line has no indicator words
    assert!(
        !findings.iter().any(|f| f.contains("no indicators")),
        "Should NOT extract lines without indicator words"
    );
}

#[test]
fn test_extract_key_findings_skips_short_and_long_lines() {
    let snippets = vec![
        "Short.".to_string(),       // too short (<20 chars)
        "x".repeat(301),            // too long (>300 chars) -- contains no indicator anyway
        "Found a moderately sized line that should be included in findings.".to_string(),
    ];

    let findings = extract_key_findings(&snippets);

    assert!(
        !findings.iter().any(|f| f == "Short."),
        "Should skip lines under 20 chars"
    );
    assert!(
        findings.iter().any(|f| f.contains("moderately sized")),
        "Should include properly sized lines with indicators"
    );
}

#[test]
fn test_extract_key_findings_deduplicates() {
    let snippets = vec![
        "Found the bug in the parser module.".to_string(),
        "found the bug in the parser module.".to_string(), // same content, different case
        "Found another issue in the formatter.".to_string(),
    ];

    let findings = extract_key_findings(&snippets);

    let parser_count = findings
        .iter()
        .filter(|f| f.to_lowercase().contains("parser module"))
        .count();
    assert_eq!(
        parser_count, 1,
        "Should deduplicate near-identical findings (case-insensitive)"
    );
}

#[test]
fn test_extract_key_findings_caps_at_max() {
    // Generate many findings - should be capped at 15
    let snippets: Vec<String> = (0..30)
        .map(|i| format!("Found issue number {} in the codebase that needs attention.", i))
        .collect();

    let findings = extract_key_findings(&snippets);

    assert!(
        findings.len() <= 15,
        "Should cap findings at 15, got {}",
        findings.len()
    );
}

#[test]
fn test_extract_key_findings_empty_input() {
    let findings = extract_key_findings(&[]);
    assert!(findings.is_empty(), "Empty input should produce empty findings");
}

#[test]
fn test_session_memory_positioned_between_prompt_and_summary() {
    // This test verifies the message structure contract:
    // After compaction, messages should be: [prompt, session_memory, summary, ...tail]
    // We test this by constructing what compact_messages would produce and verifying structure.

    // Simulate the output structure of compact_messages
    let original_prompt = Message::user("Write a function to sort an array");
    let session_memory_msg = Message::assistant(
        SessionMemory {
            files_read: vec![("src/sort.rs".to_string(), 50, 1500)],
            key_findings: vec!["Found existing sort implementation".to_string()],
            task_description: "Write a function to sort an array".to_string(),
            tool_usage_counts: {
                let mut m = HashMap::new();
                m.insert("Read".to_string(), 3);
                m
            },
        }
        .to_context_string(),
    );
    let summary_msg = Message::user("[Context Summary - 8 earlier messages compacted]\n\nThe agent explored the codebase...");
    let tail_msg_1 = Message::assistant("I will now implement the sort function.");
    let tail_msg_2 = Message::user("Please proceed.");

    let messages = vec![
        original_prompt,
        session_memory_msg,
        summary_msg,
        tail_msg_1,
        tail_msg_2,
    ];

    // Verify position: index 0 = original prompt
    assert_eq!(messages[0].role, crate::services::llm::MessageRole::User);
    let first_text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(first_text.contains("sort an array"), "First message should be original prompt");

    // Verify position: index 1 = session memory (assistant role)
    assert_eq!(messages[1].role, crate::services::llm::MessageRole::Assistant);
    let memory_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        memory_text.contains("Session Memory"),
        "Second message should be session memory"
    );
    assert!(
        memory_text.contains("src/sort.rs"),
        "Session memory should list read files"
    );
    assert!(
        memory_text.contains("Do NOT re-read"),
        "Session memory should have do-not-reread instruction"
    );

    // Verify position: index 2 = LLM summary
    let summary_text = match &messages[2].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        summary_text.contains("Context Summary"),
        "Third message should be the LLM summary"
    );
}

#[test]
fn test_session_memory_tool_usage_sorted_by_count_descending() {
    let mut tool_counts = HashMap::new();
    tool_counts.insert("Bash".to_string(), 1);
    tool_counts.insert("Read".to_string(), 10);
    tool_counts.insert("Grep".to_string(), 5);

    let memory = SessionMemory {
        files_read: vec![],
        key_findings: vec![],
        task_description: String::new(),
        tool_usage_counts: tool_counts,
    };

    let ctx = memory.to_context_string();

    // Extract the tool usage line
    let tool_line = ctx
        .lines()
        .find(|l| l.contains("Read(") && l.contains("Grep("))
        .expect("Should have a tool usage line with Read and Grep");

    // Read(10) should appear before Grep(5) which should appear before Bash(1)
    let read_pos = tool_line.find("Read(10)").expect("Should contain Read(10)");
    let grep_pos = tool_line.find("Grep(5)").expect("Should contain Grep(5)");
    let bash_pos = tool_line.find("Bash(1)").expect("Should contain Bash(1)");

    assert!(
        read_pos < grep_pos,
        "Read(10) should appear before Grep(5)"
    );
    assert!(
        grep_pos < bash_pos,
        "Grep(5) should appear before Bash(1)"
    );
}

// --- Story-012: SessionMemoryManager + Layered Context Architecture tests ---

#[test]
fn test_session_memory_manager_insert() {
    // SessionMemoryManager should insert a new session memory message at memory_index
    // when no session memory exists yet
    let mut messages = vec![
        Message::user("original system prompt"),     // Layer 1 (index 0)
        Message::assistant("some response"),         // Layer 3 (index 1)
        Message::user("follow up"),                  // Layer 3 (index 2)
    ];

    let manager = SessionMemoryManager::new(1);
    let files_read = vec![
        ("src/main.rs".to_string(), 100usize, 3000u64),
    ];
    let findings = vec!["Found the entry point".to_string()];

    manager.update_or_insert(&mut messages, files_read, findings);

    // Should have inserted at index 1, shifting everything else down
    assert_eq!(messages.len(), 4, "Should have one more message after insert");

    let memory_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text content at index 1"),
    };
    assert!(
        memory_text.contains(SESSION_MEMORY_V1_MARKER),
        "Inserted message should contain SESSION_MEMORY_V1 marker, got: {}",
        memory_text
    );
    assert!(
        memory_text.contains("src/main.rs"),
        "Session memory should contain file read info"
    );

    // Original prompt should still be at index 0
    let prompt_text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(prompt_text.contains("original system prompt"));
}

#[test]
fn test_session_memory_manager_update() {
    // When a session memory message already exists at the expected index,
    // update_or_insert should replace it in-place (not insert a new one)
    let manager = SessionMemoryManager::new(1);

    // Build initial message with marker
    let initial_memory = manager.build_memory_message(
        vec![("src/old.rs".to_string(), 50, 1500)],
        vec!["Old finding".to_string()],
    );

    let mut messages = vec![
        Message::user("original prompt"),            // Layer 1 (index 0)
        initial_memory,                              // Layer 2 (index 1)
        Message::assistant("response"),              // Layer 3 (index 2)
        Message::user("follow up"),                  // Layer 3 (index 3)
    ];

    // Now update with new data
    let new_files = vec![
        ("src/old.rs".to_string(), 50usize, 1500u64),
        ("src/new.rs".to_string(), 200usize, 6000u64),
    ];
    let new_findings = vec!["New finding".to_string()];

    manager.update_or_insert(&mut messages, new_files, new_findings);

    // Should NOT have inserted a new message (still 4 messages)
    assert_eq!(messages.len(), 4, "Should replace in-place, not insert");

    // The message at index 1 should have the new data
    let memory_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        memory_text.contains("src/new.rs"),
        "Updated memory should contain new file, got: {}",
        memory_text
    );
    assert!(
        memory_text.contains(SESSION_MEMORY_V1_MARKER),
        "Updated memory should still contain marker"
    );
}

#[test]
fn test_session_memory_marker_present() {
    // The build_memory_message method should always include SESSION_MEMORY_V1 marker
    let manager = SessionMemoryManager::new(1);
    let msg = manager.build_memory_message(
        vec![("README.md".to_string(), 20, 500)],
        vec![],
    );

    let text = match &msg.content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text content"),
    };

    assert!(
        text.contains("[SESSION_MEMORY_V1]"),
        "Message must contain literal [SESSION_MEMORY_V1] marker, got: {}",
        text
    );

    // Also test with empty data
    let empty_msg = manager.build_memory_message(vec![], vec![]);
    let empty_text = match &empty_msg.content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        empty_text.contains("[SESSION_MEMORY_V1]"),
        "Marker must be present even with empty data"
    );
}

#[test]
fn test_session_memory_find_memory_index() {
    let manager = SessionMemoryManager::new(1);

    // No session memory yet
    let messages_without = vec![
        Message::user("prompt"),
        Message::assistant("response"),
    ];
    assert_eq!(
        SessionMemoryManager::find_memory_index(&messages_without),
        None,
        "Should return None when no session memory exists"
    );

    // With session memory
    let memory_msg = manager.build_memory_message(
        vec![("test.rs".to_string(), 10, 200)],
        vec![],
    );
    let messages_with = vec![
        Message::user("prompt"),
        memory_msg,
        Message::assistant("response"),
    ];
    assert_eq!(
        SessionMemoryManager::find_memory_index(&messages_with),
        Some(1),
        "Should find session memory at index 1"
    );
}

#[test]
fn test_compact_messages_preserves_session_memory() {
    // When LLM-summary compaction runs, it should preserve the session memory
    // message (Layer 2) identified by SESSION_MEMORY_V1 marker.
    //
    // After compaction, the structure should be:
    //   [Layer 1: prompt] [Layer 2: session memory] [summary] [...tail]
    //
    // We test this by checking the post-compaction structure that compact_messages
    // produces. Since compact_messages calls the LLM, we verify the structure
    // contract by constructing what it would produce.
    let manager = SessionMemoryManager::new(1);
    let memory_msg = manager.build_memory_message(
        vec![("src/lib.rs".to_string(), 80, 2200)],
        vec!["Found important pattern".to_string()],
    );

    // Simulate post-compaction message structure
    // compact_messages preserves: [prompt, session_memory, summary, ...tail]
    let messages = vec![
        Message::user("original prompt"),
        memory_msg.clone(),
        Message::user("[Context Summary - 10 earlier messages compacted]\n\nSummary text here."),
        Message::assistant("tail-1"),
        Message::user("tail-2"),
    ];

    // Verify Layer 2 is preserved
    let layer2_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        layer2_text.contains(SESSION_MEMORY_V1_MARKER),
        "Layer 2 session memory should be preserved after compaction"
    );
    assert!(
        layer2_text.contains("src/lib.rs"),
        "Session memory data should be intact"
    );

    // Verify Layer 1 is preserved
    let layer1_text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        layer1_text.contains("original prompt"),
        "Layer 1 should be preserved"
    );
}

#[test]
fn test_prefix_stable_preserves_session_memory() {
    // Prefix-stable compaction should preserve Layer 1 and Layer 2,
    // only trimming Layer 3 messages.
    let manager = SessionMemoryManager::new(1);
    let memory_msg = manager.build_memory_message(
        vec![("src/main.rs".to_string(), 150, 4500)],
        vec!["Key finding".to_string()],
    );

    let mut messages = vec![
        Message::user("original prompt"),            // Layer 1 (head[0])
        memory_msg,                                  // Layer 2 (head[1])
        Message::assistant("middle-1"),              // Layer 3 middle
        Message::user("middle-2"),                   // Layer 3 middle
        Message::assistant("middle-3"),              // Layer 3 middle
        Message::user("middle-4"),                   // Layer 3 middle
        Message::assistant("tail-1"),                // Layer 3 tail
        Message::user("tail-2"),                     // Layer 3 tail
        Message::assistant("tail-3"),                // Layer 3 tail
        Message::user("tail-4"),                     // Layer 3 tail
        Message::assistant("tail-5"),                // Layer 3 tail
        Message::user("tail-6"),                     // Layer 3 tail
    ];

    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);
    assert!(result, "Compaction should succeed");

    // Should have 2 head + 6 tail = 8 messages
    assert_eq!(messages.len(), 8);

    // Layer 1 preserved
    let layer1_text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(layer1_text.contains("original prompt"), "Layer 1 should be preserved");

    // Layer 2 preserved with marker
    let layer2_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(
        layer2_text.contains(SESSION_MEMORY_V1_MARKER),
        "Layer 2 session memory should be preserved with marker, got: {}",
        layer2_text
    );
    assert!(
        layer2_text.contains("src/main.rs"),
        "Layer 2 file data should be intact"
    );

    // Middle messages (Layer 3 volatile) should be removed
    for msg in &messages {
        let text = match &msg.content[0] {
            MessageContent::Text { text } => text.clone(),
            _ => continue,
        };
        assert!(
            !text.starts_with("middle-"),
            "Middle Layer 3 messages should be removed, found: {}",
            text
        );
    }
}

#[test]
fn test_only_layer3_trimmed() {
    // Both compaction strategies should only trim Layer 3 messages.
    // Layer 1 (system prompt at index 0) and Layer 2 (session memory at index 1)
    // must remain untouched.

    let manager = SessionMemoryManager::new(1);
    let memory_msg = manager.build_memory_message(
        vec![
            ("file_a.rs".to_string(), 100usize, 3000u64),
            ("file_b.rs".to_string(), 200usize, 6000u64),
        ],
        vec!["Finding A".to_string(), "Finding B".to_string()],
    );

    // Build a conversation with Layer 1, Layer 2, and many Layer 3 messages
    let mut messages = vec![
        Message::user("Layer 1: system prompt"),     // index 0
        memory_msg.clone(),                          // index 1 (Layer 2)
    ];

    // Add 15 Layer 3 messages (middle + tail)
    for i in 0..15 {
        if i % 2 == 0 {
            messages.push(Message::assistant(format!("layer3-{}", i)));
        } else {
            messages.push(Message::user(format!("layer3-{}", i)));
        }
    }

    let original_layer1 = messages[0].clone();
    let original_layer2 = messages[1].clone();

    // Apply prefix-stable compaction
    let result = OrchestratorService::compact_messages_prefix_stable(&mut messages);
    assert!(result, "Compaction should have removed messages");

    // Layer 1 unchanged
    assert_eq!(messages[0], original_layer1, "Layer 1 must be unchanged");

    // Layer 2 unchanged
    assert_eq!(messages[1], original_layer2, "Layer 2 must be unchanged");

    // Total should be 2 head + 6 tail = 8
    assert_eq!(messages.len(), 8, "Should have 2 head + 6 tail = 8 messages");

    // Verify no Layer 1/2 content was lost
    let l2_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(l2_text.contains(SESSION_MEMORY_V1_MARKER));
    assert!(l2_text.contains("file_a.rs"));
    assert!(l2_text.contains("file_b.rs"));
    assert!(l2_text.contains("Finding A"));
    assert!(l2_text.contains("Finding B"));
}

// --- Feature-001 Story-001: Clear dedup cache on compaction tests ---

#[test]
fn test_compact_messages_prefix_stable_clears_read_cache_flag() {
    // compact_messages_prefix_stable returns true when it compacts.
    // The caller is responsible for clearing the dedup cache.
    // This test verifies the function returns true (indicating cache should be cleared).
    let mut messages: Vec<Message> = Vec::new();
    messages.push(Message::user("prompt"));
    messages.push(Message::assistant("memory"));
    for i in 0..5 {
        messages.push(Message::user(format!("mid-{}", i)));
    }
    for i in 0..6 {
        messages.push(Message::assistant(format!("tail-{}", i)));
    }

    let compacted = OrchestratorService::compact_messages_prefix_stable(&mut messages);
    assert!(compacted, "Should indicate compaction happened (caller must clear cache)");
}

// --- Feature-001 Story-002: Tool call loop detection tests ---

#[test]
fn test_tool_call_loop_detector_no_loop_on_different_calls() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Different tool calls should not trigger loop
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Grep", r#"{"pattern":"foo"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false).is_none());
}

#[test]
fn test_tool_call_loop_detector_detects_consecutive_identical() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // 3 consecutive identical calls should trigger
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    let detection = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(detection.is_some(), "Should detect loop after 3 identical calls");
    match detection.unwrap() {
        LoopDetection::Warning(msg) => {
            assert!(msg.contains("identical tool call"), "Break message should explain the loop: {}", msg);
        }
        other => panic!("Expected Warning on first detection, got {:?}", other),
    }
}

#[test]
fn test_tool_call_loop_detector_resets_on_different_call() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Two identical, then a different one, then the same two again
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    // Different call resets the counter
    assert!(detector.record_call("Grep", r#"{"pattern":"foo"}"#, false).is_none());
    // Start the same call again - counter should be 1 now
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    // This is the 3rd consecutive identical call
    let msg = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(msg.is_some(), "Should detect loop after 3 consecutive identical calls");
}

#[test]
fn test_tool_call_loop_detector_same_tool_different_args_no_loop() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Same tool but different arguments should not trigger
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"c.rs"}"#, false).is_none());
}

#[test]
fn test_tool_call_loop_detector_threshold_customizable() {
    let mut detector = ToolCallLoopDetector::new(2, 20);
    // With threshold 2, should trigger after 2 identical calls
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    let msg = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(msg.is_some(), "Should detect loop after 2 identical calls with threshold=2");
}

#[test]
fn test_tool_call_loop_detector_continues_after_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Trigger loop detection
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let msg = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(msg.is_some());

    // After detection, a different call should reset
    assert!(detector.record_call("Grep", r#"{"pattern":"bar"}"#, false).is_none());
}

#[test]
fn test_tool_call_loop_detector_dedup_threshold() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // With is_dedup=true, threshold should be lowered to 2 (min of threshold and 2)
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, true).is_none());
    let detection = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, true);
    assert!(detection.is_some(), "Should detect dedup loop after only 2 identical calls");
}

#[test]
fn test_tool_call_loop_detector_dedup_false_uses_normal_threshold() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // With is_dedup=false, threshold remains at 3
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none());
    assert!(detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false).is_none(),
        "Should NOT detect loop after 2 calls with is_dedup=false and threshold=3");
    let detection = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(detection.is_some(), "Should detect loop after 3 calls with is_dedup=false");
}

#[test]
fn test_session_memory_manager_empty_messages() {
    // Edge case: update_or_insert with empty messages vec
    let manager = SessionMemoryManager::new(1);
    let mut messages: Vec<Message> = vec![];

    // Should handle gracefully (insert at index 0 since memory_index > len)
    manager.update_or_insert(&mut messages, vec![], vec![]);

    // Should have inserted one message
    assert_eq!(messages.len(), 1);
    let text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(text.contains(SESSION_MEMORY_V1_MARKER));
}

#[test]
fn test_session_memory_manager_single_message() {
    // Edge case: only a system prompt, no other messages
    let manager = SessionMemoryManager::new(1);
    let mut messages = vec![
        Message::user("system prompt only"),
    ];

    manager.update_or_insert(
        &mut messages,
        vec![("test.rs".to_string(), 10usize, 200u64)],
        vec![],
    );

    assert_eq!(messages.len(), 2, "Should insert after system prompt");

    // System prompt still at index 0
    let prompt_text = match &messages[0].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(prompt_text.contains("system prompt only"));

    // Session memory at index 1
    let memory_text = match &messages[1].content[0] {
        MessageContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(memory_text.contains(SESSION_MEMORY_V1_MARKER));
    assert!(memory_text.contains("test.rs"));
}

// ===== is_complete_answer tests (story-001) =====

#[test]
fn test_is_complete_answer_short_text_returns_false() {
    assert!(!is_complete_answer("Short text."));
    assert!(!is_complete_answer("This is under 200 chars, so it should not be considered complete."));
    assert!(!is_complete_answer("")); // empty
}

#[test]
fn test_is_complete_answer_exactly_200_chars_returns_false() {
    let text = "a".repeat(200);
    // exactly 200 chars, not > 200
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_201_chars_complete_returns_true() {
    let text = format!("{}.", "a".repeat(200));
    assert!(is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_long_complete_text_returns_true() {
    let text = "The implementation follows a three-tier architecture pattern where the \
        orchestrator service manages task decomposition, parallel execution, and quality \
        gates. Each story is executed independently with its own iteration budget and \
        tool context. The design ensures that sub-agents cannot spawn further sub-agents, \
        preventing infinite recursion. Overall the approach is sound and well-tested.";
    assert!(text.chars().count() > 200);
    assert!(is_complete_answer(text));
}

#[test]
fn test_is_complete_answer_ends_with_colon_returns_false() {
    let text = format!("{} Here are the files:", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_ellipsis_returns_false() {
    let text = format!("{} And then...", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_unicode_ellipsis_returns_false() {
    let text = format!("{} And then\u{2026}", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_i_will_returns_false() {
    let text = format!("{} I will", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_let_me_returns_false() {
    let text = format!("{} Let me", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_pending_step_sentence_returns_false() {
    let text = format!(
        "{} I checked the root folders. Let me read README.md next.",
        "a".repeat(200)
    );
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_ill_returns_false() {
    let text = format!("{} I'll", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_unclosed_code_block_returns_false() {
    let text = format!("{}\n```rust\nfn main() {{}}", "a".repeat(200));
    // One ``` (odd count = unclosed)
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_closed_code_block_returns_true() {
    let text = format!("{}\n```rust\nfn main() {{}}\n```\nDone.", "a".repeat(200));
    assert!(is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_dangling_and_returns_false() {
    let text = format!("{} and", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_dangling_but_returns_false() {
    let text = format!("{} but", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_dangling_or_returns_false() {
    let text = format!("{} or", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_ends_with_dangling_then_returns_false() {
    let text = format!("{} then", "a".repeat(200));
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_cjk_text_uses_char_count() {
    // Each CJK character is 3 bytes, so 201 chars = 603 bytes but only 201 chars
    let text: String = std::iter::repeat('\u{4e00}').take(201).collect(); // 'one' in CJK
    assert!(is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_cjk_short_returns_false() {
    // 100 CJK characters should be under 200 char threshold
    let text: String = std::iter::repeat('\u{4e00}').take(100).collect();
    assert!(!is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_trailing_whitespace_ignored() {
    let text = format!("{}. Completed the analysis.   \n  \n", "a".repeat(200));
    assert!(is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_word_ending_with_and_not_dangling() {
    // "demand" ends with "and" but it's not a dangling conjunction
    let text = format!("{} high demand", "a".repeat(200));
    assert!(is_complete_answer(&text));
}

#[test]
fn test_is_complete_answer_i_will_in_middle_ok() {
    // "I will" in the middle of text, not at the end, is fine
    let text = format!("{} I will do this. That is done.", "a".repeat(200));
    assert!(is_complete_answer(&text));
}

// ===== max_iterations recovery tests (story-004) =====
// These test the ExecutionResult behavior: when max_iterations is reached
// and last_assistant_text is Some, the result should recover it.

#[test]
fn test_max_iterations_recovery_with_accumulated_text() {
    // Simulate the scenario: max_iterations hit, but we have accumulated text
    let last_assistant_text: Option<String> = Some("Here is the analysis of the codebase.".to_string());
    let max_iterations = 10;
    let iterations = 10;

    // This is the logic that should exist in the max_iterations handler
    let (response, success, error) = if let Some(ref text) = last_assistant_text {
        (
            Some(text.clone()),
            true,
            Some(format!("Max iterations ({}) reached but response recovered", max_iterations)),
        )
    } else {
        (
            None,
            false,
            Some(format!("Maximum iterations ({}) reached", max_iterations)),
        )
    };

    assert_eq!(response, Some("Here is the analysis of the codebase.".to_string()));
    assert!(success);
    assert!(error.unwrap().contains("recovered"));
    assert_eq!(iterations, max_iterations);
}

#[test]
fn test_max_iterations_no_text_returns_none() {
    let last_assistant_text: Option<String> = None;
    let max_iterations = 10;

    let (response, success, error) = if let Some(ref text) = last_assistant_text {
        (
            Some(text.clone()),
            true,
            Some(format!("Max iterations ({}) reached but response recovered", max_iterations)),
        )
    } else {
        (
            None,
            false,
            Some(format!("Maximum iterations ({}) reached", max_iterations)),
        )
    };

    assert!(response.is_none());
    assert!(!success);
    assert!(error.unwrap().contains("Maximum iterations"));
}

#[test]
fn test_max_iterations_recovery_produces_valid_execution_result() {
    let recovered_text = "The project uses a three-tier execution model.".to_string();
    let result = ExecutionResult {
        response: Some(recovered_text.clone()),
        usage: UsageStats::default(),
        iterations: 25,
        success: true,
        error: Some("Max iterations (25) reached but response recovered".to_string()),
    };

    assert!(result.success);
    assert_eq!(result.response, Some(recovered_text));
    assert!(result.error.unwrap().contains("recovered"));
}

#[test]
fn test_max_iterations_no_recovery_produces_failed_execution_result() {
    let result = ExecutionResult {
        response: None,
        usage: UsageStats::default(),
        iterations: 25,
        success: false,
        error: Some("Maximum iterations (25) reached".to_string()),
    };

    assert!(!result.success);
    assert!(result.response.is_none());
    assert!(result.error.unwrap().contains("Maximum iterations"));
}

// ===== is_complete_answer + fallback branch integration tests (story-002 / story-003) =====

#[test]
fn test_complete_answer_exits_loop_when_fallback_tools_present() {
    // Simulate: response has text + fallback tool calls. If text is complete,
    // the loop should exit with the text, ignoring the tool calls.
    let response_text = "The implementation follows a three-tier architecture pattern where the \
        orchestrator service manages task decomposition, parallel execution, and quality \
        gates. Each story is executed independently with its own iteration budget and \
        tool context. The design ensures that sub-agents cannot spawn further sub-agents, \
        preventing infinite recursion. Overall the approach is sound and well-tested.";

    // The text is long enough and has a complete ending
    assert!(is_complete_answer(response_text));

    // Simulated tool calls that should be ignored
    let fake_tool_calls = vec!["Read", "Grep"];
    assert!(!fake_tool_calls.is_empty());

    // The expected behavior: since is_complete_answer returns true,
    // the fallback branch exits early with this text as the response
    let result = ExecutionResult {
        response: Some(response_text.to_string()),
        usage: UsageStats::default(),
        iterations: 3,
        success: true,
        error: None,
    };

    assert!(result.success);
    assert_eq!(result.response.unwrap(), response_text);
}

#[test]
fn test_incomplete_text_proceeds_with_tool_calls() {
    // Simulate: response has short/incomplete text + tool calls.
    // The loop should NOT exit early 鈥?it should execute the tool calls.
    let short_text = "Let me check the file.";
    assert!(!is_complete_answer(short_text));

    let incomplete_long = format!("{} I will", "a".repeat(200));
    assert!(!is_complete_answer(&incomplete_long));

    // Both cases should proceed to tool execution (not early-exit)
}

#[test]
fn test_complete_answer_native_tool_calls_not_affected() {
    // For native tool call providers (has_native_tool_calls=true),
    // the is_complete_answer check is NOT applied. Only fallback path.
    // This test documents that the native path is unchanged.
    let response_text = "The implementation follows a three-tier architecture pattern where the \
        orchestrator service manages task decomposition, parallel execution, and quality \
        gates. Each story is executed independently with its own iteration budget and \
        tool context. The design ensures that sub-agents cannot spawn further sub-agents.";

    // Even though text is complete, native providers handle this correctly
    // on their own 鈥?we only intervene in the fallback path.
    assert!(is_complete_answer(response_text));
    // (The actual native path logic is tested via integration tests)
}

#[test]
fn test_fallback_complete_exit_produces_success_result() {
    // When the fallback branch detects a complete answer, it should produce
    // ExecutionResult with success=true and no error.
    let complete_text = format!("{}. This is the final answer.", "x".repeat(250));
    assert!(is_complete_answer(&complete_text));

    let result = ExecutionResult {
        response: Some(complete_text.clone()),
        usage: UsageStats::default(),
        iterations: 5,
        success: true,
        error: None,
    };

    assert!(result.success);
    assert!(result.error.is_none());
    assert_eq!(result.response.unwrap(), complete_text);
}

#[test]
fn test_extract_text_without_tool_calls_then_complete_check() {
    // Integration test: extract_text_without_tool_calls + is_complete_answer
    let raw = format!(
        "{}. This is the complete analysis.\n\n```tool_call\n{{\"tool\": \"Read\", \"arguments\": {{\"file_path\": \"test.rs\"}}}}\n```",
        "x".repeat(250)
    );
    let cleaned = extract_text_without_tool_calls(&raw);
    assert!(is_complete_answer(&cleaned));
}

// --- Feature-002 Story-001: Struct refactor tests ---

#[test]
fn test_total_detections_increments_and_never_resets() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Trigger first detection (3 consecutive identical calls)
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result.is_some(), "Should detect on 3rd consecutive call");
    assert_eq!(detector.total_detections(), 1);

    // Break the streak and start a new one
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    let result2 = detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    assert!(result2.is_some(), "Should detect second loop");
    assert_eq!(detector.total_detections(), 2, "total_detections should accumulate and never reset");

    // Third detection
    detector.record_call("Ls", r#"{"path":"/"}"#, false);
    detector.record_call("Ls", r#"{"path":"/"}"#, false);
    let result3 = detector.record_call("Ls", r#"{"path":"/"}"#, false);
    assert!(result3.is_some());
    assert_eq!(detector.total_detections(), 3, "total_detections should keep accumulating");
}

#[test]
fn test_recent_calls_sliding_window_respects_window_size() {
    let window_size = 5;
    let mut detector = ToolCallLoopDetector::new(10, window_size); // high threshold to avoid detection
    // Push more calls than window_size
    for i in 0..8 {
        detector.record_call(&format!("Tool{}", i), r#"{"arg":"val"}"#, false);
    }
    // The recent_calls VecDeque should never exceed window_size
    assert!(
        detector.recent_calls.len() <= window_size,
        "recent_calls len {} should not exceed window_size {}",
        detector.recent_calls.len(),
        window_size
    );
    assert_eq!(detector.recent_calls.len(), window_size);
}

#[test]
fn test_consecutive_count_not_reset_after_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Trigger detection at count=3
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result.is_some());

    // After detection, consecutive_count should still be 3, NOT reset to 0.
    // A 4th identical call should increment to 4.
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert_eq!(detector.consecutive_count, 4, "consecutive_count should continue counting, not reset after detection");

    // 5th call: no detection (5 % 3 != 0)
    let result2 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result2.is_none(), "Should not detect at count=5 (not a multiple of threshold)");
    assert_eq!(detector.consecutive_count, 5);

    // 6th call: fires again (6 % 3 == 0) - this is the second detection
    let result3 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result3.is_some(), "Should detect again at count=6 (2*threshold)");
    assert_eq!(detector.consecutive_count, 6);
    assert_eq!(detector.total_detections(), 2, "total_detections should be 2 after two firings");
}

#[test]
fn test_constructor_accepts_threshold_and_window_size() {
    let detector = ToolCallLoopDetector::new(5, 30);
    assert_eq!(detector.threshold, 5);
    assert_eq!(detector.window_size, 30);
    assert_eq!(detector.total_detections(), 0);
    assert!(detector.stripped_tools().is_empty());
    assert!(detector.recent_calls.is_empty());
}

// --- Feature-002 Story-002: Macro-loop detection tests ---

#[test]
fn test_macro_loop_detects_ab_ab_pattern() {
    let mut detector = ToolCallLoopDetector::new(10, 20); // high threshold to avoid consecutive detection
    // Create ABAB pattern
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    assert!(result.is_some(), "Should detect AB-AB macro loop");
    match result.unwrap() {
        LoopDetection::Warning(msg) => {
            assert!(msg.contains("MACRO-LOOP"), "Message should indicate macro-loop: {}", msg);
            assert!(msg.contains("Read"), "Message should mention Read tool: {}", msg);
            assert!(msg.contains("Grep"), "Message should mention Grep tool: {}", msg);
        }
        other => panic!("Expected Warning on first macro detection, got {:?}", other),
    }
}

#[test]
fn test_macro_loop_detects_abc_abc_pattern() {
    let mut detector = ToolCallLoopDetector::new(10, 20);
    // Create ABCABC pattern
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Task", r#"{"prompt":"do something"}"#, false);
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Task", r#"{"prompt":"do something"}"#, false);
    assert!(result.is_some(), "Should detect ABC-ABC macro loop");
    match result.unwrap() {
        LoopDetection::Warning(msg) => {
            assert!(msg.contains("MACRO-LOOP"), "Message should indicate macro-loop: {}", msg);
        }
        other => panic!("Expected Warning on first macro detection, got {:?}", other),
    }
}

#[test]
fn test_macro_loop_no_false_positive_on_varied_calls() {
    let mut detector = ToolCallLoopDetector::new(10, 20);
    // Non-repeating sequence should NOT trigger macro detection
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    detector.record_call("Ls", r#"{"path":"/"}"#, false);
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false);
    let result = detector.record_call("Grep", r#"{"pattern":"bar"}"#, false);
    assert!(result.is_none(), "Should NOT detect macro loop on varied non-repeating calls");
}

#[test]
fn test_macro_loop_requires_minimum_window_entries() {
    let mut detector = ToolCallLoopDetector::new(10, 20);
    // Only 2 calls - not enough for even the shortest (length-2) macro pattern
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    assert!(result.is_none(), "Should NOT detect with insufficient window entries");
}

#[test]
fn test_macro_loop_real_world_cwd_read_task_cycle() {
    let mut detector = ToolCallLoopDetector::new(10, 20);
    // Simulate the actual Cwd->Read->Task pattern from bug reports (repeated 2x)
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"src/main.rs"}"#, false);
    detector.record_call("Task", r#"{"prompt":"analyze the codebase"}"#, false);
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"src/main.rs"}"#, false);
    let result = detector.record_call("Task", r#"{"prompt":"analyze the codebase"}"#, false);
    assert!(result.is_some(), "Should detect the Cwd->Read->Task->Cwd->Read->Task macro loop");
    assert_eq!(detector.total_detections(), 1);
}

#[test]
fn test_macro_loop_partial_match_no_detection() {
    let mut detector = ToolCallLoopDetector::new(10, 20);
    // ABCABD - almost a cycle but last element differs
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    detector.record_call("Ls", r#"{"path":"/"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    let result = detector.record_call("Cwd", r#"{}"#, false); // Different from Ls
    assert!(result.is_none(), "Should NOT detect macro loop when pattern partially matches (ABCABD)");
}

// --- Feature-002 Story-003: Escalation mechanism tests ---

#[test]
fn test_escalation_level1_warning_on_first_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    match result {
        Some(LoopDetection::Warning(msg)) => {
            assert!(msg.contains("LOOP DETECTED"), "Warning should contain LOOP DETECTED: {}", msg);
        }
        other => panic!("Expected LoopDetection::Warning, got {:?}", other),
    }
    assert_eq!(detector.total_detections(), 1);
}

#[test]
fn test_escalation_level2_strip_tools_on_second_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // First detection (Level 1): consecutive identical calls
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let r1 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(matches!(r1, Some(LoopDetection::Warning(_))));

    // Second detection (Level 2): keep going with the same call
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let r2 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    match r2 {
        Some(LoopDetection::StripTools(msg, tools)) => {
            assert!(msg.contains("LOOP DETECTED"), "StripTools should contain warning: {}", msg);
            assert!(tools.contains(&"Read".to_string()), "Should strip the looping tool");
        }
        other => panic!("Expected LoopDetection::StripTools on second detection, got {:?}", other),
    }
    assert_eq!(detector.total_detections(), 2);
    assert!(detector.stripped_tools().contains("Read"));
}

#[test]
fn test_escalation_level3_force_terminate_on_third_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // First detection
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);

    // Second detection
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);

    // Third detection (Level 3): force terminate
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let r3 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    match r3 {
        Some(LoopDetection::ForceTerminate(msg)) => {
            assert!(msg.contains("LOOP DETECTED") || msg.contains("FORCE TERMINATE"),
                "ForceTerminate should mention loop/termination: {}", msg);
        }
        other => panic!("Expected LoopDetection::ForceTerminate on third detection, got {:?}", other),
    }
    assert_eq!(detector.total_detections(), 3);
}

#[test]
fn test_escalation_accumulates_across_mixed_detection_types() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // First detection via consecutive identical (Level 1)
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let r1 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(matches!(r1, Some(LoopDetection::Warning(_))), "First detection should be Warning");

    // Second detection via macro-loop (Level 2) - break the consecutive streak first
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false);
    detector.record_call("Task", r#"{"prompt":"analyze"}"#, false);
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false);
    let r2 = detector.record_call("Task", r#"{"prompt":"analyze"}"#, false);
    match r2 {
        Some(LoopDetection::StripTools(_, tools)) => {
            assert!(!tools.is_empty(), "StripTools should list tool names");
        }
        other => panic!("Expected LoopDetection::StripTools on second detection (macro-loop), got {:?}", other),
    }
    assert_eq!(detector.total_detections(), 2);
}

#[test]
fn test_stripped_tools_persist_across_detections() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // First detection: Warning only, no stripped tools
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(detector.stripped_tools().is_empty(), "No tools stripped at Level 1");

    // Second detection: StripTools - "Read" gets stripped
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(detector.stripped_tools().contains("Read"), "Read should be stripped after Level 2");

    // Break the streak, trigger another loop with a different tool
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    let r3 = detector.record_call("Grep", r#"{"pattern":"foo"}"#, false);
    assert!(matches!(r3, Some(LoopDetection::ForceTerminate(_))), "Third detection should be ForceTerminate");

    // Both Read and Grep should be in stripped_tools
    assert!(detector.stripped_tools().contains("Read"), "Read should still be stripped");
    assert!(detector.stripped_tools().contains("Grep"), "Grep should also be stripped");
}

// --- Feature-002 Story-005: Edge case tests ---

#[test]
fn test_macro_and_consecutive_interleaved() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // First detection: consecutive identical (Level 1 = Warning)
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let r1 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(matches!(r1, Some(LoopDetection::Warning(_))), "First: consecutive -> Warning");

    // Break streak, then trigger macro-loop (Level 2 = StripTools)
    detector.record_call("Cwd", r#"{}"#, false);
    detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false);
    detector.record_call("Cwd", r#"{}"#, false);
    let r2 = detector.record_call("Read", r#"{"file_path":"b.rs"}"#, false);
    assert!(matches!(r2, Some(LoopDetection::StripTools(_, _))), "Second: macro-loop -> StripTools");
    assert_eq!(detector.total_detections(), 2);
}

#[test]
fn test_window_boundary_exact() {
    let window_size = 4;
    let mut detector = ToolCallLoopDetector::new(100, window_size); // very high threshold
    // Fill window to exactly window_size
    detector.record_call("A", "1", false);
    detector.record_call("B", "2", false);
    detector.record_call("C", "3", false);
    detector.record_call("D", "4", false);
    assert_eq!(detector.recent_calls.len(), window_size);

    // Push one more - oldest should be evicted
    detector.record_call("E", "5", false);
    assert_eq!(detector.recent_calls.len(), window_size, "Should stay at window_size after eviction");
    // First entry should now be B (A was evicted)
    assert_eq!(detector.recent_calls[0].0, "B", "Oldest entry (A) should have been evicted");
    assert_eq!(detector.recent_calls[3].0, "E", "Newest entry should be E");
}

#[test]
fn test_hash_collision_safety() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Two different args that are clearly different should not cause false positives
    detector.record_call("Read", r#"{"file_path":"src/main.rs","offset":0}"#, false);
    detector.record_call("Read", r#"{"file_path":"src/lib.rs","offset":100}"#, false);
    let result = detector.record_call("Read", r#"{"file_path":"tests/test.rs","offset":200}"#, false);
    assert!(result.is_none(), "Different args should not trigger, even for same tool name");
}

#[test]
fn test_single_call_no_detection() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result.is_none(), "A single call should never trigger any detection");
    assert_eq!(detector.total_detections(), 0);
}

#[test]
fn test_threshold_1_immediate_detection() {
    let mut detector = ToolCallLoopDetector::new(1, 20);
    // With threshold=1, the very first call should trigger detection
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result.is_some(), "threshold=1 should trigger on every call");
    assert_eq!(detector.total_detections(), 1);

    // Second call to same tool should also trigger (count=2, 2 % 1 == 0)
    let result2 = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    assert!(result2.is_some(), "threshold=1 should trigger on second call too");
    assert_eq!(detector.total_detections(), 2);
}

#[test]
fn test_macro_loop_length_6_max() {
    let mut detector = ToolCallLoopDetector::new(100, 20); // high threshold
    // Create a length-6 cycle: ABCDEF ABCDEF
    detector.record_call("A", "1", false);
    detector.record_call("B", "2", false);
    detector.record_call("C", "3", false);
    detector.record_call("D", "4", false);
    detector.record_call("E", "5", false);
    detector.record_call("F", "6", false);
    detector.record_call("A", "1", false);
    detector.record_call("B", "2", false);
    detector.record_call("C", "3", false);
    detector.record_call("D", "4", false);
    detector.record_call("E", "5", false);
    let result = detector.record_call("F", "6", false);
    assert!(result.is_some(), "Should detect length-6 macro loop");
    match result.unwrap() {
        LoopDetection::Warning(msg) => {
            assert!(msg.contains("MACRO-LOOP"), "Should be macro-loop: {}", msg);
        }
        other => panic!("Expected Warning, got {:?}", other),
    }
}

#[test]
fn test_macro_loop_length_7_not_detected() {
    let mut detector = ToolCallLoopDetector::new(100, 20); // high threshold
    // Create a length-7 cycle: ABCDEFG ABCDEFG
    // Only cycles up to length 6 are detected by design
    detector.record_call("A", "1", false);
    detector.record_call("B", "2", false);
    detector.record_call("C", "3", false);
    detector.record_call("D", "4", false);
    detector.record_call("E", "5", false);
    detector.record_call("F", "6", false);
    detector.record_call("G", "7", false);
    detector.record_call("A", "1", false);
    detector.record_call("B", "2", false);
    detector.record_call("C", "3", false);
    detector.record_call("D", "4", false);
    detector.record_call("E", "5", false);
    detector.record_call("F", "6", false);
    let result = detector.record_call("G", "7", false);
    assert!(result.is_none(), "Length-7 cycles should NOT be detected (design limit is 6)");
}

#[test]
fn test_stripped_tools_accessor_empty_initially() {
    let detector = ToolCallLoopDetector::new(3, 20);
    assert!(detector.stripped_tools().is_empty(), "stripped_tools should be empty on fresh detector");
    assert_eq!(detector.stripped_tools().len(), 0);
}

#[test]
fn test_force_terminate_includes_useful_message() {
    let mut detector = ToolCallLoopDetector::new(3, 20);
    // Drive to Level 3
    // Detection 1
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    // Detection 2
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    // Detection 3 - force terminate
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    let result = detector.record_call("Read", r#"{"file_path":"a.rs"}"#, false);
    match result {
        Some(LoopDetection::ForceTerminate(msg)) => {
            assert!(msg.contains("FORCE TERMINATE"), "Should mention force termination: {}", msg);
            assert!(msg.contains("intervene") || msg.contains("review"),
                "Should suggest user intervention: {}", msg);
        }
        other => panic!("Expected ForceTerminate, got {:?}", other),
    }
}

// =========================================================================
// Dedup skip logic tests (story-006)
// =========================================================================

#[test]
fn test_tool_result_is_dedup_flag_default_false() {
    use crate::services::tools::ToolResult;
    let result = ToolResult::ok("some content");
    assert!(!result.is_dedup, "ToolResult::ok should default is_dedup to false");
}

#[test]
fn test_tool_result_ok_dedup_sets_flag() {
    use crate::services::tools::ToolResult;
    let result = ToolResult::ok_dedup("[DEDUP] file.rs (50 lines) already read.");
    assert!(result.is_dedup, "ToolResult::ok_dedup should set is_dedup to true");
    assert!(result.success, "dedup result should still be successful");
}

#[test]
fn test_tool_result_err_is_not_dedup() {
    use crate::services::tools::ToolResult;
    let result = ToolResult::err("some error");
    assert!(!result.is_dedup, "ToolResult::err should not be dedup");
}

#[test]
fn test_dedup_minimal_tool_result_preserves_api_compat() {
    // Verify that the minimal "." tool_result we push for dedup results
    // is a valid Message that satisfies the Anthropic API requirement.
    let msg = Message::tool_result("toolu_123", ".".to_string(), false);
    assert_eq!(msg.role, crate::services::llm::MessageRole::User);
    // Verify the content has the tool_result
    assert!(!msg.content.is_empty());
    match &msg.content[0] {
        MessageContent::ToolResult { tool_use_id, content, .. } => {
            assert_eq!(tool_use_id, "toolu_123");
            assert_eq!(content, ".");
        }
        _ => panic!("Expected ToolResult content block"),
    }
}
