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
    let text = "浣犲ソ涓栫晫浣犲ソ涓栫晫";
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
            },
            crate::services::orchestrator::analysis_index::FileInventoryItem {
                path: "tests/test_orchestrator.py".to_string(),
                component: "python-tests".to_string(),
                language: "python".to_string(),
                extension: Some("py".to_string()),
                size_bytes: 80,
                line_count: 8,
                is_test: true,
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
    assert!(text_describes_tool_intent("让我使用 Read 工具来读取文件。"));
    assert!(text_describes_tool_intent(
        "我将调用 Bash 来执行测试命令。"
    ));
    assert!(text_describes_tool_intent("接下来使用 Grep 搜索代码。"));
}

#[test]
fn test_text_describes_tool_intent_no_tool_name() {
    // Intent phrases without tool names should NOT trigger
    assert!(!text_describes_tool_intent(
        "Let me use the function to check."
    ));
    assert!(!text_describes_tool_intent("我将调用函数来处理数据。"));
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
        prompt.contains("请使用以下格式调用工具"),
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
    // User explicitly set Off → no fallback instructions even for unreliable provider
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
