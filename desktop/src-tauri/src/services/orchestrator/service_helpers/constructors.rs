use super::*;

#[async_trait]
impl TaskSpawner for OrchestratorTaskSpawner {
    async fn spawn_task(
        &self,
        prompt: String,
        subagent_type: SubAgentType,
        depth: u32,
        tx: mpsc::Sender<UnifiedStreamEvent>,
        cancellation_token: CancellationToken,
    ) -> TaskExecutionResult {
        // 1. Build type-specific system prompt
        let task_prefix = build_subagent_prompt(subagent_type, depth, &self.detected_language);

        // 2. Get tools filtered by sub-agent type
        let tools = crate::services::tools::definitions::get_tool_definitions_for_subagent(subagent_type);

        // 3. Configure sub-agent
        let mut sub_provider = self.provider_config.clone();
        // Inherit thinking from parent when the provider supports it
        sub_provider.enable_thinking =
            self.parent_supports_thinking && sub_provider.enable_thinking;

        let sub_config = OrchestratorConfig {
            provider: sub_provider,
            system_prompt: Some(task_prefix),
            max_iterations: subagent_max_iterations(subagent_type),
            max_total_tokens: subagent_token_budget_typed(
                self.context_window,
                subagent_type,
                depth,
            ),
            project_root: self.project_root.clone(),
            analysis_artifacts_root: default_analysis_artifacts_root(),
            streaming: true,
            enable_compaction: true,
            analysis_profile: AnalysisProfile::default(),
            analysis_limits: AnalysisLimits::default(),
            analysis_session_id: None,
            project_id: None,
            compaction_config: CompactionConfig::default(),
            task_type: Some(subagent_type.legacy_task_type().to_string()),
            // general-purpose at this depth can spawn further sub-agents
            sub_agent_depth: if subagent_type.can_spawn_subagents()
                && depth + 1 < MAX_SUB_AGENT_DEPTH
            {
                Some(depth + 1)
            } else {
                None
            },
        };

        // Truncate knowledge block for sub-agents to avoid blowing their context budget.
        const SUB_AGENT_KNOWLEDGE_BLOCK_CAP: usize = 4096;
        let knowledge_block_snapshot = self.knowledge_block_snapshot.clone().map(|block| {
            if block.len() > SUB_AGENT_KNOWLEDGE_BLOCK_CAP {
                let mut truncated = block[..SUB_AGENT_KNOWLEDGE_BLOCK_CAP].to_string();
                truncated.push_str("\n\n[Knowledge context truncated for sub-agent]");
                truncated
            } else {
                block
            }
        });

        // Give each sub-agent a fresh read cache.
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
            self.detected_language.clone(),
            self.skills_snapshot.clone(),
            self.memories_snapshot.clone(),
            knowledge_block_snapshot,
        );
        let result = sub_agent.execute_story(&prompt, &tools, tx).await;

        TaskExecutionResult {
            response: result.response,
            usage: result.usage,
            iterations: result.iterations,
            success: result.success,
            error: result.error,
        }
    }
}

