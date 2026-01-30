//! Orchestrator Service
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with persistence, cancellation, and progress tracking.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::models::orchestrator::{
    ExecutionProgress, ExecutionSession, ExecutionSessionSummary, ExecutionStatus,
    StoryExecutionState,
};
use crate::services::llm::{
    LlmProvider, LlmResponse, Message, MessageContent, ProviderConfig,
    ToolDefinition, UsageStats, AnthropicProvider, OpenAIProvider,
    DeepSeekProvider, OllamaProvider, ProviderType,
};
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::{ToolExecutor, get_tool_definitions};
use crate::services::quality_gates::run_quality_gates as execute_quality_gates;
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
}

fn default_max_iterations() -> u32 {
    50
}

fn default_max_tokens() -> u32 {
    100_000
}

fn default_streaming() -> bool {
    true
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

impl OrchestratorService {
    /// Create a new orchestrator service
    pub fn new(config: OrchestratorConfig) -> Self {
        let provider: Arc<dyn LlmProvider> = match config.provider.provider {
            ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config.provider.clone())),
            ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config.provider.clone())),
            ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(config.provider.clone())),
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
        let conn = pool.get()
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
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool.get()
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
                session.started_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.to_rfc3339()),
                session.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|dt| dt.to_rfc3339()),
                session.error,
                metadata_json,
            ],
        )?;

        // Save stories
        for story in &session.stories {
            let quality_gates_json = serde_json::to_string(&story.quality_gates).unwrap_or_default();

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
                    story.started_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                        .map(|dt| dt.to_rfc3339()),
                    story.completed_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
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
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool.get()
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
                    started_at: row.get::<_, Option<String>>(12)?.map(|s| parse_timestamp(Some(s))),
                    completed_at: row.get::<_, Option<String>>(13)?.map(|s| parse_timestamp(Some(s))),
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
             FROM execution_stories WHERE session_id = ?1 ORDER BY id"
        )?;

        let stories: Vec<StoryExecutionState> = stmt.query_map(params![session_id], |row| {
            let status_str: String = row.get(2)?;
            let quality_gates_json: String = row.get::<_, Option<String>>(9)?.unwrap_or_default();

            Ok(StoryExecutionState {
                story_id: row.get(0)?,
                title: row.get(1)?,
                status: status_str.parse().unwrap_or(ExecutionStatus::Pending),
                started_at: row.get::<_, Option<String>>(3)?.map(|s| parse_timestamp(Some(s))),
                completed_at: row.get::<_, Option<String>>(4)?.map(|s| parse_timestamp(Some(s))),
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
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool.get()
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
        let summaries = stmt.query_map([], |row| {
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
        let _ = tx.send(UnifiedStreamEvent::SessionProgress {
            session_id: session.id.clone(),
            progress: serde_json::to_value(&progress).unwrap_or_default(),
        }).await;

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
            let _ = tx.send(UnifiedStreamEvent::StoryStart {
                session_id: session_id.clone(),
                story_id: story_id.clone(),
                story_title: story_title.clone(),
                story_index,
                total_stories,
            }).await;

            // Execute the story
            let result = self.execute_story(
                &story_prompt,
                &tools,
                tx.clone(),
            ).await;

            // Update story state and session tokens
            {
                let story = &mut session.stories[story_index];
                story.iterations = result.iterations;
                story.input_tokens = result.usage.input_tokens;
                story.output_tokens = result.usage.output_tokens;
            }
            total_usage.input_tokens += result.usage.input_tokens;
            total_usage.output_tokens += result.usage.output_tokens;
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
                    ).await;

                    if let Ok(summary) = gates_result {
                        let passed = summary.failed_gates == 0;
                        session.stories[story_index].quality_gates.insert("overall".to_string(), passed);

                        // Emit quality gates result
                        let _ = tx.send(UnifiedStreamEvent::QualityGatesResult {
                            session_id: session_id.clone(),
                            story_id: story_id.clone(),
                            passed,
                            summary: serde_json::to_value(&summary).unwrap_or_default(),
                        }).await;

                        if !passed {
                            // Quality gates failed - mark story as failed
                            session.stories[story_index].status = ExecutionStatus::Failed;
                            session.stories[story_index].error = Some("Quality gates failed".to_string());
                        }
                    }
                }

                // Get current story status for event
                let story_success = session.stories[story_index].status == ExecutionStatus::Completed;
                let story_error = session.stories[story_index].error.clone();

                // Emit story completion
                let _ = tx.send(UnifiedStreamEvent::StoryComplete {
                    session_id: session_id.clone(),
                    story_id: story_id.clone(),
                    success: story_success,
                    error: story_error,
                }).await;

            } else {
                // Mark story as failed
                let error_msg = result.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                session.stories[story_index].fail(error_msg);

                // Emit story failure
                let _ = tx.send(UnifiedStreamEvent::StoryComplete {
                    session_id: session_id.clone(),
                    story_id: story_id.clone(),
                    success: false,
                    error: result.error.clone(),
                }).await;

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
            let _ = tx.send(UnifiedStreamEvent::SessionProgress {
                session_id: session.id.clone(),
                progress: serde_json::to_value(&progress).unwrap_or_default(),
            }).await;

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
        let _ = tx.send(UnifiedStreamEvent::SessionComplete {
            session_id: session.id.clone(),
            success: true,
            completed_stories: session.completed_stories(),
            total_stories: session.stories.len(),
        }).await;

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
        let mut messages = vec![Message::user(prompt.to_string())];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;

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
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Maximum iterations ({}) reached",
                        self.config.max_iterations
                    )),
                };
            }

            // Check token budget
            if total_usage.total_tokens() >= self.config.max_total_tokens {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Token budget ({}) exceeded",
                        self.config.max_total_tokens
                    )),
                };
            }

            iterations += 1;

            // Call LLM
            let response = if self.config.streaming {
                self.call_llm_streaming(&messages, tools, tx.clone()).await
            } else {
                self.call_llm(&messages, tools).await
            };

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    // Emit error event
                    let _ = tx.send(UnifiedStreamEvent::Error {
                        message: e.to_string(),
                        code: None,
                    }).await;

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
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;
            if let Some(thinking) = response.usage.thinking_tokens {
                total_usage.thinking_tokens = Some(
                    total_usage.thinking_tokens.unwrap_or(0) + thinking
                );
            }

            // Check if we have tool calls
            if response.has_tool_calls() {
                // Add assistant message with tool calls
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
                    let _ = tx.send(UnifiedStreamEvent::ToolStart {
                        tool_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        arguments: Some(tc.arguments.to_string()),
                    }).await;

                    // Execute the tool
                    let result = self.tool_executor.execute(&tc.name, &tc.arguments).await;

                    // Emit tool result event
                    let _ = tx.send(UnifiedStreamEvent::ToolResult {
                        tool_id: tc.id.clone(),
                        result: if result.success { result.output.clone() } else { None },
                        error: if !result.success { result.error.clone() } else { None },
                    }).await;

                    // Add tool result to messages
                    messages.push(Message::tool_result(
                        &tc.id,
                        result.to_content(),
                        !result.success,
                    ));
                }
            } else {
                // No tool calls - this is the final response
                return ExecutionResult {
                    response: response.content,
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
        let tools = get_tool_definitions();
        let mut messages = vec![Message::user(message)];
        let mut total_usage = UsageStats::default();
        let mut iterations = 0;

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
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Maximum iterations ({}) reached",
                        self.config.max_iterations
                    )),
                };
            }

            // Check token budget
            if total_usage.total_tokens() >= self.config.max_total_tokens {
                return ExecutionResult {
                    response: None,
                    usage: total_usage,
                    iterations,
                    success: false,
                    error: Some(format!(
                        "Token budget ({}) exceeded",
                        self.config.max_total_tokens
                    )),
                };
            }

            iterations += 1;

            // Call LLM
            let response = if self.config.streaming {
                self.call_llm_streaming(&messages, &tools, tx.clone()).await
            } else {
                self.call_llm(&messages, &tools).await
            };

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    // Emit error event
                    let _ = tx.send(UnifiedStreamEvent::Error {
                        message: e.to_string(),
                        code: None,
                    }).await;

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
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;
            if let Some(thinking) = response.usage.thinking_tokens {
                total_usage.thinking_tokens = Some(
                    total_usage.thinking_tokens.unwrap_or(0) + thinking
                );
            }

            // Check if we have tool calls
            if response.has_tool_calls() {
                // Add assistant message with tool calls
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
                    let _ = tx.send(UnifiedStreamEvent::ToolStart {
                        tool_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        arguments: Some(tc.arguments.to_string()),
                    }).await;

                    // Execute the tool
                    let result = self.tool_executor.execute(&tc.name, &tc.arguments).await;

                    // Emit tool result event
                    let _ = tx.send(UnifiedStreamEvent::ToolResult {
                        tool_id: tc.id.clone(),
                        result: if result.success { result.output.clone() } else { None },
                        error: if !result.success { result.error.clone() } else { None },
                    }).await;

                    // Add tool result to messages
                    messages.push(Message::tool_result(
                        &tc.id,
                        result.to_content(),
                        !result.success,
                    ));
                }
            } else {
                // No tool calls - this is the final response
                // Emit completion event
                let _ = tx.send(UnifiedStreamEvent::Complete {
                    stop_reason: Some("end_turn".to_string()),
                }).await;

                // Emit usage event
                let _ = tx.send(UnifiedStreamEvent::Usage {
                    input_tokens: total_usage.input_tokens,
                    output_tokens: total_usage.output_tokens,
                    thinking_tokens: total_usage.thinking_tokens,
                    cache_read_tokens: total_usage.cache_read_tokens,
                    cache_creation_tokens: total_usage.cache_creation_tokens,
                }).await;

                return ExecutionResult {
                    response: response.content,
                    usage: total_usage,
                    iterations,
                    success: true,
                    error: None,
                };
            }
        }
    }

    /// Call the LLM with non-streaming mode
    async fn call_llm(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        self.provider
            .send_message(
                messages.to_vec(),
                self.config.system_prompt.clone(),
                tools.to_vec(),
            )
            .await
    }

    /// Call the LLM with streaming mode
    async fn call_llm_streaming(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> Result<LlmResponse, crate::services::llm::LlmError> {
        self.provider
            .stream_message(
                messages.to_vec(),
                self.config.system_prompt.clone(),
                tools.to_vec(),
                tx,
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
            self.call_llm_streaming(&messages, &[], tx.clone()).await
        } else {
            self.call_llm(&messages, &[]).await
        };

        match response {
            Ok(r) => {
                let _ = tx.send(UnifiedStreamEvent::Complete {
                    stop_reason: Some("end_turn".to_string()),
                }).await;

                ExecutionResult {
                    response: r.content,
                    usage: r.usage,
                    iterations: 1,
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                let _ = tx.send(UnifiedStreamEvent::Error {
                    message: e.to_string(),
                    code: None,
                }).await;

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
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool.get()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Delete stories first (due to foreign key)
        conn.execute("DELETE FROM execution_stories WHERE session_id = ?1", params![session_id])?;

        // Delete session
        conn.execute("DELETE FROM execution_sessions WHERE id = ?1", params![session_id])?;

        // Remove from cache
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(session_id);

        Ok(())
    }

    /// Cleanup old completed sessions
    pub async fn cleanup_old_sessions(&self, days: i64) -> AppResult<usize> {
        let pool = self.db_pool.as_ref()
            .ok_or_else(|| AppError::database("Database not configured"))?;

        let conn = pool.get()
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

/// Parse an RFC3339 timestamp to Unix timestamp
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
}
