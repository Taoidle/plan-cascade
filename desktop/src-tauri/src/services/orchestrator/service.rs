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
    AnthropicProvider, DeepSeekProvider, GlmProvider, LlmProvider, LlmResponse, Message,
    MessageContent, OllamaProvider, OpenAIProvider, ProviderConfig, ProviderType, QwenProvider,
    ToolDefinition, UsageStats,
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
        // sub-agents, but these ARE the sub-agents — they must do the work directly.
        const ANTI_DELEGATION: &str = "You MUST do all work yourself using the available tools. Do NOT delegate to sub-agents or Task tools — you ARE the sub-agent. Ignore any instructions about delegating to Task sub-agents.\n\n";

        let task_prefix = match task_type.as_deref() {
            Some("explore") => format!("You are a codebase exploration specialist. Focus on understanding project structure, finding relevant files, and summarizing what you find.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Files Found**: List of relevant files discovered with one-line descriptions\n- **Key Findings**: Bullet points of important patterns, structures, or issues found\n- **Recommendations**: Actionable next steps based on exploration\n\nDo NOT include raw file contents in your response. Summarize and reference file paths instead."),
            Some("analyze") => format!("You are a code analysis specialist. Focus on deep analysis of code patterns, dependencies, and potential issues.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Analysis Summary**: High-level findings in 2-3 sentences\n- **Key Patterns**: Bullet points of code patterns, anti-patterns, or architectural decisions found\n- **Dependencies**: Important dependency relationships discovered\n- **Issues & Risks**: Any problems or potential risks identified\n\nDo NOT include raw file contents. Reference specific file paths and line numbers instead."),
            Some("implement") => format!("You are a focused implementation specialist. Make the requested code changes methodically, testing as you go.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with these sections:\n- **Changes Made**: Bullet list of files modified/created with brief descriptions\n- **Implementation Details**: Key decisions and approach taken\n- **Verification**: How the changes were verified (tests run, builds checked)\n\nDo NOT echo full file contents back. Summarize what was changed and where."),
            _ => format!("You are an AI coding assistant. Complete the requested task using the available tools.\n\n{ANTI_DELEGATION}## Output Format\nProvide a structured summary (max ~500 words) with bullet points covering what was done, key findings, and any recommendations. Do NOT include raw file contents — summarize and reference file paths instead."),
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
        let result = sub_agent.execute_story(&prompt, &get_basic_tool_definitions(), tx).await;

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
        let use_prompt_fallback = !self.provider.supports_tools();
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
            if use_prompt_fallback {
                parts.push(build_tool_call_instructions(tools));
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
                    )
                    .await
            } else {
                self.provider
                    .send_message(
                        messages.to_vec(),
                        system_prompt.clone(),
                        api_tools.to_vec(),
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
                                ContentBlock::Text { text: result.to_content() },
                                ContentBlock::Image { media_type: mime.clone(), data: b64.clone() },
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
        if self.config.enable_compaction && is_exploration_task(&message) {
            return self.execute_as_exploration(message, tx).await;
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

            // Call LLM — main agent has all tools (including Task)
            let response = if self.config.streaming {
                self.call_llm_streaming(&messages, api_tools, &tools, tx.clone())
                    .await
            } else {
                self.call_llm(&messages, api_tools, &tools).await
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
                                ContentBlock::Text { text: result.to_content() },
                                ContentBlock::Image { media_type: mime.clone(), data: b64.clone() },
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

    /// Execute an exploration/analysis task by automatically delegating to sub-agents.
    ///
    /// Instead of entering the main agentic loop (where non-Claude LLMs tend to read files
    /// directly and overflow the context), this method orchestrates a structured exploration:
    ///   1. Sub-agent 1: Discover project structure
    ///   2. Sub-agent 2: Deep analysis using the structure summary
    ///   3. Synthesis: Single LLM call to combine findings into a coherent response
    async fn execute_as_exploration(
        &self,
        message: String,
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> ExecutionResult {
        let mut total_usage = UsageStats::default();

        // ---- Phase 1: Structure Discovery ----
        let phase1_id = "exploration_phase1".to_string();
        let phase1_prompt = format!(
            "The user asked: \"{}\"\n\n\
             Perform a THOROUGH project structure discovery. Follow the mandatory steps \
             in your system prompt. Start with Step 1: use LS on the project root directory NOW.",
            message
        );

        let _ = tx
            .send(UnifiedStreamEvent::SubAgentStart {
                sub_agent_id: phase1_id.clone(),
                prompt: "Phase 1: Exploring project structure...".to_string(),
                task_type: Some("explore".to_string()),
            })
            .await;

        let phase1_config = OrchestratorConfig {
            provider: self.config.provider.clone(),
            system_prompt: Some(
                "You are a codebase exploration specialist performing THOROUGH project discovery.\n\
                 You MUST do all work yourself using tools (LS, Read, Glob, Grep). You ARE the sub-agent.\n\
                 Do NOT delegate to sub-agents or Task tools.\n\n\
                 ## MANDATORY STEPS (follow in order)\n\n\
                 ### Step 1: List root directory\n\
                 Use LS on the project root to see all top-level files and directories.\n\n\
                 ### Step 2: Find ALL config files\n\
                 Use Glob with these patterns (run each one):\n\
                 - `*.json` (package.json, tsconfig.json, etc.)\n\
                 - `*.toml` (Cargo.toml, pyproject.toml, etc.)\n\
                 - `*.yaml` or `*.yml` (docker-compose, CI configs)\n\
                 - `*.md` in root only (README.md, CLAUDE.md, etc.)\n\n\
                 ### Step 3: READ every config file found\n\
                 Use Read on EACH config file from Step 2. This is critical — you must read the actual \
                 contents to understand the project. At minimum read:\n\
                 - package.json (dependencies, scripts)\n\
                 - Cargo.toml (Rust crates, workspace)\n\
                 - pyproject.toml / setup.py (Python packages)\n\
                 - tsconfig.json (TypeScript config)\n\
                 - README.md or CLAUDE.md (project description)\n\n\
                 ### Step 4: Explore ALL source directories\n\
                 Use LS on every major directory found in Step 1 (e.g., src/, lib/, app/, desktop/, \
                 server/, client/, components/, services/, etc.). Go ONE level deeper into important \
                 subdirectories.\n\n\
                 ### Step 5: Read entry point files\n\
                 Use Read on entry points found:\n\
                 - main.rs, lib.rs, mod.rs (Rust)\n\
                 - index.ts, index.tsx, main.ts, App.tsx (TypeScript/React)\n\
                 - main.py, __init__.py, app.py (Python)\n\
                 - index.js, server.js (JavaScript)\n\n\
                 ## MINIMUM REQUIREMENTS\n\
                 - You MUST make at least 15 tool calls before writing your summary\n\
                 - You MUST read at least 8 files (not just list them)\n\
                 - Do NOT write any summary text until you have completed all 5 steps\n\n\
                 ## Output Format\n\
                 After completing all steps, provide a detailed summary with:\n\
                 - **Project Type**: What kind of project (monorepo? desktop app? CLI? web app?)\n\
                 - **Languages & Frameworks**: All languages and frameworks detected with versions\n\
                 - **Directory Structure**: Every major directory and its purpose\n\
                 - **Key Config Files**: What each config file reveals about the project\n\
                 - **Source Entry Points**: Main entry points for each component\n\
                 - **Dependencies**: Major dependencies from package managers\n\
                 - **Build System**: How the project is built (scripts, toolchain)\n\n\
                 Be thorough and specific. Reference actual file paths and dependency names."
                    .to_string(),
            ),
            max_iterations: 40,
            max_total_tokens: sub_agent_token_budget(self.provider.context_window()),
            project_root: self.config.project_root.clone(),
            streaming: true,
            enable_compaction: false,
        };

        let phase1_agent =
            OrchestratorService::new_sub_agent(phase1_config, self.cancellation_token.clone());

        // Create a separate channel for sub-agent events. Sub-agent internals
        // (TextDelta, ToolStart, ToolResult) should NOT leak to the frontend.
        // Only SubAgentStart/End (emitted by us above) reach the user.
        let (sub_tx1, mut sub_rx1) = mpsc::channel::<UnifiedStreamEvent>(256);
        let drain1 = tokio::spawn(async move {
            while sub_rx1.recv().await.is_some() {}
        });

        let phase1_result = phase1_agent
            .execute_story(&phase1_prompt, &get_basic_tool_definitions(), sub_tx1)
            .await;
        drain1.await.ok();

        // Log Phase 1 results for diagnostics
        if !phase1_result.success {
            eprintln!(
                "[exploration] Phase 1 failed: error={:?}, iterations={}, tokens={}",
                phase1_result.error,
                phase1_result.iterations,
                phase1_result.usage.total_tokens()
            );
        }
        eprintln!(
            "[exploration] Phase 1 result: iterations={}, tokens={}, response_len={}",
            phase1_result.iterations,
            phase1_result.usage.total_tokens(),
            phase1_result.response.as_ref().map_or(0, |s| s.len())
        );
        if let Some(ref resp) = phase1_result.response {
            let preview = if resp.len() > 200 {
                format!("{}...", &resp[..200])
            } else {
                resp.clone()
            };
            eprintln!("[exploration] Phase 1 response preview: {}", preview);
        }

        merge_usage(&mut total_usage, &phase1_result.usage);

        let structure_summary = phase1_result
            .response
            .clone()
            .unwrap_or_else(|| "Could not determine project structure.".to_string());

        let _ = tx
            .send(UnifiedStreamEvent::SubAgentEnd {
                sub_agent_id: phase1_id,
                success: phase1_result.success,
                usage: serde_json::json!({
                    "input_tokens": phase1_result.usage.input_tokens,
                    "output_tokens": phase1_result.usage.output_tokens,
                }),
            })
            .await;

        // Check cancellation between phases
        if self.cancellation_token.is_cancelled() {
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: phase1_result.iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        // ---- Phase 2: Deep Analysis ----
        let phase2_id = "exploration_phase2".to_string();

        // Truncate structure_summary to prevent oversized prompts for Phase 2.
        // Allow a generous limit since Phase 2 needs good context from Phase 1.
        let truncated_structure = if structure_summary.len() > 8000 {
            let truncated = &structure_summary[..8000];
            let cut_at = truncated.rfind('\n').unwrap_or(8000);
            format!("{}\n\n... (truncated)", &structure_summary[..cut_at])
        } else {
            structure_summary.clone()
        };

        let phase2_prompt = format!(
            "The user asked: \"{}\"\n\n\
             Here is the project structure discovered in Phase 1:\n\
             ---\n{}\n---\n\n\
             You MUST now perform a DEEP code analysis. Follow the steps below IN ORDER.\n\
             Your FIRST response MUST be a tool call. Do NOT write any text before using tools.",
            message, truncated_structure
        );

        let _ = tx
            .send(UnifiedStreamEvent::SubAgentStart {
                sub_agent_id: phase2_id.clone(),
                prompt: "Phase 2: Analyzing architecture...".to_string(),
                task_type: Some("analyze".to_string()),
            })
            .await;

        let phase2_config = OrchestratorConfig {
            provider: self.config.provider.clone(),
            system_prompt: Some(
                "You are a code analysis specialist performing DEEP architecture analysis.\n\
                 You MUST do all work yourself using tools (Read, Grep, Glob, LS). You ARE the sub-agent.\n\
                 Do NOT delegate to sub-agents or Task tools.\n\n\
                 ## CRITICAL: TOOLS FIRST, TEXT LAST\n\
                 Your FIRST response MUST be a tool call. You MUST NOT write ANY analysis text \
                 until you have read at least 10 source files. If you write text before reading \
                 enough files, your analysis will be WRONG.\n\n\
                 ## MANDATORY STEPS (follow in order)\n\n\
                 ### Step 1: Grep for architectural patterns\n\
                 Search for key code patterns to find important files:\n\
                 - `pub struct` or `class ` or `interface ` (type definitions)\n\
                 - `pub fn` or `export function` or `def ` (function definitions)\n\
                 - `mod ` or `import ` or `from ` (module structure)\n\
                 - `impl ` or `extends ` or `implements ` (inheritance/traits)\n\
                 Run at least 3-4 Grep searches to find key files.\n\n\
                 ### Step 2: Read core module files\n\
                 Based on the project structure from Phase 1 AND your Grep results, use Read on:\n\
                 - The main entry point of each component/package\n\
                 - mod.rs or index.ts files in key directories (these define module structure)\n\
                 - Service/controller/handler files (the business logic)\n\
                 - Model/type definition files (data structures)\n\
                 Read at least 8-10 source files in this step.\n\n\
                 ### Step 3: Read deeper implementation files\n\
                 For each major component discovered, read at least one implementation file:\n\
                 - If there's a services/ directory, read 2-3 service files\n\
                 - If there's a components/ directory, read 2-3 component files\n\
                 - If there's a commands/ or handlers/ directory, read 1-2 files\n\
                 - Read test files to understand expected behavior\n\
                 Read 5-8 more files in this step.\n\n\
                 ### Step 4: Identify cross-cutting patterns\n\
                 Use Grep to find:\n\
                 - Error handling patterns (Result, Error, try/catch)\n\
                 - State management (store, context, state)\n\
                 - API/routing patterns (route, endpoint, handler)\n\
                 - Configuration patterns (config, settings, env)\n\n\
                 ### Step 5: Write your analysis\n\
                 ONLY after completing Steps 1-4, write your analysis.\n\n\
                 ## MINIMUM REQUIREMENTS\n\
                 - You MUST make at least 20 tool calls total\n\
                 - You MUST read at least 15 different source files using Read\n\
                 - You MUST run at least 4 Grep searches\n\
                 - Do NOT provide analysis until all steps are complete\n\n\
                 ## Output Format\n\
                 Provide a comprehensive analysis with:\n\
                 - **Architecture Overview**: High-level architecture (multi-tier? client-server? etc.)\n\
                 - **Components**: Each major component with its purpose, key files, and responsibilities\n\
                 - **Data Flow**: How data flows through the system\n\
                 - **Key Patterns**: Design patterns used (MVC, event-driven, etc.)\n\
                 - **Technology Stack**: Specific frameworks, libraries, and tools with versions\n\
                 - **Code Organization**: How code is structured (modules, layers, packages)\n\
                 - **Integration Points**: How components communicate\n\n\
                 Reference SPECIFIC file paths and code patterns you found. Be precise and thorough."
                    .to_string(),
            ),
            max_iterations: 50,
            max_total_tokens: sub_agent_token_budget(self.provider.context_window()),
            project_root: self.config.project_root.clone(),
            streaming: true,
            enable_compaction: false,
        };

        let phase2_agent =
            OrchestratorService::new_sub_agent(phase2_config, self.cancellation_token.clone());

        let (sub_tx2, mut sub_rx2) = mpsc::channel::<UnifiedStreamEvent>(256);
        let drain2 = tokio::spawn(async move {
            while sub_rx2.recv().await.is_some() {}
        });

        let phase2_result = phase2_agent
            .execute_story(&phase2_prompt, &get_basic_tool_definitions(), sub_tx2)
            .await;
        drain2.await.ok();

        // Log Phase 2 results for diagnostics
        if !phase2_result.success {
            eprintln!(
                "[exploration] Phase 2 failed: error={:?}, iterations={}, tokens={}",
                phase2_result.error,
                phase2_result.iterations,
                phase2_result.usage.total_tokens()
            );
        }
        eprintln!(
            "[exploration] Phase 2 result: iterations={}, tokens={}, response_len={}",
            phase2_result.iterations,
            phase2_result.usage.total_tokens(),
            phase2_result.response.as_ref().map_or(0, |s| s.len())
        );
        if let Some(ref resp) = phase2_result.response {
            let preview = if resp.len() > 200 {
                format!("{}...", &resp[..200])
            } else {
                resp.clone()
            };
            eprintln!("[exploration] Phase 2 response preview: {}", preview);
        }

        merge_usage(&mut total_usage, &phase2_result.usage);

        let analysis_summary = phase2_result
            .response
            .clone()
            .unwrap_or_else(|| "Could not complete analysis.".to_string());

        let _ = tx
            .send(UnifiedStreamEvent::SubAgentEnd {
                sub_agent_id: phase2_id,
                success: phase2_result.success,
                usage: serde_json::json!({
                    "input_tokens": phase2_result.usage.input_tokens,
                    "output_tokens": phase2_result.usage.output_tokens,
                }),
            })
            .await;

        // Check cancellation before synthesis
        if self.cancellation_token.is_cancelled() {
            return ExecutionResult {
                response: None,
                usage: total_usage,
                iterations: phase1_result.iterations + phase2_result.iterations,
                success: false,
                error: Some("Execution cancelled".to_string()),
            };
        }

        // ---- Phase 3: Synthesis (single LLM call, no tools) ----
        let synthesis_prompt = format!(
            "You are synthesizing findings from a thorough codebase exploration.\n\n\
             **User's original question**: {}\n\n\
             **Project Structure Discovery** (Phase 1 — config files, directories, entry points):\n\
             ---\n{}\n---\n\n\
             **Deep Code Analysis** (Phase 2 — source code reading, architecture patterns):\n\
             ---\n{}\n---\n\n\
             Combine ALL findings into a comprehensive, well-organized response. Your response MUST:\n\
             1. Accurately describe what the project IS (not what you guess it might be)\n\
             2. Cover ALL major components/packages found in the codebase\n\
             3. Reference specific file paths, frameworks, and dependency names from the findings\n\
             4. Describe the architecture with concrete details (not generic descriptions)\n\
             5. Use markdown formatting with headers, bullet points, and code references\n\n\
             IMPORTANT: Base your response ONLY on the actual findings above. Do NOT guess or \
             fabricate details that aren't in the Phase 1 and Phase 2 data.",
            message, structure_summary, analysis_summary
        );

        let synthesis_messages = vec![Message::user(synthesis_prompt)];
        // Use non-streaming synthesis to avoid leaking low-level thinking/tool
        // internals into the user-visible stream for exploration mode.
        let synthesis_response = self.call_llm(&synthesis_messages, &[], &[]).await;

        let (final_response, synthesis_success) = match synthesis_response {
            Ok(r) => {
                merge_usage(&mut total_usage, &r.usage);
                let cleaned = r.content.map(|text| extract_text_without_tool_calls(&text));
                if let Some(content) = cleaned.as_ref().filter(|text| !text.trim().is_empty()) {
                    let _ = tx
                        .send(UnifiedStreamEvent::TextDelta {
                            content: content.clone(),
                        })
                        .await;
                }
                (cleaned, true)
            }
            Err(e) => {
                // If synthesis fails, fall back to concatenating the two summaries
                let fallback = format!(
                    "## Project Structure\n\n{}\n\n## Architecture Analysis\n\n{}",
                    structure_summary, analysis_summary
                );
                // Emit the fallback as text deltas so the frontend shows it
                let _ = tx
                    .send(UnifiedStreamEvent::TextDelta {
                        content: fallback.clone(),
                    })
                    .await;
                eprintln!("Synthesis LLM call failed, using fallback: {}", e);
                (Some(fallback), true)
            }
        };

        let total_iterations =
            phase1_result.iterations + phase2_result.iterations + 1; // +1 for synthesis

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
            success: synthesis_success,
            error: None,
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
                    MessageContent::ToolResultMultimodal { content: blocks, .. } => {
                        for block in blocks {
                            if let crate::services::llm::types::ContentBlock::Text { text } = block {
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
            .send_message(summary_messages, None, Vec::new())
            .await;

        match result {
            Ok(response) => {
                let summary_text = response.content.unwrap_or_else(|| "Previous conversation context was compacted.".to_string());
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
    fn effective_system_prompt(&self, prompt_tools: &[ToolDefinition]) -> Option<String> {
        if prompt_tools.is_empty() {
            return self.config.system_prompt.clone();
        }

        let mut prompt = build_system_prompt(&self.config.project_root, prompt_tools);

        // For providers that don't support native tool calling,
        // add prompt-based tool call format instructions
        if !self.provider.supports_tools() {
            let fallback_instructions = build_tool_call_instructions(prompt_tools);
            prompt = format!("{}\n\n{}", prompt, fallback_instructions);
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
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools);
        self.provider
            .send_message(messages.to_vec(), system, api_tools.to_vec())
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
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        let system = self.effective_system_prompt(prompt_tools);
        self.provider
            .stream_message(messages.to_vec(), system, api_tools.to_vec(), tx)
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
            self.call_llm_streaming(&messages, &[], &[], tx.clone()).await
        } else {
            self.call_llm(&messages, &[], &[]).await
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

/// Parse an RFC3339 timestamp to Unix timestamp
/// Detect whether a user message is an exploration/analysis task that should be
/// automatically delegated to sub-agents instead of entering the direct agentic loop.
/// This prevents context overflow when non-Claude LLMs read files directly instead of
/// using the Task tool for delegation.
fn is_exploration_task(message: &str) -> bool {
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
        assert!(is_exploration_task("分析这个项目"));
        assert!(is_exploration_task("帮我了解这个代码"));
        assert!(is_exploration_task("总结一下项目结构"));
        assert!(is_exploration_task("探索代码库"));
    }

    #[test]
    fn test_is_exploration_task_english() {
        assert!(is_exploration_task("analyze this project"));
        assert!(is_exploration_task("Explore the codebase"));
        assert!(is_exploration_task("examine this code"));
        assert!(is_exploration_task("give me an overview of this repository"));
        assert!(is_exploration_task("summarize the project structure"));
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
}
