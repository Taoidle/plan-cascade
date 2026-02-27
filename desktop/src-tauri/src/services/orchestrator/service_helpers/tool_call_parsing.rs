use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct ParsedFallbackCalls {
    pub(super) calls: Vec<ParsedToolCall>,
    pub(super) dropped_reasons: Vec<String>,
}

/// Determine whether an LLM response text constitutes a complete answer.
///
/// ADR-001: The heuristic checks:
///   1. text character count > 200 (using `.chars().count()` for CJK correctness)
///   2. the text does NOT end with incomplete sentence patterns such as:
///      - trailing colons, ellipses
///      - "I will", "Let me", "I'll"
///      - unclosed code blocks (odd number of ```)
///      - dangling conjunctions: "and", "but", "or", "then"
///
/// Returns true when the text looks like a substantive, complete answer.
pub(super) fn is_complete_answer(text: &str) -> bool {
    let trimmed = text.trim();
    // Must be > 200 characters (char count for CJK safety)
    if trimmed.chars().count() <= 200 {
        return false;
    }

    // Check for unclosed code blocks (odd count of ```)
    let backtick_block_count = trimmed.matches("```").count();
    if backtick_block_count % 2 != 0 {
        return false;
    }

    // Get the last non-empty line for trailing-pattern checks
    let last_line = trimmed
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let last_trimmed = last_line.trim();
    let last_lower = last_trimmed.to_lowercase();

    // Incomplete trailing patterns
    let incomplete_endings: &[&str] = &[
        ":", "...", "\u{2026}", // ellipsis unicode
    ];
    for pat in incomplete_endings {
        if last_trimmed.ends_with(pat) {
            return false;
        }
    }

    // Intent phrases that suggest the model is about to do something, not done
    let intent_prefixes: &[&str] = &[
        "i will",
        "i'll",
        "let me",
        "i am going to",
        "i'm going to",
        "next i will",
        "next, i will",
        "now i will",
        "now i'll",
        "now let me",
    ];
    for prefix in intent_prefixes {
        if last_lower.ends_with(prefix) || last_lower.ends_with(&format!("{prefix},")) {
            return false;
        }
    }

    // Dangling conjunctions at end of last line
    let dangling: &[&str] = &["and", "but", "or", "then", "and,", "but,", "or,", "then,"];
    for word in dangling {
        if last_lower.ends_with(word) {
            // Ensure it's a whole word; check that the char before is whitespace or start.
            let prefix_len = last_lower.len() - word.len();
            if prefix_len == 0 || last_lower.as_bytes().get(prefix_len - 1) == Some(&b' ') {
                return false;
            }
        }
    }

    // If the last line still narrates a pending next step, it's not complete yet.
    if text_describes_pending_action(last_trimmed) {
        return false;
    }

    true
}

/// Check if text is a rhetorical question (offer to do more) rather than a pending action.
/// Detects patterns like "需要我进一步分析吗？", "如果你想了解更多...", etc.
pub(super) fn is_rhetorical_offer(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Conditional offers: "如果...", "是否...", "你想...", "你需要...", "需要我..."
    let zh_conditional_markers = [
        "\u{5982}\u{679C}",         // 如果
        "\u{662F}\u{5426}",         // 是否
        "\u{4F60}\u{60F3}",         // 你想
        "\u{4F60}\u{9700}\u{8981}", // 你需要
        "\u{9700}\u{8981}\u{6211}", // 需要我
    ];

    // Question suffixes that indicate rhetorical question
    let question_suffixes = [
        "\u{5417}\u{FF1F}", // 吗？
        "\u{5417}?",        // 吗?
        "\u{4E48}\u{FF1F}", // 么？
        "\u{4E48}?",        // 么?
        "\u{FF1F}",         // ？(full-width)
        "?",                // ? (half-width)
    ];

    let has_question_suffix = question_suffixes.iter().any(|s| trimmed.ends_with(s));

    // Pattern: conditional marker + question suffix → rhetorical offer
    if has_question_suffix && zh_conditional_markers.iter().any(|p| trimmed.contains(p)) {
        return true;
    }

    // Pattern: invitation markers like "可以告诉我", "请告诉我", "可以进一步"
    let zh_invitation_markers = [
        "\u{53EF}\u{4EE5}\u{544A}\u{8BC9}\u{6211}", // 可以告诉我
        "\u{8BF7}\u{544A}\u{8BC9}\u{6211}",         // 请告诉我
        "\u{53EF}\u{4EE5}\u{8FDB}\u{4E00}\u{6B65}", // 可以进一步
    ];

    if has_question_suffix && zh_invitation_markers.iter().any(|m| trimmed.contains(m)) {
        return true;
    }

    false
}