/// Build a type-specific system prompt for sub-agents.
fn build_subagent_prompt(
    subagent_type: SubAgentType,
    _depth: u32,
    detected_language: &Option<String>,
) -> String {
    const ANTI_DELEGATION: &str = "You MUST do all work yourself using the available tools. Do NOT delegate to sub-agents or Task tools - you ARE the sub-agent. Ignore any instructions about delegating to Task sub-agents.\n\n";

    let mut prompt = match subagent_type {
        SubAgentType::Explore => format!(
            "You are a codebase exploration specialist. Your goal is DEEP understanding through reading actual code.\n\n\
             {ANTI_DELEGATION}\
             ## Exploration Strategy\n\
             1. **CodebaseSearch**(query='<your assigned area>', scope='all') — find key symbols, files, components\n\
             2. **LS** on your assigned directory to understand structure\n\
             3. **Read implementation files** (8-15 files) — the core logic, not just declarations\n\
             4. **Bash** for git context: `git log --oneline -10` for recent changes, `git log --oneline -5 <file>` for file history\n\
             5. **Grep** for specific patterns when you need to trace connections\n\n\
             ## What to Read (Priority Order)\n\
             - **Core implementation files**: service logic, algorithms, handlers, processors — where the actual work happens\n\
             - **Type/model definitions**: structs, interfaces, enums that define the domain\n\
             - **Entry points**: main.rs, index.ts, app.py — understand how things start and connect\n\
             - **Config files**: only if needed to understand build/dependency setup\n\
             - **Do NOT** stop at mod.rs/index.ts re-exports — follow them to the actual implementation\n\n\
             ## Depth Rules\n\
             Choose your depth based on the task description:\n\
             - **Quick** (prompt says \"quick\", \"brief\", \"overview\"): Read 3-5 key files, focus on entry points and main types.\n\
             - **Medium** (default): Read 8-15 files, trace 2-3 levels of call chains. Cover core implementation files.\n\
             - **Very thorough** (prompt says \"thorough\", \"deep\", \"comprehensive\", \"detailed\"): Read 15-30 files, trace all major code paths. Include test files, config, and edge cases.\n\n\
             General rules:\n\
             - Read FULL implementation functions (not just signatures) for core logic\n\
             - When you find a function call to another module, use Grep/CodebaseSearch to trace it\n\
             - Read 2-3 levels deep: if module A calls B which uses C, read all three\n\
             - Use `git log --oneline -10` to understand recent project activity\n\
             - Use `git log --oneline -5 <file>` for important files to see recent change context\n\n\
             ## Output Format\n\
             Be thorough but concise. Focus on insights over raw data.\n\
             - **Architecture**: Directory layout, module relationships, data flow\n\
             - **Key Components**: Important types/functions with WHAT THEY DO (not just names)\n\
             - **Patterns**: Design patterns, conventions, architectural decisions\n\
             - **Dependencies**: Key internal and external dependencies\n\n\
             Reference specific file paths (e.g., `src/services/auth.rs:42`). Summarize logic — do NOT paste raw code."
        ),
        SubAgentType::Plan => format!(
            "You are a code analysis specialist. Focus on deep analysis of code patterns, \
             dependencies, and potential issues.\n\n\
             {ANTI_DELEGATION}\
             ## Analysis Strategy\n\
             1. **CodebaseSearch**(query='<topic>', scope='all') — find relevant symbols and files\n\
             2. Read the relevant source files — understand the actual implementation\n\
             3. Trace data flow and control flow through the code\n\
             4. Use **Bash** for git context: `git log --oneline -10 <file>` for change history\n\
             5. Identify architectural patterns and anti-patterns\n\
             6. Note dependency relationships and coupling\n\n\
             ## Output Format\n\
             Provide a structured analysis with these sections:\n\
             - **Analysis Summary**: High-level findings in 2-3 sentences\n\
             - **Key Patterns**: Code patterns, anti-patterns, or architectural decisions found\n\
             - **Dependencies**: Important dependency relationships discovered\n\
             - **Issues & Risks**: Any problems or potential risks identified\n\n\
             Reference specific file paths and line numbers. Summarize findings, don't paste code."
        ),
        SubAgentType::GeneralPurpose => {
            "You are a coordinator agent. Your job is to discover the project structure and \
             decompose complex tasks into parallel sub-agent calls tailored to this specific project.\n\n\
             ## Strategy\n\
             Step 1: DISCOVER the project structure\n\
               - Use LS on the project root to see the top-level directories and files\n\
               - Use CodebaseSearch(query='project structure', scope='files') to see key files\n\
               - Read key config files (package.json, Cargo.toml, pyproject.toml, etc.) if present\n\
               - Understand what kind of project this is and how it's organized\n\n\
             Step 2: PARTITION the exploration based on what you discovered\n\
               - Launch multiple Task(subagent_type='explore') in ONE response\n\
               - Each Explore agent should focus on a SPECIFIC, NON-OVERLAPPING directory or domain\n\
               - Partition based on the ACTUAL project structure (NOT hardcoded assumptions)\n\
               - Tell each Explore agent to use CodebaseSearch(query='<area-specific query>') as their first step\n\n\
             Step 3: SYNTHESIZE the results\n\
               - Combine all sub-agent reports into a unified analysis\n\
               - Identify cross-cutting concerns and relationships between areas\n\n\
             ## Partitioning Guidelines\n\
             - Partition by top-level directories (e.g., src/, lib/, tests/, docs/)\n\
             - For monorepos: partition by package/crate/workspace member\n\
             - For frontend+backend: separate by technology boundary\n\
             - For large directories: split into 2-3 sub-tasks by subdirectory\n\
             - Aim for 3-6 parallel Explore tasks (not too few, not too many)\n\n\
             ## Rules\n\
             - ALWAYS discover the project structure FIRST — never assume directory names\n\
             - Emit ALL independent Task calls in a SINGLE response for parallel execution\n\
             - Use 'explore' for reading code, 'plan' for design, 'bash' for commands\n\
             - Do NOT over-delegate — handle simple operations directly\n\n\
             ## Output Rules\n\
             - Summarize sub-agent reports, don't repeat them verbatim\n\
             - Focus on insights and connections, not raw data"
                .to_string()
        }
        SubAgentType::Bash => {
            "Execute the requested shell commands. Report stdout/stderr and exit codes.\n\n\
             ## Output Format\n\
             Report command output concisely. Include exit codes for non-zero results."
                .to_string()
        }
    };

    // Append language instruction
    if detected_language.as_deref() == Some("zh") {
        prompt.push_str(
            "\n\nIMPORTANT: Respond in Chinese (简体中文). Only use English for code and tool parameters.",
        );
    }

    prompt
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
        let compactor = build_compactor(&provider);

        Self {
            config,
            provider,
            tool_executor,
            compactor,
            cancellation_token: CancellationToken::new(),
            paused: Arc::new(AtomicBool::new(false)),
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
            composer_registry: None,
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
        let compactor = build_compactor(&provider);

        Self {
            config,
            provider,
            tool_executor,
            compactor,
            cancellation_token,
            paused: Arc::new(AtomicBool::new(false)),
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
            composer_registry: None,
        }
    }

    /// Create a sub-agent orchestrator that shares the parent's read cache, index store,
    /// embedding service, and embedding manager. This avoids redundant file reads and
    /// enables CodebaseSearch in sub-agents.
    pub(super) fn new_sub_agent_with_shared_state(
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
        detected_language: Option<String>,
        skills_snapshot: Vec<crate::services::skills::model::SkillMatch>,
        memories_snapshot: Vec<crate::services::memory::store::MemoryEntry>,
        knowledge_block_snapshot: Option<String>,
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

        let compactor = build_compactor(&provider);

        // Wrap non-empty snapshots in Arc<RwLock<...>> so the sub-agent's
        // prompt builder can read them through the same field types as the parent.
        let selected_skills = if skills_snapshot.is_empty() {
            None
        } else {
            Some(Arc::new(RwLock::new(skills_snapshot)))
        };
        let loaded_memories = if memories_snapshot.is_empty() {
            None
        } else {
            Some(Arc::new(RwLock::new(memories_snapshot)))
        };

        Self {
            config,
            provider,
            tool_executor,
            compactor,
            cancellation_token,
            paused: Arc::new(AtomicBool::new(false)),
            db_pool: None,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            analysis_store: AnalysisRunStore::new(analysis_artifacts_root),
            index_store: shared_index_store,
            detected_language: Mutex::new(detected_language),
            hooks: crate::services::orchestrator::hooks::AgenticHooks::new(),
            selected_skills,
            loaded_memories,
            knowledge_context: None,
            knowledge_context_config: KnowledgeContextConfig::default(),
            cached_knowledge_block: Mutex::new(knowledge_block_snapshot),
            composer_registry: None,
        }
    }

    /// Set the index store for project summary injection into the system prompt.
    /// Also wires the store to the tool executor so CodebaseSearch works.
    pub fn with_index_store(mut self, store: Arc<IndexStore>) -> Self {
        self.tool_executor.set_index_store(Arc::clone(&store));
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

    /// Set the composer registry for agent transfer support.
    ///
    /// When configured, the agentic loop can transfer execution to named agents
    /// via the `TransferHandler` when `apply_actions` returns a `transfer_target`.
    pub fn with_composer_registry(mut self, registry: Arc<ComposerRegistry>) -> Self {
        self.composer_registry = Some(registry);
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

    /// Pause the agentic loop. The loop will sleep-poll until unpaused or cancelled.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// Resume a paused agentic loop.
    pub fn unpause(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    /// Check if execution is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
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
        let tools = get_tool_definitions_from_registry();

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
