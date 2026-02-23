use super::*;

impl OrchestratorService {
    fn analyze_cache_file_path(&self) -> PathBuf {
        if let Some(parent) = self.config.analysis_artifacts_root.parent() {
            return parent.join("analysis-tool-cache.json");
        }
        self.config
            .analysis_artifacts_root
            .join("analysis-tool-cache.json")
    }

    fn normalize_analyze_cache_fragment(value: &str) -> String {
        value
            .trim()
            .replace('\\', "/")
            .trim_matches('/')
            .to_ascii_lowercase()
    }

    fn active_analysis_session_fragment(&self) -> Option<String> {
        self.config
            .analysis_session_id
            .as_deref()
            .map(Self::normalize_analyze_cache_fragment)
            .filter(|value| !value.is_empty())
    }

    fn normalize_analyze_query_signature(query: &str) -> String {
        let joined = query
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.to_ascii_lowercase().starts_with("focus path hint:") {
                    return None;
                }
                Some(trimmed)
            })
            .collect::<Vec<_>>()
            .join(" ");

        let mut normalized = String::with_capacity(joined.len());
        for ch in joined.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                normalized.push(ch.to_ascii_lowercase());
            } else {
                normalized.push(' ');
            }
        }

        normalized
            .split_whitespace()
            .filter(|token| token.len() >= 2)
            .take(40)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn analyze_query_similarity(a: &str, b: &str) -> f64 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let a_set = a.split_whitespace().collect::<HashSet<_>>();
        let b_set = b.split_whitespace().collect::<HashSet<_>>();
        if a_set.is_empty() || b_set.is_empty() {
            return 0.0;
        }
        let intersection = a_set.intersection(&b_set).count() as f64;
        let union = a_set.union(&b_set).count() as f64;
        if union <= 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    fn should_bypass_analyze_cache(query: &str) -> bool {
        let lower = query.to_ascii_lowercase();
        [
            "force",
            "refresh",
            "reanalyze",
            "re-analyze",
            "\u{5f3a}\u{5236}",                 // 强制
            "\u{91cd}\u{65b0}\u{5206}\u{6790}", // 重新分析
            "\u{5237}\u{65b0}",                 // 刷新
        ]
        .iter()
        .any(|kw| lower.contains(kw))
    }

    fn analyze_cache_key(&self, mode: &str, query: &str, path_hint: Option<&str>) -> String {
        let session = self
            .active_analysis_session_fragment()
            .unwrap_or_else(|| "no-session".to_string());
        let root =
            Self::normalize_analyze_cache_fragment(&self.config.project_root.to_string_lossy());
        let mode_norm = mode.trim().to_ascii_lowercase();
        if mode_norm == "project" {
            return format!("{session}::{root}::project");
        }

        let mut local_scope = path_hint
            .map(Self::normalize_analyze_cache_fragment)
            .filter(|value| !value.is_empty());
        if local_scope.is_none() {
            local_scope = extract_path_candidates_from_text(query)
                .into_iter()
                .next()
                .map(|v| Self::normalize_analyze_cache_fragment(&v))
                .filter(|value| !value.is_empty());
        }
        let scope = local_scope.unwrap_or_else(|| "generic".to_string());
        format!("{session}::{root}::local::{scope}")
    }

    fn load_analyze_cache(path: &PathBuf) -> AnalyzeCacheFile {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str::<AnalyzeCacheFile>(&raw).unwrap_or_default(),
            Err(_) => AnalyzeCacheFile::default(),
        }
    }

    fn save_analyze_cache(path: &PathBuf, cache: &AnalyzeCacheFile) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(serialized) = serde_json::to_string_pretty(cache) {
            let _ = fs::write(path, serialized);
        }
    }

    fn get_cached_analyze_response(
        &self,
        mode: &str,
        query: &str,
        path_hint: Option<&str>,
    ) -> Option<String> {
        if Self::should_bypass_analyze_cache(query) {
            return None;
        }
        if self.active_analysis_session_fragment().is_none() {
            return None;
        }

        let now = chrono::Utc::now().timestamp();
        let cache_path = self.analyze_cache_file_path();
        let mut cache = Self::load_analyze_cache(&cache_path);
        let key = self.analyze_cache_key(mode, query, path_hint);
        let query_signature = Self::normalize_analyze_query_signature(query);
        let similarity_threshold = if mode.eq_ignore_ascii_case("project") {
            0.45
        } else {
            0.60
        };

        cache
            .entries
            .retain(|entry| now - entry.updated_at <= ANALYZE_CACHE_TTL_SECS);
        let mut best_hit: Option<(usize, f64)> = None;
        for (idx, entry) in cache.entries.iter().enumerate() {
            if entry.key != key || entry.response.trim().is_empty() {
                continue;
            }

            let score = if query_signature.is_empty() && entry.query_signature.is_empty() {
                1.0
            } else if query_signature.is_empty() || entry.query_signature.is_empty() {
                0.0
            } else {
                Self::analyze_query_similarity(&query_signature, &entry.query_signature)
            };
            if score < similarity_threshold {
                continue;
            }
            match best_hit {
                Some((_, best_score)) if score <= best_score => {}
                _ => {
                    best_hit = Some((idx, score));
                }
            }
        }

        let mut hit = None;
        if let Some((idx, _score)) = best_hit {
            if let Some(entry) = cache.entries.get_mut(idx) {
                entry.updated_at = now;
                entry.access_count = entry.access_count.saturating_add(1);
                hit = Some(entry.response.clone());
            }
        }
        if hit.is_some() {
            Self::save_analyze_cache(&cache_path, &cache);
        }
        hit
    }

    fn store_analyze_response_cache(
        &self,
        mode: &str,
        query: &str,
        path_hint: Option<&str>,
        response: &str,
    ) {
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.active_analysis_session_fragment().is_none() {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        let cache_path = self.analyze_cache_file_path();
        let mut cache = Self::load_analyze_cache(&cache_path);
        let key = self.analyze_cache_key(mode, query, path_hint);
        let project_root = self.config.project_root.to_string_lossy().to_string();
        let query_signature = Self::normalize_analyze_query_signature(query);

        cache
            .entries
            .retain(|entry| now - entry.updated_at <= ANALYZE_CACHE_TTL_SECS);
        if let Some(existing) = cache
            .entries
            .iter_mut()
            .find(|entry| entry.key == key && entry.query_signature == query_signature)
        {
            existing.mode = mode.to_string();
            existing.query_signature = query_signature.clone();
            existing.project_root = project_root;
            existing.response = trimmed.to_string();
            existing.updated_at = now;
            existing.access_count = existing.access_count.saturating_add(1);
        } else {
            cache.entries.push(AnalyzeCacheEntry {
                key,
                query_signature,
                mode: mode.to_string(),
                project_root,
                response: trimmed.to_string(),
                created_at: now,
                updated_at: now,
                access_count: 1,
            });
        }

        cache.entries.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.key.cmp(&b.key))
        });
        cache.entries.truncate(ANALYZE_CACHE_MAX_ENTRIES);
        cache.version = 1;
        Self::save_analyze_cache(&cache_path, &cache);
    }

    async fn run_project_analyze_with_cache(
        &self,
        enriched_query: &str,
        path_hint: Option<&str>,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        if let Some(cached) = self.get_cached_analyze_response("project", enriched_query, path_hint)
        {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                    phase_id: "analysis".to_string(),
                    message: "Analyze cache hit (project scope)".to_string(),
                })
                .await;
            return ExecutionResult {
                response: Some(cached),
                usage: UsageStats::default(),
                iterations: 0,
                success: true,
                error: None,
            };
        }

        let result = self
            .execute_with_analysis_pipeline(enriched_query.to_string(), tx.clone())
            .await;
        if result.success {
            if let Some(response) = result.response.as_ref() {
                self.store_analyze_response_cache("project", enriched_query, path_hint, response);
            }
        }
        result
    }

    async fn run_local_analyze_with_cache(
        &self,
        enriched_query: &str,
        path_hint: Option<&str>,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        if let Some(cached) = self.get_cached_analyze_response("local", enriched_query, path_hint) {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                    phase_id: "analysis".to_string(),
                    message: "Analyze cache hit (local scope)".to_string(),
                })
                .await;
            return ExecutionResult {
                response: Some(cached),
                usage: UsageStats::default(),
                iterations: 0,
                success: true,
                error: None,
            };
        }

        if let Some(brief) = self.build_local_preanalysis_brief(enriched_query, tx).await {
            self.store_analyze_response_cache("local", enriched_query, path_hint, &brief);
            ExecutionResult {
                response: Some(brief),
                usage: UsageStats::default(),
                iterations: 0,
                success: true,
                error: None,
            }
        } else {
            ExecutionResult {
                response: None,
                usage: UsageStats::default(),
                iterations: 0,
                success: false,
                error: Some("Analyze(local) could not build a local brief".to_string()),
            }
        }
    }

    async fn run_analyze_tool(
        &self,
        arguments: &serde_json::Value,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let mode = arguments
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "auto".to_string());

        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Analyze the relevant project scope for this task");
        let path_hint = arguments
            .get("path_hint")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let enriched_query = if let Some(hint) = path_hint.as_deref() {
            format!("{query}\n\nFocus path hint: {hint}")
        } else {
            query.to_string()
        };
        let path_hint_ref = path_hint.as_deref();

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: "analysis".to_string(),
                message: format!("Analyze tool invoked (mode={mode})"),
            })
            .await;

        match mode.as_str() {
            "deep" | "project" | "global" | "full" => {
                self.run_project_analyze_with_cache(&enriched_query, path_hint_ref, tx)
                    .await
            }
            "local" | "focused" => {
                self.run_local_analyze_with_cache(&enriched_query, path_hint_ref, tx)
                    .await
            }
            "auto" | "quick" => {
                // Quick mode (default): lightweight context brief from file inventory
                self.run_local_analyze_with_cache(&enriched_query, path_hint_ref, tx)
                    .await
            }
            _ => ExecutionResult {
                response: None,
                usage: UsageStats::default(),
                iterations: 0,
                success: false,
                error: Some(format!(
                    "Invalid Analyze mode '{}'. Use quick|deep|local.",
                    mode
                )),
            },
        }
    }

    async fn execute_analyze_tool_result(
        &self,
        arguments: &serde_json::Value,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> (
        crate::services::tools::executor::ToolResult,
        UsageStats,
        u32,
    ) {
        let result = self.run_analyze_tool(arguments, tx).await;
        if result.success {
            let text = result
                .response
                .unwrap_or_else(|| "Analyze completed with no output".to_string());
            (
                crate::services::tools::executor::ToolResult::ok(truncate_for_log(&text, 18_000)),
                result.usage,
                result.iterations,
            )
        } else {
            (
                crate::services::tools::executor::ToolResult::err(
                    result.error.unwrap_or_else(|| "Analyze failed".to_string()),
                ),
                result.usage,
                result.iterations,
            )
        }
    }

    pub(super) async fn execute_story_with_request_options(
        &self,
        prompt: &str,
        tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
        force_prompt_fallback: bool,
    ) -> ExecutionResult {
        let reliability = self.provider.tool_call_reliability();
        let use_prompt_fallback =
            force_prompt_fallback || matches!(reliability, ToolCallReliability::None);
        let mut messages = vec![Message::user(prompt.to_string())];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;
        let mut fallback_call_counter = 0u32;
        let mut repair_retry_count = 0u32;
        let mut last_assistant_text: Option<String> = None;
        let mut loop_detector = ToolCallLoopDetector::new(3, 20);
        // Track whether any tool has been successfully executed in this loop.
        // When true and this is a sub-agent, a text-only response is treated as
        // a final summary rather than a repair-worthy narration.
        let mut has_executed_tools = false;
        let is_sub_agent = self.config.task_type.is_some();
        // Track consecutive iterations where the ONLY tool calls are Task
        // delegations (lower threshold than main agent since sub-agents should
        // do direct work more often).
        let mut consecutive_task_only_iterations = 0u32;
        const SUB_AGENT_MAX_CONSECUTIVE_TASK_ONLY: u32 = 2;

        // Build TaskContext so coordinator sub-agents can spawn nested sub-agents.
        let task_ctx = self.build_task_context(&tx);
        if task_ctx.is_some() {
            eprintln!(
                "[sub-agent-loop] TaskContext created at depth={}, task_type={:?}",
                self.config.sub_agent_depth.unwrap_or(0),
                self.config.task_type,
            );
        }

        // Build a minimal system prompt for sub-agents.
        // Unlike the main agent, sub-agents do NOT get the full build_system_prompt()
        // (which includes guidelines about delegating to Task sub-agents and other
        // instructions that conflict with sub-agent behavior). Instead:
        //   1. Config system prompt (task-specific instructions)
        //   2. Tool call format instructions (for prompt-fallback providers only)
        //   3. Brief working directory info
        let system_prompt = {
            let mut parts = Vec::new();

            // Config system prompt first (the caller's task-specific instructions)
            if let Some(ref config_prompt) = self.config.system_prompt {
                parts.push(config_prompt.clone());
            }

            // Working directory context
            parts.push(format!(
                "Working directory: {}",
                self.config.project_root.display()
            ));

            // Inject project summary for sub-agents (CodebaseSearch awareness)
            if let Some(ref store) = self.index_store {
                let project_path = self.config.project_root.to_string_lossy();
                if let Ok(summary) = store.get_project_summary(&project_path) {
                    if summary.total_files > 0 {
                        parts.push(build_project_summary(&summary));
                        // Inject tool preference guidance so sub-agents prefer
                        // CodebaseSearch over Grep when the index is available.
                        let guidance = build_sub_agent_tool_guidance(
                            summary.total_symbols > 0,
                            summary.embedding_chunks > 0,
                            self.config.task_type.as_deref(),
                        );
                        if !guidance.is_empty() {
                            parts.push(guidance);
                        }
                    }
                }
            }

            // Inject skills section (framework best practices) from parent snapshot
            if let Some(ref skills_lock) = self.selected_skills {
                if let Ok(guard) = skills_lock.try_read() {
                    if !guard.is_empty() {
                        parts.push(build_skills_section(&guard));
                    }
                }
            }

            // Inject memory section (project facts from previous sessions)
            if let Some(ref memories_lock) = self.loaded_memories {
                if let Ok(guard) = memories_lock.try_read() {
                    if !guard.is_empty() {
                        let section = build_memory_section(Some(&guard));
                        if !section.is_empty() {
                            parts.push(section);
                        }
                    }
                }
            }

            // Inject cached knowledge context (RAG)
            if let Ok(cached) = self.cached_knowledge_block.lock() {
                if let Some(ref block) = *cached {
                    parts.push(block.clone());
                }
            }

            // Determine effective fallback mode for sub-agent
            let sub_effective_mode = self
                .config
                .provider
                .fallback_tool_format_mode
                .unwrap_or_else(|| {
                    if use_prompt_fallback {
                        FallbackToolFormatMode::Soft
                    } else if !matches!(
                        request_options.fallback_tool_format_mode,
                        FallbackToolFormatMode::Off
                    ) {
                        request_options.fallback_tool_format_mode
                    } else {
                        self.provider.default_fallback_mode()
                    }
                });

            // Add tool call format instructions when mode is not Off
            if !matches!(sub_effective_mode, FallbackToolFormatMode::Off) {
                parts.push(build_tool_call_instructions(tools));
                if matches!(sub_effective_mode, FallbackToolFormatMode::Strict) {
                    parts.push(
                        "Strict mode: every tool call MUST be emitted in the exact tool_call format. \
                         If your prior output was not parseable, output only valid tool_call blocks now."
                            .to_string(),
                    );
                }
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n\n"))
            }
        };

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some("Execution cancelled".to_string()),
                };
            }

            // Wait while paused (sleep-poll until unpaused or cancelled)
            while self.is_paused() {
                if self.cancellation_token.is_cancelled() {
                    emit_usage(&tx, &total_usage).await;
                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some("Execution cancelled".to_string()),
                    };
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }

            // Check iteration limit
            if iterations >= self.config.max_iterations {
                // Recover last_assistant_text if available (story-004)
                let (response, success, error_msg, stop_reason) =
                    if let Some(ref text) = last_assistant_text {
                        eprintln!(
                        "[max-iterations] execute_task: recovering {} chars of accumulated text",
                        text.len()
                    );
                        (
                            Some(text.clone()),
                            true,
                            format!(
                                "Max iterations ({}) reached but response recovered",
                                self.config.max_iterations
                            ),
                            "max_iterations_with_recovery".to_string(),
                        )
                    } else {
                        (
                            None,
                            false,
                            format!(
                                "Maximum iterations ({}) reached",
                                self.config.max_iterations
                            ),
                            "max_iterations".to_string(),
                        )
                    };

                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("max_iterations".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some(stop_reason),
                    })
                    .await;
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response,
                    usage: total_usage,
                    iterations,
                    success,
                    error: Some(error_msg),
                };
            }

            // Check token budget
            if total_usage.total_tokens() >= self.config.max_total_tokens {
                let error_msg = format!(
                    "Token budget ({}) exceeded (used {})",
                    self.config.max_total_tokens,
                    total_usage.total_tokens()
                );
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("token_budget".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("token_budget".to_string()),
                    })
                    .await;
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(error_msg),
                };
            }

            iterations += 1;

            // Determine which tools to pass to the LLM API, filtering out any
            // tools that have been stripped by Level 2 escalation.
            let stripped = loop_detector.stripped_tools();
            let filtered_tools: Vec<ToolDefinition> = if !stripped.is_empty() {
                tools
                    .iter()
                    .filter(|t| !stripped.contains(&t.name))
                    .cloned()
                    .collect()
            } else {
                tools.to_vec()
            };
            let api_tools: &[ToolDefinition] = if use_prompt_fallback {
                // Don't pass tools to the API; they're in the system prompt
                &[]
            } else {
                &filtered_tools
            };

            // Call LLM directly with the minimal system prompt (bypasses
            // build_system_prompt which has conflicting sub-agent instructions).
            // Uses retry loop with exponential backoff for transient errors
            // (rate-limits, network, server errors) — same pattern as
            // call_llm_streaming/call_llm for the main agent.
            let max_retries: u32 = 10;
            let max_delay_secs: u64 = 60;
            let response = 'retry_loop: {
                let mut last_err = None;
                for attempt in 0..=max_retries {
                    let result = if self.config.streaming {
                        self.provider
                            .stream_message(
                                messages.to_vec(),
                                system_prompt.clone(),
                                api_tools.to_vec(),
                                tx.clone(),
                                request_options.clone(),
                            )
                            .await
                    } else {
                        self.provider
                            .send_message(
                                messages.to_vec(),
                                system_prompt.clone(),
                                api_tools.to_vec(),
                                request_options.clone(),
                            )
                            .await
                    };
                    match result {
                        Ok(r) => break 'retry_loop r,
                        Err(e) if e.is_retryable() && attempt < max_retries => {
                            let delay = std::cmp::min(1u64 << attempt, max_delay_secs);
                            let wait = e.retry_after_secs().map_or(delay, |r| std::cmp::max(r, delay));
                            eprintln!(
                                "[sub-agent:retry] {} on attempt {}/{}, retrying in {}s",
                                e,
                                attempt + 1,
                                max_retries,
                                wait
                            );
                            // Notify frontend about the retry
                            let _ = tx
                                .send(UnifiedStreamEvent::Error {
                                    message: format!(
                                        "Rate limited, retrying in {}s (attempt {}/{})",
                                        wait, attempt + 1, max_retries
                                    ),
                                    code: Some("retrying".to_string()),
                                })
                                .await;
                            // Wait with cancellation support
                            tokio::select! {
                                _ = tokio::time::sleep(std::time::Duration::from_secs(wait)) => {}
                                _ = self.cancellation_token.cancelled() => {
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult {
                                        response: None,
                                        usage: total_usage,
                                        iterations,
                                        success: false,
                                        error: Some("Execution cancelled during retry wait".to_string()),
                                    };
                                }
                            }
                        }
                        Err(e) => {
                            last_err = Some(e);
                            break;
                        }
                    }
                }
                // All retries exhausted or non-retryable error
                let e = last_err.unwrap_or_else(|| {
                    crate::services::llm::LlmError::Other {
                        message: "Max retries exhausted".to_string(),
                    }
                });
                // Emit error event
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: e.to_string(),
                        code: None,
                    })
                    .await;
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(e.to_string()),
                };
            };

            // Update usage
            let last_input_tokens = response.usage.input_tokens;
            merge_usage(&mut total_usage, &response.usage);
            // Persist per-call usage to analytics database
            track_analytics(
                &self.analytics_tx,
                &self.config.provider.provider.to_string(),
                &self.config.provider.model,
                &response.usage,
                self.config.analysis_session_id.as_deref(),
                self.config.project_id.as_deref(),
                &self.analytics_cost_calculator,
                iterations,
                self.config.task_type.is_some(),
            );

            // Check for context compaction before processing tool calls.
            // In analysis mode, use cheap deterministic trimming (Codex-like)
            // instead of summary LLM calls to avoid extra token spikes.
            if self.should_compact(
                last_input_tokens,
                request_options.analysis_phase.as_ref().is_some(),
            ) {
                if request_options.analysis_phase.is_some() {
                    let removed = Self::trim_messages_for_analysis(&mut messages);
                    if removed > 0 {
                        let _ = tx
                            .send(UnifiedStreamEvent::ContextCompaction {
                                messages_compacted: removed,
                                messages_preserved: messages.len(),
                                compaction_tokens: 0,
                            })
                            .await;
                    }
                } else {
                    // ADR-F006: Delegate to pluggable compactor trait.
                    // Compactor was selected at construction time based on provider reliability.
                    match self.compactor.compact(&messages, &self.config.compaction_config).await {
                        Ok(result) if result.messages_removed > 0 => {
                            let removed_count = result.messages_removed;
                            let preserved_count = result.messages_preserved;
                            let compaction_tokens = result.compaction_tokens;
                            messages = result.messages;

                            // ADR-004: Clear dedup cache after compaction
                            self.tool_executor.clear_read_cache();
                            self.tool_executor.clear_task_cache();

                            let _ = tx
                                .send(UnifiedStreamEvent::ContextCompaction {
                                    messages_compacted: removed_count,
                                    messages_preserved: preserved_count,
                                    compaction_tokens,
                                })
                                .await;

                            eprintln!(
                                "[compaction] {} compacted {} messages, preserved {}, tokens {}",
                                self.compactor.name(), removed_count, preserved_count, compaction_tokens,
                            );
                        }
                        Err(e) => {
                            eprintln!("[compaction] {} failed: {}", self.compactor.name(), e);
                        }
                        _ => {
                            // No compaction needed (too few messages or disabled)
                        }
                    }
                }
            }

            // Track the latest assistant text for fallback if the final
            // response is empty after tool-calling iterations.
            if let Some(text) = &response.content {
                if !text.trim().is_empty() {
                    last_assistant_text = Some(text.clone());
                }
            }

            // Handle tool calls - either native or prompt-based fallback
            let has_native_tool_calls = response.has_tool_calls();
            let parsed_fallback = if !has_native_tool_calls {
                parse_fallback_tool_calls(&response, request_options.analysis_phase.as_deref())
            } else {
                ParsedFallbackCalls::default()
            };

            if has_native_tool_calls {
                repair_retry_count = 0; // Reset on successful tool calls
                                        // Native tool calling path
                let mut content = Vec::new();
                if let Some(text) = &response.content {
                    content.push(MessageContent::Text { text: text.clone() });
                }
                for tc in &response.tool_calls {
                    content.push(MessageContent::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: tc.arguments.clone(),
                    });
                }
                messages.push(Message {
                    role: crate::services::llm::MessageRole::Assistant,
                    content,
                });

                // Step 1: Validate all native tool calls
                let mut valid_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
                for tc in &response.tool_calls {
                    match prepare_tool_call_for_execution(
                        &tc.name,
                        &tc.arguments,
                        request_options.analysis_phase.as_deref(),
                    ) {
                        Ok((name, args)) => valid_calls.push((tc.id.clone(), name, args)),
                        Err(error_message) => {
                            let _ = tx
                                .send(UnifiedStreamEvent::ToolResult {
                                    tool_id: tc.id.clone(),
                                    result: None,
                                    error: Some(error_message.clone()),
                                })
                                .await;
                            messages.push(Message::tool_result(&tc.id, error_message, true));
                        }
                    }
                }
                let successful_native_calls = valid_calls.len();

                // Step 2: Check if all validated calls are parallel-safe
                let all_parallel = valid_calls.len() > 1
                    && valid_calls.iter().all(|(_, name, _)| {
                        crate::services::tools::definitions::is_tool_parallel_safe(name)
                    });

                if all_parallel {
                    // ═══════════════════════════════════════════════════
                    // Step 3a: PARALLEL native execution (sub-agent)
                    // ═══════════════════════════════════════════════════

                    // Emit all ToolStart events
                    for (tc_id, name, args) in &valid_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tc_id.clone(),
                                tool_name: name.clone(),
                                arguments: Some(args.to_string()),
                            })
                            .await;
                    }

                    // Build execution context once (shared across parallel calls)
                    let exec_ctx = Arc::new(match task_ctx.as_ref() {
                        Some(tc) => self.tool_executor.build_tool_context_with_task(tc),
                        None => self.tool_executor.build_tool_context(),
                    });

                    // Spawn all tool executions concurrently
                    let mut futures = Vec::with_capacity(valid_calls.len());
                    for (tc_id, name, args) in &valid_calls {
                        let ctx_ref = Arc::clone(&exec_ctx);
                        let name = name.clone();
                        let args = args.clone();
                        let tc_id = tc_id.clone();
                        futures.push(async move {
                            let registry = crate::services::tools::definitions::cached_registry();
                            let result = registry.execute(&name, &ctx_ref, args).await;
                            (tc_id, name, result)
                        });
                    }

                    let results = futures_util::future::join_all(futures).await;
                    let analysis_phase = request_options.analysis_phase.as_deref();

                    // Process results in original order
                    for (tc_id, effective_tool_name, result) in results {
                        let context_tool_output = tool_output_for_model_context(
                            &effective_tool_name, &result, analysis_phase,
                        );

                        // Emit tool result event
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tc_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        // Add to messages
                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            messages.push(Message::tool_result(&tc_id, dedup_msg.to_string(), false));
                        } else if let Some((mime, b64)) = &result.image_data {
                            if self.provider.supports_multimodal() {
                                use crate::services::llm::types::ContentBlock;
                                let blocks = vec![
                                    ContentBlock::Text { text: context_tool_output.clone() },
                                    ContentBlock::Image { media_type: mime.clone(), data: b64.clone() },
                                ];
                                messages.push(Message::tool_result_multimodal(&tc_id, blocks, !result.success));
                            } else {
                                messages.push(Message::tool_result(&tc_id, context_tool_output.clone(), !result.success));
                            }
                        } else {
                            messages.push(Message::tool_result(&tc_id, context_tool_output.clone(), !result.success));
                        }

                        // Loop detection (sequential after collecting results)
                        let args_str = valid_calls.iter()
                            .find(|(id, _, _)| id == &tc_id)
                            .map(|(_, _, a)| a.to_string())
                            .unwrap_or_default();
                        if let Some(detection) = loop_detector.record_call(
                            &effective_tool_name, &args_str, result.is_dedup,
                        ) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation (sub-native-parallel): {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!("[loop-detector] Level 2 escalation (sub-native-parallel): {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 (sub-native-parallel): force terminating for {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                } else {
                    // ═══════════════════════════════════════════════════
                    // Step 3b: SEQUENTIAL native execution (sub-agent)
                    // ═══════════════════════════════════════════════════
                    for (tc_id, effective_tool_name, effective_args) in &valid_calls {
                        // Emit tool start event
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tc_id.clone(),
                                tool_name: effective_tool_name.clone(),
                                arguments: Some(effective_args.to_string()),
                            })
                            .await;

                        // Execute the tool with TaskContext for sub-agent spawning support
                        let result = self
                            .tool_executor
                            .execute_with_context(effective_tool_name, effective_args, task_ctx.as_ref())
                            .await;
                        let context_tool_output = tool_output_for_model_context(
                            effective_tool_name,
                            &result,
                            request_options.analysis_phase.as_deref(),
                        );

                        // Emit tool result event
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tc_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        // Handle dedup and message construction
                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            messages.push(Message::tool_result(tc_id, dedup_msg.to_string(), false));
                        } else if let Some((mime, b64)) = &result.image_data {
                            if self.provider.supports_multimodal() {
                                use crate::services::llm::types::ContentBlock;
                                let blocks = vec![
                                    ContentBlock::Text { text: context_tool_output.clone() },
                                    ContentBlock::Image { media_type: mime.clone(), data: b64.clone() },
                                ];
                                messages.push(Message::tool_result_multimodal(tc_id, blocks, !result.success));
                            } else {
                                messages.push(Message::tool_result(tc_id, context_tool_output.clone(), !result.success));
                            }
                        } else {
                            messages.push(Message::tool_result(tc_id, context_tool_output.clone(), !result.success));
                        }

                        // Check for tool call loop
                        if let Some(detection) = loop_detector.record_call(
                            effective_tool_name,
                            &effective_args.to_string(),
                            result.is_dedup,
                        ) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation: {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!("[loop-detector] Level 2 escalation: stripping tools for {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 escalation: force terminating for {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                }

                // Story-003: When ALL native tool calls failed validation (e.g. empty
                // tool names, malformed arguments from Qwen thinking mode), inject a
                // repair hint so the model retries with correct tool call format.
                if successful_native_calls == 0 && !response.tool_calls.is_empty() {
                    eprintln!(
                        "[native-tool-repair] All {} native tool call(s) failed validation, injecting repair hint",
                        response.tool_calls.len()
                    );
                    messages.push(Message::user(
                        "All tool calls failed validation. Please retry with correct tool names and valid JSON arguments. \
                        Available tools: Read, Write, Edit, Bash, Glob, Grep, LS, Cwd, CodebaseSearch, WebFetch, WebSearch.".to_string()
                    ));
                }
                if successful_native_calls > 0 {
                    has_executed_tools = true;
                }
            } else if !parsed_fallback.calls.is_empty() {
                repair_retry_count = 0; // Reset on successful tool calls

                // Story-002: Check if the text alongside fallback tool calls is
                // already a complete answer. If so, exit the loop with that text
                // instead of executing the (unnecessary) tool calls.
                if let Some(text) = &response.content {
                    let cleaned = extract_text_without_tool_calls(text);
                    if is_complete_answer(&cleaned) {
                        eprintln!(
                            "[loop-exit] Exiting with complete text response, ignoring {} fallback tool calls",
                            parsed_fallback.calls.len()
                        );
                        emit_usage(&tx, &total_usage).await;
                        return ExecutionResult {
                            response: Some(cleaned),
                            usage: total_usage,
                            iterations,
                            success: true,
                            error: None,
                        };
                    }
                }

                // Prompt-based fallback path
                if let Some(text) = &response.content {
                    let cleaned = extract_text_without_tool_calls(text);
                    if !cleaned.is_empty() {
                        messages.push(Message::assistant(cleaned));
                    }
                }

                // Step 1: Validate all parsed fallback tool calls
                let mut valid_fallback_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
                let mut tool_results = Vec::new();
                for ptc in &parsed_fallback.calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("story_fallback_{}", fallback_call_counter);

                    match prepare_tool_call_for_execution(
                        &ptc.tool_name,
                        &ptc.arguments,
                        request_options.analysis_phase.as_deref(),
                    ) {
                        Ok((name, args)) => valid_fallback_calls.push((tool_id, name, args)),
                        Err(error_message) => {
                            let _ = tx
                                .send(UnifiedStreamEvent::ToolResult {
                                    tool_id: tool_id.clone(),
                                    result: None,
                                    error: Some(error_message.clone()),
                                })
                                .await;
                            tool_results.push(format_tool_result(
                                &ptc.tool_name,
                                &tool_id,
                                &error_message,
                                true,
                            ));
                        }
                    }
                }

                // Step 2: Check if all valid calls are parallel-safe
                let all_parallel_fb = valid_fallback_calls.len() > 1
                    && valid_fallback_calls.iter().all(|(_, name, _)| {
                        crate::services::tools::definitions::is_tool_parallel_safe(name)
                    });

                if all_parallel_fb {
                    // ═══════════════════════════════════════════════════
                    // Parallel fallback execution
                    // ═══════════════════════════════════════════════════

                    // Emit all ToolStart events
                    for (tool_id, name, args) in &valid_fallback_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tool_id.clone(),
                                tool_name: name.clone(),
                                arguments: Some(args.to_string()),
                            })
                            .await;
                    }

                    // Spawn all futures
                    let exec_ctx = Arc::new(match task_ctx.as_ref() {
                        Some(tc) => self.tool_executor.build_tool_context_with_task(tc),
                        None => self.tool_executor.build_tool_context(),
                    });
                    let mut futures = Vec::with_capacity(valid_fallback_calls.len());
                    for (tool_id, name, args) in &valid_fallback_calls {
                        let ctx_ref = Arc::clone(&exec_ctx);
                        let name = name.clone();
                        let args = args.clone();
                        let tool_id = tool_id.clone();
                        futures.push(async move {
                            let registry = crate::services::tools::definitions::cached_registry();
                            let result = registry.execute(&name, &ctx_ref, args).await;
                            (tool_id, name, result)
                        });
                    }

                    let results = futures_util::future::join_all(futures).await;
                    let analysis_phase = request_options.analysis_phase.as_deref();

                    // Process results in original order
                    for (tool_id, effective_tool_name, result) in results {
                        let context_tool_output = tool_output_for_model_context(
                            &effective_tool_name, &result, analysis_phase,
                        );
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tool_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            tool_results.push(format_tool_result(&effective_tool_name, &tool_id, dedup_msg, false));
                        } else {
                            tool_results.push(format_tool_result(&effective_tool_name, &tool_id, &context_tool_output, !result.success));
                        }

                        // Loop detection (sequential after collecting results)
                        let args_str = valid_fallback_calls.iter()
                            .find(|(id, _, _)| id == &tool_id)
                            .map(|(_, _, a)| a.to_string())
                            .unwrap_or_default();
                        if let Some(detection) = loop_detector.record_call(&effective_tool_name, &args_str, result.is_dedup) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation (fallback-parallel): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!("[loop-detector] Level 2 escalation (fallback-parallel): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 escalation (fallback-parallel): force terminating for {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                } else {
                    // ═══════════════════════════════════════════════════
                    // Sequential fallback execution (existing logic)
                    // ═══════════════════════════════════════════════════
                    for (tool_id, effective_tool_name, effective_args) in &valid_fallback_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tool_id.clone(),
                                tool_name: effective_tool_name.clone(),
                                arguments: Some(effective_args.to_string()),
                            })
                            .await;

                        let result = self
                            .tool_executor
                            .execute_with_context(effective_tool_name, effective_args, task_ctx.as_ref())
                            .await;
                        let context_tool_output = tool_output_for_model_context(
                            effective_tool_name,
                            &result,
                            request_options.analysis_phase.as_deref(),
                        );

                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tool_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            tool_results.push(format_tool_result(effective_tool_name, tool_id, dedup_msg, false));
                        } else {
                            tool_results.push(format_tool_result(effective_tool_name, tool_id, &context_tool_output, !result.success));
                        }

                        if let Some(detection) = loop_detector.record_call(effective_tool_name, &effective_args.to_string(), result.is_dedup) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation (fallback): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!("[loop-detector] Level 2 escalation (fallback): stripping tools for {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 escalation (fallback): force terminating for {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                }

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
                has_executed_tools = true;
            } else if !parsed_fallback.dropped_reasons.is_empty() {
                let repair_hint = format!(
                    "Tool call validation failed. Emit valid tool_call blocks with required arguments.\nIssues:\n- {}",
                    parsed_fallback
                        .dropped_reasons
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n- ")
                );
                if let Some(phase_id) = request_options.analysis_phase.as_ref() {
                    let _ = tx
                        .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                            phase_id: phase_id.clone(),
                            message: "Invalid fallback tool calls were dropped and a correction hint was injected.".to_string(),
                        })
                        .await;
                }
                messages.push(Message::user(repair_hint));
            } else {
                // No tool calls (native or fallback) detected.
                // Check whether the model is narrating "next steps" instead of
                // executing tools, then inject a repair hint.
                let response_text = response.content.as_deref().unwrap_or("");
                let content_is_empty = response_text.trim().is_empty();
                let thinking_text = response.thinking.as_deref().unwrap_or("");
                let thinking_has_content = !thinking_text.trim().is_empty();

                // Check content first, then thinking if content is empty.
                // We repair when the model narrates a pending next step
                // without issuing tool calls.
                let pending_action_intent = text_describes_pending_action(response_text)
                    || (content_is_empty
                        && thinking_has_content
                        && text_describes_pending_action(thinking_text));
                let tool_intent_without_call = text_describes_tool_intent(response_text)
                    || (content_is_empty
                        && thinking_has_content
                        && text_describes_tool_intent(thinking_text));
                let has_cached_assistant_text = last_assistant_text
                    .as_ref()
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false);
                let empty_response_without_signals =
                    content_is_empty && !thinking_has_content && !has_cached_assistant_text;

                // Sub-agents that have already executed tools are producing a
                // final summary — do NOT repair-hint their report text.
                let sub_agent_final_summary = is_sub_agent && has_executed_tools && !content_is_empty;

                let needs_repair = repair_retry_count < 2
                    && !sub_agent_final_summary
                    && (empty_response_without_signals
                        || pending_action_intent
                        || (!matches!(reliability, ToolCallReliability::Reliable)
                            && tool_intent_without_call));

                if needs_repair {
                    repair_retry_count += 1;
                    // Push thinking or content as assistant context
                    if let Some(text) = &response.content {
                        if !text.trim().is_empty() {
                            messages.push(Message::assistant(text.clone()));
                        }
                    }
                    if content_is_empty && thinking_has_content {
                        // For thinking-only responses, include thinking as context
                        messages.push(Message::assistant(format!(
                            "[Your reasoning contained tool usage plans but no tool was called]\n{}",
                            thinking_text
                        )));
                    }
                    let repair_msg = if empty_response_without_signals {
                        concat!(
                            "Your previous response was empty.\n",
                            "Either emit a valid tool_call block now, or provide a direct final answer.\n\n",
                            "Tool call format:\n",
                            "```tool_call\n",
                            "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                            "```"
                        )
                    } else {
                        concat!(
                            "Your response describes a pending next step, but no actual tool call was emitted.\n",
                            "Either emit the required tool call now, or provide the final answer directly.\n\n",
                            "Tool call format:\n",
                            "```tool_call\n",
                            "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                            "```\n\n",
                            "Do NOT narrate future actions without executing them."
                        )
                    };
                    messages.push(Message::user(repair_msg.to_string()));
                    continue;
                }

                if empty_response_without_signals {
                    emit_usage(&tx, &total_usage).await;
                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some(
                            "Model returned an empty response without tool calls after retries."
                                .to_string(),
                        ),
                    };
                }

                // Build final content: prefer content, fall back to thinking, then last_assistant_text
                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t))
                    .filter(|t| !t.trim().is_empty())
                    .or_else(|| {
                        // When content is empty but thinking has content (and no tool intent),
                        // use thinking as fallback response
                        if thinking_has_content {
                            Some(thinking_text.to_string())
                        } else {
                            None
                        }
                    })
                    .or(last_assistant_text);

                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: final_content,
                    usage: total_usage,
                    iterations,
                    success: true,
                    error: None,
                };
            }

            // Track consecutive Task-only iterations to prevent infinite delegation.
            if task_ctx.is_some() {
                let tool_names: Vec<&str> = if has_native_tool_calls {
                    response.tool_calls.iter().map(|tc| tc.name.as_str()).collect()
                } else if !parsed_fallback.calls.is_empty() {
                    parsed_fallback.calls.iter().map(|ptc| ptc.tool_name.as_str()).collect()
                } else {
                    vec![]
                };
                let all_task = !tool_names.is_empty()
                    && tool_names.iter().all(|n| *n == "Task");
                if all_task {
                    consecutive_task_only_iterations += 1;
                } else {
                    consecutive_task_only_iterations = 0;
                }
                if consecutive_task_only_iterations >= SUB_AGENT_MAX_CONSECUTIVE_TASK_ONLY {
                    eprintln!(
                        "[task-delegation-limit] sub-agent: {} consecutive Task-only iterations, injecting direct-work hint",
                        consecutive_task_only_iterations
                    );
                    messages.push(Message::user(
                        "[DELEGATION LIMIT] You have delegated to sub-agents multiple times \
                         without doing direct work. Use tools directly (Read, Grep, LS, etc.) \
                         before delegating again.".to_string(),
                    ));
                    consecutive_task_only_iterations = 0;
                }
            }
        }
    }

    /// Build a TaskContext for sub-agent spawning, if this agent is allowed.
    ///
    /// For the main agent (sub_agent_depth is None), depth defaults to 0.
    /// For coordinator sub-agents (sub_agent_depth is Some(n)), depth is n.
    /// When sub_agent_depth is None AND this is a sub-agent (task_type is set),
    /// TaskContext is NOT created, making the Task tool unavailable (leaf node).
    fn build_task_context(
        &self,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> Option<TaskContext> {
        let current_depth = self.config.sub_agent_depth.unwrap_or(0);
        let is_leaf_subagent = self.config.sub_agent_depth.is_none()
            && self.config.task_type.is_some();
        if is_leaf_subagent {
            return None;
        }

        // Capture read-only snapshots from the parent for sub-agent injection.
        let skills_snapshot = self.selected_skills
            .as_ref()
            .and_then(|lock| lock.try_read().ok())
            .map(|guard| guard.clone())
            .unwrap_or_default();

        let memories_snapshot = self.loaded_memories
            .as_ref()
            .and_then(|lock| lock.try_read().ok())
            .map(|guard| guard.clone())
            .unwrap_or_default();

        let knowledge_block_snapshot = self.cached_knowledge_block
            .lock().ok()
            .and_then(|guard| guard.clone());

        let task_spawner = Arc::new(OrchestratorTaskSpawner {
            provider_config: self.config.provider.clone(),
            project_root: self.config.project_root.clone(),
            context_window: self.provider.context_window(),
            shared_read_cache: self.tool_executor.shared_read_cache(),
            shared_index_store: self.tool_executor.get_index_store(),
            shared_embedding_service: self.tool_executor.get_embedding_service(),
            shared_embedding_manager: self.tool_executor.get_embedding_manager(),
            shared_hnsw_index: self.tool_executor.get_hnsw_index(),
            detected_language: self.detected_language.lock().unwrap().clone(),
            parent_supports_thinking: self.provider.supports_thinking(),
            skills_snapshot,
            memories_snapshot,
            knowledge_block_snapshot,
            shared_analytics_tx: self.analytics_tx.clone(),
            shared_analytics_cost_calculator: self.analytics_cost_calculator.clone(),
        });
        let max_concurrent = self.config.provider.effective_max_concurrent_subagents();
        Some(TaskContext {
            spawner: task_spawner,
            tx: tx.clone(),
            cancellation_token: self.cancellation_token.clone(),
            depth: current_depth,
            max_depth: MAX_SUB_AGENT_DEPTH,
            llm_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent)),
        })
    }

    /// Execute a user message through the agentic loop
    pub async fn execute(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let user_message = message;

        // Detect user language from the first message for consistent response language
        {
            let mut lang = self.detected_language.lock().unwrap();
            *lang = Some(detect_language(&user_message).to_string());
        }

        let tools = get_tool_definitions_from_registry();
        let reliability = self.provider.tool_call_reliability();
        // For None reliability (Ollama), don't pass tools to API at all
        let use_prompt_fallback = matches!(reliability, ToolCallReliability::None);
        // Clone user_message before moving it into messages, so we can use it
        // later for knowledge context population.
        let user_message_for_knowledge = user_message.clone();
        let mut messages = vec![Message::user(user_message)];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;
        let mut fallback_call_counter = 0u32;
        let mut repair_retry_count = 0u32;
        let mut last_assistant_text: Option<String> = None;
        let mut loop_detector = ToolCallLoopDetector::new(3, 20);
        // Track whether any tool has been successfully executed in this loop.
        // Used to suppress repair hints when the model produces a final summary.
        let mut has_executed_tools = false;
        // Track consecutive iterations where the ONLY tool calls are Task
        // delegations. This catches the "infinite delegation" pattern where the
        // main agent keeps spawning sub-agents with different prompts but never
        // does any direct work itself.
        let mut consecutive_task_only_iterations = 0u32;
        const MAX_CONSECUTIVE_TASK_ONLY: u32 = 3;

        let task_ctx = self.build_task_context(&tx);

        // Session memory manager for Layer 2 context (placed at index 1, after system prompt)
        let mut session_memory_manager = SessionMemoryManager::new(1);

        // EventActions state map: accumulates state deltas from tool EventActions.
        // The orchestrator is the sole action applicator - all agent-initiated
        // state changes flow through this map via the event_actions_applicator.
        let mut event_actions_state: HashMap<String, serde_json::Value> = HashMap::new();

        // Build hook context for lifecycle hooks
        let hook_ctx = crate::services::orchestrator::hooks::HookContext {
            session_id: uuid::Uuid::new_v4().to_string(),
            project_path: self.config.project_root.clone(),
            provider_name: self.provider.name().to_string(),
            model_name: self.config.provider.model.clone(),
        };

        // Hook: on_session_start
        self.hooks.fire_on_session_start(&hook_ctx).await;

        // Hook: on_user_message - allow hooks to modify the initial user message
        if let Some(first_msg) = messages.first_mut() {
            for content in first_msg.content.iter_mut() {
                if let MessageContent::Text { text } = content {
                    let modified = self.hooks.fire_on_user_message(&hook_ctx, text.clone()).await;
                    if modified != *text {
                        *text = modified;
                    }
                    break;
                }
            }
        }

        // Populate knowledge context from RAG pipeline (if configured)
        self.populate_knowledge_context(&user_message_for_knowledge).await;

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some("Execution cancelled".to_string()),
                };
            }

            // Wait while paused (sleep-poll until unpaused or cancelled)
            while self.is_paused() {
                if self.cancellation_token.is_cancelled() {
                    emit_usage(&tx, &total_usage).await;
                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some("Execution cancelled".to_string()),
                    };
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }

            // Check iteration limit
            if iterations >= self.config.max_iterations {
                // Recover last_assistant_text if available (story-004)
                let (response, success, error_msg, stop_reason) =
                    if let Some(ref text) = last_assistant_text {
                        eprintln!(
                            "[max-iterations] execute: recovering {} chars of accumulated text",
                            text.len()
                        );
                        (
                            Some(text.clone()),
                            true,
                            format!(
                                "Max iterations ({}) reached but response recovered",
                                self.config.max_iterations
                            ),
                            "max_iterations_with_recovery".to_string(),
                        )
                    } else {
                        (
                            None,
                            false,
                            format!(
                                "Maximum iterations ({}) reached",
                                self.config.max_iterations
                            ),
                            "max_iterations".to_string(),
                        )
                    };

                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("max_iterations".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some(stop_reason),
                    })
                    .await;
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response,
                    usage: total_usage,
                    iterations,
                    success,
                    error: Some(error_msg),
                };
            }

            // Check token budget
            if total_usage.total_tokens() >= self.config.max_total_tokens {
                let error_msg = format!(
                    "Token budget ({}) exceeded (used {})",
                    self.config.max_total_tokens,
                    total_usage.total_tokens()
                );
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("token_budget".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("token_budget".to_string()),
                    })
                    .await;
                emit_usage(&tx, &total_usage).await;
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(error_msg),
                };
            }

            iterations += 1;

            // Update Layer 2 session memory before each LLM call.
            // Accumulates file reads from the tool executor and key findings
            // from conversation snippets, updating the memory in-place.
            {
                let files_read = self.tool_executor.get_read_file_summary();
                if !files_read.is_empty() {
                    // Extract findings from recent assistant messages
                    let recent_snippets: Vec<String> = messages
                        .iter()
                        .rev()
                        .take(6)
                        .filter_map(|msg| {
                            msg.content.iter().find_map(|c| {
                                if let MessageContent::Text { text } = c {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                        })
                        .collect();
                    let findings = extract_key_findings(&recent_snippets);
                    session_memory_manager.update_or_insert(&mut messages, files_read, findings);
                }
            }

            // Determine which tools to pass to the LLM API, filtering out any
            // tools that have been stripped by Level 2 escalation.
            let stripped = loop_detector.stripped_tools();
            let filtered_tools: Vec<ToolDefinition> = if !stripped.is_empty() {
                tools
                    .iter()
                    .filter(|t| !stripped.contains(&t.name))
                    .cloned()
                    .collect()
            } else {
                tools.clone()
            };
            let api_tools: &[ToolDefinition] = if use_prompt_fallback {
                // Don't pass tools to the API; they're in the system prompt
                &[]
            } else {
                &filtered_tools
            };

            // Hook: on_before_llm
            self.hooks.fire_on_before_llm(&hook_ctx, iterations).await;

            // Call LLM - main agent has all tools (including Task)
            let response = if self.config.streaming {
                self.call_llm_streaming(
                    &messages,
                    api_tools,
                    &tools,
                    tx.clone(),
                    LlmRequestOptions::default(),
                )
                .await
            } else {
                self.call_llm(&messages, api_tools, &tools, LlmRequestOptions::default())
                    .await
            };

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    // Emit error event
                    let _ = tx
                        .send(UnifiedStreamEvent::Error {
                            message: e.to_string(),
                            code: None,
                        })
                        .await;

                    emit_usage(&tx, &total_usage).await;
                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some(e.to_string()),
                    };
                }
            };

            // Update usage
            let last_input_tokens = response.usage.input_tokens;
            merge_usage(&mut total_usage, &response.usage);
            // Persist per-call usage to analytics database
            track_analytics(
                &self.analytics_tx,
                &self.config.provider.provider.to_string(),
                &self.config.provider.model,
                &response.usage,
                self.config.analysis_session_id.as_deref(),
                self.config.project_id.as_deref(),
                &self.analytics_cost_calculator,
                iterations,
                self.config.task_type.is_some(),
            );

            // Hook: on_after_llm
            self.hooks.fire_on_after_llm(&hook_ctx, response.content.clone()).await;

            // Check for context compaction before processing tool calls (ADR-F006).
            // Delegates to pluggable compactor selected at construction time.
            if self.should_compact(last_input_tokens, false) {
                // Hook: on_compaction - notify hooks before compaction
                {
                    let compaction_snippets: Vec<String> = messages
                        .iter()
                        .filter_map(|msg| {
                            msg.content.iter().find_map(|c| {
                                if let MessageContent::Text { text } = c {
                                    Some(truncate_for_log(text, 200))
                                } else {
                                    None
                                }
                            })
                        })
                        .collect();
                    self.hooks.fire_on_compaction(&hook_ctx, compaction_snippets).await;
                }
                // ADR-F006: Delegate to pluggable compactor trait.
                match self.compactor.compact(&messages, &self.config.compaction_config).await {
                    Ok(result) if result.messages_removed > 0 => {
                        let removed_count = result.messages_removed;
                        let preserved_count = result.messages_preserved;
                        let compaction_tokens = result.compaction_tokens;
                        messages = result.messages;

                        // ADR-004: Clear dedup cache after compaction
                        self.tool_executor.clear_read_cache();
                        self.tool_executor.clear_task_cache();

                        let _ = tx
                            .send(UnifiedStreamEvent::ContextCompaction {
                                messages_compacted: removed_count,
                                messages_preserved: preserved_count,
                                compaction_tokens,
                            })
                            .await;

                        eprintln!(
                            "[compaction] {} compacted {} messages, preserved {}, tokens {}",
                            self.compactor.name(), removed_count, preserved_count, compaction_tokens,
                        );
                    }
                    Err(e) => {
                        eprintln!("[compaction] {} failed: {}", self.compactor.name(), e);
                    }
                    _ => {
                        // No compaction needed (too few messages or disabled)
                    }
                }
            }

            // Track the latest assistant text so we can return it if the
            // loop ends during a tool-calling turn (e.g. iteration/token limit).
            if let Some(text) = &response.content {
                if !text.trim().is_empty() {
                    last_assistant_text = Some(text.clone());
                }
            }

            // Handle tool calls - either native or prompt-based fallback
            let has_native_tool_calls = response.has_tool_calls();
            let parsed_fallback = if !has_native_tool_calls {
                // Check both assistant text and thinking content for prompt-based tool calls.
                parse_fallback_tool_calls(&response, None)
            } else {
                ParsedFallbackCalls::default()
            };

            if has_native_tool_calls {
                repair_retry_count = 0; // Reset on successful tool calls
                                        // Native tool calling path (unchanged)
                let mut content = Vec::new();
                if let Some(text) = &response.content {
                    content.push(MessageContent::Text { text: text.clone() });
                }
                for tc in &response.tool_calls {
                    content.push(MessageContent::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: tc.arguments.clone(),
                    });
                }
                messages.push(Message {
                    role: crate::services::llm::MessageRole::Assistant,
                    content,
                });

                // Step 1: Validate all tool calls
                let mut valid_calls: Vec<(String, String, serde_json::Value)> = Vec::new(); // (tc_id, name, args)
                for tc in &response.tool_calls {
                    match prepare_tool_call_for_execution(
                        &tc.name,
                        &tc.arguments,
                        None,
                    ) {
                        Ok((name, args)) => valid_calls.push((tc.id.clone(), name, args)),
                        Err(error_message) => {
                            let _ = tx
                                .send(UnifiedStreamEvent::ToolResult {
                                    tool_id: tc.id.clone(),
                                    result: None,
                                    error: Some(error_message.clone()),
                                })
                                .await;
                            messages.push(Message::tool_result(&tc.id, error_message, true));
                        }
                    }
                }
                let successful_native_calls = valid_calls.len();

                // Step 2: Check if all validated calls are parallel-safe
                let all_parallel = valid_calls.len() > 1
                    && valid_calls.iter().all(|(_, name, _)| {
                        name != "Analyze"
                            && crate::services::tools::definitions::is_tool_parallel_safe(name)
                    });

                if all_parallel {
                    // ═══════════════════════════════════════════════════════
                    // Step 3a: PARALLEL execution path
                    // ═══════════════════════════════════════════════════════

                    // Emit all ToolStart events
                    for (tc_id, name, args) in &valid_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tc_id.clone(),
                                tool_name: name.clone(),
                                arguments: Some(args.to_string()),
                            })
                            .await;
                    }

                    // Build execution context once (shared across parallel calls)
                    let exec_ctx = match task_ctx.as_ref() {
                        Some(tc) => self.tool_executor.build_tool_context_with_task(tc),
                        None => self.tool_executor.build_tool_context(),
                    };
                    let exec_ctx = Arc::new(exec_ctx);

                    // Spawn all tool executions concurrently
                    let mut futures = Vec::with_capacity(valid_calls.len());
                    for (tc_id, name, args) in &valid_calls {
                        let ctx_ref = Arc::clone(&exec_ctx);
                        let name = name.clone();
                        let args = args.clone();
                        let tc_id = tc_id.clone();
                        futures.push(async move {
                            let registry = crate::services::tools::definitions::cached_registry();
                            let result = registry.execute(&name, &ctx_ref, args).await;
                            (tc_id, name, result)
                        });
                    }

                    let results = futures_util::future::join_all(futures).await;

                    // Process results in original order (events, messages, loop detection)
                    for (tc_id, effective_tool_name, result) in results {
                        // Emit tool result event
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tc_id.clone(),
                                result: if result.success {
                                    result.output.clone()
                                } else {
                                    None
                                },
                                error: if !result.success {
                                    result.error.clone()
                                } else {
                                    None
                                },
                            })
                            .await;

                        // Hook: on_after_tool
                        self.hooks.fire_on_after_tool(
                            &hook_ctx,
                            &effective_tool_name,
                            result.success,
                            result.output.as_ref().map(|o| truncate_for_log(o, 200)),
                        ).await;

                        // Add to messages
                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            messages.push(Message::tool_result(&tc_id, dedup_msg.to_string(), false));
                        } else {
                            let context_content =
                                truncate_tool_output_for_context(&effective_tool_name, &result.to_content());
                            if let Some((mime, b64)) = &result.image_data {
                                if self.provider.supports_multimodal() {
                                    use crate::services::llm::types::ContentBlock;
                                    let blocks = vec![
                                        ContentBlock::Text {
                                            text: context_content.clone(),
                                        },
                                        ContentBlock::Image {
                                            media_type: mime.clone(),
                                            data: b64.clone(),
                                        },
                                    ];
                                    messages.push(Message::tool_result_multimodal(
                                        &tc_id,
                                        blocks,
                                        !result.success,
                                    ));
                                } else {
                                    messages.push(Message::tool_result(
                                        &tc_id,
                                        context_content,
                                        !result.success,
                                    ));
                                }
                            } else {
                                messages.push(Message::tool_result(
                                    &tc_id,
                                    context_content,
                                    !result.success,
                                ));
                            }
                        }

                        // Loop detection (sequential, after collecting results)
                        let args_str = valid_calls.iter()
                            .find(|(id, _, _)| id == &tc_id)
                            .map(|(_, _, a)| a.to_string())
                            .unwrap_or_default();
                        if let Some(detection) = loop_detector.record_call(
                            &effective_tool_name,
                            &args_str,
                            result.is_dedup,
                        ) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation: {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!("[loop-detector] Level 2 escalation: stripping tools for {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 escalation: force terminating for {}", effective_tool_name);
                                    let _ = tx
                                        .send(UnifiedStreamEvent::Error {
                                            message: msg.clone(),
                                            code: None,
                                        })
                                        .await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult {
                                        response: last_assistant_text,
                                        usage: total_usage,
                                        iterations,
                                        success: false,
                                        error: Some(msg),
                                    };
                                }
                            }
                        }
                    }
                } else {
                    // ═══════════════════════════════════════════════════════
                    // Step 3b: SEQUENTIAL execution path (existing logic)
                    // ═══════════════════════════════════════════════════════
                    for (tc_id, effective_tool_name, effective_args) in &valid_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tc_id.clone(),
                                tool_name: effective_tool_name.clone(),
                                arguments: Some(effective_args.to_string()),
                            })
                            .await;

                        // Hook: on_before_tool - can skip tool execution
                        if let Some(skip_result) = self.hooks.fire_on_before_tool(
                            &hook_ctx,
                            effective_tool_name,
                            &effective_args.to_string(),
                        ).await {
                            let skip_msg = skip_result.skip_reason.unwrap_or_else(|| "Skipped by hook".to_string());
                            let _ = tx
                                .send(UnifiedStreamEvent::ToolResult {
                                    tool_id: tc_id.clone(),
                                    result: None,
                                    error: Some(skip_msg.clone()),
                                })
                                .await;
                            messages.push(Message::tool_result(tc_id, skip_msg, true));
                            continue;
                        }

                        let (result, nested_usage, nested_iterations) = if effective_tool_name == "Analyze" {
                            self.execute_analyze_tool_result(effective_args, &tx).await
                        } else {
                            (
                                self.tool_executor
                                    .execute_with_context(effective_tool_name, effective_args, task_ctx.as_ref())
                                    .await,
                                UsageStats::default(),
                                0,
                            )
                        };
                        merge_usage(&mut total_usage, &nested_usage);
                        iterations += nested_iterations;

                        // Emit tool result event (always for frontend display)
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tc_id.clone(),
                                result: if result.success {
                                    result.output.clone()
                                } else {
                                    None
                                },
                                error: if !result.success {
                                    result.error.clone()
                                } else {
                                    None
                                },
                            })
                            .await;

                        // Hook: on_after_tool
                        self.hooks.fire_on_after_tool(
                            &hook_ctx,
                            effective_tool_name,
                            result.success,
                            result.output.as_ref().map(|o| truncate_for_log(o, 200)),
                        ).await;

                        // Apply EventActions if the tool declared any side effects.
                        if let Some(ref actions) = result.event_actions {
                            if actions.has_actions() {
                                let apply_result = crate::services::orchestrator::event_actions_applicator::apply_actions(
                                    actions,
                                    &mut event_actions_state,
                                    None,
                                    &self.config.project_root.to_string_lossy(),
                                    &hook_ctx.session_id,
                                    &[],
                                    &tx,
                                ).await;
                                match apply_result {
                                    Ok(ref action_outcome) => {
                                        if let Some(ref target_agent) = action_outcome.transfer_target {
                                            self.handle_agent_transfer(
                                                target_agent,
                                                &hook_ctx.session_id,
                                                &event_actions_state,
                                                &tx,
                                            ).await;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[event-actions] Failed to apply actions from {}: {}", effective_tool_name, e);
                                    }
                                }
                            }
                        }

                        // Handle dedup and message construction
                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            messages.push(Message::tool_result(tc_id, dedup_msg.to_string(), false));
                        } else {
                            let context_content =
                                truncate_tool_output_for_context(effective_tool_name, &result.to_content());
                            if let Some((mime, b64)) = &result.image_data {
                                if self.provider.supports_multimodal() {
                                    use crate::services::llm::types::ContentBlock;
                                    let blocks = vec![
                                        ContentBlock::Text {
                                            text: context_content.clone(),
                                        },
                                        ContentBlock::Image {
                                            media_type: mime.clone(),
                                            data: b64.clone(),
                                        },
                                    ];
                                    messages.push(Message::tool_result_multimodal(
                                        tc_id,
                                        blocks,
                                        !result.success,
                                    ));
                                } else {
                                    messages.push(Message::tool_result(
                                        tc_id,
                                        context_content,
                                        !result.success,
                                    ));
                                }
                            } else {
                                messages.push(Message::tool_result(
                                    tc_id,
                                    context_content,
                                    !result.success,
                                ));
                            }
                        }

                        // Check for tool call loop
                        if let Some(detection) = loop_detector.record_call(
                            effective_tool_name,
                            &effective_args.to_string(),
                            result.is_dedup,
                        ) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 escalation: {}", effective_tool_name);
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::StripTools(msg, _tools) => {
                                    eprintln!(
                                        "[loop-detector] Level 2 escalation: stripping tools for {}",
                                        effective_tool_name
                                    );
                                    messages.push(Message::user(msg));
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!(
                                        "[loop-detector] Level 3 escalation: force terminating for {}",
                                        effective_tool_name
                                    );
                                    let _ = tx
                                        .send(UnifiedStreamEvent::Error {
                                            message: msg.clone(),
                                            code: None,
                                        })
                                        .await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult {
                                        response: last_assistant_text,
                                        usage: total_usage,
                                        iterations,
                                        success: false,
                                        error: Some(msg),
                                    };
                                }
                            }
                        }
                    }
                }

                // Clear temp: scoped state after native tool execution round.
                // Temp state is ephemeral scratch data that should not persist
                // across tool rounds.
                session_memory_manager.clear_temp_state();
                if successful_native_calls > 0 {
                    has_executed_tools = true;
                }

                // Story-003: When ALL native tool calls failed validation (e.g. empty
                // tool names, malformed arguments from Qwen thinking mode), inject a
                // repair hint so the model retries with correct tool call format.
                if successful_native_calls == 0 && !response.tool_calls.is_empty() {
                    eprintln!(
                        "[native-tool-repair] All {} native tool call(s) failed validation, injecting repair hint",
                        response.tool_calls.len()
                    );
                    messages.push(Message::user(
                        "All tool calls failed validation. Please retry with correct tool names and valid JSON arguments. \
                        Available tools: Read, Write, Edit, Bash, Glob, Grep, LS, Cwd, CodebaseSearch, WebFetch, WebSearch.".to_string()
                    ));
                }
            } else if !parsed_fallback.calls.is_empty() {
                repair_retry_count = 0; // Reset on successful tool calls

                // Prompt-based fallback path
                // Add assistant message with tool call blocks stripped from text
                // (keeps conversation history clean for subsequent LLM calls)
                if let Some(text) = &response.content {
                    let cleaned = extract_text_without_tool_calls(text);
                    // Emit TextReplace so the frontend can remove raw tool call
                    // XML/blocks that were already streamed as text deltas
                    let _ = tx
                        .send(UnifiedStreamEvent::TextReplace {
                            content: cleaned.clone(),
                        })
                        .await;
                    if !cleaned.is_empty() {
                        messages.push(Message::assistant(cleaned));
                    }
                }

                // Step 1: Validate all parsed fallback tool calls
                let mut valid_fallback_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
                let mut tool_results = Vec::new();
                for ptc in &parsed_fallback.calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("fallback_{}", fallback_call_counter);
                    // Validate and prepare each call
                    match prepare_tool_call_for_execution(&ptc.tool_name, &ptc.arguments, None) {
                        Ok((name, args)) => valid_fallback_calls.push((tool_id, name, args)),
                        Err(error_message) => {
                            let _ = tx
                                .send(UnifiedStreamEvent::ToolResult {
                                    tool_id: tool_id.clone(),
                                    result: None,
                                    error: Some(error_message.clone()),
                                })
                                .await;
                            tool_results.push(format_tool_result(&ptc.tool_name, &tool_id, &error_message, true));
                        }
                    }
                }

                // Step 2: Check if all valid calls are parallel-safe
                // Exclude Analyze from parallel (needs special handler)
                let all_parallel_fb = valid_fallback_calls.len() > 1
                    && valid_fallback_calls.iter().all(|(_, name, _)| {
                        name != "Analyze"
                            && crate::services::tools::definitions::is_tool_parallel_safe(name)
                    });

                if all_parallel_fb {
                    // ═══════════════════════════════════════════════════
                    // Parallel fallback execution
                    // ═══════════════════════════════════════════════════
                    // Parallel-safe tools don't have hooks or event_actions,
                    // so we can safely parallelize without those features.

                    // Emit all ToolStart events
                    for (tool_id, name, args) in &valid_fallback_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tool_id.clone(),
                                tool_name: name.clone(),
                                arguments: Some(args.to_string()),
                            })
                            .await;
                    }

                    let exec_ctx = match task_ctx.as_ref() {
                        Some(tc) => self.tool_executor.build_tool_context_with_task(tc),
                        None => self.tool_executor.build_tool_context(),
                    };
                    let exec_ctx = Arc::new(exec_ctx);

                    let mut futures = Vec::with_capacity(valid_fallback_calls.len());
                    for (tool_id, name, args) in &valid_fallback_calls {
                        let ctx_ref = Arc::clone(&exec_ctx);
                        let name = name.clone();
                        let args = args.clone();
                        let tool_id = tool_id.clone();
                        futures.push(async move {
                            let registry = crate::services::tools::definitions::cached_registry();
                            let result = registry.execute(&name, &ctx_ref, args).await;
                            (tool_id, name, result)
                        });
                    }

                    let results = futures_util::future::join_all(futures).await;

                    for (tool_id, effective_tool_name, result) in results {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tool_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        // Hook: on_after_tool (parallel-safe tools have minimal hooks)
                        self.hooks.fire_on_after_tool(
                            &hook_ctx, &effective_tool_name, result.success,
                            result.output.as_ref().map(|o| truncate_for_log(o, 200)),
                        ).await;

                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            tool_results.push(format_tool_result(&effective_tool_name, &tool_id, dedup_msg, false));
                        } else {
                            let context_content = truncate_tool_output_for_context(&effective_tool_name, &result.to_content());
                            tool_results.push(format_tool_result(&effective_tool_name, &tool_id, &context_content, !result.success));
                        }

                        let args_str = valid_fallback_calls.iter()
                            .find(|(id, _, _)| id == &tool_id)
                            .map(|(_, _, a)| a.to_string())
                            .unwrap_or_default();
                        if let Some(detection) = loop_detector.record_call(&effective_tool_name, &args_str, result.is_dedup) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 (fallback-parallel): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::StripTools(msg, _) => {
                                    eprintln!("[loop-detector] Level 2 (fallback-parallel): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 (fallback-parallel): force terminating {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                } else {
                    // ═══════════════════════════════════════════════════
                    // Sequential fallback execution (existing logic with hooks/event_actions)
                    // ═══════════════════════════════════════════════════
                    for (tool_id, effective_tool_name, effective_args) in &valid_fallback_calls {
                        let _ = tx
                            .send(UnifiedStreamEvent::ToolStart {
                                tool_id: tool_id.clone(),
                                tool_name: effective_tool_name.clone(),
                                arguments: Some(effective_args.to_string()),
                            })
                            .await;

                        // Hook: on_before_tool (fallback path)
                        if let Some(skip_result) = self.hooks.fire_on_before_tool(
                            &hook_ctx, effective_tool_name, &effective_args.to_string(),
                        ).await {
                            let skip_msg = skip_result.skip_reason.unwrap_or_else(|| "Skipped by hook".to_string());
                            tool_results.push(format_tool_result(effective_tool_name, tool_id, &skip_msg, true));
                            continue;
                        }

                        let (result, nested_usage, nested_iterations) = if effective_tool_name == "Analyze" {
                            self.execute_analyze_tool_result(effective_args, &tx).await
                        } else {
                            (
                                self.tool_executor.execute_with_context(effective_tool_name, effective_args, task_ctx.as_ref()).await,
                                UsageStats::default(),
                                0,
                            )
                        };
                        merge_usage(&mut total_usage, &nested_usage);
                        iterations += nested_iterations;

                        let _ = tx
                            .send(UnifiedStreamEvent::ToolResult {
                                tool_id: tool_id.clone(),
                                result: if result.success { result.output.clone() } else { None },
                                error: if !result.success { result.error.clone() } else { None },
                            })
                            .await;

                        // Hook: on_after_tool (fallback path)
                        self.hooks.fire_on_after_tool(
                            &hook_ctx, effective_tool_name, result.success,
                            result.output.as_ref().map(|o| truncate_for_log(o, 200)),
                        ).await;

                        // Apply EventActions if the tool declared any side effects
                        if let Some(ref actions) = result.event_actions {
                            if actions.has_actions() {
                                let apply_result = crate::services::orchestrator::event_actions_applicator::apply_actions(
                                    actions, &mut event_actions_state, None,
                                    &self.config.project_root.to_string_lossy(),
                                    &hook_ctx.session_id, &[], &tx,
                                ).await;
                                match apply_result {
                                    Ok(ref action_outcome) => {
                                        if let Some(ref target_agent) = action_outcome.transfer_target {
                                            self.handle_agent_transfer(target_agent, &hook_ctx.session_id, &event_actions_state, &tx).await;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[event-actions] Failed to apply actions from {} (fallback): {}", effective_tool_name, e);
                                    }
                                }
                            }
                        }

                        if result.is_dedup {
                            let dedup_msg = result.output.as_deref().unwrap_or(
                                "[File already read] Content is in session memory above. Do NOT re-read."
                            );
                            tool_results.push(format_tool_result(effective_tool_name, tool_id, dedup_msg, false));
                        } else {
                            let context_content = truncate_tool_output_for_context(effective_tool_name, &result.to_content());
                            tool_results.push(format_tool_result(effective_tool_name, tool_id, &context_content, !result.success));
                        }

                        if let Some(detection) = loop_detector.record_call(effective_tool_name, &effective_args.to_string(), result.is_dedup) {
                            match detection {
                                LoopDetection::Warning(msg) => {
                                    eprintln!("[loop-detector] Level 1 (fallback): {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::StripTools(msg, _) => {
                                    eprintln!("[loop-detector] Level 2 (fallback): stripping tools for {}", effective_tool_name);
                                    tool_results.push(msg);
                                }
                                LoopDetection::ForceTerminate(msg) => {
                                    eprintln!("[loop-detector] Level 3 (fallback): force terminating for {}", effective_tool_name);
                                    let _ = tx.send(UnifiedStreamEvent::Error { message: msg.clone(), code: None }).await;
                                    emit_usage(&tx, &total_usage).await;
                                    return ExecutionResult { response: last_assistant_text, usage: total_usage, iterations, success: false, error: Some(msg) };
                                }
                            }
                        }
                    }
                }

                // Clear temp: scoped state after fallback tool execution round.
                session_memory_manager.clear_temp_state();

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
                has_executed_tools = true;
            } else if !parsed_fallback.dropped_reasons.is_empty() {
                let repair_hint = format!(
                    "Tool call parsing detected invalid calls. Please emit valid tool_call JSON blocks.\nIssues:\n- {}",
                    parsed_fallback
                        .dropped_reasons
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n- ")
                );
                messages.push(Message::user(repair_hint));
            } else {
                // No tool calls (native or fallback) detected.
                // Check whether the model is narrating "next steps" instead of
                // executing tools, then inject a repair hint.
                let response_text = response.content.as_deref().unwrap_or("");
                let content_is_empty = response_text.trim().is_empty();
                let thinking_text = response.thinking.as_deref().unwrap_or("");
                let thinking_has_content = !thinking_text.trim().is_empty();

                // Check content first, then thinking if content is empty.
                // We repair when the model narrates a pending next step
                // without issuing tool calls.
                let pending_action_intent = text_describes_pending_action(response_text)
                    || (content_is_empty
                        && thinking_has_content
                        && text_describes_pending_action(thinking_text));
                let tool_intent_without_call = text_describes_tool_intent(response_text)
                    || (content_is_empty
                        && thinking_has_content
                        && text_describes_tool_intent(thinking_text));
                let has_cached_assistant_text = last_assistant_text
                    .as_ref()
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false);
                let empty_response_without_signals =
                    content_is_empty && !thinking_has_content && !has_cached_assistant_text;

                // When the model has already executed tools and now provides a
                // substantial text response (>80 chars), treat it as a final
                // answer rather than a repair-worthy narration.
                let likely_final_answer = has_executed_tools
                    && !content_is_empty
                    && response_text.len() > 80;

                let needs_repair = repair_retry_count < 2
                    && !likely_final_answer
                    && (empty_response_without_signals
                        || pending_action_intent
                        || (!matches!(reliability, ToolCallReliability::Reliable)
                            && tool_intent_without_call));

                if needs_repair {
                    // Send a repair hint to nudge the model into actually calling tools
                    repair_retry_count += 1;
                    if let Some(text) = &response.content {
                        if !text.trim().is_empty() {
                            messages.push(Message::assistant(text.clone()));
                        }
                    }
                    if content_is_empty && thinking_has_content {
                        // For thinking-only responses, include thinking as context
                        messages.push(Message::assistant(format!(
                            "[Your reasoning contained tool usage plans but no tool was called]\n{}",
                            thinking_text
                        )));
                    }
                    let repair_hint = if empty_response_without_signals {
                        concat!(
                            "Your previous response was empty.\n",
                            "Either emit a valid tool_call block now, or provide a direct final answer.\n\n",
                            "Tool call format:\n",
                            "```tool_call\n",
                            "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                            "```"
                        )
                    } else {
                        concat!(
                            "Your response describes a pending next step, but no actual tool call was emitted.\n",
                            "Either emit the required tool call now, or provide the final answer directly.\n\n",
                            "Tool call format:\n",
                            "```tool_call\n",
                            "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                            "```\n\n",
                            "Do NOT narrate future actions without executing them."
                        )
                    };
                    messages.push(Message::user(repair_hint.to_string()));
                    // Continue the loop; do not return as final response.
                    continue;
                }

                if empty_response_without_signals {
                    let error_msg =
                        "Model returned an empty response without tool calls after retries."
                            .to_string();
                    let _ = tx
                        .send(UnifiedStreamEvent::Error {
                            message: error_msg.clone(),
                            code: Some("empty_response".to_string()),
                        })
                        .await;
                    emit_usage(&tx, &total_usage).await;
                    return ExecutionResult {
                        response: None,
                        usage: total_usage,
                        iterations,
                        success: false,
                        error: Some(error_msg),
                    };
                }

                // Build final content: prefer content, fall back to thinking, then last_assistant_text
                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t))
                    .filter(|t| !t.trim().is_empty())
                    .or_else(|| {
                        // When content is empty but thinking has content (and no tool intent),
                        // use thinking as fallback response
                        if thinking_has_content {
                            Some(thinking_text.to_string())
                        } else {
                            None
                        }
                    })
                    .or(last_assistant_text);

                // Hook: on_session_end
                {
                    let files_read = self.tool_executor.get_read_file_summary()
                        .iter()
                        .map(|(path, _, _)| path.clone())
                        .collect::<Vec<_>>();
                    let recent_snippets: Vec<String> = messages
                        .iter()
                        .rev()
                        .take(6)
                        .filter_map(|msg| {
                            msg.content.iter().find_map(|c| {
                                if let MessageContent::Text { text } = c {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                        })
                        .collect();
                    let key_findings = extract_key_findings(&recent_snippets);
                    let task_desc = messages.first()
                        .and_then(|m| m.content.first())
                        .and_then(|c| if let MessageContent::Text { text } = c { Some(truncate_for_log(text, 200)) } else { None })
                        .unwrap_or_default();
                    let summary = crate::services::orchestrator::hooks::SessionSummary {
                        task_description: task_desc,
                        files_read,
                        key_findings,
                        tool_usage: std::collections::HashMap::new(),
                        total_turns: iterations,
                        success: true,
                    };
                    self.hooks.fire_on_session_end(&hook_ctx, summary).await;
                }

                // Emit completion event
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("end_turn".to_string()),
                    })
                    .await;

                // Emit usage event
                let _ = tx
                    .send(UnifiedStreamEvent::Usage {
                        input_tokens: total_usage.input_tokens,
                        output_tokens: total_usage.output_tokens,
                        thinking_tokens: total_usage.thinking_tokens,
                        cache_read_tokens: total_usage.cache_read_tokens,
                        cache_creation_tokens: total_usage.cache_creation_tokens,
                    })
                    .await;

                return ExecutionResult {
                    response: final_content,
                    usage: total_usage,
                    iterations,
                    success: true,
                    error: None,
                };
            }

            // Track consecutive iterations where ALL tool calls are Task delegations.
            // This catches the "infinite delegation" anti-pattern where the main agent
            // keeps spawning sub-agents with different prompts but never does any
            // direct work itself. The existing loop detector cannot catch this because
            // each Task call has different arguments (different prompts).
            {
                let tool_names: Vec<&str> = if has_native_tool_calls {
                    response.tool_calls.iter().map(|tc| tc.name.as_str()).collect()
                } else if !parsed_fallback.calls.is_empty() {
                    parsed_fallback.calls.iter().map(|ptc| ptc.tool_name.as_str()).collect()
                } else {
                    vec![]
                };
                let all_task = !tool_names.is_empty()
                    && tool_names.iter().all(|n| *n == "Task");
                if all_task {
                    consecutive_task_only_iterations += 1;
                } else {
                    consecutive_task_only_iterations = 0;
                }
                if consecutive_task_only_iterations >= MAX_CONSECUTIVE_TASK_ONLY {
                    eprintln!(
                        "[task-delegation-limit] {} consecutive Task-only iterations, injecting direct-work hint",
                        consecutive_task_only_iterations
                    );
                    messages.push(Message::user(
                        "[DELEGATION LIMIT] You have delegated to sub-agents multiple times in a row \
                         without doing any direct work yourself. Sub-agents have limited capabilities \
                         and cannot fully replace your own tool use. You MUST now use tools directly \
                         (Read, Grep, LS, Edit, Write, Bash, etc.) to make progress on the task. \
                         Do NOT call the Task tool again until you have done meaningful direct work."
                            .to_string(),
                    ));
                    // Reset counter so the model gets another chance
                    consecutive_task_only_iterations = 0;
                }
            }
        }
    }

    async fn build_local_preanalysis_brief(
        &self,
        message: &str,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> Option<String> {
        let excluded_roots = analysis_excluded_roots_for_message(message);
        let inventory = build_file_inventory(&self.config.project_root, &excluded_roots).ok()?;
        if inventory.total_files == 0 {
            return None;
        }

        let mut selected = Vec::<String>::new();
        let candidates = extract_path_candidates_from_text(message);
        for candidate in candidates {
            for item in &inventory.items {
                if item.path == candidate
                    || item.path.starts_with(&format!("{}/", candidate))
                    || candidate.starts_with(&format!("{}/", item.path))
                {
                    selected.push(item.path.clone());
                }
            }
        }

        if selected.is_empty() {
            selected.extend(select_local_seed_files(&inventory));
        }

        selected.sort();
        selected.dedup();
        selected.truncate(20);
        if selected.is_empty() {
            return None;
        }

        let related_tests = related_test_candidates(&selected, &inventory.items);
        let test_count = related_tests.len();

        let mut component_counts = HashMap::<String, usize>::new();
        for item in &inventory.items {
            *component_counts.entry(item.component.clone()).or_insert(0) += 1;
        }
        let mut component_pairs = component_counts.into_iter().collect::<Vec<_>>();
        component_pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut lines = Vec::new();
        lines.push(format!(
            "Auto local analysis indexed {} files and selected {} likely-relevant files:",
            inventory.total_files,
            selected.len()
        ));
        for path in &selected {
            let digest = summarize_file_head(&self.config.project_root.join(path), 4)
                .unwrap_or_else(|| "head unreadable".to_string());
            lines.push(format!("- {} :: {}", path, truncate_for_log(&digest, 140)));
        }
        if !related_tests.is_empty() {
            lines.push("Related test files:".to_string());
            for test_path in related_tests.iter().take(10) {
                lines.push(format!("- {}", test_path));
            }
        } else {
            lines.push("Related test files: (none detected in quick local pass)".to_string());
        }
        lines.push(format!(
            "Top components by file count: {}",
            component_pairs
                .iter()
                .take(5)
                .map(|(component, count)| format!("{}={}", component, count))
                .collect::<Vec<_>>()
                .join(", ")
        ));

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseProgress {
                phase_id: "analysis".to_string(),
                message: format!(
                    "Auto local pre-analysis covered {} candidate files and {} related tests",
                    selected.len(),
                    test_count
                ),
            })
            .await;

        Some(lines.join("\n"))
    }

    /// Check if context compaction should be triggered based on input token usage.
    ///
    /// Compaction triggers when the last LLM response's input_tokens exceeds 60% of max_total_tokens.
    /// This uses per-call input_tokens (not cumulative) since it reflects the actual current context size.
    fn should_compact(&self, last_input_tokens: u32, aggressive: bool) -> bool {
        if !self.config.enable_compaction {
            return false;
        }
        let ratio = if aggressive { 0.35 } else { 0.6 };
        let threshold = (self.config.max_total_tokens as f64 * ratio) as u32;
        last_input_tokens > threshold
    }

    /// Deterministically trim analysis conversation history without making an extra LLM call.
    /// Returns the number of removed messages.
    fn trim_messages_for_analysis(messages: &mut Vec<Message>) -> usize {
        let keep_head = 1usize;
        let keep_tail = 8usize;
        if messages.len() <= keep_head + keep_tail {
            return 0;
        }
        let removable = messages.len().saturating_sub(keep_head + keep_tail);
        let to_remove = removable.min(4).max(1);
        let start = keep_head;
        let end = keep_head + to_remove;
        messages.drain(start..end);
        to_remove
    }

    /// Compact conversation messages by summarizing older messages while preserving recent ones.
    ///
    /// Builds a `SessionMemory` from the tool executor's read cache and conversation snippets,
    /// then calls the LLM to summarize the compacted portion. The final message structure is:
    ///
    /// ```text
    /// [original_prompt, session_memory_msg, llm_summary, ...preserved_tail]
    /// ```
    ///
    /// The session memory explicitly lists all previously-read files with sizes and an
    /// instruction to avoid re-reading them, preventing wasteful duplicate reads after compaction.
    ///
    /// Returns `true` if compaction was successful, `false` if it failed or was skipped.
    /// On failure, messages are left untouched and execution continues normally.
    async fn compact_messages(
        &self,
        messages: &mut Vec<Message>,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> bool {
        // Guard: need at least 9 messages (first prompt + 2 to compact + 6 preserved tail)
        if messages.len() < 9 {
            return false;
        }

        // Preserve the first message (original prompt / Layer 1) and last 6 messages (recent context)
        let preserved_tail_count = 6;
        let first_msg = messages[0].clone();
        let compact_range_end = messages.len() - preserved_tail_count;

        // Determine the start of the compaction range.
        // If a Layer 2 session memory (identified by SESSION_MEMORY_V1 marker) exists
        // at index 1, skip it 閳?it will be rebuilt after compaction.
        let compact_range_start =
            if messages.len() > 1 && SessionMemoryManager::message_has_marker(&messages[1]) {
                2 // Skip both Layer 1 (index 0) and existing Layer 2 (index 1)
            } else {
                1 // Skip only Layer 1 (index 0)
            };

        // Nothing to compact if range is too small
        if compact_range_end <= compact_range_start {
            return false;
        }

        let messages_to_compact = &messages[compact_range_start..compact_range_end];
        let messages_compacted_count = messages_to_compact.len();

        // Extract summary information from messages being compacted
        let mut tool_usage_counts: HashMap<String, usize> = HashMap::new();
        let mut file_paths: Vec<String> = Vec::new();
        let mut conversation_snippets: Vec<String> = Vec::new();

        for msg in messages_to_compact {
            for content in &msg.content {
                match content {
                    MessageContent::Text { text } => {
                        let snippet = truncate_for_log(text, 500);
                        conversation_snippets.push(snippet);
                    }
                    MessageContent::ToolUse { name, .. } => {
                        *tool_usage_counts.entry(name.clone()).or_insert(0) += 1;
                    }
                    MessageContent::ToolResult { content, .. } => {
                        // Extract file paths from tool results
                        for line in content.lines().take(5) {
                            let trimmed = line.trim();
                            if trimmed.contains('/') || trimmed.contains('\\') {
                                if trimmed.len() < 200 {
                                    let path = trimmed.split_whitespace().next().unwrap_or(trimmed);
                                    if !file_paths.contains(&path.to_string()) {
                                        file_paths.push(path.to_string());
                                    }
                                }
                            }
                        }
                        let snippet = truncate_for_log(content, 500);
                        conversation_snippets.push(snippet);
                    }
                    MessageContent::ToolResultMultimodal {
                        content: blocks, ..
                    } => {
                        for block in blocks {
                            if let crate::services::llm::types::ContentBlock::Text { text } = block
                            {
                                let snippet = truncate_for_log(text, 500);
                                conversation_snippets.push(snippet);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Build SessionMemory from tool executor cache + extracted findings
        let files_read = self.tool_executor.get_read_file_summary();
        let key_findings = extract_key_findings(&conversation_snippets);

        // Extract task description from the first user message
        let task_description = first_msg
            .content
            .iter()
            .find_map(|c| {
                if let MessageContent::Text { text } = c {
                    Some(truncate_for_log(text, 500))
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let session_memory = SessionMemory {
            files_read,
            key_findings,
            task_description,
            tool_usage_counts: tool_usage_counts.clone(),
        };

        // Collect unique tool names for the compaction prompt
        let tool_names: Vec<String> = {
            let mut names: Vec<String> = tool_usage_counts.keys().cloned().collect();
            names.sort();
            names
        };

        // Truncate collected data to keep the compaction prompt reasonable
        let snippets_summary = conversation_snippets
            .iter()
            .take(20)
            .map(|s| format!("- {}", s))
            .collect::<Vec<_>>()
            .join("\n");

        let compaction_prompt = format!(
            "Summarize the following conversation history concisely in under 800 words. \
             Focus on: what was asked, what tools were used, what was discovered, and what decisions were made.\n\n\
             Tools used: {}\n\
             Files touched: {}\n\n\
             Conversation excerpts:\n{}\n\n\
             Provide a clear, structured summary that preserves the key context needed to continue the task.",
            if tool_names.is_empty() { "none".to_string() } else { tool_names.join(", ") },
            if file_paths.is_empty() { "none".to_string() } else { file_paths.iter().take(20).cloned().collect::<Vec<_>>().join(", ") },
            snippets_summary,
        );

        // Call LLM to generate summary (non-streaming, no tools)
        let summary_messages = vec![Message::user(compaction_prompt)];
        let result = self
            .provider
            .send_message(
                summary_messages,
                None,
                Vec::new(),
                LlmRequestOptions::default(),
            )
            .await;

        match result {
            Ok(response) => {
                let summary_text = response
                    .content
                    .unwrap_or_else(|| "Previous conversation context was compacted.".to_string());
                let compaction_tokens = response.usage.output_tokens;

                // Build session memory message with V1 marker for compaction identification.
                // The marker allows both LLM-summary and prefix-stable compaction to
                // locate and preserve this Layer 2 message in subsequent compaction rounds.
                let session_memory_msg = Message::assistant(format!(
                    "{}\n{}",
                    SESSION_MEMORY_V1_MARKER,
                    session_memory.to_context_string()
                ));

                // Build new message list: original prompt + session memory + summary + preserved tail
                let preserved_tail: Vec<Message> = messages[compact_range_end..].to_vec();
                let summary_msg = Message::user(format!(
                    "[Context Summary - {} earlier messages compacted]\n\n{}",
                    messages_compacted_count, summary_text
                ));

                messages.clear();
                messages.push(first_msg);
                messages.push(session_memory_msg);
                messages.push(summary_msg);
                messages.extend(preserved_tail);

                // Emit compaction event
                let _ = tx
                    .send(UnifiedStreamEvent::ContextCompaction {
                        messages_compacted: messages_compacted_count,
                        messages_preserved: preserved_tail_count,
                        compaction_tokens,
                    })
                    .await;

                // ADR-004: Clear the dedup cache after compaction so files can be
                // re-read fresh. Without this, the cache retains stale entries for
                // file reads that were just compacted away, causing LLMs to get
                // only the short dedup message instead of actual content.
                self.tool_executor.clear_read_cache();
                self.tool_executor.clear_task_cache();

                eprintln!(
                    "[compaction] Compacted {} messages, preserved {}, summary {} tokens, session memory with {} files (dedup cache cleared)",
                    messages_compacted_count, preserved_tail_count, compaction_tokens,
                    session_memory.files_read.len(),
                );

                true
            }
            Err(e) => {
                eprintln!("[compaction] Failed to compact messages: {}", e);
                false
            }
        }
    }

    /// Prefix-stable compaction: remove middle messages without inserting new content.
    ///
    /// Preserves the head (first 2 messages: original prompt + session memory) and
    /// the tail (last 6 messages: recent context). All middle messages are deleted.
    /// This is a synchronous, deterministic operation that does NOT call the LLM,
    /// making it suitable for providers with unreliable or no tool calling support
    /// (Ollama, Qwen, DeepSeek, GLM) where an LLM-summary compaction call may fail
    /// or produce poor results.
    ///
    /// Returns `true` if messages were removed, `false` if skipped (too few messages).
    pub(super) fn compact_messages_prefix_stable(messages: &mut Vec<Message>) -> bool {
        let keep_head = 2usize;
        let keep_tail = 6usize;
        let min_required = keep_head + keep_tail + 1; // need at least 1 middle message

        if messages.len() < min_required {
            return false;
        }

        let middle_end = messages.len() - keep_tail;
        let removed = middle_end - keep_head;

        messages.drain(keep_head..middle_end);

        eprintln!(
            "[compaction] Prefix-stable: removed {} middle messages, kept {} head + {} tail = {} total",
            removed,
            keep_head,
            keep_tail,
            messages.len(),
        );

        true
    }

    /// Build the effective system prompt, merging tool context with user prompt.
    ///
    /// When `tools` is non-empty, the tool usage system prompt is always included.
    /// When the provider doesn't support native tool calling (prompt fallback mode),
    /// additional tool call format instructions are injected.
    /// Build the effective system prompt from the given tool set.
    ///
    /// `prompt_tools` are the tools listed in the system prompt (for guidance).
    /// If empty, only the config system prompt is returned.
    pub(super) fn effective_system_prompt(
        &self,
        prompt_tools: &[ToolDefinition],
        request_options: &LlmRequestOptions,
    ) -> Option<String> {
        if prompt_tools.is_empty() {
            return self.config.system_prompt.clone();
        }

        // Fetch project summary from index store if available
        let project_summary = self.index_store.as_ref().and_then(|store| {
            let project_path = self.config.project_root.to_string_lossy();
            store.get_project_summary(&project_path).ok()
        });

        let provider_name = self.provider.name();
        let model_name = self.provider.model();
        let detected_lang = self.detected_language.lock().unwrap();
        let language = detected_lang.as_deref().unwrap_or("en");

        // Read loaded memories from shared state (populated by memory hooks)
        let memories_snapshot: Option<Vec<crate::services::memory::store::MemoryEntry>> =
            if let Some(ref mem_lock) = self.loaded_memories {
                // Use try_read to avoid blocking; fall back to None if lock is held
                mem_lock.try_read().ok().map(|guard| guard.clone())
            } else {
                None
            };

        let mut prompt = build_system_prompt_with_memories(
            &self.config.project_root,
            prompt_tools,
            project_summary.as_ref(),
            memories_snapshot.as_deref(),
            provider_name,
            model_name,
            language,
        );

        // Inject selected skills section (populated by skill hooks)
        if let Some(ref skills_lock) = self.selected_skills {
            if let Ok(guard) = skills_lock.try_read() {
                if !guard.is_empty() {
                    let skills_section = build_skills_section(&guard);
                    prompt.push_str(&skills_section);
                }
            }
        }
        drop(detected_lang);

        // Inject cached knowledge context (populated by populate_knowledge_context)
        if let Ok(cached) = self.cached_knowledge_block.lock() {
            if let Some(ref block) = *cached {
                prompt.push_str("\n\n");
                prompt.push_str(block);
            }
        }

        // Determine effective fallback mode:
        // 1. User override from ProviderConfig.fallback_tool_format_mode (highest priority)
        // 2. Explicit request_options.fallback_tool_format_mode (if not Off)
        // 3. Auto-determine from provider reliability
        let effective_mode = self
            .config
            .provider
            .fallback_tool_format_mode
            .unwrap_or_else(|| {
                if !matches!(
                    request_options.fallback_tool_format_mode,
                    FallbackToolFormatMode::Off
                ) {
                    request_options.fallback_tool_format_mode
                } else {
                    // Auto-determine based on provider reliability
                    self.provider.default_fallback_mode()
                }
            });

        // Inject fallback instructions when mode is not Off
        if !matches!(effective_mode, FallbackToolFormatMode::Off) {
            let fallback_instructions = build_tool_call_instructions(prompt_tools);
            prompt = if matches!(effective_mode, FallbackToolFormatMode::Strict) {
                format!(
                    "{}\n\n{}\n\n{}",
                    prompt,
                    fallback_instructions,
                    "STRICT TOOL FORMAT MODE: emit only parseable tool_call blocks when using tools. \
                     If your previous output used prose or malformed tags for tools, fix it and output \
                     valid tool_call blocks only before any explanation."
                )
            } else {
                format!("{}\n\n{}", prompt, fallback_instructions)
            };
        }

        Some(merge_system_prompts(
            &prompt,
            self.config.system_prompt.as_deref(),
        ))
    }

    /// Call the LLM with non-streaming mode.
    ///
    /// `api_tools` are sent to the provider API (empty for prompt-fallback providers).
    /// `prompt_tools` are listed in the system prompt for guidance.
    ///
    /// Retries transient errors (network, rate-limit, server, provider-unavailable)
    /// with exponential backoff (1s → 60s cap, max 10 retries).
    pub(super) async fn call_llm(
        &self,
        messages: &[Message],
        api_tools: &[ToolDefinition],
        prompt_tools: &[ToolDefinition],
        request_options: LlmRequestOptions,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools, &request_options);
        let max_retries: u32 = 10;
        let max_delay_secs: u64 = 60;

        for attempt in 0..=max_retries {
            let result = self
                .provider
                .send_message(
                    messages.to_vec(),
                    system.clone(),
                    api_tools.to_vec(),
                    request_options.clone(),
                )
                .await;
            match result {
                Ok(r) => return Ok(r),
                Err(e) if e.is_retryable() && attempt < max_retries => {
                    let delay = std::cmp::min(1u64 << attempt, max_delay_secs);
                    let wait = e.retry_after_secs().map_or(delay, |r| std::cmp::max(r, delay));
                    eprintln!(
                        "[llm:retry] {} on attempt {}/{}, retrying in {}s",
                        e,
                        attempt + 1,
                        max_retries,
                        wait
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Call the LLM with streaming mode.
    ///
    /// `api_tools` are sent to the provider API (empty for prompt-fallback providers).
    /// `prompt_tools` are listed in the system prompt for guidance.
    ///
    /// Retries transient errors (network, rate-limit, server, provider-unavailable)
    /// with exponential backoff (1s → 60s cap, max 10 retries).
    /// Network errors from `.send().await` mean the connection never succeeded,
    /// so no streaming events were emitted and retry is safe.
    async fn call_llm_streaming(
        &self,
        messages: &[Message],
        api_tools: &[ToolDefinition],
        prompt_tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools, &request_options);
        let max_retries: u32 = 10;
        let max_delay_secs: u64 = 60;

        for attempt in 0..=max_retries {
            let result = self
                .provider
                .stream_message(
                    messages.to_vec(),
                    system.clone(),
                    api_tools.to_vec(),
                    tx.clone(),
                    request_options.clone(),
                )
                .await;
            match result {
                Ok(r) => return Ok(r),
                Err(e) if e.is_retryable() && attempt < max_retries => {
                    let delay = std::cmp::min(1u64 << attempt, max_delay_secs);
                    let wait = e.retry_after_secs().map_or(delay, |r| std::cmp::max(r, delay));
                    eprintln!(
                        "[llm:retry] {} on attempt {}/{}, retrying in {}s",
                        e,
                        attempt + 1,
                        max_retries,
                        wait
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Execute a simple message without the agentic loop (single turn)
    pub async fn execute_single(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let messages = vec![Message::user(message)];

        let response = if self.config.streaming {
            self.call_llm_streaming(
                &messages,
                &[],
                &[],
                tx.clone(),
                LlmRequestOptions::default(),
            )
            .await
        } else {
            self.call_llm(&messages, &[], &[], LlmRequestOptions::default())
                .await
        };

        match response {
            Ok(r) => {
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("end_turn".to_string()),
                    })
                    .await;

                ExecutionResult {
                    response: r.content,
                    usage: r.usage,
                    iterations: 1,
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: e.to_string(),
                        code: None,
                    })
                    .await;

                ExecutionResult {
                    response: None,
                    usage: UsageStats::default(),
                    iterations: 1,
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Check if the provider is healthy
    pub async fn health_check(&self) -> Result<(), crate::services::llm::LlmError> {
        self.provider.health_check().await
    }

    /// Get the current configuration
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Get provider information
    pub fn provider_info(&self) -> ProviderInfo {
        ProviderInfo {
            name: self.provider.name().to_string(),
            model: self.provider.model().to_string(),
            supports_thinking: self.provider.supports_thinking(),
            supports_tools: self.provider.supports_tools(),
        }
    }

    /// Delete a session from the database
    pub async fn delete_session(&self, session_id: &str) -> AppResult<()> {
        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Delete stories first (due to foreign key)
        conn.execute(
            "DELETE FROM execution_stories WHERE session_id = ?1",
            params![session_id],
        )?;

        // Delete session
        conn.execute(
            "DELETE FROM execution_sessions WHERE id = ?1",
            params![session_id],
        )?;

        // Remove from cache
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(session_id);

        Ok(())
    }

    /// Cleanup old completed sessions
    pub async fn cleanup_old_sessions(&self, days: i64) -> AppResult<usize> {
        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Delete old stories
        conn.execute(
            "DELETE FROM execution_stories WHERE session_id IN (
                SELECT id FROM execution_sessions
                WHERE status IN ('completed', 'cancelled')
                AND created_at < datetime('now', ?1 || ' days')
            )",
            params![format!("-{}", days)],
        )?;

        // Delete old sessions
        let count = conn.execute(
            "DELETE FROM execution_sessions
             WHERE status IN ('completed', 'cancelled')
             AND created_at < datetime('now', ?1 || ' days')",
            params![format!("-{}", days)],
        )?;

        Ok(count)
    }

    /// Handle an agent transfer requested by EventActions.
    ///
    /// Creates a `TransferHandler` with the registered `ComposerRegistry`,
    /// invokes `handle_transfer` for the target agent, and forwards the
    /// resulting `AgentEventStream` events through the `tx` channel to the
    /// frontend.
    ///
    /// Errors during transfer are logged but do not crash the agentic loop
    /// (acceptance criterion 7).
    async fn handle_agent_transfer(
        &self,
        target_agent: &str,
        session_id: &str,
        event_actions_state: &std::collections::HashMap<String, serde_json::Value>,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) {
        use crate::services::orchestrator::transfer::TransferHandler;
        use crate::services::agent_composer::types::{AgentContext, AgentConfig, AgentInput, AgentEvent};
        use futures_util::StreamExt;

        // Require a ComposerRegistry to be configured
        let registry = match &self.composer_registry {
            Some(r) => Arc::clone(r),
            None => {
                eprintln!(
                    "[transfer] Transfer to '{}' requested but no ComposerRegistry configured — skipping",
                    target_agent
                );
                return;
            }
        };

        let from_agent = "orchestrator";
        let transfer_message = format!("Transferred from orchestrator to '{}'", target_agent);

        // Create TransferHandler with depth limits and cycle detection
        let mut handler = TransferHandler::new(registry);

        // Build AgentContext from the orchestrator's current state
        let shared_state = event_actions_state.clone();

        // Create an OrchestratorContext for the transferred agent sharing
        // the memory store from the current event_actions_state.
        let transfer_orch_ctx = {
            let initial_state: std::collections::HashMap<String, serde_json::Value> =
                event_actions_state.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
            Arc::new(
                plan_cascade_core::context::OrchestratorContext::new(
                    session_id,
                    &self.config.project_root,
                    target_agent,
                ).with_initial_state(initial_state)
            )
        };

        let agent_ctx = AgentContext {
            session_id: session_id.to_string(),
            project_root: self.config.project_root.clone(),
            provider: Arc::clone(&self.provider),
            tool_executor: Arc::new(crate::services::tools::ToolExecutor::new(&self.config.project_root)),
            plugin_manager: None,
            hooks: Arc::new(crate::services::orchestrator::hooks::AgenticHooks::new()),
            input: AgentInput::Text(format!("Transfer to agent '{}'", target_agent)),
            shared_state: Arc::new(tokio::sync::RwLock::new(shared_state)),
            config: AgentConfig::default(),
            orchestrator_ctx: Some(transfer_orch_ctx),
        };

        // Invoke handle_transfer with the target agent name
        let stream_result = handler
            .handle_transfer(from_agent, target_agent, &transfer_message, &agent_ctx)
            .await;

        match stream_result {
            Ok(mut event_stream) => {
                let depth = handler.chain().depth();

                // Emit AgentTransferStart after successful setup (depth is now known)
                let _ = tx
                    .send(UnifiedStreamEvent::AgentTransferStart {
                        from_agent: from_agent.to_string(),
                        to_agent: target_agent.to_string(),
                        message: transfer_message.clone(),
                        depth,
                    })
                    .await;

                // Forward agent events through the tx channel
                let mut transfer_success = true;
                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(agent_event) => {
                            // Map AgentEvent to UnifiedStreamEvent for frontend
                            let unified_event = match agent_event {
                                AgentEvent::TextDelta { content } => {
                                    Some(UnifiedStreamEvent::TextDelta { content })
                                }
                                AgentEvent::ThinkingDelta { content } => {
                                    Some(UnifiedStreamEvent::ThinkingDelta {
                                        content,
                                        thinking_id: None,
                                    })
                                }
                                AgentEvent::ToolCall { name, args, .. } => {
                                    Some(UnifiedStreamEvent::ToolStart {
                                        tool_id: format!("transfer:{}:{}", target_agent, name),
                                        tool_name: name,
                                        arguments: Some(args),
                                    })
                                }
                                AgentEvent::ToolResult { name, result, .. } => {
                                    Some(UnifiedStreamEvent::ToolResult {
                                        tool_id: format!("transfer:{}:{}", target_agent, name),
                                        result: Some(result),
                                        error: None,
                                    })
                                }
                                AgentEvent::Done { output: _ } => {
                                    // Transfer agent completed; no separate event needed
                                    None
                                }
                                AgentEvent::AgentTransfer { target, message } => {
                                    // Sub-agent requests nested transfer — log it.
                                    // Nested transfers are not re-dispatched here;
                                    // they are handled by TransferHandler's chain/depth logic.
                                    tracing::info!(
                                        "[transfer] Sub-agent requests nested transfer to '{}': {}",
                                        target, message
                                    );
                                    None
                                }
                                _ => None,
                            };

                            if let Some(event) = unified_event {
                                let _ = tx.send(event).await;
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "[transfer] Error in agent '{}' event stream: {}",
                                target_agent, e
                            );
                            transfer_success = false;
                            break;
                        }
                    }
                }

                // Emit AgentTransferEnd event
                let _ = tx
                    .send(UnifiedStreamEvent::AgentTransferEnd {
                        from_agent: from_agent.to_string(),
                        to_agent: target_agent.to_string(),
                        success: transfer_success,
                        error: if transfer_success {
                            None
                        } else {
                            Some(format!("Transfer to '{}' encountered an error in event stream", target_agent))
                        },
                    })
                    .await;
            }
            Err(e) => {
                // Transfer setup failed (agent not found, cycle detected, depth limit, etc.)
                // Errors do not crash the agentic loop — they are logged and execution continues.
                eprintln!(
                    "[transfer] Failed to transfer to agent '{}': {}",
                    target_agent, e
                );
                let _ = tx
                    .send(UnifiedStreamEvent::AgentTransferEnd {
                        from_agent: from_agent.to_string(),
                        to_agent: target_agent.to_string(),
                        success: false,
                        error: Some(e.to_string()),
                    })
                    .await;
            }
        }
    }
}