/// Detect when the model describes tool usage intent in text without actually invoking tools.
///
/// Returns true if the text mentions known tool names combined with action/intent phrases
/// (in both English and Chinese), suggesting the model wants to call tools but failed to
/// emit them in the expected format.
pub(super) fn text_describes_tool_intent(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    // Exclude rhetorical questions / offers to do more
    if is_rhetorical_offer(text) {
        return false;
    }

    let text_lower = text.to_lowercase();

    // Known tool names to detect
    let tool_names = [
        "read",
        "write",
        "edit",
        "bash",
        "glob",
        "grep",
        "ls",
        "cwd",
        "analyze",
        "task",
        "webfetch",
        "websearch",
    ];

    // English intent phrases
    let en_intent = [
        "let me use",
        "i will call",
        "i'll call",
        "i will use",
        "i'll use",
        "let me call",
        "i need to use",
        "i need to call",
        "using the",
        "let me run",
        "i will run",
        "i'll run",
        "let me check",
        "let me read",
        "let me execute",
        "i will execute",
    ];

    // Chinese intent phrases
    let zh_intent = [
        "\u{8c03}\u{7528}",
        "\u{6267}\u{884c}",
        "\u{4f7f}\u{7528}\u{5de5}\u{5177}",
        "\u{8ba9}\u{6211}\u{4f7f}\u{7528}",
        "\u{8ba9}\u{6211}\u{8c03}\u{7528}",
        "\u{6211}\u{5c06}\u{4f7f}\u{7528}",
        "\u{6211}\u{5c06}\u{8c03}\u{7528}",
        "\u{6211}\u{6765}\u{4f7f}\u{7528}",
        "\u{6211}\u{6765}\u{8c03}\u{7528}",
        "\u{6211}\u{9700}\u{8981}\u{4f7f}\u{7528}",
        "\u{6211}\u{9700}\u{8981}\u{8c03}\u{7528}",
        "\u{63a5}\u{4e0b}\u{6765}\u{4f7f}\u{7528}",
        "\u{63a5}\u{4e0b}\u{6765}\u{8c03}\u{7528}",
        "\u{5148}\u{4f7f}\u{7528}",
        "\u{5148}\u{8c03}\u{7528}",
        "\u{67e5}\u{770b}",
        "\u{8bfb}\u{53d6}",
        "\u{68c0}\u{67e5}",
    ];

    let has_tool_mention = tool_names.iter().any(|t| {
        // Check for tool name as a word boundary (not inside another word)
        let t_lower = *t;
        text_lower.contains(t_lower)
    });

    if !has_tool_mention {
        return false;
    }

    let has_en_intent = en_intent.iter().any(|p| text_lower.contains(p));
    let has_zh_intent = zh_intent.iter().any(|p| text.contains(p));

    has_en_intent || has_zh_intent
}

