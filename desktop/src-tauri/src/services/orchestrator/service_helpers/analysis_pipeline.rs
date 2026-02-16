use super::*;


#[derive(Debug, Clone, Default)]
pub(super) struct PhaseCapture {
    pub(super) tool_calls: usize,
    pub(super) read_calls: usize,
    pub(super) grep_calls: usize,
    pub(super) glob_calls: usize,
    pub(super) ls_calls: usize,
    pub(super) cwd_calls: usize,
    pub(super) observed_paths: HashSet<String>,
    pub(super) read_paths: HashSet<String>,
    pub(super) evidence_lines: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) pending_tools: HashMap<String, PendingAnalysisToolCall>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PendingAnalysisToolCall {
    pub(super) tool_name: String,
    pub(super) arguments: Option<serde_json::Value>,
}

impl PhaseCapture {
    pub(super) fn search_calls(&self) -> usize {
        self.grep_calls + self.glob_calls
    }

    pub(super) fn tool_call_count(&self, name: &str) -> usize {
        match name {
            "Read" => self.read_calls,
            "Grep" => self.grep_calls,
            "Glob" => self.glob_calls,
            "LS" => self.ls_calls,
            "Cwd" => self.cwd_calls,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AnalysisPhaseOutcome {
    pub(super) phase: AnalysisPhase,
    pub(super) response: Option<String>,
    pub(super) usage: UsageStats,
    pub(super) iterations: u32,
    pub(super) status: AnalysisPhaseStatus,
    pub(super) error: Option<String>,
    pub(super) capture: PhaseCapture,
}

#[derive(Debug, Clone, Default)]
pub(super) struct AnalysisLedger {
    pub(super) observed_paths: HashSet<String>,
    pub(super) read_paths: HashSet<String>,
    pub(super) evidence_lines: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) phase_summaries: Vec<String>,
    pub(super) chunk_summaries: Vec<ChunkSummaryRecord>,
    pub(super) successful_phases: usize,
    pub(super) partial_phases: usize,
    pub(super) total_phases: usize,
    pub(super) inventory: Option<FileInventory>,
    pub(super) chunk_plan: Option<ChunkPlan>,
    pub(super) coverage_report: Option<AnalysisCoverageReport>,
}

impl AnalysisLedger {
    pub(super) fn record(&mut self, outcome: &AnalysisPhaseOutcome) {
        self.total_phases += 1;
        match outcome.status {
            AnalysisPhaseStatus::Passed => self.successful_phases += 1,
            AnalysisPhaseStatus::Partial => {
                self.partial_phases += 1;
                if let Some(err) = outcome.error.as_ref() {
                    self.warnings.push(format!(
                        "{} completed with partial evidence: {}",
                        outcome.phase.title(),
                        err
                    ));
                }
            }
            AnalysisPhaseStatus::Failed => {
                if let Some(err) = outcome.error.as_ref() {
                    self.warnings
                        .push(format!("{} failed: {}", outcome.phase.title(), err));
                }
            }
        }

        self.observed_paths
            .extend(outcome.capture.observed_paths.iter().cloned());
        self.read_paths
            .extend(outcome.capture.read_paths.iter().cloned());

        self.evidence_lines
            .extend(outcome.capture.evidence_lines.iter().cloned());
        self.warnings
            .extend(outcome.capture.warnings.iter().cloned());

        if let Some(summary) = outcome.response.as_ref() {
            let trimmed = summary.trim();
            if !trimmed.is_empty() {
                let compact =
                    condense_phase_summary_for_context(trimmed, MAX_SYNTHESIS_PHASE_CONTEXT_CHARS);
                self.phase_summaries.push(format!(
                    "## {} ({})\n{}",
                    outcome.phase.title(),
                    outcome.phase.id(),
                    compact
                ));
            }
        }
    }
}

impl OrchestratorService {
    /// Execute an analysis task with an evidence-first multi-phase pipeline.
    pub(super) async fn execute_with_analysis_pipeline(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let mut total_usage = UsageStats::default();
        let mut total_iterations = 0;
        let mut ledger = AnalysisLedger::default();
        let scope_guidance = analysis_scope_guidance(&message);
        let run_handle = match self
            .analysis_store
            .start_run(&message, &self.config.project_root)
        {
            Ok(handle) => {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisRunStarted {
                        run_id: handle.run_id().to_string(),
                        run_dir: handle.run_dir().to_string_lossy().to_string(),
                        request: message.clone(),
                    })
                    .await;
                Some(handle)
            }
            Err(err) => {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                        phase_id: "analysis".to_string(),
                        message: format!(
                            "Analysis artifact persistence unavailable for this run: {}",
                            err
                        ),
                    })
                    .await;
                None
            }
        };

