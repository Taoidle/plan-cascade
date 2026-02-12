use super::*;

#[derive(Debug, Clone, Default)]
struct PhaseCapture {
    tool_calls: usize,
    read_calls: usize,
    grep_calls: usize,
    glob_calls: usize,
    ls_calls: usize,
    cwd_calls: usize,
    observed_paths: HashSet<String>,
    read_paths: HashSet<String>,
    evidence_lines: Vec<String>,
    warnings: Vec<String>,
    pending_tools: HashMap<String, PendingAnalysisToolCall>,
}

#[derive(Debug, Clone, Default)]
struct PendingAnalysisToolCall {
    tool_name: String,
    arguments: Option<serde_json::Value>,
}

impl PhaseCapture {
    fn search_calls(&self) -> usize {
        self.grep_calls + self.glob_calls
    }

    fn tool_call_count(&self, name: &str) -> usize {
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
struct AnalysisPhaseOutcome {
    phase: AnalysisPhase,
    response: Option<String>,
    usage: UsageStats,
    iterations: u32,
    status: AnalysisPhaseStatus,
    error: Option<String>,
    capture: PhaseCapture,
}

#[derive(Debug, Clone, Default)]
struct AnalysisLedger {
    observed_paths: HashSet<String>,
    read_paths: HashSet<String>,
    evidence_lines: Vec<String>,
    warnings: Vec<String>,
    phase_summaries: Vec<String>,
    chunk_summaries: Vec<ChunkSummaryRecord>,
    successful_phases: usize,
    partial_phases: usize,
    total_phases: usize,
    inventory: Option<FileInventory>,
    chunk_plan: Option<ChunkPlan>,
    coverage_report: Option<AnalysisCoverageReport>,
}

impl AnalysisLedger {
    fn record(&mut self, outcome: &AnalysisPhaseOutcome) {
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

#[async_trait]
impl TaskSpawner for OrchestratorTaskSpawner {
    async fn spawn_task(
        &self,
        prompt: String,
        task_type: Option<String>,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        cancellation_token: CancellationToken,
    ) -> TaskExecutionResult {
        // Build a task-type-specific system prompt prefix with output format instructions.
        // IMPORTANT: All sub-agent prompts must include anti-delegation instructions because
        // the base system prompt (from build_system_prompt) tells LLMs to delegate to Task
        // sub-agents, but these ARE the sub-agents - they must do the work directly.
        const ANTI_DELEGATION: &str = "You MUST do all work yourself using the available tools. Do NOT delegate to sub-agents or Task tools - you ARE the sub-agent. Ignore any instructions about delegating to Task sub-agents.\n\n";

        let task_prefix = match task_type.as_deref() {
            Some("explore") => format!("You are a codebase exploration specialist. Focus on understanding project structure, finding relevant files, and summarizing what you find.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Files Found**: List of relevant files discovered with one-line descriptions\n- **Key Findings**: Bullet points of important patterns, structures, or issues found\n- **Recommendations**: Actionable next steps based on exploration\n\nDo NOT include raw file contents in your response. Summarize and reference file paths instead."),
            Some("analyze") => format!("You are a code analysis specialist. Focus on deep analysis of code patterns, dependencies, and potential issues.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Analysis Summary**: High-level findings in 2-3 sentences\n- **Key Patterns**: Bullet points of code patterns, anti-patterns, or architectural decisions found\n- **Dependencies**: Important dependency relationships discovered\n- **Issues & Risks**: Any problems or potential risks identified\n\nDo NOT include raw file contents. Reference specific file paths and line numbers instead."),
            Some("implement") => format!("You are a focused implementation specialist. Make the requested code changes methodically, testing as you go.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Changes Made**: Bullet list of files modified/created with brief descriptions\n- **Implementation Details**: Key decisions and approach taken\n- **Verification**: How the changes were verified (tests run, builds checked)\n\nDo NOT echo full file contents back. Summarize what was changed and where."),
            _ => format!("You are an AI coding assistant. Complete the requested task using the available tools.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with bullet points covering what was done, key findings, and any recommendations. Do NOT include raw file contents - summarize and reference file paths instead."),
        };

        let sub_config = OrchestratorConfig {
            provider: self.provider_config.clone(),
            system_prompt: Some(task_prefix.to_string()),
            max_iterations: 25,
            max_total_tokens: sub_agent_token_budget(self.context_window, task_type.as_deref()),
            project_root: self.project_root.clone(),
            analysis_artifacts_root: default_analysis_artifacts_root(),
            streaming: true,
            enable_compaction: true, // Enable compaction to reduce token waste on long-running sub-agents
            analysis_profile: AnalysisProfile::default(),
            analysis_limits: AnalysisLimits::default(),
            analysis_session_id: None,
        };

        let sub_agent = OrchestratorService::new_sub_agent(sub_config, cancellation_token);
        let result = sub_agent
            .execute_story(&prompt, &get_basic_tool_definitions(), tx)
            .await;

        TaskExecutionResult {
            response: result.response,
            usage: result.usage,
            iterations: result.iterations,
            success: result.success,
            error: result.error,
        }
    }
}

/// Session memory injected into compacted conversations to prevent post-compaction re-reads.
///
/// Built from the tool executor's read cache and conversation snippets before LLM summary
/// compaction. After compaction, the memory is placed between the original prompt and the
/// LLM summary so the agent retains awareness of files it has already read and key findings.
#[derive(Debug, Clone)]
struct SessionMemory {
    /// Files previously read in this session: (path, line_count, size_bytes)
    files_read: Vec<(String, usize, u64)>,
    /// Key findings extracted from compacted conversation snippets
    key_findings: Vec<String>,
    /// Original task description (first user message, truncated)
    task_description: String,
    /// Tool usage counts: tool_name -> count
    tool_usage_counts: HashMap<String, usize>,
}

impl SessionMemory {
    /// Generate a structured context string for injection into the conversation.
    ///
    /// The output explicitly lists files already read with sizes and includes a
    /// "Do NOT re-read" instruction to prevent wasteful duplicate file reads after
    /// context compaction.
    fn to_context_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        parts.push("[Session Memory - Preserved across context compaction]".to_string());

        // Task description
        if !self.task_description.is_empty() {
            parts.push(format!("\n## Task\n{}", self.task_description));
        }

        // Files already read
        if !self.files_read.is_empty() {
            parts.push("\n## Files Already Read".to_string());
            parts.push(
                "IMPORTANT: Do NOT re-read these files. Their contents were already processed."
                    .to_string(),
            );
            for (path, lines, bytes) in &self.files_read {
                parts.push(format!("- {} ({} lines, {} bytes)", path, lines, bytes));
            }
        }

        // Key findings
        if !self.key_findings.is_empty() {
            parts.push("\n## Key Findings".to_string());
            for finding in &self.key_findings {
                parts.push(format!("- {}", finding));
            }
        }

        // Tool usage summary
        if !self.tool_usage_counts.is_empty() {
            let mut sorted_tools: Vec<(&String, &usize)> =
                self.tool_usage_counts.iter().collect();
            sorted_tools.sort_by(|a, b| b.1.cmp(a.1));
            let tool_summary: Vec<String> = sorted_tools
                .iter()
                .map(|(name, count)| format!("{}({})", name, count))
                .collect();
            parts.push(format!("\n## Tool Usage\n{}", tool_summary.join(", ")));
        }

        parts.join("\n")
    }
}

/// Extract key findings from conversation snippets being compacted.
///
/// Scans text snippets for lines that look like conclusions, discoveries, or decisions.
/// Returns deduplicated findings sorted by length (shortest first) to keep summaries concise.
fn extract_key_findings(snippets: &[String]) -> Vec<String> {
    let finding_indicators = [
        "found",
        "discovered",
        "confirmed",
        "determined",
        "decided",
        "issue:",
        "error:",
        "warning:",
        "note:",
        "important:",
        "conclusion:",
        "result:",
        "observation:",
        "the file contains",
        "the code uses",
        "the project uses",
        "implemented",
        "fixed",
        "created",
        "modified",
        "updated",
    ];

    let mut findings: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let max_findings = 15;

    for snippet in snippets {
        for line in snippet.lines() {
            let trimmed = line.trim();
            if trimmed.len() < 20 || trimmed.len() > 300 {
                continue;
            }
            let lower = trimmed.to_lowercase();
            let is_finding = finding_indicators.iter().any(|ind| lower.contains(ind));
            if is_finding {
                // Normalize to avoid near-duplicates
                let normalized = trimmed.to_string();
                if !seen.contains(&lower) {
                    seen.insert(lower);
                    findings.push(normalized);
                    if findings.len() >= max_findings {
                        return findings;
                    }
                }
            }
        }
    }

    findings
}

/// Detects consecutive identical tool calls to break infinite loops.
///
/// Tracks the last (tool_name, args_hash) and counts consecutive repetitions.
/// When the count reaches the configured threshold, returns a break message
/// that can be injected into the conversation to redirect the LLM.
///
/// ADR-004: Pattern-based loop detection is cheaper than waiting for max_iterations=50.
#[derive(Debug)]
struct ToolCallLoopDetector {
    /// Threshold of consecutive identical calls before triggering
    threshold: u32,
    /// Last seen (tool_name, args_hash) tuple
    last_call: Option<(String, u64)>,
    /// Count of consecutive identical calls
    consecutive_count: u32,
}

impl ToolCallLoopDetector {
    fn new(threshold: u32) -> Self {
        Self {
            threshold,
            last_call: None,
            consecutive_count: 0,
        }
    }

    /// Record a tool call and return a break message if a loop is detected.
    ///
    /// Returns `Some(message)` when the same tool+args have been called `threshold`
    /// times consecutively, `None` otherwise.
    fn record_call(&mut self, tool_name: &str, args_str: &str) -> Option<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        args_str.hash(&mut hasher);
        let args_hash = hasher.finish();

        let call_key = (tool_name.to_string(), args_hash);

        if self.last_call.as_ref() == Some(&call_key) {
            self.consecutive_count += 1;
        } else {
            self.last_call = Some(call_key);
            self.consecutive_count = 1;
        }

        if self.consecutive_count >= self.threshold {
            // Reset after detection so the detector can catch new loops
            self.consecutive_count = 0;
            Some(format!(
                "[LOOP DETECTED] You have made the same identical tool call ({}) {} times consecutively \
                 with the same arguments. This is an infinite loop. STOP repeating this call. \
                 Use the information you already have from previous tool results to proceed with the task. \
                 If the previous result was a dedup/cache message, the file content was already read earlier \
                 in this session — refer to the session memory above for details.",
                tool_name, self.threshold
            ))
        } else {
            None
        }
    }
}

/// Marker string embedded in session memory messages for compaction identification.
///
/// Both `compact_messages()` (LLM-summary) and `compact_messages_prefix_stable()` use
/// this marker to locate and preserve the Layer 2 session memory message during compaction.
const SESSION_MEMORY_V1_MARKER: &str = "[SESSION_MEMORY_V1]";

/// Manages the Layer 2 session memory within the three-layer context architecture.
///
/// # Three-Layer Context Architecture
/// - **Layer 1 (Stable):** System prompt + index summary + tools (message index 0)
/// - **Layer 2 (Semi-stable):** Session memory — files read, key findings (fixed index)
/// - **Layer 3 (Volatile):** Conversation messages (tool calls, responses, etc.)
///
/// `SessionMemoryManager` maintains the session memory at a fixed message index,
/// accumulates file reads and findings, and updates the memory in-place before
/// each LLM call. The `[SESSION_MEMORY_V1]` marker enables compaction strategies
/// to identify and preserve this layer.
struct SessionMemoryManager {
    /// Fixed position in the messages vec (after system prompt at index 0)
    memory_index: usize,
    /// Marker string prepended to session memory content
    marker: &'static str,
}

impl SessionMemoryManager {
    /// Create a new SessionMemoryManager with the given memory index.
    ///
    /// Typically `memory_index` is 1 (right after the system prompt at index 0).
    fn new(memory_index: usize) -> Self {
        Self {
            memory_index,
            marker: SESSION_MEMORY_V1_MARKER,
        }
    }

    /// Build a session memory message with the V1 marker prepended.
    ///
    /// The message is an assistant-role message containing:
    /// 1. The `[SESSION_MEMORY_V1]` marker (for compaction identification)
    /// 2. The full session memory context string (files read, findings, etc.)
    fn build_memory_message(
        &self,
        files_read: Vec<(String, usize, u64)>,
        findings: Vec<String>,
    ) -> Message {
        let memory = SessionMemory {
            files_read,
            key_findings: findings,
            task_description: String::new(),
            tool_usage_counts: HashMap::new(),
        };

        let content = format!("{}\n{}", self.marker, memory.to_context_string());
        Message::assistant(content)
    }

    /// Update existing session memory in-place, or insert a new one if none exists.
    ///
    /// If the message at `memory_index` contains the `SESSION_MEMORY_V1` marker,
    /// it is replaced with a new session memory message built from the provided data.
    /// Otherwise, a new message is inserted at `memory_index`.
    fn update_or_insert(
        &self,
        messages: &mut Vec<Message>,
        files_read: Vec<(String, usize, u64)>,
        findings: Vec<String>,
    ) {
        let new_msg = self.build_memory_message(files_read, findings);

        // Check if there's already a session memory message at the expected index
        if self.memory_index < messages.len() {
            if Self::message_has_marker(&messages[self.memory_index]) {
                // Replace in-place
                messages[self.memory_index] = new_msg;
                return;
            }
        }

        // Also scan for the marker elsewhere (in case messages shifted)
        if let Some(idx) = Self::find_memory_index(messages) {
            messages[idx] = new_msg;
            return;
        }

        // No existing session memory — insert at the memory_index position
        let insert_at = self.memory_index.min(messages.len());
        messages.insert(insert_at, new_msg);
    }

    /// Scan messages for the SESSION_MEMORY_V1 marker and return the index if found.
    fn find_memory_index(messages: &[Message]) -> Option<usize> {
        for (i, msg) in messages.iter().enumerate() {
            if Self::message_has_marker(msg) {
                return Some(i);
            }
        }
        None
    }

    /// Check whether a message contains the SESSION_MEMORY_V1 marker.
    fn message_has_marker(msg: &Message) -> bool {
        for content in &msg.content {
            if let MessageContent::Text { text } = content {
                if text.contains(SESSION_MEMORY_V1_MARKER) {
                    return true;
                }
            }
        }
        false
    }
}

impl OrchestratorService {
    /// Create a new orchestrator service
    pub fn new(config: OrchestratorConfig) -> Self {
        let analysis_artifacts_root = config.analysis_artifacts_root.clone();
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
            ProviderType::Glm => Arc::new(GlmProvider::new(config.provider.clone())),
            ProviderType::Qwen => Arc::new(QwenProvider::new(config.provider.clone())),
            ProviderType::Ollama => Arc::new(OllamaProvider::new(config.provider.clone())),
        };

        let tool_executor = ToolExecutor::new(&config.project_root);

        Self {
            config,
            provider,
            tool_executor,
            cancellation_token: CancellationToken::new(),
            db_pool: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            analysis_store: AnalysisRunStore::new(analysis_artifacts_root),
            index_store: None,
        }
    }

    /// Create a sub-agent orchestrator (no Task tool, no database, inherits cancellation)
    fn new_sub_agent(config: OrchestratorConfig, cancellation_token: CancellationToken) -> Self {
        let analysis_artifacts_root = config.analysis_artifacts_root.clone();
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
            ProviderType::Glm => Arc::new(GlmProvider::new(config.provider.clone())),
            ProviderType::Qwen => Arc::new(QwenProvider::new(config.provider.clone())),
            ProviderType::Ollama => Arc::new(OllamaProvider::new(config.provider.clone())),
        };

        let tool_executor = ToolExecutor::new(&config.project_root);

        Self {
            config,
            provider,
            tool_executor,
            cancellation_token,
            db_pool: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            analysis_store: AnalysisRunStore::new(analysis_artifacts_root),
            index_store: None,
        }
    }

