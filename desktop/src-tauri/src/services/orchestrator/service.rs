//! Orchestrator Service
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with persistence, cancellation, and progress tracking.

use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::orchestrator::{
    ExecutionProgress, ExecutionSession, ExecutionSessionSummary, ExecutionStatus,
    StoryExecutionState,
};
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, FallbackToolFormatMode, GlmProvider, LlmProvider,
    LlmRequestOptions, LlmResponse, Message, MessageContent, OllamaProvider, OpenAIProvider,
    ProviderConfig, ProviderType, QwenProvider, ToolCallMode, ToolDefinition, UsageStats,
};
use crate::services::quality_gates::run_quality_gates as execute_quality_gates;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::{
    build_system_prompt, build_tool_call_instructions, extract_text_without_tool_calls,
    format_tool_result, get_basic_tool_definitions, get_tool_definitions, merge_system_prompts,
    parse_tool_calls, ParsedToolCall, TaskContext, TaskExecutionResult, TaskSpawner, ToolExecutor,
};
use crate::utils::error::{AppError, AppResult};

/// Configuration for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// LLM provider configuration
    pub provider: ProviderConfig,
    /// System prompt for the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Maximum iterations before stopping (prevents infinite loops)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Maximum total tokens to use
    #[serde(default = "default_max_tokens")]
    pub max_total_tokens: u32,
    /// Project root directory
    pub project_root: PathBuf,
    /// Whether to enable streaming
    #[serde(default = "default_streaming")]
    pub streaming: bool,
    /// Whether to enable automatic context compaction when input tokens exceed threshold
    #[serde(default = "default_enable_compaction")]
    pub enable_compaction: bool,
}

fn default_max_iterations() -> u32 {
    50
}

fn default_max_tokens() -> u32 {
    1_000_000
}

fn default_streaming() -> bool {
    true
}

fn default_enable_compaction() -> bool {
    true
}

/// Compute a reasonable token budget for sub-agents based on the model's context window.
///
/// Sub-agents do multiple iterations, each re-sending the full conversation. The total
/// tokens consumed across all iterations is typically 2-3x the context window for a
/// productive 6-10 iteration session. We use `context_window * 2` as the budget,
/// capped at a sensible maximum.
fn sub_agent_token_budget(context_window: u32) -> u32 {
    // Budget = 2x context window, allowing ~6-10 iterations with growing conversation.
    // Minimum 20k (even tiny models need some room), maximum 500k.
    (context_window * 2).clamp(20_000, 500_000)
}

/// Result of an orchestration execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Final response from the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// Total usage across all iterations
    pub usage: UsageStats,
    /// Number of iterations performed
    pub iterations: u32,
    /// Whether execution completed successfully
    pub success: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Session-based execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExecutionResult {
    /// Session ID
    pub session_id: String,
    /// Overall success
    pub success: bool,
    /// Number of completed stories
    pub completed_stories: usize,
    /// Number of failed stories
    pub failed_stories: usize,
    /// Total stories
    pub total_stories: usize,
    /// Total usage
    pub usage: UsageStats,
    /// Error message if session failed
    pub error: Option<String>,
    /// Quality gates summary (if run)
    pub quality_gates_passed: Option<bool>,
}

/// Orchestrator service for standalone LLM execution
pub struct OrchestratorService {
    config: OrchestratorConfig,
    provider: Arc<dyn LlmProvider>,
    tool_executor: ToolExecutor,
    cancellation_token: CancellationToken,
    /// Database pool for session persistence
    db_pool: Option<Pool<SqliteConnectionManager>>,
    /// Active sessions (in-memory cache)
    active_sessions: Arc<RwLock<HashMap<String, ExecutionSession>>>,
}

/// Task spawner that creates sub-agent OrchestratorService instances
struct OrchestratorTaskSpawner {
    provider_config: ProviderConfig,
    project_root: PathBuf,
    context_window: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisPhase {
    StructureDiscovery,
    ArchitectureTrace,
    ConsistencyCheck,
}

impl AnalysisPhase {
    fn id(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "structure_discovery",
            AnalysisPhase::ArchitectureTrace => "architecture_trace",
            AnalysisPhase::ConsistencyCheck => "consistency_check",
        }
    }

    fn title(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "Structure Discovery",
            AnalysisPhase::ArchitectureTrace => "Architecture Trace",
            AnalysisPhase::ConsistencyCheck => "Consistency Check",
        }
    }

    fn objective(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => {
                "Enumerate real project structure and verify manifests/entrypoints."
            }
            AnalysisPhase::ArchitectureTrace => {
                "Trace major modules, data flow, and integration boundaries using concrete files."
            }
            AnalysisPhase::ConsistencyCheck => {
                "Verify claims against file reads and grep results; explicitly mark unknowns."
            }
        }
    }

    fn task_type(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "explore",
            AnalysisPhase::ArchitectureTrace => "analyze",
            AnalysisPhase::ConsistencyCheck => "analyze",
        }
    }

    fn max_iterations(self) -> u32 {
        match self {
            AnalysisPhase::StructureDiscovery => 35,
            AnalysisPhase::ArchitectureTrace => 50,
            AnalysisPhase::ConsistencyCheck => 28,
        }
    }
}