/// Detect unfinished "next-step narration" even when tool names are not explicitly mentioned.
///
/// Examples:
/// - "Let me check README next."
/// - "我先查看 README 文件。"
/// - "接下来我会继续分析。"
pub(crate) fn text_describes_pending_action(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }

    // Exclude rhetorical questions / offers to do more
    if is_rhetorical_offer(text) {
        return false;
    }

    let text_lower = text.to_lowercase();

    let en_pending_markers = [
        "let me",
        "i will",
        "i'll",
        "i am going to",
        "i'm going to",
        "i need to",
        "next i",
        "next, i",
        "now i",
        "now, i",
    ];

    let zh_pending_markers = [
        "\u{8ba9}\u{6211}",         // 让我
        "\u{8ba9}\u{6211}\u{5148}", // 让我先
        "\u{6211}\u{5148}",         // 我先
        "\u{6211}\u{5c06}",         // 我将
        "\u{6211}\u{4f1a}",         // 我会
        "\u{6211}\u{6765}",         // 我来
        "\u{63a5}\u{4e0b}\u{6765}", // 接下来
        "\u{4e0b}\u{4e00}\u{6b65}", // 下一步
    ];

    let en_action_terms = [
        "check",
        "read",
        "inspect",
        "analyze",
        "review",
        "open",
        "list",
        "search",
        "explore",
        "verify",
        "look at",
        "look into",
    ];

    let zh_action_terms = [
        "\u{67e5}\u{770b}",         // 查看
        "\u{8bfb}\u{53d6}",         // 读取
        "\u{9605}\u{8bfb}",         // 阅读
        "\u{68c0}\u{67e5}",         // 检查
        "\u{5206}\u{6790}",         // 分析
        "\u{641c}\u{7d22}",         // 搜索
        "\u{6253}\u{5f00}",         // 打开
        "\u{5217}\u{51fa}",         // 列出
        "\u{770b}\u{4e00}\u{4e0b}", // 看一下
    ];

    let has_pending_marker = en_pending_markers.iter().any(|p| text_lower.contains(p))
        || zh_pending_markers.iter().any(|p| text.contains(p));
    if !has_pending_marker {
        return false;
    }

    let has_action_term = en_action_terms.iter().any(|p| text_lower.contains(p))
        || zh_action_terms.iter().any(|p| text.contains(p));
    if has_action_term {
        return true;
    }

    // Fallback: mention of file/directory targets after a pending marker.
    text_lower.contains("readme")
        || text_lower.contains(".md")
        || text_lower.contains(".rs")
        || text_lower.contains(".ts")
        || text_lower.contains(".py")
        || text.contains("\u{6587}\u{4ef6}") // 文件
        || text.contains("\u{76ee}\u{5f55}") // 目录
}

pub(super) fn parse_fallback_tool_calls(
    response: &LlmResponse,
    analysis_phase: Option<&str>,
) -> ParsedFallbackCalls {
    let mut parsed = ParsedFallbackCalls::default();
    let mut seen = HashSet::new();

    for text in [response.content.as_deref(), response.thinking.as_deref()]
        .into_iter()
        .flatten()
    {
        for call in parse_tool_calls(text) {
            match prepare_tool_call_for_execution(&call.tool_name, &call.arguments, analysis_phase)
            {
                Ok((tool_name, arguments)) => {
                    let signature = format!("{}:{}", tool_name, arguments);
                    if seen.insert(signature) {
                        parsed.calls.push(ParsedToolCall {
                            tool_name,
                            arguments,
                            raw_text: call.raw_text,
                        });
                    }
                }
                Err(reason) => parsed.dropped_reasons.push(reason),
            }
        }
    }

    parsed
}

pub(super) fn canonical_tool_name(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "read" => Some("Read"),
        "write" => Some("Write"),
        "edit" => Some("Edit"),
        "bash" => Some("Bash"),
        "glob" => Some("Glob"),
        "grep" => Some("Grep"),
        "ls" => Some("LS"),
        "cwd" => Some("Cwd"),
        "analyze" => Some("Analyze"),
        "task" => Some("Task"),
        "webfetch" => Some("WebFetch"),
        "websearch" => Some("WebSearch"),
        "notebookedit" => Some("NotebookEdit"),
        "codebasesearch" => Some("CodebaseSearch"),
        "searchknowledge" => Some("SearchKnowledge"),
        _ => None,
    }
}

pub(super) fn analysis_excluded_roots() -> &'static [&'static str] {
    // Delegate to the single source of truth in analysis_index.
    crate::services::orchestrator::analysis_index::default_excluded_roots()
}

pub(super) fn is_analysis_excluded_path(path: &str) -> bool {
    let normalized = normalize_candidate_path(path).unwrap_or_else(|| path.replace('\\', "/"));
    let mut segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".");
    let first = match segments.next() {
        Some(segment) => segment.to_ascii_lowercase(),
        None => return false,
    };
    analysis_excluded_roots()
        .iter()
        .any(|excluded| *excluded == first)
}