    /// Set the index store for project summary injection into the system prompt.
    pub fn with_index_store(mut self, store: IndexStore) -> Self {
        self.index_store = Some(store);
        self
    }

    /// Wire an embedding service to the tool executor for semantic CodebaseSearch.
    pub fn with_embedding_service(mut self, svc: Arc<EmbeddingService>) -> Self {
        self.tool_executor.set_embedding_service(svc);
        self
    }

    /// Set the database pool for session persistence.
    ///
    /// Indexing is no longer started here; use `IndexManager::ensure_indexed()`
    /// instead.
    pub fn with_database(mut self, pool: Pool<SqliteConnectionManager>) -> Self {
        if let Err(e) = self.init_session_schema(&pool) {
            eprintln!("Failed to initialize session schema: {}", e);
        }
        let store = IndexStore::new(pool.clone());
        // Wire the index store to the tool executor so CodebaseSearch works
        self.tool_executor
            .set_index_store(Arc::new(store.clone()));
        self.index_store = Some(store);
        self.db_pool = Some(pool);
        self
    }

    /// Initialize the session persistence schema
    fn init_session_schema(&self, pool: &Pool<SqliteConnectionManager>) -> AppResult<()> {
        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Create execution_sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS execution_sessions (
                id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                prd_path TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                system_prompt TEXT,
                current_story_index INTEGER DEFAULT 0,
                total_input_tokens INTEGER DEFAULT 0,
                total_output_tokens INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                started_at TEXT,
                completed_at TEXT,
                error TEXT,
                metadata TEXT
            )",
            [],
        )?;

