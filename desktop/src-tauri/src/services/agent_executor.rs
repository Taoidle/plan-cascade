//! Agent Executor
//!
//! Executes AI agents with custom system prompts and tool configurations.
//! Supports streaming responses and background execution.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, RwLock};

use crate::models::agent::{Agent, AgentRun};
use crate::services::agent::AgentService;
use crate::services::llm::{
    LlmError, LlmProvider, LlmRequestOptions, LlmResult, Message, ToolDefinition,
};
use crate::services::streaming::UnifiedStreamEvent;
use crate::utils::error::{AppError, AppResult};

/// Configuration for agent execution
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum retries on transient errors
    pub max_retries: u32,
    /// Base delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Whether to allow tool usage
    pub enable_tools: bool,
    /// Execution timeout in milliseconds (0 = no timeout)
    pub timeout_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_tools: true,
            timeout_ms: 300_000, // 5 minutes
        }
    }
}

/// Event emitted during agent execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Execution started
    Started { run_id: String },
    /// Text content being streamed
    ContentDelta { delta: String },
    /// Thinking/reasoning content (if available)
    ThinkingDelta { delta: String },
    /// A tool is being called
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool call completed
    ToolResult {
        id: String,
        result: String,
        is_error: bool,
    },
    /// Execution completed successfully
    Completed {
        run_id: String,
        output: String,
        duration_ms: u64,
    },
    /// Execution failed
    Failed {
        run_id: String,
        error: String,
        duration_ms: u64,
    },
    /// Execution was cancelled
    Cancelled { run_id: String, duration_ms: u64 },
    /// Token usage update
    Usage {
        input_tokens: u32,
        output_tokens: u32,
    },
}

/// Handle to a running agent execution
#[derive(Debug)]
pub struct ExecutionHandle {
    /// Unique run ID
    pub run_id: String,
    /// Cancellation flag
    cancelled: Arc<RwLock<bool>>,
}

impl ExecutionHandle {
    /// Create a new execution handle
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            cancelled: Arc::new(RwLock::new(false)),
        }
    }

    /// Cancel the execution
    pub async fn cancel(&self) {
        let mut flag = self.cancelled.write().await;
        *flag = true;
    }

    /// Check if cancelled
    pub async fn is_cancelled(&self) -> bool {
        *self.cancelled.read().await
    }

    /// Get the cancellation flag for sharing
    pub fn cancellation_flag(&self) -> Arc<RwLock<bool>> {
        Arc::clone(&self.cancelled)
    }
}

/// Tool filter that restricts available tools based on agent configuration
pub struct ToolFilter {
    allowed_tools: Vec<String>,
    allow_all: bool,
}

impl ToolFilter {
    /// Create a new tool filter
    pub fn new(allowed_tools: Vec<String>) -> Self {
        let allow_all = allowed_tools.is_empty();
        Self {
            allowed_tools,
            allow_all,
        }
    }

    /// Check if a tool is allowed
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        self.allow_all || self.allowed_tools.contains(&tool_name.to_string())
    }

    /// Filter tool definitions to only include allowed tools
    pub fn filter_tools(&self, tools: Vec<ToolDefinition>) -> Vec<ToolDefinition> {
        if self.allow_all {
            return tools;
        }

        tools
            .into_iter()
            .filter(|t| self.allowed_tools.contains(&t.name))
            .collect()
    }
}

/// Agent executor that runs agents with custom configurations
pub struct AgentExecutor {
    agent_service: Arc<AgentService>,
    config: ExecutorConfig,
}