pub(super) fn ensure_object_arguments(
    arguments: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    arguments
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new)
}

pub(super) fn has_nonempty_string_arg(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub(super) fn prepare_tool_call_for_execution(
    tool_name: &str,
    arguments: &serde_json::Value,
    analysis_phase: Option<&str>,
) -> Result<(String, serde_json::Value), String> {
    let canonical = canonical_tool_name(tool_name)
        .ok_or_else(|| format!("Unsupported tool name '{}'", tool_name.trim()))?;
    let strict_analysis = analysis_phase.is_some();
    let mut map = ensure_object_arguments(arguments);

    match canonical {
        "Cwd" => {}
        "LS" => {
            if has_nonempty_string_arg(&map, "path").is_none() {
                map.insert(
                    "path".to_string(),
                    serde_json::Value::String(".".to_string()),
                );
            }
        }
        "Glob" => {
            if has_nonempty_string_arg(&map, "pattern").is_none() {
                map.insert(
                    "pattern".to_string(),
                    serde_json::Value::String(if strict_analysis {
                        "*".to_string()
                    } else {
                        "**/*".to_string()
                    }),
                );
            }
            if has_nonempty_string_arg(&map, "path").is_none() {
                map.insert(
                    "path".to_string(),
                    serde_json::Value::String(".".to_string()),
                );
            }
            if strict_analysis {
                map.entry("head_limit".to_string())
                    .or_insert_with(|| serde_json::Value::Number(serde_json::Number::from(120)));
            }
        }
        "Grep" => {
            let pattern = has_nonempty_string_arg(&map, "pattern")
                .ok_or_else(|| "Grep requires non-empty 'pattern'".to_string())?;
            if pattern == "(missing pattern)" {
                return Err("Grep requires non-empty 'pattern'".to_string());
            }
            if has_nonempty_string_arg(&map, "path").is_none() {
                map.insert(
                    "path".to_string(),
                    serde_json::Value::String(".".to_string()),
                );
            }
            if strict_analysis {
                map.entry("output_mode".to_string())
                    .or_insert_with(|| serde_json::Value::String("files_with_matches".to_string()));
                map.entry("head_limit".to_string())
                    .or_insert_with(|| serde_json::Value::Number(serde_json::Number::from(40)));
            }
        }
        "Read" => {
            let file_path = has_nonempty_string_arg(&map, "file_path")
                .or_else(|| has_nonempty_string_arg(&map, "path"));
            match file_path {
                Some(path) => {
                    map.insert("file_path".to_string(), serde_json::Value::String(path));
                }
                None => return Err("Read requires non-empty 'file_path'".to_string()),
            }
            if strict_analysis {
                map.entry("offset".to_string())
                    .or_insert_with(|| serde_json::Value::Number(serde_json::Number::from(1)));
                map.entry("limit".to_string())
                    .or_insert_with(|| serde_json::Value::Number(serde_json::Number::from(120)));
            }
        }
        "Bash" => {
            if strict_analysis {
                return Err("Bash is disabled during analysis phases".to_string());
            }
            if has_nonempty_string_arg(&map, "command").is_none() {
                return Err("Bash requires non-empty 'command'".to_string());
            }
        }
        "Analyze" => {
            if strict_analysis {
                return Err("Analyze is disabled during analysis phases".to_string());
            }
            let query = has_nonempty_string_arg(&map, "query")
                .or_else(|| has_nonempty_string_arg(&map, "prompt"))
                .ok_or_else(|| "Analyze requires non-empty 'query'".to_string())?;
            map.insert("query".to_string(), serde_json::Value::String(query));
            if has_nonempty_string_arg(&map, "mode").is_none() {
                map.insert(
                    "mode".to_string(),
                    serde_json::Value::String("auto".to_string()),
                );
            }
        }
        "Write" | "Edit" | "Task" | "WebFetch" | "WebSearch" | "NotebookEdit" => {
            if strict_analysis {
                return Err(format!(
                    "{} is disabled during analysis phases; use read-only tools",
                    canonical
                ));
            }
        }
        _ => {}
    }

    if strict_analysis {
        for key in [
            "path",
            "file_path",
            "working_dir",
            "notebook_path",
            "path_hint",
        ] {
            if let Some(path) = has_nonempty_string_arg(&map, key) {
                if is_analysis_excluded_path(&path) {
                    return Err(format!("Path '{}' is outside analysis scope", path));
                }
            }
        }
    }

    Ok((canonical.to_string(), serde_json::Value::Object(map)))
}

pub(super) fn build_coverage_metrics(
    ledger: &AnalysisLedger,
    coverage_report: &AnalysisCoverageReport,
) -> CoverageMetrics {
    let failed = ledger
        .total_phases
        .saturating_sub(ledger.successful_phases + ledger.partial_phases);
    CoverageMetrics {
        observed_paths: ledger.observed_paths.len(),
        evidence_records: ledger.evidence_lines.len(),
        successful_phases: ledger.successful_phases,
        partial_phases: ledger.partial_phases,
        failed_phases: failed,
        inventory_total_files: coverage_report.inventory_total_files,
        inventory_indexed_files: coverage_report.inventory_indexed_files,
        sampled_read_files: coverage_report.sampled_read_files,
        test_files_total: coverage_report.test_files_total,
        test_files_read: coverage_report.test_files_read,
        coverage_ratio: coverage_report.coverage_ratio,
        test_coverage_ratio: coverage_report.test_coverage_ratio,
        sampled_read_ratio: coverage_report.sampled_read_ratio,
        observed_test_coverage_ratio: coverage_report.observed_test_coverage_ratio,
        chunk_count: coverage_report.chunk_count,
        synthesis_rounds: coverage_report.synthesis_rounds,
    }
}

pub(super) fn analysis_phase_token_budget(context_window: u32, phase: AnalysisPhase) -> u32 {
    let phase_cap = match phase {
        AnalysisPhase::StructureDiscovery => 80_000,
        AnalysisPhase::ArchitectureTrace => 100_000,
        AnalysisPhase::ConsistencyCheck => 80_000,
    };
    let scaled = (context_window as f64 * 0.55) as u32;
    scaled.clamp(20_000, phase_cap)
}

pub(super) fn analysis_layer_goal_satisfied(phase: AnalysisPhase, capture: &PhaseCapture) -> bool {
    match phase {
        AnalysisPhase::StructureDiscovery => {
            capture.read_calls >= 2 && capture.observed_paths.len() >= 4
        }
        AnalysisPhase::ArchitectureTrace => {
            capture.read_calls >= 3 && capture.observed_paths.len() >= 8
        }
        AnalysisPhase::ConsistencyCheck => {
            capture.read_calls >= 3
                && (capture.grep_calls + capture.glob_calls) >= 1
                && capture.observed_paths.len() >= 6
        }
    }
}

pub(super) fn analysis_scope_guidance(message: &str) -> String {
    let excludes = analysis_excluded_roots_for_message(message);

    format!(
        "Focus on first-party project files under the working directory. \
Avoid expensive full-repo scans. Exclude top-level directories by default: {}. \
Only enter excluded directories when explicitly requested by the user.",
        excludes.join(", ")
    )
}

pub(super) fn analysis_excluded_roots_for_message(message: &str) -> Vec<String> {
    let lower = message.to_lowercase();
    let user_mentions_cloned_repos = lower.contains("claude-code") || lower.contains("codex");

    let mut excludes = analysis_excluded_roots()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if user_mentions_cloned_repos {
        excludes.retain(|item| item != "claude-code" && item != "codex");
    }
    excludes
}

pub(super) fn is_valid_analysis_tool_start(
    tool_name: &str,
    args: Option<&serde_json::Value>,
) -> bool {
    match tool_name {
        "Cwd" => true,
        "LS" => args
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "Read" => args
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "Glob" => args
            .and_then(|v| v.get("pattern"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "Grep" => args
            .and_then(|v| v.get("pattern"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .map(|s| !s.is_empty() && s != "(missing pattern)")
            .unwrap_or(false),
        _ => true,
    }
}