        // Create execution_stories table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS execution_stories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                story_id TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at TEXT,
                completed_at TEXT,
                error TEXT,
                iterations INTEGER DEFAULT 0,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                quality_gates TEXT,
                FOREIGN KEY (session_id) REFERENCES execution_sessions(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exec_sessions_project ON execution_sessions(project_path)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exec_sessions_status ON execution_sessions(status)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exec_stories_session ON execution_stories(session_id)",
            [],
        )?;

        Ok(())
    }

    /// Get the cancellation token for external cancellation
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Cancel the current execution
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Check if execution has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Save a session to the database
    pub async fn save_session(&self, session: &ExecutionSession) -> AppResult<()> {
        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let metadata_json = serde_json::to_string(&session.metadata).unwrap_or_default();

        // Upsert session
        conn.execute(
            "INSERT INTO execution_sessions
             (id, project_path, prd_path, status, provider, model, system_prompt,
              current_story_index, total_input_tokens, total_output_tokens,
              created_at, updated_at, started_at, completed_at, error, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(id) DO UPDATE SET
             status = excluded.status,
             current_story_index = excluded.current_story_index,
             total_input_tokens = excluded.total_input_tokens,
             total_output_tokens = excluded.total_output_tokens,
             updated_at = excluded.updated_at,
             started_at = excluded.started_at,
             completed_at = excluded.completed_at,
             error = excluded.error,
             metadata = excluded.metadata",
            params![
                session.id,
                session.project_path,
                session.prd_path,
                session.status.to_string(),
                session.provider,
                session.model,
                session.system_prompt,
                session.current_story_index as i64,
                session.total_input_tokens as i64,
                session.total_output_tokens as i64,
                chrono::DateTime::from_timestamp(session.created_at, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                chrono::DateTime::from_timestamp(session.updated_at, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                session
                    .started_at
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.to_rfc3339()),
                session
                    .completed_at
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.to_rfc3339()),
                session.error,
                metadata_json,
            ],
        )?;

        // Save stories
        for story in &session.stories {
            let quality_gates_json =
                serde_json::to_string(&story.quality_gates).unwrap_or_default();

            conn.execute(
                "INSERT INTO execution_stories
                 (session_id, story_id, title, status, started_at, completed_at, error,
                  iterations, input_tokens, output_tokens, quality_gates)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(session_id, story_id) DO UPDATE SET
                 status = excluded.status,
                 started_at = excluded.started_at,
                 completed_at = excluded.completed_at,
                 error = excluded.error,
                 iterations = excluded.iterations,
                 input_tokens = excluded.input_tokens,
                 output_tokens = excluded.output_tokens,
                 quality_gates = excluded.quality_gates",
                params![
                    session.id,
                    story.story_id,
                    story.title,
                    story.status.to_string(),
                    story
                        .started_at
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                        .map(|dt| dt.to_rfc3339()),
                    story
                        .completed_at
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                        .map(|dt| dt.to_rfc3339()),
                    story.error,
                    story.iterations as i64,
                    story.input_tokens as i64,
                    story.output_tokens as i64,
                    quality_gates_json,
                ],
            )?;
        }

        // Update in-memory cache
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());

        Ok(())
    }

    /// Load a session from the database
    pub async fn load_session(&self, session_id: &str) -> AppResult<Option<ExecutionSession>> {
        // Check cache first
        {
            let sessions = self.active_sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                return Ok(Some(session.clone()));
            }
        }

        // Do all database work synchronously first
        let session = self.load_session_from_db(session_id)?;

        // Cache the loaded session if found
        if let Some(ref sess) = session {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(sess.id.clone(), sess.clone());
        }

        Ok(session)
    }

    /// Synchronous database loading (internal helper)
    fn load_session_from_db(&self, session_id: &str) -> AppResult<Option<ExecutionSession>> {
        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Load session
        let session_result = conn.query_row(
            "SELECT id, project_path, prd_path, status, provider, model, system_prompt,
                    current_story_index, total_input_tokens, total_output_tokens,
                    created_at, updated_at, started_at, completed_at, error, metadata
             FROM execution_sessions WHERE id = ?1",
            params![session_id],
            |row| {
                let status_str: String = row.get(3)?;
                let metadata_json: String = row.get::<_, Option<String>>(15)?.unwrap_or_default();

                Ok(ExecutionSession {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    prd_path: row.get(2)?,
                    status: status_str.parse().unwrap_or(ExecutionStatus::Pending),
                    provider: row.get(4)?,
                    model: row.get(5)?,
                    system_prompt: row.get(6)?,
                    stories: Vec::new(), // Loaded separately
                    current_story_index: row.get::<_, i64>(7)? as usize,
                    total_input_tokens: row.get::<_, i64>(8)? as u32,
                    total_output_tokens: row.get::<_, i64>(9)? as u32,
                    created_at: parse_timestamp(row.get::<_, Option<String>>(10)?),
                    updated_at: parse_timestamp(row.get::<_, Option<String>>(11)?),
                    started_at: row
                        .get::<_, Option<String>>(12)?
                        .map(|s| parse_timestamp(Some(s))),
                    completed_at: row
                        .get::<_, Option<String>>(13)?
                        .map(|s| parse_timestamp(Some(s))),
                    error: row.get(14)?,
                    metadata: serde_json::from_str(&metadata_json).unwrap_or_default(),
                })
            },
        );

        let mut session = match session_result {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(AppError::database(e.to_string())),
        };

        // Load stories
        let mut stmt = conn.prepare(
            "SELECT story_id, title, status, started_at, completed_at, error,
                    iterations, input_tokens, output_tokens, quality_gates
             FROM execution_stories WHERE session_id = ?1 ORDER BY id",
        )?;

        let stories: Vec<StoryExecutionState> = stmt
            .query_map(params![session_id], |row| {
                let status_str: String = row.get(2)?;
                let quality_gates_json: String =
                    row.get::<_, Option<String>>(9)?.unwrap_or_default();

                Ok(StoryExecutionState {
                    story_id: row.get(0)?,
                    title: row.get(1)?,
                    status: status_str.parse().unwrap_or(ExecutionStatus::Pending),
                    started_at: row
                        .get::<_, Option<String>>(3)?
                        .map(|s| parse_timestamp(Some(s))),
                    completed_at: row
                        .get::<_, Option<String>>(4)?
                        .map(|s| parse_timestamp(Some(s))),
                    error: row.get(5)?,
                    iterations: row.get::<_, i64>(6)? as u32,
                    input_tokens: row.get::<_, i64>(7)? as u32,
                    output_tokens: row.get::<_, i64>(8)? as u32,
                    quality_gates: serde_json::from_str(&quality_gates_json).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        session.stories = stories;

        Ok(Some(session))
    }

    /// List sessions with optional filters
    pub async fn list_sessions(
        &self,
        status_filter: Option<ExecutionStatus>,
        limit: Option<usize>,
    ) -> AppResult<Vec<ExecutionSessionSummary>> {
        let pool = self
            .db_pool
            .as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool
            .get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let limit = limit.unwrap_or(50) as i64;
        let query = match status_filter {
            Some(status) => format!(
                "SELECT id, project_path, status, provider, model, current_story_index,
                        total_input_tokens, total_output_tokens, created_at, updated_at,
                        (SELECT COUNT(*) FROM execution_stories WHERE session_id = execution_sessions.id) as total_stories,
                        (SELECT COUNT(*) FROM execution_stories WHERE session_id = execution_sessions.id AND status = 'completed') as completed_stories
                 FROM execution_sessions
                 WHERE status = '{}'
                 ORDER BY updated_at DESC
                 LIMIT {}",
                status, limit
            ),
            None => format!(
                "SELECT id, project_path, status, provider, model, current_story_index,
                        total_input_tokens, total_output_tokens, created_at, updated_at,
                        (SELECT COUNT(*) FROM execution_stories WHERE session_id = execution_sessions.id) as total_stories,
                        (SELECT COUNT(*) FROM execution_stories WHERE session_id = execution_sessions.id AND status = 'completed') as completed_stories
                 FROM execution_sessions
                 ORDER BY updated_at DESC
                 LIMIT {}",
                limit
            ),
        };

        let mut stmt = conn.prepare(&query)?;
        let summaries = stmt
            .query_map([], |row| {
                let status_str: String = row.get(2)?;
                let total_stories: i64 = row.get(10)?;
                let completed_stories: i64 = row.get(11)?;

                let progress = if total_stories > 0 {
                    (completed_stories as f32 / total_stories as f32) * 100.0
                } else {
                    0.0
                };

                Ok(ExecutionSessionSummary {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    status: status_str.parse().unwrap_or(ExecutionStatus::Pending),
                    progress_percentage: progress,
                    completed_stories: completed_stories as usize,
                    total_stories: total_stories as usize,
                    provider: row.get(3)?,
                    model: row.get(4)?,
                    created_at: parse_timestamp(row.get::<_, Option<String>>(8)?),
                    updated_at: parse_timestamp(row.get::<_, Option<String>>(9)?),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(summaries)
    }

    /// Get progress for an active session
    pub async fn get_progress(&self, session_id: &str) -> AppResult<Option<ExecutionProgress>> {
        let session = self.load_session(session_id).await?;
        Ok(session.map(|s| ExecutionProgress::from_session(&s)))
    }

    /// Execute a session with stories
    pub async fn execute_session(
        &self,
        session: &mut ExecutionSession,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        run_quality_gates: bool,
    ) -> SessionExecutionResult {
        let tools = get_tool_definitions();

        session.start();
        if let Err(e) = self.save_session(session).await {
            eprintln!("Failed to save session start: {}", e);
        }

        // Emit session start event
        let progress = ExecutionProgress::from_session(session);
        let _ = tx
            .send(UnifiedStreamEvent::SessionProgress {
                session_id: session.id.clone(),
                progress: serde_json::to_value(&progress).unwrap_or_default(),
            })
            .await;

        let mut total_usage = UsageStats::default();

        // Execute each story starting from current_story_index
        while session.current_story_index < session.stories.len() {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                session.cancel();
                if let Err(e) = self.save_session(session).await {
                    eprintln!("Failed to save cancelled session: {}", e);
                }

                return SessionExecutionResult {
                    session_id: session.id.clone(),
                    success: false,
                    completed_stories: session.completed_stories(),
                    failed_stories: session.failed_stories(),
                    total_stories: session.stories.len(),
                    usage: total_usage,
                    error: Some("Execution cancelled".to_string()),
                    quality_gates_passed: None,
                };
            }

            // Get story info for event emission (clone to avoid borrow issues)
            let story_index = session.current_story_index;
            let total_stories = session.stories.len();
            let session_id = session.id.clone();
            let project_path = session.project_path.clone();

            let (story_id, story_title, story_prompt) = {
                let story = &mut session.stories[story_index];
                story.start();
                let story_id = story.story_id.clone();
                let story_title = story.title.clone();
                let prompt = format!(
                    "Execute the following story from the PRD:\n\n\
                     Story ID: {}\n\
                     Title: {}\n\n\
                     Please implement this story completely, creating or modifying files as needed. \
                     Use the available tools to read, write, and execute commands. \
                     When you have fully implemented the story, provide a summary of what was done.",
                    story_id, story_title
                );
                (story_id, story_title, prompt)
            };

            // Emit story start event
            let _ = tx
                .send(UnifiedStreamEvent::StoryStart {
                    session_id: session_id.clone(),
                    story_id: story_id.clone(),
                    story_title: story_title.clone(),
                    story_index,
                    total_stories,
                })
                .await;

            // Execute the story
            let result = self.execute_story(&story_prompt, &tools, tx.clone()).await;

            // Update story state and session tokens
            {
                let story = &mut session.stories[story_index];
                story.iterations = result.iterations;
                story.input_tokens = result.usage.input_tokens;
                story.output_tokens = result.usage.output_tokens;
            }
            merge_usage(&mut total_usage, &result.usage);
            session.add_tokens(result.usage.input_tokens, result.usage.output_tokens);

            if result.success {
                // Mark story complete
                session.stories[story_index].complete();

                // Run quality gates if enabled
                if run_quality_gates {
                    let gates_result = execute_quality_gates(
                        &project_path,
                        None,
                        self.db_pool.clone(),
                        Some(session_id.clone()),
                    )
                    .await;

                    if let Ok(summary) = gates_result {
                        let passed = summary.failed_gates == 0;
                        session.stories[story_index]
                            .quality_gates
                            .insert("overall".to_string(), passed);

                        // Emit quality gates result
                        let _ = tx
                            .send(UnifiedStreamEvent::QualityGatesResult {
                                session_id: session_id.clone(),
                                story_id: story_id.clone(),
                                passed,
                                summary: serde_json::to_value(&summary).unwrap_or_default(),
                            })
                            .await;

                        if !passed {
                            // Quality gates failed - mark story as failed
                            session.stories[story_index].status = ExecutionStatus::Failed;
                            session.stories[story_index].error =
                                Some("Quality gates failed".to_string());
                        }
                    }
                }

                // Get current story status for event
                let story_success =
                    session.stories[story_index].status == ExecutionStatus::Completed;
                let story_error = session.stories[story_index].error.clone();

                // Emit story completion
                let _ = tx
                    .send(UnifiedStreamEvent::StoryComplete {
                        session_id: session_id.clone(),
                        story_id: story_id.clone(),
                        success: story_success,
                        error: story_error,
                    })
                    .await;
            } else {
                // Mark story as failed
                let error_msg = result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());
                session.stories[story_index].fail(error_msg);

                // Emit story failure
                let _ = tx
                    .send(UnifiedStreamEvent::StoryComplete {
                        session_id: session_id.clone(),
                        story_id: story_id.clone(),
                        success: false,
                        error: result.error.clone(),
                    })
                    .await;

                // Pause session on story failure for potential resume
                session.pause();
                if let Err(e) = self.save_session(session).await {
                    eprintln!("Failed to save paused session: {}", e);
                }

                return SessionExecutionResult {
                    session_id: session.id.clone(),
                    success: false,
                    completed_stories: session.completed_stories(),
                    failed_stories: session.failed_stories(),
                    total_stories: session.stories.len(),
                    usage: total_usage,
                    error: result.error,
                    quality_gates_passed: None,
                };
            }

            // Save progress
            if let Err(e) = self.save_session(session).await {
                eprintln!("Failed to save session progress: {}", e);
            }

            // Emit progress update
            let progress = ExecutionProgress::from_session(session);
            let _ = tx
                .send(UnifiedStreamEvent::SessionProgress {
                    session_id: session.id.clone(),
                    progress: serde_json::to_value(&progress).unwrap_or_default(),
                })
                .await;

            // Move to next story
            if !session.advance_to_next_story() {
                break;
            }
        }

        // Session completed
        session.complete();
        if let Err(e) = self.save_session(session).await {
            eprintln!("Failed to save completed session: {}", e);
        }

        // Emit completion
        let _ = tx
            .send(UnifiedStreamEvent::SessionComplete {
                session_id: session.id.clone(),
                success: true,
                completed_stories: session.completed_stories(),
                total_stories: session.stories.len(),
            })
            .await;

        SessionExecutionResult {
            session_id: session.id.clone(),
            success: true,
            completed_stories: session.completed_stories(),
            failed_stories: session.failed_stories(),
            total_stories: session.stories.len(),
            usage: total_usage,
            error: None,
            quality_gates_passed: Some(session.failed_stories() == 0),
        }
    }

    /// Execute a single story (internal method)
    async fn execute_story(
        &self,
        prompt: &str,
        tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        self.execute_story_with_request_options(
            prompt,
            tools,
            tx,
            LlmRequestOptions::default(),
            false,
        )
        .await
    }

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
            "\u{5f3a}\u{5236}", // 寮哄埗
            "\u{91cd}\u{65b0}\u{5206}\u{6790}", // 閲嶆柊鍒嗘瀽
            "\u{5237}\u{65b0}", // 鍒锋柊
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

    async fn execute_story_with_request_options(
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
        let mut loop_detector = ToolCallLoopDetector::new(3);

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
                         If your prior output was not parseable, output only valid tool_call blocks now.\n\
                         严格模式：所有工具调用必须以 tool_call 格式输出。"
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
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some("Execution cancelled".to_string()),
                };
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
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(error_msg),
                };
            }

            iterations += 1;

            // Determine which tools to pass to the LLM API
            let api_tools = if use_prompt_fallback {
                // Don't pass tools to the API; they're in the system prompt
                &[] as &[ToolDefinition]
            } else {
                tools
            };

            // Call LLM directly with the minimal system prompt (bypasses
            // build_system_prompt which has conflicting sub-agent instructions).
            let response = if self.config.streaming {
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
                    // Provider-aware compaction: Reliable -> LLM summary,
                    // Unreliable/None -> prefix-stable deletion.
                    match self.provider.tool_call_reliability() {
                        ToolCallReliability::Reliable => {
                            self.compact_messages(&mut messages, &tx).await;
                        }
                        ToolCallReliability::Unreliable | ToolCallReliability::None => {
                            let before = messages.len();
                            if Self::compact_messages_prefix_stable(&mut messages) {
                                // ADR-004: Clear dedup cache after prefix-stable compaction
                                self.tool_executor.clear_read_cache();
                                let removed_count = before - messages.len();
                                let _ = tx
                                    .send(UnifiedStreamEvent::ContextCompaction {
                                        messages_compacted: removed_count,
                                        messages_preserved: messages.len(),
                                        compaction_tokens: 0,
                                    })
                                    .await;
                            }
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

                // Execute each tool call
                for tc in &response.tool_calls {
                    let (effective_tool_name, effective_args) =
                        match prepare_tool_call_for_execution(
                            &tc.name,
                            &tc.arguments,
                            request_options.analysis_phase.as_deref(),
                        ) {
                            Ok(prepared) => prepared,
                            Err(error_message) => {
                                let _ = tx
                                    .send(UnifiedStreamEvent::ToolResult {
                                        tool_id: tc.id.clone(),
                                        result: None,
                                        error: Some(error_message.clone()),
                                    })
                                    .await;
                                messages.push(Message::tool_result(&tc.id, error_message, true));
                                continue;
                            }
                        };

                    // Emit tool start event
                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tc.id.clone(),
                            tool_name: effective_tool_name.clone(),
                            arguments: Some(effective_args.to_string()),
                        })
                        .await;

                    // Execute the tool (supports orchestrator-native tools like Analyze)
                    let result = self
                        .tool_executor
                        .execute(&effective_tool_name, &effective_args)
                        .await;
                    let context_tool_output = tool_output_for_model_context(
                        &effective_tool_name,
                        &result,
                        request_options.analysis_phase.as_deref(),
                    );

                    // Emit tool result event
                    let _ = tx
                        .send(UnifiedStreamEvent::ToolResult {
                            tool_id: tc.id.clone(),
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

                    // Add tool result to messages (with multimodal support)
                    if let Some((mime, b64)) = &result.image_data {
                        if self.provider.supports_multimodal() {
                            use crate::services::llm::types::ContentBlock;
                            let blocks = vec![
                                ContentBlock::Text {
                                    text: context_tool_output.clone(),
                                },
                                ContentBlock::Image {
                                    media_type: mime.clone(),
                                    data: b64.clone(),
                                },
                            ];
                            messages.push(Message::tool_result_multimodal(
                                &tc.id,
                                blocks,
                                !result.success,
                            ));
                        } else {
                            messages.push(Message::tool_result(
                                &tc.id,
                                context_tool_output.clone(),
                                !result.success,
                            ));
                        }
                    } else {
                        messages.push(Message::tool_result(
                            &tc.id,
                            context_tool_output.clone(),
                            !result.success,
                        ));
                    }

                    // Check for tool call loop (same tool+args repeated consecutively)
                    if let Some(break_msg) = loop_detector.record_call(
                        &effective_tool_name,
                        &effective_args.to_string(),
                    ) {
                        eprintln!("[loop-detector] Detected loop: {} called {} consecutive times", effective_tool_name, 3);
                        messages.push(Message::user(break_msg));
                    }
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

                // Execute each parsed tool call and collect results
                let mut tool_results = Vec::new();
                for ptc in &parsed_fallback.calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("story_fallback_{}", fallback_call_counter);

                    let (effective_tool_name, effective_args) =
                        match prepare_tool_call_for_execution(
                            &ptc.tool_name,
                            &ptc.arguments,
                            request_options.analysis_phase.as_deref(),
                        ) {
                            Ok(prepared) => prepared,
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
                                continue;
                            }
                        };

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tool_id.clone(),
                            tool_name: effective_tool_name.clone(),
                            arguments: Some(effective_args.to_string()),
                        })
                        .await;

                    let result = self
                        .tool_executor
                        .execute(&effective_tool_name, &effective_args)
                        .await;
                    let context_tool_output = tool_output_for_model_context(
                        &effective_tool_name,
                        &result,
                        request_options.analysis_phase.as_deref(),
                    );

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolResult {
                            tool_id: tool_id.clone(),
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

                    tool_results.push(format_tool_result(
                        &effective_tool_name,
                        &tool_id,
                        &context_tool_output,
                        !result.success,
                    ));

                    // Check for tool call loop in fallback path
                    if let Some(break_msg) = loop_detector.record_call(
                        &effective_tool_name,
                        &effective_args.to_string(),
                    ) {
                        eprintln!("[loop-detector] Detected loop in fallback path: {} called {} consecutive times", effective_tool_name, 3);
                        tool_results.push(break_msg);
                    }
                }

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
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
                // Check for tool-intent-without-invocation pattern
                let response_text = response.content.as_deref().unwrap_or("");
                let needs_repair = !matches!(reliability, ToolCallReliability::Reliable)
                    && repair_retry_count < 2
                    && text_describes_tool_intent(response_text);

                if needs_repair {
                    repair_retry_count += 1;
                    if let Some(text) = &response.content {
                        messages.push(Message::assistant(text.clone()));
                    }
                    let repair_msg = concat!(
                        "You described what tools you would use but did not actually call them. ",
                        "Please emit the actual tool call now using this exact format:\n\n",
                        "```tool_call\n",
                        "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                        "```\n\n",
                        "你描述了要使用的工具但没有实际调用。请直接输出 tool_call 代码块。\n",
                        "Do NOT describe what you will do. Just emit the tool_call block."
                    );
                    messages.push(Message::user(repair_msg.to_string()));
                    continue;
                }

                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t))
                    .filter(|t| !t.trim().is_empty())
                    .or(last_assistant_text);

                return ExecutionResult {
                    response: final_content,
                    usage: total_usage,
                    iterations,
                    success: true,
                    error: None,
                };
            }
        }
    }

    /// Execute a user message through the agentic loop
    pub async fn execute(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let user_message = message;

        let tools = get_tool_definitions();
        let reliability = self.provider.tool_call_reliability();
        // For None reliability (Ollama), don't pass tools to API at all
        let use_prompt_fallback = matches!(reliability, ToolCallReliability::None);
        let mut messages = vec![Message::user(user_message)];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;
        let mut fallback_call_counter = 0u32;
        let mut repair_retry_count = 0u32;
        let mut last_assistant_text: Option<String> = None;
        let mut loop_detector = ToolCallLoopDetector::new(3);

        // Create TaskContext for Task tool support in the agentic loop
        let task_spawner = Arc::new(OrchestratorTaskSpawner {
            provider_config: self.config.provider.clone(),
            project_root: self.config.project_root.clone(),
            context_window: self.provider.context_window(),
        });
        let task_ctx = TaskContext {
            spawner: task_spawner,
            tx: tx.clone(),
            cancellation_token: self.cancellation_token.clone(),
        };

        // Session memory manager for Layer 2 context (placed at index 1, after system prompt)
        let session_memory_manager = SessionMemoryManager::new(1);

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some("Execution cancelled".to_string()),
                };
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
                    session_memory_manager.update_or_insert(
                        &mut messages,
                        files_read,
                        findings,
                    );
                }
            }

            // Determine which tools to pass to the LLM API
            let api_tools = if use_prompt_fallback {
                // Don't pass tools to the API; they're in the system prompt
                &[] as &[ToolDefinition]
            } else {
                &tools
            };

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

            // Check for context compaction before processing tool calls.
            // Strategy selection: Reliable providers (Anthropic, OpenAI) use
            // LLM-summary compaction; Unreliable/None providers (Ollama, Qwen,
            // DeepSeek, GLM) use prefix-stable sliding-window deletion to avoid
            // an extra LLM call and preserve KV-cache prefix stability.
            if self.should_compact(last_input_tokens, false) {
                match self.provider.tool_call_reliability() {
                    ToolCallReliability::Reliable => {
                        self.compact_messages(&mut messages, &tx).await;
                    }
                    ToolCallReliability::Unreliable | ToolCallReliability::None => {
                        let removed = messages.len();
                        if Self::compact_messages_prefix_stable(&mut messages) {
                            // ADR-004: Clear dedup cache after prefix-stable compaction
                            self.tool_executor.clear_read_cache();
                            let removed_count = removed - messages.len();
                            let _ = tx
                                .send(UnifiedStreamEvent::ContextCompaction {
                                    messages_compacted: removed_count,
                                    messages_preserved: messages.len(),
                                    compaction_tokens: 0,
                                })
                                .await;
                        }
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

                // Execute each tool call
                for tc in &response.tool_calls {
                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            arguments: Some(tc.arguments.to_string()),
                        })
                        .await;

                    let (result, nested_usage, nested_iterations) = if tc.name == "Analyze" {
                        self.execute_analyze_tool_result(&tc.arguments, &tx).await
                    } else {
                        (
                            self.tool_executor
                                .execute_with_context(&tc.name, &tc.arguments, Some(&task_ctx))
                                .await,
                            UsageStats::default(),
                            0,
                        )
                    };
                    merge_usage(&mut total_usage, &nested_usage);
                    iterations += nested_iterations;

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolResult {
                            tool_id: tc.id.clone(),
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

                    // Truncate tool output for messages vec (LLM context)
                    // while keeping full content in the ToolResult event above.
                    let context_content = truncate_tool_output_for_context(
                        &tc.name,
                        &result.to_content(),
                    );

                    // Add tool result to messages (with multimodal support)
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
                                &tc.id,
                                blocks,
                                !result.success,
                            ));
                        } else {
                            messages.push(Message::tool_result(
                                &tc.id,
                                context_content,
                                !result.success,
                            ));
                        }
                    } else {
                        messages.push(Message::tool_result(
                            &tc.id,
                            context_content,
                            !result.success,
                        ));
                    }

                    // Check for tool call loop (same tool+args repeated consecutively)
                    if let Some(break_msg) = loop_detector.record_call(
                        &tc.name,
                        &tc.arguments.to_string(),
                    ) {
                        eprintln!("[loop-detector] Detected loop: {} called {} consecutive times", tc.name, 3);
                        messages.push(Message::user(break_msg));
                    }
                }
            } else if !parsed_fallback.calls.is_empty() {
                repair_retry_count = 0; // Reset on successful tool calls

                // Story-003: Check if the text alongside fallback tool calls is
                // already a complete answer. If so, exit the loop with that text
                // instead of executing the (unnecessary) tool calls.
                if let Some(text) = &response.content {
                    let cleaned = extract_text_without_tool_calls(text);
                    if is_complete_answer(&cleaned) {
                        eprintln!(
                            "[loop-exit] Exiting execute with complete text response, ignoring {} fallback tool calls",
                            parsed_fallback.calls.len()
                        );

                        // Emit completion event with special stop_reason
                        let _ = tx
                            .send(UnifiedStreamEvent::Complete {
                                stop_reason: Some("complete_text_exit".to_string()),
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
                            response: Some(cleaned),
                            usage: total_usage,
                            iterations,
                            success: true,
                            error: None,
                        };
                    }
                }

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

                // Execute each parsed tool call and collect results
                let mut tool_results = Vec::new();
                for ptc in &parsed_fallback.calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("fallback_{}", fallback_call_counter);

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tool_id.clone(),
                            tool_name: ptc.tool_name.clone(),
                            arguments: Some(ptc.arguments.to_string()),
                        })
                        .await;

                    let (result, nested_usage, nested_iterations) = if ptc.tool_name == "Analyze" {
                        self.execute_analyze_tool_result(&ptc.arguments, &tx).await
                    } else {
                        (
                            self.tool_executor
                                .execute_with_context(
                                    &ptc.tool_name,
                                    &ptc.arguments,
                                    Some(&task_ctx),
                                )
                                .await,
                            UsageStats::default(),
                            0,
                        )
                    };
                    merge_usage(&mut total_usage, &nested_usage);
                    iterations += nested_iterations;

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolResult {
                            tool_id: tool_id.clone(),
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

                    // Truncate tool output for messages vec (LLM context)
                    // while keeping full content in the ToolResult event above.
                    let context_content = truncate_tool_output_for_context(
                        &ptc.tool_name,
                        &result.to_content(),
                    );

                    tool_results.push(format_tool_result(
                        &ptc.tool_name,
                        &tool_id,
                        &context_content,
                        !result.success,
                    ));

                    // Check for tool call loop in fallback path
                    if let Some(break_msg) = loop_detector.record_call(
                        &ptc.tool_name,
                        &ptc.arguments.to_string(),
                    ) {
                        eprintln!("[loop-detector] Detected loop in fallback path: {} called {} consecutive times", ptc.tool_name, 3);
                        tool_results.push(break_msg);
                    }
                }

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
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
                // For Unreliable providers: check if the model described tool usage
                // in text without actually invoking tools (repair-hint pattern).
                let response_text = response.content.as_deref().unwrap_or("");
                let needs_repair = !matches!(reliability, ToolCallReliability::Reliable)
                    && repair_retry_count < 2
                    && text_describes_tool_intent(response_text);

                if needs_repair {
                    // Send a repair hint to nudge the model into actually calling tools
                    repair_retry_count += 1;
                    if let Some(text) = &response.content {
                        messages.push(Message::assistant(text.clone()));
                    }
                    let repair_hint = concat!(
                        "You described what tools you would use but did not actually call them. ",
                        "Please emit the actual tool call now using this exact format:\n\n",
                        "```tool_call\n",
                        "{\"tool\": \"ToolName\", \"arguments\": {\"param\": \"value\"}}\n",
                        "```\n\n",
                        "你描述了要使用的工具但没有实际调用。请使用以下格式直接输出工具调用：\n\n",
                        "```tool_call\n",
                        "{\"tool\": \"工具名\", \"arguments\": {\"参数\": \"值\"}}\n",
                        "```\n\n",
                        "Do NOT describe what you will do. Just emit the tool_call block.\n",
                        "不要描述你将要做什么，直接输出 tool_call 代码块。"
                    );
                    messages.push(Message::user(repair_hint.to_string()));
                    // Continue the loop — don't return as final response
                    continue;
                }

                // Reset repair counter on successful completion
                // (reaching here means we have a genuine final response)

                // Always strip any tool call blocks from the response text,
                // since models may emit text-based tool calls even when using native mode
                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t))
                    .filter(|t| !t.trim().is_empty())
                    .or(last_assistant_text);

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

    /// Execute an analysis task with an evidence-first multi-phase pipeline.
    async fn execute_with_analysis_pipeline(
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

    fn baseline_steps_for_phase(
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
        // at index 1, skip it — it will be rebuilt after compaction.
        let compact_range_start = if messages.len() > 1
            && SessionMemoryManager::message_has_marker(&messages[1])
        {
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
    fn compact_messages_prefix_stable(messages: &mut Vec<Message>) -> bool {
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
    fn effective_system_prompt(
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

        let mut prompt = build_system_prompt(
            &self.config.project_root,
            prompt_tools,
            project_summary.as_ref(),
        );

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
                     valid tool_call blocks only before any explanation.\n\
                     严格工具格式模式：使用工具时必须且只能输出可解析的 tool_call 代码块。\
                     如果之前的输出使用了文字描述或格式错误的标签来调用工具，请修正并仅输出有效的 tool_call 代码块。"
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
    async fn call_llm(
        &self,
        messages: &[Message],
        api_tools: &[ToolDefinition],
        prompt_tools: &[ToolDefinition],
        request_options: LlmRequestOptions,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools, &request_options);
        self.provider
            .send_message(
                messages.to_vec(),
                system,
                api_tools.to_vec(),
                request_options,
            )
            .await
    }

    /// Call the LLM with streaming mode.
    ///
    /// `api_tools` are sent to the provider API (empty for prompt-fallback providers).
    /// `prompt_tools` are listed in the system prompt for guidance.
    async fn call_llm_streaming(
        &self,
        messages: &[Message],
        api_tools: &[ToolDefinition],
        prompt_tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools, &request_options);
        self.provider
            .stream_message(
                messages.to_vec(),
                system,
                api_tools.to_vec(),
                tx,
                request_options,
            )
            .await
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
}

#[derive(Debug, Clone, Default)]
struct ParsedFallbackCalls {
    calls: Vec<ParsedToolCall>,
    dropped_reasons: Vec<String>,
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
fn is_complete_answer(text: &str) -> bool {
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
    let last_line = trimmed.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
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
            // Ensure it's a whole word — check that the char before is whitespace or start
            let prefix_len = last_lower.len() - word.len();
            if prefix_len == 0 || last_lower.as_bytes().get(prefix_len - 1) == Some(&b' ') {
                return false;
            }
        }
    }

    true
}

/// Detect when the model describes tool usage intent in text without actually invoking tools.
///
/// Returns true if the text mentions known tool names combined with action/intent phrases
/// (in both English and Chinese), suggesting the model wants to call tools but failed to
/// emit them in the expected format.
fn text_describes_tool_intent(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let text_lower = text.to_lowercase();

    // Known tool names to detect
    let tool_names = [
        "read", "write", "edit", "bash", "glob", "grep", "ls", "cwd", "analyze", "task",
        "webfetch", "websearch",
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
        "调用",
        "执行",
        "使用工具",
        "让我使用",
        "让我调用",
        "我将使用",
        "我将调用",
        "我来使用",
        "我来调用",
        "我需要使用",
        "我需要调用",
        "接下来使用",
        "接下来调用",
        "先使用",
        "先调用",
        "查看一下",
        "读取",
        "检查一下",
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

fn parse_fallback_tool_calls(
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

fn canonical_tool_name(name: &str) -> Option<&'static str> {
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
        _ => None,
    }
}

fn analysis_excluded_roots() -> &'static [&'static str] {
    &[
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        "coverage",
        ".venv",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "claude-code",
        "codex",
    ]
}

fn is_analysis_excluded_path(path: &str) -> bool {
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

fn ensure_object_arguments(
    arguments: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    arguments
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new)
}

fn has_nonempty_string_arg(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn prepare_tool_call_for_execution(
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

fn merge_usage(total: &mut UsageStats, delta: &UsageStats) {
    total.input_tokens += delta.input_tokens;
    total.output_tokens += delta.output_tokens;
    if let Some(thinking) = delta.thinking_tokens {
        total.thinking_tokens = Some(total.thinking_tokens.unwrap_or(0) + thinking);
    }
    if let Some(cache_read) = delta.cache_read_tokens {
        total.cache_read_tokens = Some(total.cache_read_tokens.unwrap_or(0) + cache_read);
    }
    if let Some(cache_creation) = delta.cache_creation_tokens {
        total.cache_creation_tokens =
            Some(total.cache_creation_tokens.unwrap_or(0) + cache_creation);
    }
}

fn build_coverage_metrics(
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

fn analysis_phase_token_budget(context_window: u32, phase: AnalysisPhase) -> u32 {
    let phase_cap = match phase {
        AnalysisPhase::StructureDiscovery => 80_000,
        AnalysisPhase::ArchitectureTrace => 100_000,
        AnalysisPhase::ConsistencyCheck => 80_000,
    };
    let scaled = (context_window as f64 * 0.55) as u32;
    scaled.clamp(20_000, phase_cap)
}

fn analysis_layer_goal_satisfied(phase: AnalysisPhase, capture: &PhaseCapture) -> bool {
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

fn analysis_scope_guidance(message: &str) -> String {
    let excludes = analysis_excluded_roots_for_message(message);

    format!(
        "Focus on first-party project files under the working directory. \
Avoid expensive full-repo scans. Exclude top-level directories by default: {}. \
Only enter excluded directories when explicitly requested by the user.",
        excludes.join(", ")
    )
}

fn analysis_excluded_roots_for_message(message: &str) -> Vec<String> {
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

fn is_valid_analysis_tool_start(tool_name: &str, args: Option<&serde_json::Value>) -> bool {
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

fn analysis_phase_system_prompt(phase: AnalysisPhase) -> &'static str {
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

fn analysis_phase_worker_prompt(phase: AnalysisPhase) -> String {
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

fn analysis_phase_system_prompt_with_quota(
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

fn evaluate_analysis_quota(capture: &PhaseCapture, quota: &AnalysisToolQuota) -> Vec<String> {
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

fn build_phase_summary_from_evidence(phase: AnalysisPhase, capture: &PhaseCapture) -> String {
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

fn condense_phase_summary_for_context(summary: &str, max_chars: usize) -> String {
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

fn build_synthesis_phase_block(
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

fn build_synthesis_chunk_block(chunk_summaries: &[ChunkSummaryRecord], max_chars: usize) -> String {
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

fn build_synthesis_evidence_block(
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

fn should_rewrite_synthesis_output(text: &str) -> bool {
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

fn build_synthesis_rewrite_prompt(user_request: &str, draft: &str) -> String {
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

fn sample_paths_with_prefix(paths: &HashSet<String>, prefix: &str, limit: usize) -> Vec<String> {
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

fn observed_root_buckets(
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
            let samples = items.into_iter().take(sample_limit.max(1)).collect::<Vec<_>>();
            (root, count, samples)
        })
        .collect()
}

fn sanitize_warning_for_report(warning: &str, project_root: &std::path::Path) -> String {
    let root = project_root.to_string_lossy().replace('\\', "/");
    warning
        .replace('\\', "/")
        .replace(&root, "<project_root>")
        .replace("<project_root>//", "<project_root>/")
}

fn build_deterministic_analysis_fallback_report(
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
        || ledger.observed_paths.iter().any(|path| looks_like_test_path(path));
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
            "- Some areas may require a rerun after resolving provider/runtime limits."
                .to_string(),
        );
    }
    if unknown_count == 0 {
        lines.push("- Detailed business logic per module is not expanded in this concise fallback; raw evidence is available in analysis artifacts.".to_string());
    }

    lines.join("\n")
}

fn join_sorted_paths(paths: &HashSet<String>, limit: usize) -> String {
    if paths.is_empty() {
        return "(none)".to_string();
    }
    let mut items: Vec<String> = paths.iter().cloned().collect();
    items.sort();
    items.into_iter().take(limit).collect::<Vec<_>>().join("\n")
}

fn build_test_evidence_block(
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

fn parse_tool_arguments(arguments: &Option<String>) -> Option<serde_json::Value> {
    arguments
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
}

fn truncate_for_log(text: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    if text.len() <= limit {
        return text.to_string();
    }
    let mut cut = 0usize;
    for (idx, _) in text.char_indices() {
        if idx > limit {
            break;
        }
        cut = idx;
    }
    if cut == 0 {
        "...".to_string()
    } else {
        format!("{}...", &text[..cut])
    }
}

fn tool_output_for_model_context(
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
fn truncate_tool_output_for_context(tool_name: &str, content: &str) -> String {
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
    let mut truncated: String = content.lines().take(max_lines).collect::<Vec<_>>().join("\n");

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

fn trim_line_reference_suffix(path: &str) -> String {
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

fn normalize_candidate_path(raw: &str) -> Option<String> {
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

fn extract_primary_path_from_arguments(arguments: &serde_json::Value) -> Option<String> {
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

fn extract_all_paths_from_arguments(arguments: &serde_json::Value) -> Vec<String> {
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

fn summarize_tool_activity(
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

fn select_local_seed_files(inventory: &FileInventory) -> Vec<String> {
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

fn related_test_candidates(
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

fn summarize_file_head(path: &std::path::Path, max_lines: usize) -> Option<String> {
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

fn looks_like_test_path(path: &str) -> bool {
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

fn extract_path_candidates_from_text(text: &str) -> Vec<String> {
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

fn is_plausible_path_text(candidate: &str) -> bool {
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

fn is_observed_path(candidate: &str, observed: &HashSet<String>) -> bool {
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

fn observed_root_segments(observed: &HashSet<String>) -> HashSet<String> {
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

fn is_concrete_path_reference(candidate: &str, observed_roots: &HashSet<String>) -> bool {
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

fn find_unverified_paths(text: &str, observed: &HashSet<String>) -> Vec<String> {
    let observed_roots = observed_root_segments(observed);
    extract_path_candidates_from_text(text)
        .into_iter()
        .filter(|path| is_concrete_path_reference(path, &observed_roots))
        .filter(|path| !is_observed_path(path, observed))
        .collect()
}

/// Parse an RFC3339 timestamp to Unix timestamp.
fn parse_timestamp(s: Option<String>) -> i64 {
    s.and_then(|ts| chrono::DateTime::parse_from_rfc3339(&ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("service_tests.rs");
}