impl AgentExecutor {
    /// Create a new agent executor
    pub fn new(agent_service: Arc<AgentService>, config: ExecutorConfig) -> Self {
        Self {
            agent_service,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_default_config(agent_service: Arc<AgentService>) -> Self {
        Self::new(agent_service, ExecutorConfig::default())
    }

    /// Execute an agent with the given input
    ///
    /// Returns the execution handle and a channel for receiving events.
    pub async fn execute(
        &self,
        agent_id: &str,
        input: String,
        provider: Arc<dyn LlmProvider>,
        available_tools: Vec<ToolDefinition>,
    ) -> AppResult<(ExecutionHandle, mpsc::Receiver<AgentEvent>)> {
        // Get the agent
        let agent = self
            .agent_service
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Agent not found: {}", agent_id)))?;

        // Create the run record
        let run = self.agent_service.create_run(agent_id, &input).await?;
        let run_id = run.id.clone();

        // Create channels
        let (tx, rx) = mpsc::channel(100);

        // Create execution handle
        let handle = ExecutionHandle::new(run_id.clone());
        let cancellation_flag = handle.cancellation_flag();

        // Clone what we need for the async task
        let config = self.config.clone();

        // Spawn the execution task
        tokio::spawn(async move {
            let result = Self::run_execution(
                agent,
                run,
                input,
                provider,
                available_tools,
                tx.clone(),
                cancellation_flag,
                config,
            )
            .await;

            // Update run record with final status
            if let Err(e) = result {
                let _ = tx
                    .send(AgentEvent::Failed {
                        run_id: run_id.clone(),
                        error: e.to_string(),
                        duration_ms: 0,
                    })
                    .await;
            }
        });

        Ok((handle, rx))
    }

    /// Execute an agent without streaming (blocking)
    pub async fn execute_sync(
        &self,
        agent_id: &str,
        input: &str,
        provider: Arc<dyn LlmProvider>,
        available_tools: Vec<ToolDefinition>,
    ) -> AppResult<AgentRun> {
        // Get the agent
        let agent = self
            .agent_service
            .get_agent(agent_id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Agent not found: {}", agent_id)))?;

        // Create the run record
        let mut run = self.agent_service.create_run(agent_id, input).await?;
        run.start();
        self.agent_service.update_run(&run).await?;

        let start_time = Instant::now();

        // Filter tools based on agent configuration
        let tool_filter = ToolFilter::new(agent.allowed_tools.clone());
        let filtered_tools = tool_filter.filter_tools(available_tools);

        // Build messages
        let messages = vec![Message::user(input)];

        // Execute with retries
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Wait before retry
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.retry_delay_ms * (1 << attempt),
                ))
                .await;
            }

            match provider
                .send_message(
                    messages.clone(),
                    Some(agent.system_prompt.clone()),
                    if self.config.enable_tools {
                        filtered_tools.clone()
                    } else {
                        vec![]
                    },
                    LlmRequestOptions::default(),
                )
                .await
            {
                Ok(response) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let output = response.content.unwrap_or_default();

                    run.complete(
                        output,
                        duration_ms,
                        response.usage.input_tokens,
                        response.usage.output_tokens,
                    );
                    self.agent_service.update_run(&run).await?;
                    return Ok(run);
                }
                Err(e) => {
                    if Self::is_retryable(&e) {
                        last_error = Some(e);
                        continue;
                    } else {
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        run.fail(e.to_string(), duration_ms);
                        self.agent_service.update_run(&run).await?;
                        return Err(AppError::command(e.to_string()));
                    }
                }
            }
        }

        // All retries exhausted
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let error = last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Unknown error".to_string());
        run.fail(error.clone(), duration_ms);
        self.agent_service.update_run(&run).await?;
        Err(AppError::command(error))
    }

    /// Internal execution logic with streaming
    async fn run_execution(
        agent: Agent,
        mut run: AgentRun,
        input: String,
        provider: Arc<dyn LlmProvider>,
        available_tools: Vec<ToolDefinition>,
        tx: mpsc::Sender<AgentEvent>,
        cancellation_flag: Arc<RwLock<bool>>,
        config: ExecutorConfig,
    ) -> LlmResult<()> {
        let start_time = Instant::now();

        // Send started event
        let _ = tx
            .send(AgentEvent::Started {
                run_id: run.id.clone(),
            })
            .await;

        run.start();

        // Filter tools based on agent configuration
        let tool_filter = ToolFilter::new(agent.allowed_tools.clone());
        let filtered_tools = tool_filter.filter_tools(available_tools);

        // Build messages
        let messages = vec![Message::user(&input)];

        // Create streaming channel
        let (stream_tx, mut stream_rx) = mpsc::channel::<UnifiedStreamEvent>(100);

        // Start streaming
        let provider_clone = Arc::clone(&provider);
        let system_prompt = agent.system_prompt.clone();
        let tools = if config.enable_tools {
            filtered_tools.clone()
        } else {
            vec![]
        };

        let stream_handle = tokio::spawn(async move {
            provider_clone
                .stream_message(
                    messages,
                    Some(system_prompt),
                    tools,
                    stream_tx,
                    LlmRequestOptions::default(),
                )
                .await
        });

        let mut accumulated_content = String::new();
        let mut accumulated_thinking = String::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;

        // Process stream events
        loop {
            tokio::select! {
                // Check for cancellation
                _ = async {
                    loop {
                        if *cancellation_flag.read().await {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                } => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    run.cancel(duration_ms);
                    let _ = tx.send(AgentEvent::Cancelled {
                        run_id: run.id.clone(),
                        duration_ms,
                    }).await;
                    return Ok(());
                }

                event = stream_rx.recv() => {
                    match event {
                        Some(UnifiedStreamEvent::TextDelta { content }) => {
                            accumulated_content.push_str(&content);
                            let _ = tx.send(AgentEvent::ContentDelta { delta: content }).await;
                        }
                        Some(UnifiedStreamEvent::ThinkingDelta { content, .. }) => {
                            accumulated_thinking.push_str(&content);
                            let _ = tx.send(AgentEvent::ThinkingDelta { delta: content }).await;
                        }
                        Some(UnifiedStreamEvent::ToolStart { tool_id, tool_name, .. }) => {
                            let _ = tx.send(AgentEvent::ToolCall {
                                id: tool_id,
                                name: tool_name,
                                input: serde_json::Value::Null,
                            }).await;
                        }
                        Some(UnifiedStreamEvent::Usage { input_tokens: it, output_tokens: ot, .. }) => {
                            input_tokens = it;
                            output_tokens = ot;
                            let _ = tx.send(AgentEvent::Usage {
                                input_tokens: it,
                                output_tokens: ot,
                            }).await;
                        }
                        Some(UnifiedStreamEvent::Complete { .. }) => {
                            break;
                        }
                        Some(UnifiedStreamEvent::Error { message, .. }) => {
                            let duration_ms = start_time.elapsed().as_millis() as u64;
                            run.fail(message.clone(), duration_ms);
                            let _ = tx.send(AgentEvent::Failed {
                                run_id: run.id.clone(),
                                error: message,
                                duration_ms,
                            }).await;
                            return Err(LlmError::Other { message: "Stream error".to_string() });
                        }
                        None => break,
                        _ => {} // Ignore other events
                    }
                }
            }
        }

        // Wait for stream to complete
        let _ = stream_handle.await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Complete the run
        run.complete(
            accumulated_content.clone(),
            duration_ms,
            input_tokens,
            output_tokens,
        );

        let _ = tx
            .send(AgentEvent::Completed {
                run_id: run.id.clone(),
                output: accumulated_content,
                duration_ms,
            })
            .await;

        Ok(())
    }

    /// Check if an error is retryable
    fn is_retryable(error: &LlmError) -> bool {
        matches!(
            error,
            LlmError::RateLimited { .. }
                | LlmError::ServerError {
                    status: Some(502 | 503 | 504),
                    ..
                }
                | LlmError::NetworkError { .. }
        )
    }
}

