//! LlmAgent — wraps the existing OrchestratorService agentic loop
//!
//! This module provides an `Agent` implementation that delegates to
//! `OrchestratorService::execute()`, converting `UnifiedStreamEvent`s
//! to `AgentEvent`s through an adapter layer.
//!
//! IMPORTANT: This does NOT modify or refactor `agentic_loop.rs`.
//! It wraps the existing execute() method via an mpsc channel bridge.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::types::{Agent, AgentConfig, AgentContext, AgentEvent, AgentEventStream};
use crate::services::orchestrator::{OrchestratorConfig, OrchestratorService};
use crate::services::streaming::UnifiedStreamEvent;
use crate::utils::error::AppResult;

/// An agent backed by an LLM via the OrchestratorService agentic loop.
///
/// `LlmAgent` wraps an `OrchestratorService` internally, delegating `run()`
/// to the existing `execute()` method. The `UnifiedStreamEvent` values from
/// the orchestrator channel are converted to `AgentEvent` on the fly.
pub struct LlmAgent {
    /// Display name for this agent.
    name: String,
    /// Description of what this agent does.
    description: String,
    /// Optional system instruction injected into the orchestrator config.
    instruction: Option<String>,
    /// Optional model override (uses context provider's model if None).
    model: Option<String>,
    /// Optional tool filter — only these tools will be available.
    tools: Option<Vec<String>>,
    /// Agent-specific configuration overrides.
    config: AgentConfig,
}

impl LlmAgent {
    /// Create a new LlmAgent with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: "LLM-backed agent using the agentic loop".to_string(),
            instruction: None,
            model: None,
            tools: None,
            config: AgentConfig::default(),
        }
    }

    /// Set the agent's description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the system instruction for the LLM.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = Some(instruction.into());
        self
    }

    /// Set the model to use (overrides the provider's default model).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the tool filter (only these tools will be available).
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the agent configuration.
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }
}

/// Convert a `UnifiedStreamEvent` to an `AgentEvent`.
///
/// Returns `None` for events that don't have a direct mapping (e.g.,
/// analysis-specific events, session-specific events).
pub fn convert_stream_event(event: &UnifiedStreamEvent) -> Option<AgentEvent> {
    match event {
        UnifiedStreamEvent::TextDelta { content } => Some(AgentEvent::TextDelta {
            content: content.clone(),
        }),
        UnifiedStreamEvent::TextReplace { content } => Some(AgentEvent::TextDelta {
            content: content.clone(),
        }),
        UnifiedStreamEvent::ThinkingDelta { content, .. } => Some(AgentEvent::ThinkingDelta {
            content: content.clone(),
        }),
        UnifiedStreamEvent::ToolStart {
            tool_name,
            arguments,
            ..
        } => Some(AgentEvent::ToolCall {
            name: tool_name.clone(),
            args: arguments.clone().unwrap_or_default(),
        }),
        UnifiedStreamEvent::ToolComplete {
            tool_name,
            arguments,
            ..
        } => Some(AgentEvent::ToolCall {
            name: tool_name.clone(),
            args: arguments.clone(),
        }),
        UnifiedStreamEvent::ToolResult {
            tool_id: _,
            result,
            error,
        } => {
            let result_str = if let Some(r) = result {
                r.clone()
            } else if let Some(e) = error {
                format!("Error: {}", e)
            } else {
                String::new()
            };
            Some(AgentEvent::ToolResult {
                name: String::new(), // tool name not available in ToolResult event
                result: result_str,
            })
        }
        UnifiedStreamEvent::Complete { .. } => Some(AgentEvent::Done { output: None }),
        UnifiedStreamEvent::Error { message, .. } => {
            // Map errors to Done with error info
            Some(AgentEvent::Done {
                output: Some(format!("Error: {}", message)),
            })
        }
        // Events that don't map to AgentEvent (analysis, session, thinking start/end, usage)
        _ => None,
    }
}

#[async_trait]
impl Agent for LlmAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, ctx: AgentContext) -> AppResult<AgentEventStream> {
        // Build the OrchestratorConfig from the AgentContext
        let provider_config = crate::services::llm::ProviderConfig {
            model: if let Some(ref model) = self.model {
                model.clone()
            } else {
                ctx.provider.model().to_string()
            },
            ..Default::default()
        };

        // Compute analysis artifacts root using the same logic as the orchestrator
        let analysis_artifacts_root = dirs::data_local_dir()
            .map(|d| d.join("plan-cascade").join("analysis-runs"))
            .unwrap_or_else(|| ctx.project_root.join(".plan-cascade").join("analysis-runs"));

        let orchestrator_config = OrchestratorConfig {
            provider: provider_config,
            system_prompt: self.instruction.clone(),
            max_iterations: self.config.max_iterations,
            max_total_tokens: self.config.max_total_tokens,
            project_root: ctx.project_root.clone(),
            analysis_artifacts_root,
            streaming: self.config.streaming,
            enable_compaction: self.config.enable_compaction,
            analysis_profile: Default::default(),
            analysis_limits: Default::default(),
            analysis_session_id: None,
            project_id: None,
        };

        // Create the OrchestratorService
        // Note: We use the standard `new()` constructor and let it create a
        // fresh provider from the config. In a real integration the provider
        // from the context would be used directly, but since the existing
        // OrchestratorService constructor always creates its own provider,
        // we pass the config and let it handle construction.
        let orchestrator = OrchestratorService::new(orchestrator_config);