#[derive(Debug, Clone)]
struct AnalysisToolQuota {
    min_total_calls: usize,
    min_read_calls: usize,
    min_search_calls: usize,
    required_tools: Vec<&'static str>,
}

#[derive(Debug, Clone)]
struct AnalysisPhasePolicy {
    max_attempts: u32,
    force_tool_mode_attempts: u32,
    temperature_override: f32,
    quota: AnalysisToolQuota,
}

impl AnalysisPhasePolicy {
    fn for_phase(phase: AnalysisPhase) -> Self {
        match phase {
            AnalysisPhase::StructureDiscovery => Self {
                max_attempts: 3,
                force_tool_mode_attempts: 2,
                temperature_override: 0.1,
                quota: AnalysisToolQuota {
                    min_total_calls: 4,
                    min_read_calls: 1,
                    min_search_calls: 1,
                    required_tools: vec!["Cwd", "LS"],
                },
            },
            AnalysisPhase::ArchitectureTrace => Self {
                max_attempts: 3,
                force_tool_mode_attempts: 2,
                temperature_override: 0.1,
                quota: AnalysisToolQuota {
                    min_total_calls: 5,
                    min_read_calls: 2,
                    min_search_calls: 2,
                    required_tools: vec!["Grep"],
                },
            },
            AnalysisPhase::ConsistencyCheck => Self {
                max_attempts: 3,
                force_tool_mode_attempts: 2,
                temperature_override: 0.1,
                quota: AnalysisToolQuota {
                    min_total_calls: 3,
                    min_read_calls: 1,
                    min_search_calls: 1,
                    required_tools: vec!["Read", "Grep"],
                },
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PhaseCapture {
    tool_calls: usize,
    read_calls: usize,
    grep_calls: usize,
    glob_calls: usize,
    ls_calls: usize,
    cwd_calls: usize,
    observed_paths: HashSet<String>,
    evidence_lines: Vec<String>,
    warnings: Vec<String>,
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
    success: bool,
    error: Option<String>,
    capture: PhaseCapture,
}

#[derive(Debug, Clone, Default)]
struct AnalysisLedger {
    observed_paths: HashSet<String>,
    evidence_lines: Vec<String>,
    warnings: Vec<String>,
    phase_summaries: Vec<String>,
    successful_phases: usize,
}

impl AnalysisLedger {
    fn record(&mut self, outcome: &AnalysisPhaseOutcome) {
        if outcome.success {
            self.successful_phases += 1;
        } else if let Some(err) = outcome.error.as_ref() {
            self.warnings
                .push(format!("{} failed: {}", outcome.phase.title(), err));
        }

        self.observed_paths
            .extend(outcome.capture.observed_paths.iter().cloned());

        self.evidence_lines
            .extend(outcome.capture.evidence_lines.iter().cloned());
        self.warnings
            .extend(outcome.capture.warnings.iter().cloned());

        if let Some(summary) = outcome.response.as_ref() {
            let trimmed = summary.trim();
            if !trimmed.is_empty() {
                self.phase_summaries.push(format!(
                    "## {} ({})\n{}",
                    outcome.phase.title(),
                    outcome.phase.id(),
                    trimmed
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
            max_total_tokens: sub_agent_token_budget(self.context_window),
            project_root: self.project_root.clone(),
            streaming: true,
            enable_compaction: false, // Sub-agents are short-lived, no compaction needed
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

impl OrchestratorService {
    /// Create a new orchestrator service
    pub fn new(config: OrchestratorConfig) -> Self {
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
        }
    }

    /// Create a sub-agent orchestrator (no Task tool, no database, inherits cancellation)
    fn new_sub_agent(config: OrchestratorConfig, cancellation_token: CancellationToken) -> Self {
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
        }
    }

    /// Set the database pool for session persistence
    pub fn with_database(mut self, pool: Pool<SqliteConnectionManager>) -> Self {
        // Initialize schema
        if let Err(e) = self.init_session_schema(&pool) {
            eprintln!("Failed to initialize session schema: {}", e);
        }
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

    async fn execute_story_with_request_options(
        &self,
        prompt: &str,
        tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
        request_options: LlmRequestOptions,
        force_prompt_fallback: bool,
    ) -> ExecutionResult {
        let use_prompt_fallback = force_prompt_fallback || !self.provider.supports_tools();
        let mut messages = vec![Message::user(prompt.to_string())];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;
        let mut fallback_call_counter = 0u32;

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

            // For prompt-fallback providers, add tool call format instructions
            if use_prompt_fallback
                || !matches!(
                    request_options.fallback_tool_format_mode,
                    FallbackToolFormatMode::Off
                )
            {
                parts.push(build_tool_call_instructions(tools));
                if matches!(
                    request_options.fallback_tool_format_mode,
                    FallbackToolFormatMode::Strict
                ) {
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
                let error_msg = format!(
                    "Maximum iterations ({}) reached",
                    self.config.max_iterations
                );
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("max_iterations".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("max_iterations".to_string()),
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

            // Check for context compaction before processing tool calls
            if self.should_compact(last_input_tokens) {
                self.compact_messages(&mut messages, &tx).await;
            }

            // Handle tool calls - either native or prompt-based fallback
            let has_native_tool_calls = response.has_tool_calls();
            let parsed_fallback_calls = if !has_native_tool_calls {
                parse_fallback_tool_calls(&response)
            } else {
                Vec::new()
            };

            if has_native_tool_calls {
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
                    // Emit tool start event
                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            arguments: Some(tc.arguments.to_string()),
                        })
                        .await;

                    // Execute the tool
                    let result = self.tool_executor.execute(&tc.name, &tc.arguments).await;

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
                                    text: result.to_content(),
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
                                result.to_content(),
                                !result.success,
                            ));
                        }
                    } else {
                        messages.push(Message::tool_result(
                            &tc.id,
                            result.to_content(),
                            !result.success,
                        ));
                    }
                }
            } else if !parsed_fallback_calls.is_empty() {
                // Prompt-based fallback path
                if let Some(text) = &response.content {
                    let cleaned = extract_text_without_tool_calls(text);
                    if !cleaned.is_empty() {
                        messages.push(Message::assistant(cleaned));
                    }
                }

                // Execute each parsed tool call and collect results
                let mut tool_results = Vec::new();
                for ptc in &parsed_fallback_calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("story_fallback_{}", fallback_call_counter);

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tool_id.clone(),
                            tool_name: ptc.tool_name.clone(),
                            arguments: Some(ptc.arguments.to_string()),
                        })
                        .await;

                    let result = self
                        .tool_executor
                        .execute(&ptc.tool_name, &ptc.arguments)
                        .await;

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
                        &ptc.tool_name,
                        &tool_id,
                        &result.to_content(),
                        !result.success,
                    ));
                }

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
            } else {
                // No tool calls (native or fallback) - this is the final response
                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t));

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
        // Auto-delegate exploration tasks to sub-agents to prevent context overflow.
        // Non-Claude LLMs often read files directly instead of using the Task tool,
        // filling up the token budget after ~20 files.
        if is_exploration_task(&message) {
            return self.execute_with_analysis_pipeline(message, tx).await;
        }