impl std::fmt::Debug for AgentExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentExecutor")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_allow_all() {
        let filter = ToolFilter::new(vec![]);
        assert!(filter.is_allowed("any_tool"));
        assert!(filter.is_allowed("another_tool"));
    }

    #[test]
    fn test_tool_filter_specific() {
        let filter = ToolFilter::new(vec!["read_file".to_string(), "write_file".to_string()]);
        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("write_file"));
        assert!(!filter.is_allowed("execute_command"));
    }

    #[test]
    fn test_tool_filter_filters_definitions() {
        let tools = vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: crate::services::llm::types::ParameterSchema::string(None),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write a file".to_string(),
                input_schema: crate::services::llm::types::ParameterSchema::string(None),
            },
            ToolDefinition {
                name: "execute_command".to_string(),
                description: "Execute a command".to_string(),
                input_schema: crate::services::llm::types::ParameterSchema::string(None),
            },
        ];

        let filter = ToolFilter::new(vec!["read_file".to_string()]);
        let filtered = filter.filter_tools(tools);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "read_file");
    }

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_tools);
    }

    #[tokio::test]
    async fn test_execution_handle() {
        let handle = ExecutionHandle::new("run-123".to_string());
        assert!(!handle.is_cancelled().await);

        handle.cancel().await;
        assert!(handle.is_cancelled().await);
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors
        assert!(AgentExecutor::is_retryable(&LlmError::RateLimited {
            message: "Too many requests".to_string(),
            retry_after: Some(60),
        }));
        assert!(AgentExecutor::is_retryable(&LlmError::ServerError {
            message: "Bad gateway".to_string(),
            status: Some(502),
        }));
        assert!(AgentExecutor::is_retryable(&LlmError::NetworkError {
            message: "Connection reset".to_string(),
        }));

        // Non-retryable errors
        assert!(!AgentExecutor::is_retryable(
            &LlmError::AuthenticationFailed {
                message: "Invalid API key".to_string(),
            }
        ));
        assert!(!AgentExecutor::is_retryable(&LlmError::InvalidRequest {
            message: "Bad request".to_string(),
        }));
    }
}
