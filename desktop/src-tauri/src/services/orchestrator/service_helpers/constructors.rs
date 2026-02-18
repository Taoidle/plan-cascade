use super::*;

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

        const SEARCH_GUIDANCE: &str = "\
When exploring the codebase, prefer the **CodebaseSearch** tool over Grep/Glob for finding symbols, \
locating files by component, and understanding project structure. It queries a pre-built index and \
is much faster than scanning files. Use Grep only for full-text regex search or when CodebaseSearch \
reports the index is unavailable.\n\n";

        let mut task_prefix = match task_type.as_deref() {
            Some("explore") => format!("You are a codebase exploration specialist. Focus on understanding project structure, finding relevant files, and summarizing what you find.\n\n{ANTI_DELEGATION}{SEARCH_GUIDANCE}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Files Found**: List of relevant files discovered with one-line descriptions\n- **Key Findings**: Bullet points of important patterns, structures, or issues found\n- **Recommendations**: Actionable next steps based on exploration\n\nDo NOT include raw file contents in your response. Summarize and reference file paths instead."),
            Some("analyze") => format!("You are a code analysis specialist. Focus on deep analysis of code patterns, dependencies, and potential issues.\n\n{ANTI_DELEGATION}{SEARCH_GUIDANCE}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Analysis Summary**: High-level findings in 2-3 sentences\n- **Key Patterns**: Bullet points of code patterns, anti-patterns, or architectural decisions found\n- **Dependencies**: Important dependency relationships discovered\n- **Issues & Risks**: Any problems or potential risks identified\n\nDo NOT include raw file contents. Reference specific file paths and line numbers instead."),
            Some("implement") => format!("You are a focused implementation specialist. Make the requested code changes methodically, testing as you go.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Changes Made**: Bullet list of files modified/created with brief descriptions\n- **Implementation Details**: Key decisions and approach taken\n- **Verification**: How the changes were verified (tests run, builds checked)\n\nDo NOT echo full file contents back. Summarize what was changed and where."),
            _ => format!("You are an AI coding assistant. Complete the requested task using the available tools.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with bullet points covering what was done, key findings, and any recommendations. Do NOT include raw file contents - summarize and reference file paths instead."),
        };

        // Append language instruction for sub-agents
        match self.detected_language.as_deref() {
            Some("zh") => {
                task_prefix.push_str("\n\nIMPORTANT: Respond in Chinese (简体中文). Only use English for code and tool parameters.");
            }
            _ => {}
        }

        // Sub-agents perform basic tool calls (LS, Read, Grep, Bash) and don't
        // benefit from thinking/reasoning mode. Disable it to avoid:
        // 1. Wasting tokens on reasoning for simple file operations
        // 2. Compatibility issues (e.g., Qwen thinking mode conflicts with
        //    tool_choice and prompt fallback)
        let mut sub_provider = self.provider_config.clone();
        sub_provider.enable_thinking = false;

        let sub_config = OrchestratorConfig {
            provider: sub_provider,
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
            project_id: None,
        };

        // Give each sub-agent a fresh read cache. Sub-agents have their own conversation
        // context and cannot reference "session memory" from the parent — so a shared cache
        // would return [DEDUP] for files the sub-agent has never seen, causing loops.
        let isolated_read_cache =
            Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let sub_agent = OrchestratorService::new_sub_agent_with_shared_state(
            sub_config,
            cancellation_token,
            isolated_read_cache,
            self.shared_index_store.clone(),
            self.shared_embedding_service.clone(),
            self.shared_embedding_manager.clone(),
            self.shared_hnsw_index.clone(),
        );
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
            ProviderType::Minimax => Arc::new(MinimaxProvider::new(config.provider.clone())),
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
            detected_language: Mutex::new(None),
            hooks: crate::services::orchestrator::hooks::build_default_hooks(),
            selected_skills: None,
            loaded_memories: None,
            knowledge_context: None,
            knowledge_context_config: KnowledgeContextConfig::default(),
            cached_knowledge_block: Mutex::new(None),
        }
    }

    /// Create a sub-agent orchestrator (no Task tool, no database, inherits cancellation).
    ///
    /// Sub-agents use empty hooks (`AgenticHooks::new()`) because they have
    /// independent context windows and should not inherit the parent's
    /// memory/skill lifecycle hooks.
    pub(super) fn new_sub_agent(config: OrchestratorConfig, cancellation_token: CancellationToken) -> Self {
        let analysis_artifacts_root = config.analysis_artifacts_root.clone();
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
            ProviderType::Glm => Arc::new(GlmProvider::new(config.provider.clone())),
            ProviderType::Qwen => Arc::new(QwenProvider::new(config.provider.clone())),
            ProviderType::Minimax => Arc::new(MinimaxProvider::new(config.provider.clone())),
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
            detected_language: Mutex::new(None),
            hooks: crate::services::orchestrator::hooks::AgenticHooks::new(),
            selected_skills: None,
            loaded_memories: None,
            knowledge_context: None,
            knowledge_context_config: KnowledgeContextConfig::default(),
            cached_knowledge_block: Mutex::new(None),
        }
    }

    /// Create a sub-agent orchestrator that shares the parent's read cache, index store,
    /// embedding service, and embedding manager. This avoids redundant file reads and
    /// enables CodebaseSearch in sub-agents.
    fn new_sub_agent_with_shared_state(
        config: OrchestratorConfig,
        cancellation_token: CancellationToken,
        shared_read_cache: std::sync::Arc<
            std::sync::Mutex<
                std::collections::HashMap<
                    (PathBuf, usize, usize),
                    crate::services::tools::ReadCacheEntry,
                >,
            >,
        >,
        shared_index_store: Option<Arc<IndexStore>>,
        shared_embedding_service: Option<Arc<EmbeddingService>>,
        shared_embedding_manager: Option<Arc<EmbeddingManager>>,
        shared_hnsw_index: Option<Arc<HnswIndex>>,
    ) -> Self {
        let analysis_artifacts_root = config.analysis_artifacts_root.clone();
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
            ProviderType::Glm => Arc::new(GlmProvider::new(config.provider.clone())),
            ProviderType::Qwen => Arc::new(QwenProvider::new(config.provider.clone())),
            ProviderType::Minimax => Arc::new(MinimaxProvider::new(config.provider.clone())),
            ProviderType::Ollama => Arc::new(OllamaProvider::new(config.provider.clone())),
        };

        let mut tool_executor =
            ToolExecutor::new_with_shared_cache(&config.project_root, shared_read_cache);

        // Wire the parent's index store and embedding service to the sub-agent's tool executor
        if let Some(store) = &shared_index_store {
            tool_executor.set_index_store(Arc::clone(store));
        }
        if let Some(svc) = &shared_embedding_service {
            tool_executor.set_embedding_service(Arc::clone(svc));
        }
        if let Some(mgr) = &shared_embedding_manager {
            tool_executor.set_embedding_manager(Arc::clone(mgr));
        }
        if let Some(hnsw) = &shared_hnsw_index {
            tool_executor.set_hnsw_index(Arc::clone(hnsw));
        }

        Self {
            config,
            provider,
            tool_executor,
            cancellation_token,
            db_pool: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            analysis_store: AnalysisRunStore::new(analysis_artifacts_root),
            index_store: shared_index_store,
            detected_language: Mutex::new(None),
            hooks: crate::services::orchestrator::hooks::AgenticHooks::new(),
            selected_skills: None,
            loaded_memories: None,
            knowledge_context: None,
            knowledge_context_config: KnowledgeContextConfig::default(),
            cached_knowledge_block: Mutex::new(None),
        }
    }

    /// Set the index store for project summary injection into the system prompt.
    pub fn with_index_store(mut self, store: Arc<IndexStore>) -> Self {
        self.index_store = Some(store);
        self
    }

    /// Wire an embedding service to the tool executor for semantic CodebaseSearch.
    pub fn with_embedding_service(mut self, svc: Arc<EmbeddingService>) -> Self {
        self.tool_executor.set_embedding_service(svc);
        self
    }

    /// Wire an EmbeddingManager to the tool executor for provider-aware semantic
    /// CodebaseSearch (ADR-F002). When set, the manager's `embed_query` is used
    /// instead of the raw `EmbeddingService::embed_text`, gaining caching,
    /// fallback, and provider-agnostic query embedding.
    pub fn with_embedding_manager(mut self, mgr: Arc<EmbeddingManager>) -> Self {
        self.tool_executor.set_embedding_manager(mgr);
        self
    }

    /// Set lifecycle hooks for cross-cutting concerns (memory, skills, etc.).
    pub fn with_hooks(mut self, hooks: crate::services::orchestrator::hooks::AgenticHooks) -> Self {
        self.hooks = hooks;
        self
    }

    /// Register skill-related lifecycle hooks.
    ///
    /// Wires the SkillIndex into the agentic lifecycle so that:
    /// - `on_session_start` auto-detects applicable skills for the project
    /// - `on_user_message` refines skill selection based on message content
    ///
    /// The selected skills are stored in the provided `selected_skills` shared
    /// state, which is also retained by the orchestrator for system prompt injection.
    pub fn with_skill_hooks(
        mut self,
        skill_index: std::sync::Arc<tokio::sync::RwLock<crate::services::skills::model::SkillIndex>>,
        policy: crate::services::skills::model::SelectionPolicy,
        selected_skills: std::sync::Arc<tokio::sync::RwLock<Vec<crate::services::skills::model::SkillMatch>>>,
    ) -> Self {
        crate::services::orchestrator::hooks::register_skill_hooks(
            &mut self.hooks,
            skill_index,
            policy,
            selected_skills.clone(),
        );
        self.selected_skills = Some(selected_skills);
        self
    }

    /// Register memory-related lifecycle hooks.
    ///
    /// Wires the ProjectMemoryStore into the agentic lifecycle so that:
    /// - `on_session_start` loads relevant memories from the store
    /// - `on_session_end` extracts new memories from the session summary
    /// - `on_compaction` extracts key information from compacted content
    ///
    /// The loaded memories are stored in the provided `loaded_memories` shared
    /// state, which is also retained by the orchestrator for system prompt injection.
    pub fn with_memory_hooks(
        mut self,
        memory_store: std::sync::Arc<crate::services::memory::store::ProjectMemoryStore>,
        loaded_memories: std::sync::Arc<tokio::sync::RwLock<Vec<crate::services::memory::store::MemoryEntry>>>,
    ) -> Self {
        crate::services::orchestrator::hooks::register_memory_hooks(
            &mut self.hooks,
            memory_store,
            loaded_memories.clone(),
        );
        self.loaded_memories = Some(loaded_memories);
        self
    }

    /// Set the knowledge context provider for RAG-based context injection.
    ///
    /// When configured, the orchestrator queries project knowledge collections
    /// at the start of execution and injects relevant context into the system prompt.
    pub fn with_knowledge_context(
        mut self,
        provider: Arc<crate::services::knowledge::context_provider::KnowledgeContextProvider>,
        config: crate::services::knowledge::context_provider::KnowledgeContextConfig,
    ) -> Self {
        self.knowledge_context = Some(provider);
        self.knowledge_context_config = config;
        self
    }

    /// Populate the cached knowledge context block by querying the provider.
    ///
    /// This is called at the beginning of execution (once per session/message).
    /// The cached block is then injected into every system prompt during the
    /// agentic loop without re-querying the knowledge base.
    pub(super) async fn populate_knowledge_context(&self, user_query: &str) {
        let provider = match &self.knowledge_context {
            Some(p) => p,
            None => return,
        };

        if !self.knowledge_context_config.enabled {
            return;
        }

        // Derive project_id from config or project_root
        let project_id = self
            .config
            .project_id
            .clone()
            .unwrap_or_else(|| {
                self.config
                    .project_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "default".to_string())
            });

        match provider
            .query_for_context(&project_id, user_query, &self.knowledge_context_config)
            .await
        {
            Ok(chunks) => {
                if !chunks.is_empty() {
                    let block =
                        crate::services::knowledge::context_provider::KnowledgeContextProvider::format_context_block(&chunks);
                    let mut cached = self.cached_knowledge_block.lock().unwrap();
                    *cached = Some(block);
                }
            }
            Err(e) => {
                eprintln!("[knowledge] Failed to query knowledge context: {}", e);
            }
        }
    }

    /// Register guardrail-related lifecycle hooks.
    ///
    /// Wires the GuardrailRegistry into the agentic lifecycle so that:
    /// - `on_user_message` validates user input (may block or redact)
    /// - `on_after_tool` validates tool output (may warn)
    pub fn with_guardrail_hooks(
        mut self,
        registry: std::sync::Arc<tokio::sync::RwLock<crate::services::guardrail::GuardrailRegistry>>,
    ) -> Self {
        crate::services::guardrail::register_guardrail_hooks(
            &mut self.hooks,
            registry,
        );
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
        let store = Arc::new(IndexStore::new(pool.clone()));
        // Wire the index store to the tool executor so CodebaseSearch works
        self.tool_executor.set_index_store(Arc::clone(&store));
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
}