        let excluded_roots = analysis_excluded_roots_for_message(&message);
        let inventory = match build_file_inventory(&self.config.project_root, &excluded_roots) {
            Ok(inv) => inv,
            Err(err) => {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                        phase_id: "analysis".to_string(),
                        message: format!(
                            "Inventory build failed, fallback to baseline-only mode: {}",
                            err
                        ),
                    })
                    .await;
                FileInventory::default()
            }
        };
        let chunk_plan = if inventory.total_files == 0 {
            ChunkPlan::default()
        } else {
            build_chunk_plan(&inventory, &self.config.analysis_limits)
        };
        let effective_targets = compute_effective_analysis_targets(
            &self.config.analysis_limits,
            self.config.analysis_profile.clone(),
            &inventory,
        );
        ledger.inventory = Some(inventory.clone());
        ledger.chunk_plan = Some(chunk_plan.clone());
        if let Some(run) = run_handle.as_ref() {
            let _ = run.write_json_artifact("index/file_inventory.json", &inventory);
            let _ = run.write_json_artifact("index/chunk_plan.json", &chunk_plan);
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisIndexBuilt {
                    run_id: run.run_id().to_string(),
                    inventory_total_files: inventory.total_files,
                    test_files_total: inventory.total_test_files,
                    chunk_count: chunk_plan.chunks.len(),
                })
                .await;
        }
        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: "analysis".to_string(),
                message: format!(
                    "Indexed {} files (tests={}) into {} chunks | dynamic read target {:.2}% (budget={} files)",
                    inventory.total_files,
                    inventory.total_test_files,
                    chunk_plan.chunks.len(),
                    effective_targets.sampled_read_ratio * 100.0,
                    effective_targets.max_total_read_files
                ),
            })
            .await;

        let phase1_base_prompt = format!(
            "User request: {}\n\n\
             Scope constraints:\n{}\n\n\
             Run a strict structure discovery pass. Identify the real repository shape,\n\
             read primary manifests, and list true entrypoints with file paths.\n\
             Keep tool usage targeted and avoid broad scans after objective is satisfied.",
            message, scope_guidance
        );
        let structure_summary = self
            .run_analysis_phase_layered(
                AnalysisPhase::StructureDiscovery,
                phase1_base_prompt,
                &tx,
                &mut total_usage,
                &mut total_iterations,
                &mut ledger,
                run_handle.as_ref(),
                effective_targets.max_total_read_files,
                effective_targets.sampled_read_ratio,
            )
            .await;

        if self.cancellation_token.is_cancelled() {
            if let Some(run) = run_handle.as_ref() {
                let _ = run.complete(false, Some("Execution cancelled".to_string()));
            }
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let observed_from_phase1 = join_sorted_paths(&ledger.observed_paths, 90);

        let phase2_base_prompt = format!(
            "User request: {}\n\n\
             Scope constraints:\n{}\n\n\
             Structure summary from previous phase:\n{}\n\n\
             Observed paths so far:\n{}\n\n\
             Build a concrete architecture trace from real files. If a component cannot be verified\n\
             from tools, label it as unknown.\n\
             Prioritize high-signal files and avoid repeated reads of very large files.",
            message, scope_guidance, structure_summary, observed_from_phase1
        );
        let architecture_summary = self
            .run_analysis_phase_layered(
                AnalysisPhase::ArchitectureTrace,
                phase2_base_prompt,
                &tx,
                &mut total_usage,
                &mut total_iterations,
                &mut ledger,
                run_handle.as_ref(),
                effective_targets.max_total_read_files,
                effective_targets.sampled_read_ratio,
            )
            .await;

        if self.cancellation_token.is_cancelled() {
            if let Some(run) = run_handle.as_ref() {
                let _ = run.complete(false, Some("Execution cancelled".to_string()));
            }
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let phase3_base_prompt = format!(
            "User request: {}\n\n\
             Scope constraints:\n{}\n\n\
             Verify these findings and explicitly mark uncertain claims.\n\n\
             Structure summary:\n{}\n\n\
             Architecture summary:\n{}\n\n\
             Observed paths:\n{}\n\n\
             Output must include:\n\
             - Verified claims (with path evidence)\n\
             - Unverified claims (and why)\n\
             - Contradictions or missing data\n\
             Keep output concise and strictly evidence-backed.",
            message,
            scope_guidance,
            structure_summary,
            architecture_summary,
            join_sorted_paths(&ledger.observed_paths, 120)
        );
        let _consistency_summary = self
            .run_analysis_phase_layered(
                AnalysisPhase::ConsistencyCheck,
                phase3_base_prompt,
                &tx,
                &mut total_usage,
                &mut total_iterations,
                &mut ledger,
                run_handle.as_ref(),
                effective_targets.max_total_read_files,
                effective_targets.sampled_read_ratio,
            )
            .await;

        if self.cancellation_token.is_cancelled() {
            if let Some(run) = run_handle.as_ref() {
                let _ = run.complete(false, Some("Execution cancelled".to_string()));
            }
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let mut coverage_report = if let Some(inventory) = ledger.inventory.as_ref() {
            compute_coverage_report(
                inventory,
                &ledger.observed_paths,
                &ledger.read_paths,
                ledger
                    .chunk_plan
                    .as_ref()
                    .map(|plan| plan.chunks.len())
                    .unwrap_or(0),
                1,
            )
        } else {
            AnalysisCoverageReport::default()
        };
        ledger.coverage_report = Some(coverage_report.clone());
        if let Some(run) = run_handle.as_ref() {
            let _ = run.write_json_artifact("final/coverage.json", &coverage_report);
            let _ = run.update_coverage(build_coverage_metrics(&ledger, &coverage_report));
        }

        let needs_topup = if coverage_report.inventory_total_files == 0 {
            false
        } else {
            coverage_report.sampled_read_ratio < effective_targets.sampled_read_ratio
                || coverage_report.test_coverage_ratio < effective_targets.test_coverage_ratio
        };
        if needs_topup {
            let added = self
                .perform_coverage_topup_pass(
                    &mut ledger,
                    effective_targets,
                    &tx,
                    run_handle.as_ref(),
                )
                .await;
            if added > 0 {
                coverage_report = if let Some(inventory) = ledger.inventory.as_ref() {
                    compute_coverage_report(
                        inventory,
                        &ledger.observed_paths,
                        &ledger.read_paths,
                        ledger
                            .chunk_plan
                            .as_ref()
                            .map(|plan| plan.chunks.len())
                            .unwrap_or(0),
                        1,
                    )
                } else {
                    AnalysisCoverageReport::default()
                };
                ledger.coverage_report = Some(coverage_report.clone());
                if let Some(run) = run_handle.as_ref() {
                    let _ = run.write_json_artifact("final/coverage.json", &coverage_report);
                    let _ = run.update_coverage(build_coverage_metrics(&ledger, &coverage_report));
                }
            }
        }

        let has_evidence = !ledger.evidence_lines.is_empty();
        let usable_phases = ledger.successful_phases + ledger.partial_phases;
        let required_usable_phases = 3;
        let coverage_passed = coverage_report.coverage_ratio >= effective_targets.coverage_ratio
            || coverage_report.inventory_total_files == 0;
        let sampled_read_passed = coverage_report.sampled_read_ratio
            >= effective_targets.sampled_read_ratio
            || coverage_report.inventory_total_files == 0;
        let test_coverage_passed = coverage_report.test_coverage_ratio
            >= effective_targets.test_coverage_ratio
            || coverage_report.test_files_total == 0;
        let analysis_gate_passed = usable_phases >= required_usable_phases
            && has_evidence
            && coverage_passed
            && sampled_read_passed
            && test_coverage_passed;
        if !analysis_gate_passed {
            let mut failures = Vec::new();
            if usable_phases < required_usable_phases {
                failures.push(format!(
                    "Phase gate failed: {} usable phases (required={}, passed={}, partial={})",
                    usable_phases,
                    required_usable_phases,
                    ledger.successful_phases,
                    ledger.partial_phases
                ));
            }
            if !has_evidence {
                failures.push("Evidence gate failed: no tool evidence captured".to_string());
            }
            if !coverage_passed {
                failures.push(format!(
                    "Coverage gate failed: {:.2}% < target {:.2}% (indexed_files={}, observed_files={})",
                    coverage_report.coverage_ratio * 100.0,
                    effective_targets.coverage_ratio * 100.0,
                    coverage_report.inventory_total_files,
                    ledger.observed_paths.len()
                ));
            }
            if !sampled_read_passed {
                failures.push(format!(
                    "Read-depth gate failed: {:.2}% < target {:.2}% (indexed_files={}, sampled_read_files={})",
                    coverage_report.sampled_read_ratio * 100.0,
                    effective_targets.sampled_read_ratio * 100.0,
                    coverage_report.inventory_total_files,
                    coverage_report.sampled_read_files
                ));
            }
            if !test_coverage_passed {
                failures.push(format!(
                    "Test coverage gate failed: {:.2}% < target {:.2}% (test_files_total={}, test_files_read={})",
                    coverage_report.test_coverage_ratio * 100.0,
                    effective_targets.test_coverage_ratio * 100.0,
                    coverage_report.test_files_total,
                    coverage_report.test_files_read
                ));
            }

            let _ = tx
                .send(UnifiedStreamEvent::AnalysisRunSummary {
                    success: false,
                    phase_results: vec![
                        format!("successful_phases={}", ledger.successful_phases),
                        format!("partial_phases={}", ledger.partial_phases),
                        format!("observed_paths={}", ledger.observed_paths.len()),
                        format!("coverage_ratio={:.4}", coverage_report.coverage_ratio),
                        format!(
                            "sampled_read_ratio={:.4}",
                            coverage_report.sampled_read_ratio
                        ),
                        format!(
                            "test_coverage_ratio={:.4}",
                            coverage_report.test_coverage_ratio
                        ),
                        format!(
                            "coverage_target_ratio={:.4}",
                            effective_targets.coverage_ratio
                        ),
                        format!(
                            "sampled_read_target_ratio={:.4}",
                            effective_targets.sampled_read_ratio
                        ),
                        format!(
                            "test_coverage_target_ratio={:.4}",
                            effective_targets.test_coverage_ratio
                        ),
                    ],
                    total_metrics: serde_json::json!({
                        "input_tokens": total_usage.input_tokens,
                        "output_tokens": total_usage.output_tokens,
                        "iterations": total_iterations,
                        "evidence_lines": ledger.evidence_lines.len(),
                        "coverage_target_ratio": effective_targets.coverage_ratio,
                        "sampled_read_target_ratio": effective_targets.sampled_read_ratio,
                        "test_coverage_target_ratio": effective_targets.test_coverage_ratio,
                    }),
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisValidation {
                    status: "error".to_string(),
                    issues: failures.clone(),
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::Error {
                    message: failures.join("; "),
                    code: Some("analysis_insufficient_evidence".to_string()),
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::Usage {
                    input_tokens: total_usage.input_tokens,
                    output_tokens: total_usage.output_tokens,
                    thinking_tokens: total_usage.thinking_tokens,
                    cache_read_tokens: total_usage.cache_read_tokens,
                    cache_creation_tokens: total_usage.cache_creation_tokens,
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::Complete {
                    stop_reason: Some("analysis_gate_failed".to_string()),
                })
                .await;
            if let Some(run) = run_handle.as_ref() {
                let _ = run.complete(
                    false,
                    Some("Analysis failed: insufficient verified evidence".to_string()),
                );
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisRunCompleted {
                        run_id: run.run_id().to_string(),
                        success: false,
                        manifest_path: run.manifest_path().to_string_lossy().to_string(),
                        report_path: None,
                    })
                    .await;
            }

            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Analysis failed: insufficient verified evidence".to_string()),
            };
        }

        let evidence_block = build_synthesis_evidence_block(
            &ledger.evidence_lines,
            MAX_SYNTHESIS_EVIDENCE_LINES,
            200,
        );
        let summary_block = build_synthesis_phase_block(
            &ledger.phase_summaries,
            MAX_SYNTHESIS_PHASE_CONTEXT_CHARS,
            12,
        );
        let chunk_summary_block =
            build_synthesis_chunk_block(&ledger.chunk_summaries, MAX_SYNTHESIS_CHUNK_CONTEXT_CHARS);
        let warnings_block = if ledger.warnings.is_empty() {
            "None".to_string()
        } else {
            ledger
                .warnings
                .iter()
                .take(12)
                .map(|w| truncate_for_log(w, 220))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let test_evidence_block = if let Some(inventory) = ledger.inventory.as_ref() {
            build_test_evidence_block(inventory, &ledger.observed_paths, &ledger.read_paths)
        } else {
            "No inventory available.".to_string()
        };
        let observed_paths =
            join_sorted_paths(&ledger.observed_paths, MAX_SYNTHESIS_OBSERVED_PATHS);

        let synthesis_prompt = format!(
            "You are synthesizing a repository analysis from verified tool evidence.\n\n\
             User request:\n{}\n\n\
             Observed paths (ground truth):\n{}\n\n\
             Warnings collected:\n{}\n\n\
             Coverage metrics:\n- indexed_files={}\n- observed_paths={}\n- sampled_read_files={}\n- test_files_total={}\n- test_files_read={}\n- coverage_ratio={:.2}%\n- sampled_read_ratio={:.2}%\n- test_coverage_ratio={:.2}%\n- observed_test_coverage_ratio={:.2}%\n\n\
             Test evidence:\n{}\n\n\
             Phase summaries:\n{}\n\n\
             Chunk summaries:\n{}\n\n\
             Evidence log:\n{}\n\n\
             Requirements:\n\
             1) Use only the evidence above.\n\
             2) Do not invent files, modules, frameworks, versions, or runtime details.\n\
             3) If a claim is uncertain, place it under 'Unknowns'.\n\
             4) Include explicit file paths for major claims.\n\
             5) Do not use placeholders like '[UNVERIFIED]'. Use plain language under 'Unknowns'.\n\
             6) Mention token-budget/overflow only if it appears in 'Warnings collected'.\n\
             7) Mention version inconsistency only if at least two concrete files with conflicting versions are cited.\n\
             8) Include explicit testing evidence when test files are indexed/observed/read.\n\
             9) Do not claim tests are missing when test_files_total > 0.\n\
             10) Choose the report structure dynamically based on the request and evidence; avoid rigid boilerplate templates.\n\
             11) Ensure the final report clearly separates verified facts, risks, and unknowns, but headings/titles can be customized.\n\
             12) Keep final answer concise and user-facing (no raw phase fallback dumps, no tool logs, no chunk-by-chunk file listings).",
            message,
            observed_paths,
            warnings_block,
            coverage_report.inventory_total_files,
            ledger.observed_paths.len(),
            coverage_report.sampled_read_files,
            coverage_report.test_files_total,
            coverage_report.test_files_read,
            coverage_report.coverage_ratio * 100.0,
            coverage_report.sampled_read_ratio * 100.0,
            coverage_report.test_coverage_ratio * 100.0,
            coverage_report.observed_test_coverage_ratio * 100.0,
            test_evidence_block,
            summary_block,
            chunk_summary_block,
            evidence_block
        );

        if let Some(run) = run_handle.as_ref() {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisMergeCompleted {
                    run_id: run.run_id().to_string(),
                    phase_count: ledger.total_phases,
                    chunk_summary_count: ledger.chunk_summaries.len(),
                })
                .await;
        }

        let synthesis_messages = vec![Message::user(synthesis_prompt)];
        let synthesis_response = self
            .call_llm(&synthesis_messages, &[], &[], LlmRequestOptions::default())
            .await;
        total_iterations += 1;

        let (mut final_response, synthesis_success) = match synthesis_response {
            Ok(r) => {
                merge_usage(&mut total_usage, &r.usage);
                (
                    r.content
                        .as_deref()
                        .map(extract_text_without_tool_calls)
                        .filter(|s| !s.trim().is_empty()),
                    true,
                )
            }
            Err(e) => {
                let fallback = build_deterministic_analysis_fallback_report(
                    &message,
                    &self.config.project_root,
                    &ledger,
                    &coverage_report,
                    effective_targets,
                    Some(&e.to_string()),
                );
                ledger
                    .warnings
                    .push(format!("Synthesis call failed, fallback used: {}", e));
                (Some(fallback), false)
            }
        };

        let validation_issues = if let Some(text) = final_response.as_ref() {
            find_unverified_paths(text, &ledger.observed_paths)
                .into_iter()
                .take(20)
                .map(|p| format!("Unverified path mention: {}", p))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        if !validation_issues.is_empty() {
            if let Some(original) = final_response.clone() {
                let correction_prompt = format!(
                    "Revise this analysis to remove or mark these path claims as unverified:\n{}\n\n\
                     Observed paths:\n{}\n\n\
                     Original analysis:\n{}",
                    validation_issues.join("\n"),
                    join_sorted_paths(&ledger.observed_paths, 120),
                    original
                );
                let correction_messages = vec![Message::user(correction_prompt)];
                if let Ok(corrected) = self
                    .call_llm(&correction_messages, &[], &[], LlmRequestOptions::default())
                    .await
                {
                    merge_usage(&mut total_usage, &corrected.usage);
                    let cleaned = corrected
                        .content
                        .as_deref()
                        .map(extract_text_without_tool_calls)
                        .filter(|s| !s.trim().is_empty());
                    if cleaned.is_some() {
                        final_response = cleaned;
                    }
                }
            }
        }

        if let Some(original) = final_response.clone() {
            if should_rewrite_synthesis_output(&original) {
                let rewrite_prompt = build_synthesis_rewrite_prompt(&message, &original);
                let rewrite_messages = vec![Message::user(rewrite_prompt)];
                match self
                    .call_llm(&rewrite_messages, &[], &[], LlmRequestOptions::default())
                    .await
                {
                    Ok(rewritten) => {
                        merge_usage(&mut total_usage, &rewritten.usage);
                        let cleaned = rewritten
                            .content
                            .as_deref()
                            .map(extract_text_without_tool_calls)
                            .filter(|s| !s.trim().is_empty());
                        if cleaned.is_some() {
                            final_response = cleaned;
                        }
                    }
                    Err(err) => {
                        ledger.warnings.push(format!(
                            "Synthesis rewrite pass failed: {}",
                            truncate_for_log(&err.to_string(), 180)
                        ));
                    }
                }
            }
        }

        let mut final_validation_issues = if let Some(text) = final_response.as_ref() {
            find_unverified_paths(text, &ledger.observed_paths)
                .into_iter()
                .take(20)
                .map(|p| format!("Unverified path mention: {}", p))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        // Preserve LLM-authored synthesis in normal cases; only force deterministic
        // fallback when path-validation drift is severe.
        if final_validation_issues.len() >= 8 {
            final_response = Some(build_deterministic_analysis_fallback_report(
                &message,
                &self.config.project_root,
                &ledger,
                &coverage_report,
                effective_targets,
                None,
            ));
            final_validation_issues = if let Some(text) = final_response.as_ref() {
                find_unverified_paths(text, &ledger.observed_paths)
                    .into_iter()
                    .take(20)
                    .map(|p| format!("Unverified path mention: {}", p))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
        }
        let _ = tx
            .send(UnifiedStreamEvent::AnalysisValidation {
                status: if final_validation_issues.is_empty() {
                    "ok".to_string()
                } else {
                    "warning".to_string()
                },
                issues: final_validation_issues.clone(),
            })
            .await;

        if let Some(content) = final_response
            .as_ref()
            .filter(|text| !text.trim().is_empty())
        {
            let _ = tx
                .send(UnifiedStreamEvent::TextDelta {
                    content: content.clone(),
                })
                .await;
        }

        let has_final_response = final_response
            .as_ref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false);
        let is_partial_run = ledger.successful_phases < ledger.total_phases
            && usable_phases >= required_usable_phases;
        let final_success = analysis_gate_passed && has_final_response;

        if is_partial_run {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPartial {
                    successful_phases: ledger.successful_phases,
                    partial_phases: ledger.partial_phases,
                    failed_phases: ledger.total_phases.saturating_sub(usable_phases),
                    reason: "Analysis completed with partial phase evidence; returning best-effort verified summary.".to_string(),
                })
                .await;
        }

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisRunSummary {
                success: final_success,
                phase_results: vec![
                    format!("successful_phases={}", ledger.successful_phases),
                    format!("partial_phases={}", ledger.partial_phases),
                    format!("observed_paths={}", ledger.observed_paths.len()),
                    format!("sampled_read_files={}", coverage_report.sampled_read_files),
                    format!("coverage_ratio={:.4}", coverage_report.coverage_ratio),
                    format!(
                        "coverage_target_ratio={:.4}",
                        effective_targets.coverage_ratio
                    ),
                    format!(
                        "sampled_read_ratio={:.4}",
                        coverage_report.sampled_read_ratio
                    ),
                    format!(
                        "sampled_read_target_ratio={:.4}",
                        effective_targets.sampled_read_ratio
                    ),
                    format!(
                        "test_coverage_ratio={:.4}",
                        coverage_report.test_coverage_ratio
                    ),
                    format!(
                        "test_coverage_target_ratio={:.4}",
                        effective_targets.test_coverage_ratio
                    ),
                    format!("validation_issues={}", final_validation_issues.len()),
                    format!("synthesis_success={}", synthesis_success),
                ],
                total_metrics: serde_json::json!({
                    "input_tokens": total_usage.input_tokens,
                    "output_tokens": total_usage.output_tokens,
                    "iterations": total_iterations,
                    "evidence_lines": ledger.evidence_lines.len(),
                    "inventory_total_files": coverage_report.inventory_total_files,
                    "test_files_total": coverage_report.test_files_total,
                    "sampled_read_files": coverage_report.sampled_read_files,
                    "coverage_ratio": coverage_report.coverage_ratio,
                    "coverage_target_ratio": effective_targets.coverage_ratio,
                    "sampled_read_ratio": coverage_report.sampled_read_ratio,
                    "sampled_read_target_ratio": effective_targets.sampled_read_ratio,
                    "test_coverage_ratio": coverage_report.test_coverage_ratio,
                    "test_coverage_target_ratio": effective_targets.test_coverage_ratio,
                    "observed_test_coverage_ratio": coverage_report.observed_test_coverage_ratio,
                    "max_total_read_files": effective_targets.max_total_read_files,
                }),
            })
            .await;

        let _ = tx
            .send(UnifiedStreamEvent::Complete {
                stop_reason: Some("end_turn".to_string()),
            })
            .await;

        let _ = tx
            .send(UnifiedStreamEvent::Usage {
                input_tokens: total_usage.input_tokens,
                output_tokens: total_usage.output_tokens,
                thinking_tokens: total_usage.thinking_tokens,
                cache_read_tokens: total_usage.cache_read_tokens,
                cache_creation_tokens: total_usage.cache_creation_tokens,
            })
            .await;

        let coverage = build_coverage_metrics(&ledger, &coverage_report);
        if let Some(run) = run_handle.as_ref() {
            let _ = run.update_coverage(coverage.clone());
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisCoverageUpdated {
                    run_id: run.run_id().to_string(),
                    metrics: serde_json::to_value(&coverage).unwrap_or_default(),
                })
                .await;
        }

        if let Some(run) = run_handle.as_ref() {
            let report_path = final_response
                .as_ref()
                .filter(|text| !text.trim().is_empty())
                .and_then(|text| run.write_final_report(text).ok());
            let _ = run.complete(
                final_success,
                if final_success {
                    None
                } else {
                    Some("Analysis completed with insufficient verified output".to_string())
                },
            );
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisRunCompleted {
                    run_id: run.run_id().to_string(),
                    success: final_success,
                    manifest_path: run.manifest_path().to_string_lossy().to_string(),
                    report_path,
                })
                .await;
        }

        ExecutionResult {
            response: final_response,
            usage: total_usage,
            iterations: total_iterations,
            success: final_success,
            error: if final_success {
                None
            } else {
                Some("Analysis completed with insufficient verified output".to_string())
            },
        }
    }

    fn existing_analysis_files(&self, candidates: &[&str], limit: usize) -> Vec<String> {
        let mut files = Vec::<String>::new();
        for candidate in candidates {
            let abs = self.config.project_root.join(candidate);
            if abs.is_file() {
                files.push(candidate.replace('\\', "/"));
                if files.len() >= limit {
                    break;
                }
            }
        }
        files
    }

    fn existing_analysis_dirs(&self, candidates: &[&str], limit: usize) -> Vec<String> {
        let mut dirs = Vec::<String>::new();
        for candidate in candidates {
            let abs = self.config.project_root.join(candidate);
            if abs.is_dir() {
                dirs.push(candidate.replace('\\', "/"));
                if dirs.len() >= limit {
                    break;
                }
            }
        }
        dirs
    }

    fn merge_prioritized_files(
        &self,
        primary: Vec<String>,
        secondary: Vec<String>,
        limit: usize,
    ) -> Vec<String> {
        let mut merged = Vec::new();
        let mut seen = HashSet::new();
        for file in primary.into_iter().chain(secondary.into_iter()) {
            let normalized = file.replace('\\', "/");
            if seen.insert(normalized.clone()) {
                merged.push(normalized);
                if merged.len() >= limit {
                    break;
                }
            }
        }
        merged
    }

    fn existing_observed_files(&self, ledger: &AnalysisLedger, limit: usize) -> Vec<String> {
        let mut files = ledger
            .observed_paths
            .iter()
            .filter_map(|candidate| {
                let path = PathBuf::from(candidate);
                let exists = if path.is_absolute() {
                    path.is_file()
                } else {
                    self.config.project_root.join(&path).is_file()
                };
                if exists {
                    Some(candidate.replace('\\', "/"))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        files.truncate(limit);
        files
    }

    pub(super) fn baseline_steps_for_phase(
        &self,
        phase: AnalysisPhase,
        ledger: &AnalysisLedger,
    ) -> Vec<(String, serde_json::Value)> {
        let mut steps = vec![
            ("Cwd".to_string(), serde_json::json!({})),
            ("LS".to_string(), serde_json::json!({ "path": "." })),
        ];

        match phase {
            AnalysisPhase::StructureDiscovery => {
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "pyproject.toml", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "README*.md", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "package.json", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "Cargo.toml", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "tests/**/*.py", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "desktop/src-tauri/tests/**/*.rs", "path": "." }),
                ));
                steps.push((
                    "Glob".to_string(),
                    serde_json::json!({ "pattern": "desktop/src/components/__tests__/**/*.tsx", "path": "." }),
                ));

                let files = self.existing_analysis_files(
                    &[
                        "pyproject.toml",
                        "README.md",
                        "README_zh.md",
                        "README_zh-CN.md",
                        "package.json",
                        "desktop/package.json",
                        "desktop/src-tauri/Cargo.toml",
                        "mcp_server/server.py",
                        "src/plan_cascade/cli/main.py",
                        "tests/test_orchestrator.py",
                        "desktop/src-tauri/tests/integration/mod.rs",
                        "desktop/src/components/__tests__/SimpleMode.test.tsx",
                    ],
                    ANALYSIS_BASELINE_MAX_READ_FILES,
                );
                for file in files {
                    steps.push((
                        "Read".to_string(),
                        serde_json::json!({
                            "file_path": file,
                            "offset": 1,
                            "limit": 120
                        }),
                    ));
                }
            }
            AnalysisPhase::ArchitectureTrace => {
                let mut grep_paths = self.existing_analysis_dirs(
                    &[
                        "src",
                        "mcp_server",
                        "desktop/src-tauri/src",
                        "desktop/src",
                        "tests",
                        "desktop/src-tauri/tests",
                        "desktop/src/components/__tests__",
                    ],
                    7,
                );
                if grep_paths.is_empty() {
                    grep_paths.push(".".to_string());
                }
                for grep_path in grep_paths {
                    steps.push((
                        "Grep".to_string(),
                        serde_json::json!({
                            "pattern": "(class\\s+|def\\s+|fn\\s+|impl\\s+|tauri::command|FastMCP)",
                            "path": grep_path,
                            "output_mode": "files_with_matches",
                            "head_limit": 40
                        }),
                    ));
                }
                let seeded = self.existing_analysis_files(
                    &[
                        "src/plan_cascade/cli/main.py",
                        "src/plan_cascade/core/orchestrator.py",
                        "src/plan_cascade/backends/factory.py",
                        "src/plan_cascade/state/state_manager.py",
                        "mcp_server/server.py",
                        "mcp_server/tools/design_tools.py",
                        "desktop/src-tauri/src/main.rs",
                        "desktop/src/App.tsx",
                        "desktop/src/main.tsx",
                        "desktop/src/store/execution.ts",
                        "tests/test_orchestrator.py",
                        "desktop/src-tauri/tests/integration/mod.rs",
                        "desktop/src/components/__tests__/SimpleMode.test.tsx",
                    ],
                    ANALYSIS_BASELINE_MAX_READ_FILES,
                );
                let observed =
                    self.existing_observed_files(ledger, ANALYSIS_BASELINE_MAX_READ_FILES);
                let files = self.merge_prioritized_files(
                    seeded,
                    observed,
                    ANALYSIS_BASELINE_MAX_READ_FILES,
                );
                for file in files {
                    steps.push((
                        "Read".to_string(),
                        serde_json::json!({
                            "file_path": file,
                            "offset": 1,
                            "limit": 120
                        }),
                    ));
                }
            }
            AnalysisPhase::ConsistencyCheck => {
                let mut grep_paths = self.existing_analysis_dirs(
                    &[
                        "src",
                        "mcp_server",
                        "desktop/src-tauri/src",
                        "desktop/src",
                        "tests",
                        "desktop/src-tauri/tests",
                        "desktop/src/components/__tests__",
                    ],
                    7,
                );
                if grep_paths.is_empty() {
                    grep_paths.push(".".to_string());
                }
                for grep_path in grep_paths {
                    steps.push((
                        "Grep".to_string(),
                        serde_json::json!({
                            "pattern": "(?i)version|__version__|\\\"version\\\"|tauri|orchestrator",
                            "path": grep_path,
                            "output_mode": "files_with_matches",
                            "head_limit": 40
                        }),
                    ));
                }
                let observed =
                    self.existing_observed_files(ledger, ANALYSIS_BASELINE_MAX_READ_FILES);
                let mut files = observed;
                if files.len() < 2 {
                    files = self.merge_prioritized_files(
                        self.existing_analysis_files(
                            &[
                                "pyproject.toml",
                                "README.md",
                                "README_zh.md",
                                "src/plan_cascade/__init__.py",
                                "src/plan_cascade/cli/main.py",
                                "mcp_server/server.py",
                                "desktop/src-tauri/Cargo.toml",
                                "desktop/package.json",
                                "tests/test_orchestrator.py",
                                "desktop/src-tauri/tests/integration/mod.rs",
                                "desktop/src/components/__tests__/SimpleMode.test.tsx",
                            ],
                            ANALYSIS_BASELINE_MAX_READ_FILES,
                        ),
                        files,
                        ANALYSIS_BASELINE_MAX_READ_FILES,
                    );
                }
                files.sort();
                files.dedup();
                files.truncate(ANALYSIS_BASELINE_MAX_READ_FILES);
                for file in files {
                    steps.push((
                        "Read".to_string(),
                        serde_json::json!({
                            "file_path": file,
                            "offset": 1,
                            "limit": 120
                        }),
                    ));
                }
            }
        }

        steps
    }

    async fn execute_baseline_tool_step(
        &self,
        phase: AnalysisPhase,
        tool_id_prefix: &str,
        step_index: usize,
        tool_name: &str,
        args: &serde_json::Value,
        capture: &mut PhaseCapture,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        run_handle: Option<&AnalysisRunHandle>,
    ) {
        let tool_id = format!(
            "{}_{}_{}_{}",
            tool_id_prefix,
            phase.id(),
            step_index + 1,
            tool_name.to_ascii_lowercase()
        );
        let (effective_tool_name, effective_args) =
            match prepare_tool_call_for_execution(tool_name, args, Some(phase.id())) {
                Ok(prepared) => prepared,
                Err(err) => {
                    capture.warnings.push(format!(
                        "{} baseline step dropped: {}",
                        phase.title(),
                        err
                    ));
                    return;
                }
            };

        let start_event = UnifiedStreamEvent::ToolStart {
            tool_id: tool_id.clone(),
            tool_name: effective_tool_name.clone(),
            arguments: Some(effective_args.to_string()),
        };
        let _ = tx.send(start_event.clone()).await;
        self.observe_analysis_event(phase, &start_event, capture, tx)
            .await;

        let result = self
            .tool_executor
            .execute(&effective_tool_name, &effective_args)
            .await;
        let result_event = UnifiedStreamEvent::ToolResult {
            tool_id: tool_id.clone(),
            result: if result.success {
                result.output.clone()
            } else {
                None
            },
            error: if result.success {
                None
            } else {
                result.error.clone()
            },
        };
        let _ = tx.send(result_event.clone()).await;
        self.observe_analysis_event(phase, &result_event, capture, tx)
            .await;

        if let Some(run) = run_handle {
            let primary_path = extract_primary_path_from_arguments(&effective_args);
            let summary = summarize_tool_activity(
                &effective_tool_name,
                Some(&effective_args),
                primary_path.as_deref(),
            );
            let record = EvidenceRecord {
                evidence_id: format!(
                    "{}-{}-{}-{}",
                    phase.id(),
                    tool_id_prefix,
                    step_index + 1,
                    chrono::Utc::now().timestamp_millis()
                ),
                phase_id: phase.id().to_string(),
                sub_agent_id: "baseline".to_string(),
                tool_name: Some(effective_tool_name.clone()),
                file_path: primary_path,
                summary: truncate_for_log(&summary, 400),
                success: result.success,
                timestamp: chrono::Utc::now().timestamp(),
            };
            let _ = run.append_evidence(&record);
        }
    }

    async fn collect_phase_baseline_capture(
        &self,
        phase: AnalysisPhase,
        ledger: &AnalysisLedger,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        run_handle: Option<&AnalysisRunHandle>,
    ) -> PhaseCapture {
        let mut capture = PhaseCapture::default();
        let steps = self.baseline_steps_for_phase(phase, ledger);
        if steps.is_empty() {
            return capture;
        }

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: phase.id().to_string(),
                message: format!("Running baseline evidence pass ({} steps)", steps.len()),
            })
            .await;

        for (idx, (tool_name, args)) in steps.iter().enumerate() {
            self.execute_baseline_tool_step(
                phase,
                "analysis_baseline",
                idx,
                tool_name,
                args,
                &mut capture,
                tx,
                run_handle,
            )
            .await;
        }

        capture
    }

    fn select_chunk_read_files(
        &self,
        phase: AnalysisPhase,
        chunk: &InventoryChunk,
        limit_hint: usize,
    ) -> Vec<String> {
        let mut files = chunk.files.clone();
        files.sort_by_key(|path| {
            let mut score = 0i32;
            let lower = path.to_ascii_lowercase();
            if lower.contains("orchestrator")
                || lower.ends_with("main.py")
                || lower.ends_with("main.rs")
                || lower.ends_with("app.tsx")
                || lower.ends_with("mod.rs")
            {
                score -= 5;
            }
            if lower.contains("test") {
                score -= 3;
            }
            if matches!(phase, AnalysisPhase::ConsistencyCheck) && lower.contains("test") {
                score -= 3;
            }
            if lower.contains("readme") || lower.contains("license") {
                score += 3;
            }
            score
        });
        let limit = limit_hint.max(1).min(files.len().max(1));
        let mut selected = files.iter().take(limit).cloned().collect::<Vec<_>>();

        // Ensure test surface is sampled when a chunk contains tests.
        if chunk.test_files > 0 && !selected.iter().any(|p| looks_like_test_path(p)) {
            if let Some(test_file) = files.iter().find(|p| looks_like_test_path(p)).cloned() {
                if selected.len() >= limit {
                    selected.pop();
                }
                selected.push(test_file);
            }
        }

        selected.sort();
        selected.dedup();
        selected
    }

    fn dynamic_chunk_read_limit(
        &self,
        phase: AnalysisPhase,
        chunk: &InventoryChunk,
        read_budget_remaining: usize,
        chunks_remaining: usize,
        target_read_ratio: f64,
    ) -> usize {
        if chunk.files.is_empty() {
            return 0;
        }
        if read_budget_remaining == 0 {
            return 0;
        }

        let divisor = chunks_remaining.max(1);
        let avg_budget = (read_budget_remaining + divisor - 1) / divisor;
        let mut limit = avg_budget.min(chunk.files.len());
        let ratio_target = clamp_ratio(target_read_ratio);
        let desired_by_ratio = ((chunk.files.len() as f64) * ratio_target).ceil() as usize;

        match self.config.analysis_profile {
            AnalysisProfile::Fast => {
                limit = limit.min(self.config.analysis_limits.max_reads_per_chunk.max(1));
            }
            AnalysisProfile::Balanced => {
                let cap = self.config.analysis_limits.max_reads_per_chunk.max(4);
                limit = limit.min(cap);
            }
            AnalysisProfile::DeepCoverage => {
                // Deep mode behaves like Codex/Claude exploration: keep each chunk broad enough
                // to preserve context quality while still honoring global read budget.
                let floor = match phase {
                    AnalysisPhase::StructureDiscovery => 6,
                    AnalysisPhase::ArchitectureTrace => 8,
                    AnalysisPhase::ConsistencyCheck => 6,
                };
                let preferred = desired_by_ratio.max(floor).min(chunk.files.len());
                limit = limit.max(preferred);
                limit = limit.min(read_budget_remaining).min(chunk.files.len());
            }
        }

        if chunk.test_files > 0 {
            // Keep testing surface visible in every phase.
            limit = limit.max(3.min(chunk.files.len()));
        }

        limit.max(1).min(chunk.files.len())
    }

    async fn perform_coverage_topup_pass(
        &self,
        ledger: &mut AnalysisLedger,
        targets: EffectiveAnalysisTargets,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        run_handle: Option<&AnalysisRunHandle>,
    ) -> usize {
        let Some(inventory) = ledger.inventory.as_ref() else {
            return 0;
        };
        if inventory.total_files == 0 {
            return 0;
        }

        let current_read = inventory
            .items
            .iter()
            .filter(|item| ledger.read_paths.contains(&item.path))
            .count();
        let current_test_read = inventory
            .items
            .iter()
            .filter(|item| item.is_test && ledger.read_paths.contains(&item.path))
            .count();

        let target_read =
            ((inventory.total_files as f64) * targets.sampled_read_ratio).ceil() as usize;
        let target_test_read =
            ((inventory.total_test_files as f64) * targets.test_coverage_ratio).ceil() as usize;
        let max_read_cap = targets
            .max_total_read_files
            .min(inventory.total_files.max(1));

        let read_deficit = target_read.saturating_sub(current_read);
        let test_deficit = target_test_read.saturating_sub(current_test_read);
        let budget_remaining = max_read_cap.saturating_sub(current_read);
        let mut need = read_deficit.max(test_deficit).min(budget_remaining);
        if need == 0 {
            return 0;
        }

        let mut unread_tests = inventory
            .items
            .iter()
            .filter(|item| item.is_test && !ledger.read_paths.contains(&item.path))
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        let mut unread_non_tests = inventory
            .items
            .iter()
            .filter(|item| !item.is_test && !ledger.read_paths.contains(&item.path))
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();

        unread_tests.sort();
        unread_non_tests.sort();

        let mut selected = Vec::<String>::new();
        let test_need = test_deficit.min(need);
        selected.extend(unread_tests.into_iter().take(test_need));
        need = need.saturating_sub(selected.len());
        if need > 0 {
            selected.extend(unread_non_tests.into_iter().take(need));
        }
        if selected.is_empty() {
            return 0;
        }

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: "analysis".to_string(),
                message: format!(
                    "Coverage top-up pass: reading {} additional files (tests prioritized)",
                    selected.len()
                ),
            })
            .await;

        let mut added = 0usize;
        let mut sampled_details = Vec::new();
        for path in selected {
            if ledger.read_paths.contains(&path) {
                continue;
            }
            let abs = self.config.project_root.join(&path);
            if !abs.is_file() {
                continue;
            }
            let head = summarize_file_head(&abs, 8)
                .unwrap_or_else(|| "binary/large file (metadata-only)".to_string());
            ledger.read_paths.insert(path.clone());
            ledger.observed_paths.insert(path.clone());
            added += 1;
            if sampled_details.len() < 12 {
                sampled_details.push(format!("- {} :: {}", path, truncate_for_log(&head, 120)));
            }
        }

        if added > 0 {
            if ledger.evidence_lines.len() < MAX_ANALYSIS_EVIDENCE_LINES {
                ledger.evidence_lines.push(format!(
                    "Coverage top-up read {} additional files (sample):",
                    added
                ));
                if !sampled_details.is_empty()
                    && ledger.evidence_lines.len() < MAX_ANALYSIS_EVIDENCE_LINES
                {
                    ledger.evidence_lines.push(sampled_details.join(" | "));
                }
            }
            if let Some(run) = run_handle {
                let _ = run.write_json_artifact(
                    "final/coverage_topup.json",
                    &serde_json::json!({
                        "added_read_files": added,
                        "sampled_details": sampled_details,
                    }),
                );
            }
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                    phase_id: "analysis".to_string(),
                    message: format!("Coverage top-up completed: +{} read files", added),
                })
                .await;
        }

        added
    }

    async fn collect_chunk_capture(
        &self,
        phase: AnalysisPhase,
        chunk: &InventoryChunk,
        chunk_index: usize,
        read_limit: usize,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        run_handle: Option<&AnalysisRunHandle>,
    ) -> (PhaseCapture, ChunkSummaryRecord) {
        let mut capture = PhaseCapture::default();
        let prefix = format!("analysis_chunk_{}", chunk.chunk_id.replace('-', "_"));

        // Treat chunk enumeration as observed coverage once this chunk starts.
        for path in &chunk.files {
            capture.observed_paths.insert(path.clone());
        }

        if let Some(run) = run_handle {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisChunkStarted {
                    run_id: run.run_id().to_string(),
                    phase_id: phase.id().to_string(),
                    chunk_id: chunk.chunk_id.clone(),
                    component: chunk.component.clone(),
                    file_count: chunk.files.len(),
                })
                .await;
        }

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: phase.id().to_string(),
                message: format!(
                    "Chunk {}/{}: {} ({})",
                    chunk_index + 1,
                    self.config.analysis_limits.max_chunks_per_phase.max(1),
                    chunk.chunk_id,
                    chunk.component
                ),
            })
            .await;

        let dir_hint = chunk
            .files
            .first()
            .and_then(|p| {
                let normalized = p.replace('\\', "/");
                normalized
                    .rfind('/')
                    .map(|idx| normalized[..idx].to_string())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ".".to_string());

        let mut steps = vec![
            (
                "LS".to_string(),
                serde_json::json!({ "path": dir_hint.clone() }),
            ),
            (
                "Grep".to_string(),
                serde_json::json!({
                    "pattern": "(class\\s+|def\\s+|fn\\s+|impl\\s+|tauri::command|FastMCP|test_)",
                    "path": dir_hint,
                    "output_mode": "files_with_matches",
                    "head_limit": 30
                }),
            ),
        ];

        for file in self.select_chunk_read_files(phase, chunk, read_limit) {
            steps.push((
                "Read".to_string(),
                serde_json::json!({
                    "file_path": file,
                    "offset": 1,
                    "limit": 100
                }),
            ));
        }

        for (idx, (tool_name, args)) in steps.iter().enumerate() {
            self.execute_baseline_tool_step(
                phase,
                &prefix,
                idx,
                tool_name,
                args,
                &mut capture,
                tx,
                run_handle,
            )
            .await;
        }

        let mut observed_paths = capture.observed_paths.iter().cloned().collect::<Vec<_>>();
        observed_paths.sort();
        let mut read_files = capture.read_paths.iter().cloned().collect::<Vec<_>>();
        read_files.sort();

        let summary = format!(
            "chunk={} component={} tool_calls={} read_calls={} observed_paths={} sampled_files={}",
            chunk.chunk_id,
            chunk.component,
            capture.tool_calls,
            capture.read_calls,
            capture.observed_paths.len(),
            read_files.len()
        );

        let record = ChunkSummaryRecord {
            phase_id: phase.id().to_string(),
            chunk_id: chunk.chunk_id.clone(),
            component: chunk.component.clone(),
            summary,
            observed_paths,
            read_files,
        };

        if let Some(run) = run_handle {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisChunkCompleted {
                    run_id: run.run_id().to_string(),
                    phase_id: phase.id().to_string(),
                    chunk_id: chunk.chunk_id.clone(),
                    observed_paths: capture.observed_paths.len(),
                    read_files: capture.read_paths.len(),
                })
                .await;
        }

        (capture, record)
    }

    async fn run_analysis_phase_layered(
        &self,
        phase: AnalysisPhase,
        base_prompt: String,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        total_usage: &mut UsageStats,
        total_iterations: &mut u32,
        ledger: &mut AnalysisLedger,
        run_handle: Option<&AnalysisRunHandle>,
        max_total_read_files: usize,
        target_read_ratio: f64,
    ) -> String {
        let policy = AnalysisPhasePolicy::for_phase(phase);
        let mut layer_summaries = Vec::new();
        let mut sub_agent_results = Vec::<SubAgentResultRecord>::new();
        let mut aggregate_capture = PhaseCapture::default();
        let mut aggregate_usage = UsageStats::default();
        let mut aggregate_iterations = 0u32;

        let layers = phase.layers();
        let upstream_summary = ledger
            .phase_summaries
            .last()
            .cloned()
            .unwrap_or_else(|| "(none)".to_string());
        let baseline_capture = self
            .collect_phase_baseline_capture(phase, ledger, tx, run_handle)
            .await;
        aggregate_capture.tool_calls += baseline_capture.tool_calls;
        aggregate_capture.read_calls += baseline_capture.read_calls;
        aggregate_capture.grep_calls += baseline_capture.grep_calls;
        aggregate_capture.glob_calls += baseline_capture.glob_calls;
        aggregate_capture.ls_calls += baseline_capture.ls_calls;
        aggregate_capture.cwd_calls += baseline_capture.cwd_calls;
        aggregate_capture
            .observed_paths
            .extend(baseline_capture.observed_paths.iter().cloned());
        aggregate_capture
            .read_paths
            .extend(baseline_capture.read_paths.iter().cloned());
        aggregate_capture
            .evidence_lines
            .extend(baseline_capture.evidence_lines.iter().cloned());
        aggregate_capture
            .warnings
            .extend(baseline_capture.warnings.iter().cloned());
        let baseline_gate_failures = evaluate_analysis_quota(&baseline_capture, &policy.quota);
        let baseline_satisfies_phase = matches!(phase, AnalysisPhase::StructureDiscovery)
            && baseline_gate_failures.is_empty()
            && analysis_layer_goal_satisfied(phase, &baseline_capture);

        let selected_chunks = ledger
            .chunk_plan
            .as_ref()
            .map(|plan| {
                select_chunks_for_phase(
                    phase.id(),
                    plan,
                    &self.config.analysis_limits,
                    &self.config.analysis_profile,
                )
            })
            .unwrap_or_default();
        let mut phase_chunk_records = Vec::<ChunkSummaryRecord>::new();
        let mut read_budget_remaining =
            max_total_read_files.saturating_sub(ledger.read_paths.len());
        for (chunk_idx, chunk) in selected_chunks.iter().enumerate() {
            if read_budget_remaining == 0 {
                break;
            }
            let chunks_remaining = selected_chunks.len().saturating_sub(chunk_idx).max(1);
            let chunk_read_limit = self.dynamic_chunk_read_limit(
                phase,
                chunk,
                read_budget_remaining,
                chunks_remaining,
                target_read_ratio,
            );
            let (chunk_capture, chunk_record) = self
                .collect_chunk_capture(phase, chunk, chunk_idx, chunk_read_limit, tx, run_handle)
                .await;
            aggregate_capture.tool_calls += chunk_capture.tool_calls;
            aggregate_capture.read_calls += chunk_capture.read_calls;
            aggregate_capture.grep_calls += chunk_capture.grep_calls;
            aggregate_capture.glob_calls += chunk_capture.glob_calls;
            aggregate_capture.ls_calls += chunk_capture.ls_calls;
            aggregate_capture.cwd_calls += chunk_capture.cwd_calls;
            aggregate_capture
                .observed_paths
                .extend(chunk_capture.observed_paths.iter().cloned());
            aggregate_capture
                .read_paths
                .extend(chunk_capture.read_paths.iter().cloned());
            aggregate_capture
                .evidence_lines
                .extend(chunk_capture.evidence_lines.iter().cloned());
            aggregate_capture
                .warnings
                .extend(chunk_capture.warnings.iter().cloned());
            read_budget_remaining = read_budget_remaining.saturating_sub(chunk_capture.read_calls);
            if let Some(run) = run_handle {
                let _ = run.write_json_artifact(
                    &format!("chunks/{}/{}.json", phase.id(), chunk.chunk_id),
                    &chunk_record,
                );
            }
            phase_chunk_records.push(chunk_record.clone());
            ledger.chunk_summaries.push(chunk_record);
        }
        if !phase_chunk_records.is_empty() {
            layer_summaries.push(merge_chunk_summaries(
                phase.id(),
                phase.title(),
                &phase_chunk_records,
                MAX_ANALYSIS_PHASE_SUMMARY_CHARS * 2,
            ));
        }

        let baseline_digest = baseline_capture
            .evidence_lines
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let worker_base_prompt = if baseline_digest.is_empty() {
            base_prompt.clone()
        } else {
            format!(
                "{}\n\nVerified baseline evidence (do not re-scan blindly):\n{}",
                base_prompt, baseline_digest
            )
        };

        let plan = build_phase_plan(
            phase.id(),
            phase.title(),
            phase.objective(),
            layers,
            &worker_base_prompt,
            &analysis_scope_guidance(&worker_base_prompt),
            &upstream_summary,
        );

        if let Some(run) = run_handle {
            let _ = run.record_phase_plan(plan.clone());
        }
        if let Some(run) = run_handle {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhasePlanned {
                    run_id: run.run_id().to_string(),
                    phase_id: plan.phase_id.clone(),
                    title: plan.title.clone(),
                    objective: plan.objective.clone(),
                    worker_count: plan.workers.len(),
                    layers: plan.layers.clone(),
                })
                .await;
        }

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseStart {
                phase_id: phase.id().to_string(),
                title: phase.title().to_string(),
                objective: phase.objective().to_string(),
            })
            .await;

        for worker in &plan.workers {
            if let Some(run) = run_handle {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisSubAgentPlanned {
                        run_id: run.run_id().to_string(),
                        phase_id: phase.id().to_string(),
                        sub_agent_id: worker.sub_agent_id.clone(),
                        role: worker.role.clone(),
                        objective: worker.objective.clone(),
                    })
                    .await;
            }
        }

        for worker in &plan.workers {
            if baseline_satisfies_phase {
                break;
            }
            if let Some(run) = run_handle {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisSubAgentProgress {
                        run_id: run.run_id().to_string(),
                        phase_id: phase.id().to_string(),
                        sub_agent_id: worker.sub_agent_id.clone(),
                        status: "started".to_string(),
                        message: worker.objective.clone(),
                    })
                    .await;
            }
            let _ = tx
                .send(UnifiedStreamEvent::SubAgentStart {
                    sub_agent_id: worker.sub_agent_id.clone(),
                    prompt: worker.objective.clone(),
                    task_type: Some(phase.task_type().to_string()),
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                    phase_id: phase.id().to_string(),
                    message: format!("Running {} ({})", worker.sub_agent_id, worker.role),
                })
                .await;

            let prompt = format!(
                "{}\n\n\
                 {}\n\n\
                 Execution constraints for this worker:\n\
                 - Stop once the objective is satisfied.\n\
                 - Avoid broad rescans of previously explored areas.\n\
                 - Produce concise, evidence-backed findings only.",
                worker_base_prompt, worker.prompt_suffix
            );
            let worker_phase_id = format!("{}:{}", phase.id(), worker.sub_agent_id);
            let outcome = self
                .run_analysis_phase(phase, prompt, tx, Some(worker_phase_id), false, false)
                .await;
            merge_usage(&mut aggregate_usage, &outcome.usage);
            aggregate_iterations += outcome.iterations;

            aggregate_capture.tool_calls += outcome.capture.tool_calls;
            aggregate_capture.read_calls += outcome.capture.read_calls;
            aggregate_capture.grep_calls += outcome.capture.grep_calls;
            aggregate_capture.glob_calls += outcome.capture.glob_calls;
            aggregate_capture.ls_calls += outcome.capture.ls_calls;
            aggregate_capture.cwd_calls += outcome.capture.cwd_calls;
            aggregate_capture
                .observed_paths
                .extend(outcome.capture.observed_paths.iter().cloned());
            aggregate_capture
                .read_paths
                .extend(outcome.capture.read_paths.iter().cloned());
            aggregate_capture
                .evidence_lines
                .extend(outcome.capture.evidence_lines.iter().cloned());
            aggregate_capture
                .warnings
                .extend(outcome.capture.warnings.iter().cloned());

            if let Some(summary) = outcome.response.as_ref().filter(|s| !s.trim().is_empty()) {
                layer_summaries.push(format!(
                    "### {} - {}\n{}",
                    phase.title(),
                    worker.objective,
                    truncate_for_log(summary.trim(), MAX_ANALYSIS_PHASE_SUMMARY_CHARS)
                ));
            }

            if let Some(run) = run_handle {
                for (idx, evidence_line) in outcome.capture.evidence_lines.iter().enumerate() {
                    let file_path = extract_path_candidates_from_text(evidence_line)
                        .into_iter()
                        .next();
                    let record = EvidenceRecord {
                        evidence_id: format!(
                            "{}-{}-{}-{}",
                            phase.id(),
                            worker.layer_index,
                            idx + 1,
                            chrono::Utc::now().timestamp_millis()
                        ),
                        phase_id: phase.id().to_string(),
                        sub_agent_id: worker.sub_agent_id.clone(),
                        tool_name: None,
                        file_path,
                        summary: truncate_for_log(evidence_line, 400),
                        success: true,
                        timestamp: chrono::Utc::now().timestamp(),
                    };
                    let _ = run.append_evidence(&record);
                }
            }

            let status_text = match outcome.status {
                AnalysisPhaseStatus::Passed => "passed",
                AnalysisPhaseStatus::Partial => "partial",
                AnalysisPhaseStatus::Failed => "failed",
            }
            .to_string();
            let usage_json = serde_json::json!({
                "input_tokens": outcome.usage.input_tokens,
                "output_tokens": outcome.usage.output_tokens,
                "iterations": outcome.iterations,
            });
            let metrics_json = serde_json::json!({
                "tool_calls": outcome.capture.tool_calls,
                "read_calls": outcome.capture.read_calls,
                "grep_calls": outcome.capture.grep_calls,
                "glob_calls": outcome.capture.glob_calls,
                "ls_calls": outcome.capture.ls_calls,
                "cwd_calls": outcome.capture.cwd_calls,
                "observed_paths": outcome.capture.observed_paths.len(),
            });
            sub_agent_results.push(SubAgentResultRecord {
                sub_agent_id: worker.sub_agent_id.clone(),
                role: worker.role.clone(),
                status: status_text.clone(),
                summary: outcome
                    .response
                    .as_ref()
                    .map(|text| truncate_for_log(text, 1200)),
                usage: usage_json.clone(),
                metrics: metrics_json.clone(),
                error: outcome.error.clone(),
            });

            let _ = tx
                .send(UnifiedStreamEvent::SubAgentEnd {
                    sub_agent_id: worker.sub_agent_id.clone(),
                    success: !matches!(outcome.status, AnalysisPhaseStatus::Failed),
                    usage: usage_json,
                })
                .await;
            if let Some(run) = run_handle {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisSubAgentProgress {
                        run_id: run.run_id().to_string(),
                        phase_id: phase.id().to_string(),
                        sub_agent_id: worker.sub_agent_id.clone(),
                        status: status_text,
                        message: outcome
                            .error
                            .clone()
                            .unwrap_or_else(|| "completed".to_string()),
                    })
                    .await;
            }

            if analysis_layer_goal_satisfied(phase, &aggregate_capture)
                && !matches!(outcome.status, AnalysisPhaseStatus::Failed)
                && sub_agent_results.len() >= phase.min_workers_before_early_exit()
            {
                break;
            }
        }

        if baseline_satisfies_phase {
            layer_summaries.push(build_phase_summary_from_evidence(phase, &baseline_capture));
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                    phase_id: phase.id().to_string(),
                    message:
                        "Baseline evidence already satisfies this phase; worker execution skipped."
                            .to_string(),
                })
                .await;
        }

        let phase_gate_failures = evaluate_analysis_quota(&aggregate_capture, &policy.quota);
        let has_worker_output =
            !layer_summaries.is_empty() || !aggregate_capture.evidence_lines.is_empty();
        let has_worker_success = sub_agent_results
            .iter()
            .any(|item| item.status == "passed" || item.status == "partial");
        let phase_status = if phase_gate_failures.is_empty() && has_worker_output {
            AnalysisPhaseStatus::Passed
        } else if has_worker_success || analysis_layer_goal_satisfied(phase, &aggregate_capture) {
            AnalysisPhaseStatus::Partial
        } else {
            AnalysisPhaseStatus::Failed
        };

        let phase_summary = if layer_summaries.is_empty() {
            build_phase_summary_from_evidence(phase, &aggregate_capture)
        } else {
            layer_summaries.join("\n\n")
        };
        let phase_error = if matches!(phase_status, AnalysisPhaseStatus::Failed) {
            Some(format!(
                "{} workers failed to produce usable evidence",
                phase.title()
            ))
        } else {
            None
        };
        let aggregated_outcome = AnalysisPhaseOutcome {
            phase,
            response: Some(phase_summary.clone()),
            usage: aggregate_usage.clone(),
            iterations: aggregate_iterations,
            status: phase_status,
            error: phase_error.clone(),
            capture: aggregate_capture.clone(),
        };

        merge_usage(total_usage, &aggregate_usage);
        *total_iterations += aggregate_iterations;
        ledger.record(&aggregated_outcome);

        let phase_usage = serde_json::json!({
            "input_tokens": aggregate_usage.input_tokens,
            "output_tokens": aggregate_usage.output_tokens,
            "iterations": aggregate_iterations,
        });
        let phase_metrics = serde_json::json!({
            "tool_calls": aggregate_capture.tool_calls,
            "read_calls": aggregate_capture.read_calls,
            "grep_calls": aggregate_capture.grep_calls,
            "glob_calls": aggregate_capture.glob_calls,
            "ls_calls": aggregate_capture.ls_calls,
            "cwd_calls": aggregate_capture.cwd_calls,
            "observed_paths": aggregate_capture.observed_paths.len(),
            "workers": sub_agent_results.len(),
        });
        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseEnd {
                phase_id: phase.id().to_string(),
                success: !matches!(phase_status, AnalysisPhaseStatus::Failed),
                usage: phase_usage.clone(),
                metrics: phase_metrics.clone(),
            })
            .await;

        if !phase_gate_failures.is_empty() {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisGateFailure {
                    phase_id: phase.id().to_string(),
                    attempt: 1,
                    reasons: phase_gate_failures,
                })
                .await;
        }

        if let Some(run) = run_handle {
            let summary_path = run.write_phase_summary(phase.id(), &phase_summary).ok();
            let _ = run.record_phase_result(AnalysisPhaseResultRecord {
                phase_id: phase.id().to_string(),
                title: phase.title().to_string(),
                status: match phase_status {
                    AnalysisPhaseStatus::Passed => "passed",
                    AnalysisPhaseStatus::Partial => "partial",
                    AnalysisPhaseStatus::Failed => "failed",
                }
                .to_string(),
                summary_path,
                usage: phase_usage,
                metrics: phase_metrics,
                warnings: aggregate_capture.warnings.clone(),
                sub_agents: sub_agent_results,
            });
            let phase_coverage = if let Some(inventory) = ledger.inventory.as_ref() {
                compute_coverage_report(
                    inventory,
                    &ledger.observed_paths,
                    &ledger.read_paths,
                    ledger
                        .chunk_plan
                        .as_ref()
                        .map(|plan| plan.chunks.len())
                        .unwrap_or(0),
                    0,
                )
            } else {
                AnalysisCoverageReport::default()
            };
            let coverage = build_coverage_metrics(ledger, &phase_coverage);
            let _ = run.update_coverage(coverage.clone());
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisCoverageUpdated {
                    run_id: run.run_id().to_string(),
                    metrics: serde_json::to_value(&coverage).unwrap_or_default(),
                })
                .await;
        }

        phase_summary
    }

    async fn run_analysis_phase(
        &self,
        phase: AnalysisPhase,
        prompt: String,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
        phase_event_id: Option<String>,
        emit_lifecycle_events: bool,
        enforce_quota_gate: bool,
    ) -> AnalysisPhaseOutcome {
        let phase_id = phase_event_id.unwrap_or_else(|| phase.id().to_string());
        let policy = AnalysisPhasePolicy::for_phase(phase);
        let phase_token_budget = analysis_phase_token_budget(self.provider.context_window(), phase);
        if emit_lifecycle_events {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseStart {
                    phase_id: phase_id.clone(),
                    title: phase.title().to_string(),
                    objective: phase.objective().to_string(),
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::SubAgentStart {
                    sub_agent_id: phase_id.clone(),
                    prompt: format!("{}: {}", phase.title(), phase.objective()),
                    task_type: Some(phase.task_type().to_string()),
                })
                .await;
        }

        let tools = get_basic_tool_definitions();
        let mut total_usage = UsageStats::default();
        let mut total_iterations = 0u32;
        let mut aggregate_capture = PhaseCapture::default();
        let mut final_response: Option<String> = None;
        let mut final_error: Option<String> = None;
        let mut phase_status = AnalysisPhaseStatus::Failed;
        let mut gate_failure_history: Vec<String> = Vec::new();

        for attempt in 1..=policy.max_attempts {
            if self.cancellation_token.is_cancelled() {
                final_error = Some("Execution cancelled".to_string());
                break;
            }

            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseAttemptStart {
                    phase_id: phase_id.clone(),
                    attempt,
                    max_attempts: policy.max_attempts,
                    required_tools: policy
                        .quota
                        .required_tools
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                })
                .await;

            let phase_system_prompt = if enforce_quota_gate {
                analysis_phase_system_prompt_with_quota(phase, &policy.quota, &gate_failure_history)
            } else {
                analysis_phase_worker_prompt(phase)
            };
            let phase_config = OrchestratorConfig {
                provider: self.config.provider.clone(),
                system_prompt: Some(phase_system_prompt),
                max_iterations: phase.max_iterations(),
                max_total_tokens: phase_token_budget,
                project_root: self.config.project_root.clone(),
                analysis_artifacts_root: self.config.analysis_artifacts_root.clone(),
                streaming: true,
                enable_compaction: true,
                analysis_profile: self.config.analysis_profile.clone(),
                analysis_limits: self.config.analysis_limits.clone(),
                analysis_session_id: self.config.analysis_session_id.clone(),
            };
            let phase_agent =
                OrchestratorService::new_sub_agent(phase_config, self.cancellation_token.clone());

            let request_options = LlmRequestOptions {
                tool_call_mode: if enforce_quota_gate && attempt <= policy.force_tool_mode_attempts
                {
                    ToolCallMode::Required
                } else {
                    ToolCallMode::Auto
                },
                fallback_tool_format_mode: FallbackToolFormatMode::Strict,
                temperature_override: Some(policy.temperature_override),
                reasoning_effort_override: None,
                analysis_phase: Some(phase_id.clone()),
            };
            let force_prompt_fallback = !self.provider.supports_tools();

            let (sub_tx, mut sub_rx) = mpsc::channel::<UnifiedStreamEvent>(256);
            let (result_tx, result_rx) = tokio::sync::oneshot::channel::<ExecutionResult>();
            let attempt_prompt = prompt.clone();
            let attempt_tools = tools.clone();
            tokio::spawn(async move {
                let result = phase_agent
                    .execute_story_with_request_options(
                        &attempt_prompt,
                        &attempt_tools,
                        sub_tx,
                        request_options,
                        force_prompt_fallback,
                    )
                    .await;
                let _ = result_tx.send(result);
            });

            let mut attempt_capture = PhaseCapture::default();
            while let Some(event) = sub_rx.recv().await {
                self.observe_analysis_event(phase, &event, &mut attempt_capture, tx)
                    .await;
            }

            let attempt_result = match result_rx.await {
                Ok(result) => result,
                Err(_) => ExecutionResult {
                    response: None,
                    usage: UsageStats::default(),
                    iterations: 0,
                    success: false,
                    error: Some("Sub-agent task join error".to_string()),
                },
            };

            merge_usage(&mut total_usage, &attempt_result.usage);
            total_iterations += attempt_result.iterations;
            aggregate_capture.tool_calls += attempt_capture.tool_calls;
            aggregate_capture.read_calls += attempt_capture.read_calls;
            aggregate_capture.grep_calls += attempt_capture.grep_calls;
            aggregate_capture.glob_calls += attempt_capture.glob_calls;
            aggregate_capture.ls_calls += attempt_capture.ls_calls;
            aggregate_capture.cwd_calls += attempt_capture.cwd_calls;
            aggregate_capture
                .observed_paths
                .extend(attempt_capture.observed_paths.iter().cloned());
            aggregate_capture
                .read_paths
                .extend(attempt_capture.read_paths.iter().cloned());
            aggregate_capture
                .evidence_lines
                .extend(attempt_capture.evidence_lines.iter().cloned());
            aggregate_capture
                .warnings
                .extend(attempt_capture.warnings.iter().cloned());

            let gate_failures = if enforce_quota_gate {
                evaluate_analysis_quota(&attempt_capture, &policy.quota)
            } else {
                Vec::new()
            };
            let attempt_token_usage = attempt_result.usage.total_tokens();
            let token_pressure_threshold = (phase_token_budget as f64 * 0.85) as u32;
            let token_pressure = attempt_token_usage >= token_pressure_threshold
                || attempt_result
                    .error
                    .as_deref()
                    .map(|e| e.to_lowercase().contains("token budget"))
                    .unwrap_or(false);
            let has_min_evidence = if enforce_quota_gate {
                attempt_capture.read_calls >= 1
                    && attempt_capture.tool_calls >= 2
                    && !attempt_capture.observed_paths.is_empty()
            } else {
                attempt_capture.tool_calls >= 1 || !attempt_capture.observed_paths.is_empty()
            };
            let hard_gate_failure = if enforce_quota_gate {
                attempt_capture.read_calls == 0
                    || attempt_capture.tool_calls == 0
                    || attempt_capture.observed_paths.is_empty()
            } else {
                false
            };

            final_response = attempt_result.response.clone().or(final_response);
            let mut has_text_response = final_response
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if !has_text_response && !enforce_quota_gate && has_min_evidence {
                final_response = Some(build_phase_summary_from_evidence(phase, &attempt_capture));
                has_text_response = true;
            }
            let soft_success = if enforce_quota_gate {
                !attempt_result.success
                    && gate_failures.is_empty()
                    && has_min_evidence
                    && has_text_response
            } else {
                has_text_response && !attempt_result.success
            };
            let attempt_success =
                (attempt_result.success && gate_failures.is_empty()) || soft_success;
            let attempt_partial = !attempt_success
                && has_min_evidence
                && (!hard_gate_failure && (token_pressure || attempt == policy.max_attempts));
            if !attempt_success {
                if let Some(err) = attempt_result.error.as_ref() {
                    gate_failure_history.push(format!("attempt {} error: {}", attempt, err));
                }
                gate_failure_history.extend(gate_failures.iter().cloned());
            } else if soft_success {
                gate_failure_history.push(format!(
                    "attempt {} accepted with soft success after budget pressure",
                    attempt
                ));
            }

            let attempt_metrics = serde_json::json!({
                "tool_calls": attempt_capture.tool_calls,
                "read_calls": attempt_capture.read_calls,
                "grep_calls": attempt_capture.grep_calls,
                "glob_calls": attempt_capture.glob_calls,
                "ls_calls": attempt_capture.ls_calls,
                "cwd_calls": attempt_capture.cwd_calls,
                "observed_paths": attempt_capture.observed_paths.len(),
                "attempt_tokens": attempt_token_usage,
                "token_budget": phase_token_budget,
            });

            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseAttemptEnd {
                    phase_id: phase_id.clone(),
                    attempt,
                    success: attempt_success,
                    metrics: attempt_metrics,
                    gate_failures: gate_failures.clone(),
                })
                .await;

            if attempt_success {
                phase_status = AnalysisPhaseStatus::Passed;
                break;
            }

            if attempt_partial {
                phase_status = AnalysisPhaseStatus::Partial;
                let partial_reasons = if gate_failures.is_empty() {
                    vec!["Phase reached token/attempt budget with sufficient evidence".to_string()]
                } else {
                    gate_failures.iter().take(3).cloned().collect()
                };
                final_error = Some(if token_pressure {
                    format!(
                        "Phase reached token budget pressure ({}/{}) and returned partial evidence",
                        attempt_token_usage, phase_token_budget
                    )
                } else {
                    "Phase returned partial evidence after exhausting retry budget".to_string()
                });
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisPhaseDegraded {
                        phase_id: phase_id.clone(),
                        attempt,
                        reasons: partial_reasons,
                    })
                    .await;
                break;
            }

            if !gate_failures.is_empty() {
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisGateFailure {
                        phase_id: phase_id.clone(),
                        attempt,
                        reasons: gate_failures,
                    })
                    .await;
            }
        }

        if matches!(phase_status, AnalysisPhaseStatus::Failed) && final_error.is_none() {
            final_error = Some(if gate_failure_history.is_empty() {
                "Analysis phase failed with insufficient evidence".to_string()
            } else {
                format!(
                    "Analysis phase failed with insufficient evidence: {}",
                    gate_failure_history
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            });
        }

        let metrics = serde_json::json!({
            "tool_calls": aggregate_capture.tool_calls,
            "read_calls": aggregate_capture.read_calls,
            "grep_calls": aggregate_capture.grep_calls,
            "glob_calls": aggregate_capture.glob_calls,
            "ls_calls": aggregate_capture.ls_calls,
            "cwd_calls": aggregate_capture.cwd_calls,
            "observed_paths": aggregate_capture.observed_paths.len(),
            "attempts": policy.max_attempts,
            "token_budget": phase_token_budget,
        });
        let usage = serde_json::json!({
            "input_tokens": total_usage.input_tokens,
            "output_tokens": total_usage.output_tokens,
            "iterations": total_iterations,
        });
        let phase_success = matches!(phase_status, AnalysisPhaseStatus::Passed);
        let phase_partial = matches!(phase_status, AnalysisPhaseStatus::Partial);
        let sub_agent_success = phase_success || phase_partial;

        if emit_lifecycle_events {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseEnd {
                    phase_id: phase_id.clone(),
                    success: sub_agent_success,
                    usage: usage.clone(),
                    metrics,
                })
                .await;
            let _ = tx
                .send(UnifiedStreamEvent::SubAgentEnd {
                    sub_agent_id: phase_id,
                    success: sub_agent_success,
                    usage,
                })
                .await;
        }

        AnalysisPhaseOutcome {
            phase,
            response: final_response,
            usage: total_usage,
            iterations: total_iterations,
            status: phase_status,
            error: final_error,
            capture: aggregate_capture,
        }
    }

    async fn observe_analysis_event(
        &self,
        phase: AnalysisPhase,
        event: &UnifiedStreamEvent,
        capture: &mut PhaseCapture,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) {
        match event {
            UnifiedStreamEvent::ToolStart {
                tool_id,
                tool_name,
                arguments,
                ..
            } => {
                let args_json = parse_tool_arguments(arguments);
                let pending = capture
                    .pending_tools
                    .entry(tool_id.clone())
                    .or_insert_with(PendingAnalysisToolCall::default);
                pending.tool_name = tool_name.clone();
                if args_json.is_some() {
                    pending.arguments = args_json;
                }
            }
            UnifiedStreamEvent::ToolComplete {
                tool_id,
                tool_name,
                arguments,
            } => {
                let args_json = serde_json::from_str::<serde_json::Value>(arguments).ok();
                let pending = capture
                    .pending_tools
                    .entry(tool_id.clone())
                    .or_insert_with(PendingAnalysisToolCall::default);
                pending.tool_name = tool_name.clone();
                if args_json.is_some() {
                    pending.arguments = args_json;
                }
            }
            UnifiedStreamEvent::ToolResult { tool_id, error, .. } => {
                let pending = capture.pending_tools.remove(tool_id);
                let (tool_name, args_json) = match pending {
                    Some(p) => (p.tool_name, p.arguments),
                    None => {
                        if let Some(err) = error.as_ref() {
                            let compact_err = truncate_for_log(err, 180);
                            capture.warnings.push(format!(
                                "{} tool error: {}",
                                phase.title(),
                                compact_err
                            ));
                        }
                        return;
                    }
                };

                let is_valid = is_valid_analysis_tool_start(&tool_name, args_json.as_ref());
                let primary_path = args_json
                    .as_ref()
                    .and_then(extract_primary_path_from_arguments);
                let summary = summarize_tool_activity(
                    &tool_name,
                    args_json.as_ref(),
                    primary_path.as_deref(),
                );

                if let Some(err) = error.as_ref() {
                    let compact_err = truncate_for_log(err, 180);
                    capture.warnings.push(format!(
                        "{} tool error ({}): {}",
                        phase.title(),
                        summary,
                        compact_err
                    ));
                    return;
                }

                if !is_valid {
                    capture.warnings.push(format!(
                        "{} invalid tool call ignored for evidence: {}",
                        phase.title(),
                        summary
                    ));
                    return;
                }

                capture.tool_calls += 1;
                match tool_name.as_str() {
                    "Read" => capture.read_calls += 1,
                    "Grep" => capture.grep_calls += 1,
                    "Glob" => capture.glob_calls += 1,
                    "LS" => capture.ls_calls += 1,
                    "Cwd" => capture.cwd_calls += 1,
                    _ => {}
                }

                if let Some(path) = primary_path.as_ref() {
                    capture.observed_paths.insert(path.clone());
                    if tool_name == "Read" {
                        capture.read_paths.insert(path.clone());
                    }
                }
                if let Some(args) = args_json.as_ref() {
                    for p in extract_all_paths_from_arguments(args) {
                        capture.observed_paths.insert(p);
                    }
                }

                if capture.evidence_lines.len() < MAX_ANALYSIS_EVIDENCE_LINES {
                    capture
                        .evidence_lines
                        .push(format!("- [{}] {}", phase.id(), summary));
                }

                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisEvidence {
                        phase_id: phase.id().to_string(),
                        tool_name,
                        file_path: primary_path,
                        summary,
                        success: Some(true),
                    })
                    .await;
            }
            UnifiedStreamEvent::Error { message, .. } => {
                let compact = truncate_for_log(message, 200);
                capture
                    .warnings
                    .push(format!("{} stream error: {}", phase.title(), compact));
                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                        phase_id: phase.id().to_string(),
                        message: format!("Warning: {}", compact),
                    })
                    .await;
            }
            _ => {}
        }
    }
}