        let tools = get_tool_definitions();
        let use_prompt_fallback = !self.provider.supports_tools();
        let mut messages = vec![Message::user(message)];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;
        let mut fallback_call_counter = 0u32;

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
                let error_msg = format!(
                    "Maximum iterations ({}) reached",
                    self.config.max_iterations
                );
                let _ = tx
                    .send(UnifiedStreamEvent::Error {
                        message: error_msg.clone(),
                        code: Some("max_iterations".to_string()),
                    })
                    .await;
                let _ = tx
                    .send(UnifiedStreamEvent::Complete {
                        stop_reason: Some("max_iterations".to_string()),
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

            // Check for context compaction before processing tool calls
            if self.should_compact(last_input_tokens) {
                self.compact_messages(&mut messages, &tx).await;
            }

            // Handle tool calls - either native or prompt-based fallback
            let has_native_tool_calls = response.has_tool_calls();
            let parsed_fallback_calls = if !has_native_tool_calls {
                // Check both assistant text and thinking content for prompt-based tool calls.
                parse_fallback_tool_calls(&response)
            } else {
                Vec::new()
            };

            if has_native_tool_calls {
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

                    let result = self
                        .tool_executor
                        .execute_with_context(&tc.name, &tc.arguments, Some(&task_ctx))
                        .await;

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
                                    text: result.to_content(),
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
                                result.to_content(),
                                !result.success,
                            ));
                        }
                    } else {
                        messages.push(Message::tool_result(
                            &tc.id,
                            result.to_content(),
                            !result.success,
                        ));
                    }
                }
            } else if !parsed_fallback_calls.is_empty() {
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
                for ptc in &parsed_fallback_calls {
                    fallback_call_counter += 1;
                    let tool_id = format!("fallback_{}", fallback_call_counter);

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolStart {
                            tool_id: tool_id.clone(),
                            tool_name: ptc.tool_name.clone(),
                            arguments: Some(ptc.arguments.to_string()),
                        })
                        .await;

                    let result = self
                        .tool_executor
                        .execute_with_context(&ptc.tool_name, &ptc.arguments, Some(&task_ctx))
                        .await;

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
                        &ptc.tool_name,
                        &tool_id,
                        &result.to_content(),
                        !result.success,
                    ));
                }

                // Feed all tool results back as a user message
                let combined_results = tool_results.join("\n\n");
                messages.push(Message::user(combined_results));
            } else {
                // No tool calls (native or fallback) - this is the final response
                // Always strip any tool call blocks from the response text,
                // since models may emit text-based tool calls even when using native mode
                let final_content = response
                    .content
                    .as_ref()
                    .map(|t| extract_text_without_tool_calls(t));

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

    /// Execute an analysis task with an evidence-first multi-phase pipeline.
    async fn execute_with_analysis_pipeline(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let mut total_usage = UsageStats::default();
        let mut total_iterations = 0;
        let mut ledger = AnalysisLedger::default();

        let phase1_prompt = format!(
            "User request: {}\n\n\
             Run a strict structure discovery pass. Identify the real repository shape,\n\
             read primary manifests, and list true entrypoints with file paths.",
            message
        );
        let phase1 = self
            .run_analysis_phase(AnalysisPhase::StructureDiscovery, phase1_prompt, &tx)
            .await;
        merge_usage(&mut total_usage, &phase1.usage);
        total_iterations += phase1.iterations;
        ledger.record(&phase1);

        if self.cancellation_token.is_cancelled() {
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let structure_summary = phase1
            .response
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "Structure discovery produced no summary.".to_string());
        let observed_from_phase1 = join_sorted_paths(&ledger.observed_paths, 120);

        let phase2_prompt = format!(
            "User request: {}\n\n\
             Structure summary from previous phase:\n{}\n\n\
             Observed paths so far:\n{}\n\n\
             Build a concrete architecture trace from real files. If a component cannot be verified\n\
             from tools, label it as unknown.",
            message, structure_summary, observed_from_phase1
        );
        let phase2 = self
            .run_analysis_phase(AnalysisPhase::ArchitectureTrace, phase2_prompt, &tx)
            .await;
        merge_usage(&mut total_usage, &phase2.usage);
        total_iterations += phase2.iterations;
        ledger.record(&phase2);

        if self.cancellation_token.is_cancelled() {
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let architecture_summary = phase2
            .response
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "Architecture trace produced no summary.".to_string());
        let phase3_prompt = format!(
            "User request: {}\n\n\
             Verify these findings and explicitly mark uncertain claims.\n\n\
             Structure summary:\n{}\n\n\
             Architecture summary:\n{}\n\n\
             Observed paths:\n{}\n\n\
             Output must include:\n\
             - Verified claims (with path evidence)\n\
             - Unverified claims (and why)\n\
             - Contradictions or missing data",
            message,
            structure_summary,
            architecture_summary,
            join_sorted_paths(&ledger.observed_paths, 160)
        );
        let phase3 = self
            .run_analysis_phase(AnalysisPhase::ConsistencyCheck, phase3_prompt, &tx)
            .await;
        merge_usage(&mut total_usage, &phase3.usage);
        total_iterations += phase3.iterations;
        ledger.record(&phase3);

        if self.cancellation_token.is_cancelled() {
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        let all_phases_passed = ledger.successful_phases == 3;
        let has_evidence = !ledger.evidence_lines.is_empty();
        if !all_phases_passed || !has_evidence {
            let mut failures = Vec::new();
            if !all_phases_passed {
                failures.push(format!(
                    "Phase gate failed: {}/3 phases passed",
                    ledger.successful_phases
                ));
            }
            if !has_evidence {
                failures.push("Evidence gate failed: no tool evidence captured".to_string());
            }

            let _ = tx
                .send(UnifiedStreamEvent::AnalysisRunSummary {
                    success: false,
                    phase_results: vec![
                        format!("successful_phases={}", ledger.successful_phases),
                        format!("observed_paths={}", ledger.observed_paths.len()),
                    ],
                    total_metrics: serde_json::json!({
                        "input_tokens": total_usage.input_tokens,
                        "output_tokens": total_usage.output_tokens,
                        "iterations": total_iterations,
                        "evidence_lines": ledger.evidence_lines.len(),
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

            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: total_iterations,
                success: false,
                error: Some("Analysis failed: insufficient verified evidence".to_string()),
            };
        }

        let evidence_block = if ledger.evidence_lines.is_empty() {
            "- No tool evidence captured.".to_string()
        } else {
            ledger
                .evidence_lines
                .iter()
                .take(220)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };
        let summary_block = if ledger.phase_summaries.is_empty() {
            "No phase summaries were produced.".to_string()
        } else {
            ledger.phase_summaries.join("\n\n")
        };
        let warnings_block = if ledger.warnings.is_empty() {
            "None".to_string()
        } else {
            ledger.warnings.join("\n")
        };
        let observed_paths = join_sorted_paths(&ledger.observed_paths, 250);

        let synthesis_prompt = format!(
            "You are synthesizing a repository analysis from verified tool evidence.\n\n\
             User request:\n{}\n\n\
             Observed paths (ground truth):\n{}\n\n\
             Warnings collected:\n{}\n\n\
             Phase summaries:\n{}\n\n\
             Evidence log:\n{}\n\n\
             Requirements:\n\
             1) Use only the evidence above.\n\
             2) Do not invent files, modules, frameworks, versions, or runtime details.\n\
             3) If a claim is uncertain, place it under 'Unknowns'.\n\
             4) Include explicit file paths for major claims.\n\
             5) Output markdown with sections: Project Snapshot, Architecture, Verified Facts, Risks, Unknowns.",
            message, observed_paths, warnings_block, summary_block, evidence_block
        );

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
                let fallback = format!(
                    "{}\n\n{}\n\n{}",
                    structure_summary,
                    architecture_summary,
                    phase3.response.unwrap_or_default()
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

        if validation_issues.is_empty() {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisValidation {
                    status: "ok".to_string(),
                    issues: Vec::new(),
                })
                .await;
        } else {
            let _ = tx
                .send(UnifiedStreamEvent::AnalysisValidation {
                    status: "warning".to_string(),
                    issues: validation_issues.clone(),
                })
                .await;

            if let Some(original) = final_response.clone() {
                let correction_prompt = format!(
                    "Revise this analysis to remove or mark these path claims as unverified:\n{}\n\n\
                     Observed paths:\n{}\n\n\
                     Original analysis:\n{}",
                    validation_issues.join("\n"),
                    join_sorted_paths(&ledger.observed_paths, 250),
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

        let final_success = all_phases_passed
            && has_evidence
            && final_response
                .as_ref()
                .map(|text| !text.trim().is_empty())
                .unwrap_or(false)
            && synthesis_success;

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisRunSummary {
                success: final_success,
                phase_results: vec![
                    format!("successful_phases={}", ledger.successful_phases),
                    format!("observed_paths={}", ledger.observed_paths.len()),
                    format!("validation_issues={}", validation_issues.len()),
                ],
                total_metrics: serde_json::json!({
                    "input_tokens": total_usage.input_tokens,
                    "output_tokens": total_usage.output_tokens,
                    "iterations": total_iterations,
                    "evidence_lines": ledger.evidence_lines.len(),
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

    async fn run_analysis_phase(
        &self,
        phase: AnalysisPhase,
        prompt: String,
        tx: &mpsc::Sender<UnifiedStreamEvent>,
    ) -> AnalysisPhaseOutcome {
        let phase_id = phase.id().to_string();
        let policy = AnalysisPhasePolicy::for_phase(phase);
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

        let tools = get_basic_tool_definitions();
        let mut total_usage = UsageStats::default();
        let mut total_iterations = 0u32;
        let mut aggregate_capture = PhaseCapture::default();
        let mut final_response: Option<String> = None;
        let mut final_error: Option<String> = None;
        let mut phase_success = false;
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

            let phase_config = OrchestratorConfig {
                provider: self.config.provider.clone(),
                system_prompt: Some(analysis_phase_system_prompt_with_quota(
                    phase,
                    &policy.quota,
                    &gate_failure_history,
                )),
                max_iterations: phase.max_iterations(),
                max_total_tokens: sub_agent_token_budget(self.provider.context_window()),
                project_root: self.config.project_root.clone(),
                streaming: true,
                enable_compaction: false,
            };
            let phase_agent =
                OrchestratorService::new_sub_agent(phase_config, self.cancellation_token.clone());

            let request_options = LlmRequestOptions {
                tool_call_mode: if attempt <= policy.force_tool_mode_attempts {
                    ToolCallMode::Required
                } else {
                    ToolCallMode::Auto
                },
                fallback_tool_format_mode: FallbackToolFormatMode::Strict,
                temperature_override: Some(policy.temperature_override),
                reasoning_effort_override: None,
                analysis_phase: Some(phase.id().to_string()),
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
                .evidence_lines
                .extend(attempt_capture.evidence_lines.iter().cloned());
            aggregate_capture
                .warnings
                .extend(attempt_capture.warnings.iter().cloned());

            let gate_failures = evaluate_analysis_quota(&attempt_capture, &policy.quota);
            let attempt_success = attempt_result.success && gate_failures.is_empty();
            final_response = attempt_result.response.clone().or(final_response);
            if !attempt_success {
                if let Some(err) = attempt_result.error.as_ref() {
                    gate_failure_history.push(format!("attempt {} error: {}", attempt, err));
                }
                gate_failure_history.extend(gate_failures.iter().cloned());
            }

            let attempt_metrics = serde_json::json!({
                "tool_calls": attempt_capture.tool_calls,
                "read_calls": attempt_capture.read_calls,
                "grep_calls": attempt_capture.grep_calls,
                "glob_calls": attempt_capture.glob_calls,
                "ls_calls": attempt_capture.ls_calls,
                "cwd_calls": attempt_capture.cwd_calls,
                "observed_paths": attempt_capture.observed_paths.len(),
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
                phase_success = true;
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

        if !phase_success && final_error.is_none() {
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
        });
        let usage = serde_json::json!({
            "input_tokens": total_usage.input_tokens,
            "output_tokens": total_usage.output_tokens,
            "iterations": total_iterations,
        });

        let _ = tx
            .send(UnifiedStreamEvent::AnalysisPhaseEnd {
                phase_id: phase_id.clone(),
                success: phase_success,
                usage: usage.clone(),
                metrics,
            })
            .await;
        let _ = tx
            .send(UnifiedStreamEvent::SubAgentEnd {
                sub_agent_id: phase_id,
                success: phase_success,
                usage,
            })
            .await;

        AnalysisPhaseOutcome {
            phase,
            response: final_response,
            usage: total_usage,
            iterations: total_iterations,
            success: phase_success,
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
                tool_name,
                arguments,
                ..
            } => {
                capture.tool_calls += 1;
                match tool_name.as_str() {
                    "Read" => capture.read_calls += 1,
                    "Grep" => capture.grep_calls += 1,
                    "Glob" => capture.glob_calls += 1,
                    "LS" => capture.ls_calls += 1,
                    "Cwd" => capture.cwd_calls += 1,
                    _ => {}
                }

                let args_json = parse_tool_arguments(arguments);
                let primary_path = args_json
                    .as_ref()
                    .and_then(extract_primary_path_from_arguments);
                if let Some(path) = primary_path.as_ref() {
                    capture.observed_paths.insert(path.clone());
                }
                if let Some(args) = args_json.as_ref() {
                    for p in extract_all_paths_from_arguments(args) {
                        capture.observed_paths.insert(p);
                    }
                }

                let summary =
                    summarize_tool_activity(tool_name, args_json.as_ref(), primary_path.as_deref());
                if capture.evidence_lines.len() < 220 {
                    capture
                        .evidence_lines
                        .push(format!("- [{}] {}", phase.id(), summary));
                }

                let _ = tx
                    .send(UnifiedStreamEvent::AnalysisEvidence {
                        phase_id: phase.id().to_string(),
                        tool_name: tool_name.clone(),
                        file_path: primary_path,
                        summary,
                        success: None,
                    })
                    .await;
            }
            UnifiedStreamEvent::ToolResult { error, .. } => {
                if let Some(err) = error.as_ref() {
                    let compact_err = truncate_for_log(err, 180);
                    capture
                        .warnings
                        .push(format!("{} tool error: {}", phase.title(), compact_err));
                    let _ = tx
                        .send(UnifiedStreamEvent::AnalysisEvidence {
                            phase_id: phase.id().to_string(),
                            tool_name: "tool_result".to_string(),
                            file_path: None,
                            summary: format!("Tool error: {}", compact_err),
                            success: Some(false),
                        })
                        .await;
                }
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
    fn should_compact(&self, last_input_tokens: u32) -> bool {
        if !self.config.enable_compaction {
            return false;
        }
        let threshold = (self.config.max_total_tokens as f64 * 0.6) as u32;
        last_input_tokens > threshold
    }

    /// Compact conversation messages by summarizing older messages while preserving recent ones.
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

        // Preserve the first message (original prompt) and last 6 messages (recent context)
        let preserved_tail_count = 6;
        let first_msg = messages[0].clone();
        let compact_range_end = messages.len() - preserved_tail_count;

        // Nothing to compact if range is too small
        if compact_range_end <= 1 {
            return false;
        }

        let messages_to_compact = &messages[1..compact_range_end];
        let messages_compacted_count = messages_to_compact.len();

        // Extract summary information from messages being compacted
        let mut tool_names: Vec<String> = Vec::new();
        let mut file_paths: Vec<String> = Vec::new();
        let mut conversation_snippets: Vec<String> = Vec::new();

        for msg in messages_to_compact {
            for content in &msg.content {
                match content {
                    MessageContent::Text { text } => {
                        let snippet = if text.len() > 500 {
                            format!("{}...", &text[..500])
                        } else {
                            text.clone()
                        };
                        conversation_snippets.push(snippet);
                    }
                    MessageContent::ToolUse { name, .. } => {
                        if !tool_names.contains(name) {
                            tool_names.push(name.clone());
                        }
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
                        let snippet = if content.len() > 500 {
                            format!("{}...", &content[..500])
                        } else {
                            content.clone()
                        };
                        conversation_snippets.push(snippet);
                    }
                    MessageContent::ToolResultMultimodal {
                        content: blocks, ..
                    } => {
                        for block in blocks {
                            if let crate::services::llm::types::ContentBlock::Text { text } = block
                            {
                                let snippet = if text.len() > 500 {
                                    format!("{}...", &text[..500])
                                } else {
                                    text.clone()
                                };
                                conversation_snippets.push(snippet);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

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

                // Build new message list: original prompt + summary + preserved tail
                let preserved_tail: Vec<Message> = messages[compact_range_end..].to_vec();
                let summary_msg = Message::user(format!(
                    "[Context Summary - {} earlier messages compacted]\n\n{}",
                    messages_compacted_count, summary_text
                ));

                messages.clear();
                messages.push(first_msg);
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

                eprintln!(
                    "[compaction] Compacted {} messages, preserved {}, summary {} tokens",
                    messages_compacted_count, preserved_tail_count, compaction_tokens
                );

                true
            }
            Err(e) => {
                eprintln!("[compaction] Failed to compact messages: {}", e);
                false
            }
        }
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

        let mut prompt = build_system_prompt(&self.config.project_root, prompt_tools);

        // For providers that don't support native tool calling,
        // add prompt-based tool call format instructions
        if !self.provider.supports_tools()
            || !matches!(
                request_options.fallback_tool_format_mode,
                FallbackToolFormatMode::Off
            )
        {
            let fallback_instructions = build_tool_call_instructions(prompt_tools);
            prompt = if matches!(
                request_options.fallback_tool_format_mode,
                FallbackToolFormatMode::Strict
            ) {
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

fn parse_fallback_tool_calls(response: &LlmResponse) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();
    let mut seen = HashSet::new();

    for text in [response.content.as_deref(), response.thinking.as_deref()]
        .into_iter()
        .flatten()
    {
        for call in parse_tool_calls(text) {
            let signature = format!("{}:{}", call.tool_name, call.arguments);
            if seen.insert(signature) {
                calls.push(call);
            }
        }
    }

    calls
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

fn analysis_phase_system_prompt(phase: AnalysisPhase) -> &'static str {
    match phase {
        AnalysisPhase::StructureDiscovery => {
            "You are a repository structure investigator.\n\
             You must do all work directly with tools (Cwd, LS, Glob, Read, Grep).\n\
             Do not delegate to Task or any sub-agent.\n\n\
             Required workflow:\n\
             1) Call Cwd and LS on repository root.\n\
             2) Discover manifests/configs with Glob (json/toml/yaml/md).\n\
             3) Read key manifests (package.json, Cargo.toml, pyproject.toml, README* when present).\n\
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
    let search_calls = capture.search_calls();
    if search_calls < quota.min_search_calls {
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

fn join_sorted_paths(paths: &HashSet<String>, limit: usize) -> String {
    if paths.is_empty() {
        return "(none)".to_string();
    }
    let mut items: Vec<String> = paths.iter().cloned().collect();
    items.sort();
    items.into_iter().take(limit).collect::<Vec<_>>().join("\n")
}

fn parse_tool_arguments(arguments: &Option<String>) -> Option<serde_json::Value> {
    arguments
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
}

fn truncate_for_log(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    format!("{}...", &text[..limit])
}

fn normalize_candidate_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return None;
    }

    let normalized = trimmed
        .trim_matches(|c: char| "\"'`[](){}<>,".contains(c))
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();

    if normalized.is_empty() {
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
            || normalized.ends_with(known)
            || normalized.starts_with(known)
    })
}

fn find_unverified_paths(text: &str, observed: &HashSet<String>) -> Vec<String> {
    extract_path_candidates_from_text(text)
        .into_iter()
        .filter(|path| !is_observed_path(path, observed))
        .collect()
}

#[allow(dead_code)]
// Legacy heuristic kept for regression comparison in tests/debugging.
fn is_exploration_task_legacy(message: &str) -> bool {
    let lower = message.to_lowercase();

    // Chinese exploration keywords
    const ZH_KEYWORDS: &[&str] = &[
        "分析", "探索", "查看", "了解", "解释", "理解", "审查", "研究", "概述", "总结", "结构",
    ];

    // English exploration keywords
    const EN_KEYWORDS: &[&str] = &[
        "analyze",
        "analyse",
        "explore",
        "examine",
        "understand",
        "explain",
        "review",
        "investigate",
        "overview",
        "summarize",
        "summarise",
        "structure",
        "architecture",
        "codebase",
        "walk through",
        "walkthrough",
    ];

    // Must reference the project / codebase / code in some way, not just contain a keyword.
    // For Chinese, keywords alone are strong enough signals (e.g. "分析这个项目").
    let has_zh = ZH_KEYWORDS.iter().any(|kw| lower.contains(kw));
    if has_zh {
        return true;
    }

    let has_en_keyword = EN_KEYWORDS.iter().any(|kw| lower.contains(kw));
    if !has_en_keyword {
        return false;
    }

    // Require a project/code context word to avoid false positives on e.g. "explain how JWT works"
    const CONTEXT_WORDS: &[&str] = &[
        "project",
        "codebase",
        "code base",
        "repo",
        "repository",
        "source",
        "code",
        "this",
        "directory",
        "folder",
    ];
    CONTEXT_WORDS.iter().any(|cw| lower.contains(cw))
}

fn is_exploration_task(message: &str) -> bool {
    let lower = message.to_lowercase();

    const EN_ANALYSIS: &[&str] = &[
        "analyze",
        "analyse",
        "explore",
        "review",
        "investigate",
        "understand",
        "overview",
        "summarize",
        "summarise",
        "architecture",
        "codebase",
        "repository",
        "repo",
    ];
    const EN_CONTEXT: &[&str] = &[
        "project",
        "code",
        "codebase",
        "repository",
        "repo",
        "folder",
        "directory",
        "this",
    ];
    const EN_EXECUTION: &[&str] = &[
        "implement",
        "fix",
        "build",
        "write",
        "create",
        "add",
        "remove",
        "refactor",
    ];

    const ZH_ANALYSIS: &[&str] = &[
        "\u{5206}\u{6790}",
        "\u{63a2}\u{7d22}",
        "\u{4e86}\u{89e3}",
        "\u{603b}\u{7ed3}",
        "\u{67e5}\u{770b}",
        "\u{67b6}\u{6784}",
    ];
    const ZH_CONTEXT: &[&str] = &[
        "\u{9879}\u{76ee}",
        "\u{4ee3}\u{7801}",
        "\u{4ed3}\u{5e93}",
        "\u{76ee}\u{5f55}",
        "\u{8fd9}\u{4e2a}",
    ];
    const ZH_EXECUTION: &[&str] = &[
        "\u{5b9e}\u{73b0}",
        "\u{4fee}\u{590d}",
        "\u{65b0}\u{589e}",
        "\u{7f16}\u{5199}",
        "\u{91cd}\u{6784}",
    ];

    let has_zh_analysis = ZH_ANALYSIS.iter().any(|kw| message.contains(kw));
    let has_zh_context = ZH_CONTEXT.iter().any(|kw| message.contains(kw));
    let has_zh_execution = ZH_EXECUTION.iter().any(|kw| message.contains(kw));

    if has_zh_analysis && has_zh_context {
        return true;
    }
    if has_zh_execution && !has_zh_analysis {
        return false;
    }

    let has_en_analysis = EN_ANALYSIS.iter().any(|kw| lower.contains(kw));
    let has_en_context = EN_CONTEXT.iter().any(|kw| lower.contains(kw));
    let has_en_execution = EN_EXECUTION.iter().any(|kw| lower.contains(kw));

    if has_en_execution && !has_en_analysis {
        return false;
    }

    has_en_analysis && has_en_context
}

/// Parse an RFC3339 timestamp to Unix timestamp.
fn parse_timestamp(s: Option<String>) -> i64 {
    s.and_then(|ts| chrono::DateTime::parse_from_rfc3339(&ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp())
}

/// Information about the current provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub supports_thinking: bool,
    pub supports_tools: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_is_exploration_task_chinese() {
        assert!(is_exploration_task(
            "\u{5206}\u{6790}\u{8fd9}\u{4e2a}\u{9879}\u{76ee}"
        ));
        assert!(is_exploration_task(
            "\u{5e2e}\u{6211}\u{4e86}\u{89e3}\u{8fd9}\u{4e2a}\u{4ee3}\u{7801}\u{4ed3}\u{5e93}"
        ));
        assert!(is_exploration_task(
            "\u{603b}\u{7ed3}\u{4e00}\u{4e0b}\u{9879}\u{76ee}\u{67b6}\u{6784}"
        ));
        assert!(is_exploration_task(
            "\u{63a2}\u{7d22}\u{4ee3}\u{7801}\u{76ee}\u{5f55}"
        ));
    }

    #[test]
    fn test_is_exploration_task_english() {
        assert!(is_exploration_task("analyze this project"));
        assert!(is_exploration_task("Explore the codebase"));
        assert!(is_exploration_task(
            "give me an overview of this repository"
        ));
        assert!(is_exploration_task("summarize the repository architecture"));
        assert!(is_exploration_task("help me understand this codebase"));
    }

    #[test]
    fn test_is_exploration_task_false_positives() {
        // Should NOT match: no project/code context word
        assert!(!is_exploration_task("explain how JWT works"));
        assert!(!is_exploration_task("analyze the market trends"));
        // Should NOT match: implementation tasks
        assert!(!is_exploration_task("add a login button"));
        assert!(!is_exploration_task("fix the bug in checkout"));
        assert!(!is_exploration_task("write a test for the API"));
        assert!(!is_exploration_task("implement this endpoint"));
        assert!(!is_exploration_task(
            "\u{4fee}\u{590d}\u{767b}\u{5f55}\u{6309}\u{94ae}"
        ));
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

        let calls = parse_fallback_tool_calls(&response);
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|c| c.tool_name == "LS"));
        assert!(calls.iter().any(|c| c.tool_name == "Read"));
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
}