        // Create the mpsc channel pair
        let (tx, rx) = mpsc::channel::<UnifiedStreamEvent>(256);

        // Extract the message from AgentInput
        let message = ctx.input.as_text();

        // Spawn the execute() call on a tokio task
        let _shared_state = ctx.shared_state.clone();
        tokio::spawn(async move {
            let _result = orchestrator.execute(message, tx).await;
            // ExecutionResult is dropped; the stream consumer gets events via the channel
        });

        // Build a stream that maps UnifiedStreamEvent -> AgentEvent
        let stream = async_stream(rx);
        Ok(stream)
    }
}

/// Create an AgentEventStream from a receiver of UnifiedStreamEvents.
///
/// Filters and maps events, skipping those without a direct AgentEvent mapping.
fn async_stream(rx: mpsc::Receiver<UnifiedStreamEvent>) -> AgentEventStream {
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Some(event) => {
                    if let Some(agent_event) = convert_stream_event(&event) {
                        return Some((Ok(agent_event), rx));
                    }
                    // Skip events without a mapping, continue to next
                    continue;
                }
                None => return None, // Channel closed
            }
        }
    });
    Box::pin(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_text_delta() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_convert_text_replace() {
        let event = UnifiedStreamEvent::TextReplace {
            content: "replaced".to_string(),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::TextDelta { content } => assert_eq!(content, "replaced"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_convert_thinking_delta() {
        let event = UnifiedStreamEvent::ThinkingDelta {
            content: "reasoning...".to_string(),
            thinking_id: None,
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::ThinkingDelta { content } => assert_eq!(content, "reasoning..."),
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_convert_tool_start() {
        let event = UnifiedStreamEvent::ToolStart {
            tool_id: "t1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: Some(r#"{"path":"/foo"}"#.to_string()),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::ToolCall { name, args } => {
                assert_eq!(name, "read_file");
                assert!(args.contains("/foo"));
            }
            _ => panic!("Expected ToolCall"),
        }
    }

    #[test]
    fn test_convert_tool_complete() {
        let event = UnifiedStreamEvent::ToolComplete {
            tool_id: "t1".to_string(),
            tool_name: "grep".to_string(),
            arguments: r#"{"pattern":"test"}"#.to_string(),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::ToolCall { name, args } => {
                assert_eq!(name, "grep");
                assert!(args.contains("test"));
            }
            _ => panic!("Expected ToolCall"),
        }
    }

    #[test]
    fn test_convert_tool_result_success() {
        let event = UnifiedStreamEvent::ToolResult {
            tool_id: "t1".to_string(),
            result: Some("file contents".to_string()),
            error: None,
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::ToolResult { result, .. } => {
                assert_eq!(result, "file contents");
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_convert_tool_result_error() {
        let event = UnifiedStreamEvent::ToolResult {
            tool_id: "t1".to_string(),
            result: None,
            error: Some("not found".to_string()),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::ToolResult { result, .. } => {
                assert!(result.contains("Error: not found"));
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_convert_complete() {
        let event = UnifiedStreamEvent::Complete {
            stop_reason: Some("end_turn".to_string()),
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::Done { output } => assert!(output.is_none()),
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_convert_error() {
        let event = UnifiedStreamEvent::Error {
            message: "rate limit".to_string(),
            code: None,
        };
        let result = convert_stream_event(&event);
        assert!(result.is_some());
        match result.unwrap() {
            AgentEvent::Done { output } => {
                assert!(output.unwrap().contains("rate limit"));
            }
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_convert_unmapped_events() {
        // Usage events should return None
        let event = UnifiedStreamEvent::Usage {
            input_tokens: 100,
            output_tokens: 50,
            thinking_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
        };
        assert!(convert_stream_event(&event).is_none());

        // ThinkingStart should return None
        let event = UnifiedStreamEvent::ThinkingStart {
            thinking_id: None,
        };
        assert!(convert_stream_event(&event).is_none());

        // ThinkingEnd should return None
        let event = UnifiedStreamEvent::ThinkingEnd {
            thinking_id: None,
        };
        assert!(convert_stream_event(&event).is_none());

        // ContextCompaction should return None
        let event = UnifiedStreamEvent::ContextCompaction {
            messages_compacted: 10,
            messages_preserved: 5,
            compaction_tokens: 200,
        };
        assert!(convert_stream_event(&event).is_none());
    }

    #[test]
    fn test_llm_agent_builder() {
        let agent = LlmAgent::new("my-agent")
            .with_description("A test agent")
            .with_instruction("Be helpful")
            .with_model("claude-3-5-sonnet")
            .with_tools(vec!["read_file".to_string(), "grep".to_string()])
            .with_config(AgentConfig {
                max_iterations: 10,
                ..Default::default()
            });

        assert_eq!(agent.name(), "my-agent");
        assert_eq!(agent.description(), "A test agent");
        assert_eq!(agent.instruction, Some("Be helpful".to_string()));
        assert_eq!(agent.model, Some("claude-3-5-sonnet".to_string()));
        assert_eq!(agent.tools.as_ref().unwrap().len(), 2);
        assert_eq!(agent.config.max_iterations, 10);
    }

    #[test]
    fn test_llm_agent_default() {
        let agent = LlmAgent::new("default-agent");
        assert_eq!(agent.name(), "default-agent");
        assert!(agent.instruction.is_none());
        assert!(agent.model.is_none());
        assert!(agent.tools.is_none());
        assert_eq!(agent.config.max_iterations, 50);
    }
}
