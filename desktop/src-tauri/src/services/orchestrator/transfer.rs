//! Agent Runtime Transfer
//!
//! Enables runtime agent-to-agent handoff during execution. When an agent emits
//! an `AgentTransfer` event, the transfer handler:
//! 1. Looks up the target agent in the `ComposerRegistry`
//! 2. Creates a new `AgentContext` with shared session state
//! 3. Continues execution with the target agent
//! 4. Tracks transfer chains for debugging (with max depth to prevent cycles)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services::agent_composer::registry::ComposerRegistry;
use crate::services::agent_composer::types::{Agent, AgentContext, AgentEvent, AgentEventStream};
use crate::utils::error::{AppError, AppResult};

/// Default maximum transfer depth to prevent infinite cycles.
const DEFAULT_MAX_TRANSFER_DEPTH: usize = 10;

// ============================================================================
// Transfer Request
// ============================================================================

/// A request to transfer execution to another agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    /// Name of the target agent to transfer to.
    pub target_agent: String,
    /// Message to pass to the target agent.
    pub message: String,
    /// Optional additional context data for the target agent.
    #[serde(default)]
    pub context: HashMap<String, Value>,
}

// ============================================================================
// Transfer Chain
// ============================================================================

/// Tracks the chain of agent transfers for debugging and cycle detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferChain {
    /// Ordered list of transfer entries.
    entries: Vec<TransferEntry>,
    /// Set of visited agent names for cycle detection.
    #[serde(skip)]
    visited: HashSet<String>,
    /// Maximum allowed transfer depth.
    max_depth: usize,
}

/// A single entry in the transfer chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEntry {
    /// Source agent name.
    pub from_agent: String,
    /// Target agent name.
    pub to_agent: String,
    /// Transfer message.
    pub message: String,
    /// ISO 8601 timestamp of the transfer.
    pub timestamp: String,
}

impl TransferChain {
    /// Create a new empty transfer chain with default max depth.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            visited: HashSet::new(),
            max_depth: DEFAULT_MAX_TRANSFER_DEPTH,
        }
    }

    /// Create a new transfer chain with a custom max depth.
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            entries: Vec::new(),
            visited: HashSet::new(),
            max_depth,
        }
    }

    /// Record a transfer in the chain. Returns an error if:
    /// - The max depth would be exceeded
    /// - The target agent has already been visited (cycle)
    pub fn record_transfer(
        &mut self,
        from_agent: &str,
        to_agent: &str,
        message: &str,
    ) -> AppResult<()> {
        // Check depth limit
        if self.entries.len() >= self.max_depth {
            return Err(AppError::validation(format!(
                "Transfer chain depth limit exceeded (max {}). Chain: {}",
                self.max_depth,
                self.chain_summary()
            )));
        }

        // Check for cycles
        if self.visited.contains(to_agent) {
            return Err(AppError::validation(format!(
                "Transfer cycle detected: agent '{}' was already visited. Chain: {}",
                to_agent,
                self.chain_summary()
            )));
        }

        // Mark the source as visited (first transfer marks the originator)
        if self.entries.is_empty() {
            self.visited.insert(from_agent.to_string());
        }
        self.visited.insert(to_agent.to_string());

        self.entries.push(TransferEntry {
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        Ok(())
    }

    /// Get the current depth of the transfer chain.
    pub fn depth(&self) -> usize {
        self.entries.len()
    }

    /// Check if a specific agent has been visited.
    pub fn has_visited(&self, agent_name: &str) -> bool {
        self.visited.contains(agent_name)
    }

    /// Get all entries in the chain.
    pub fn entries(&self) -> &[TransferEntry] {
        &self.entries
    }

    /// Generate a human-readable summary of the chain.
    pub fn chain_summary(&self) -> String {
        if self.entries.is_empty() {
            return "(empty)".to_string();
        }
        self.entries
            .iter()
            .map(|e| format!("{} -> {}", e.from_agent, e.to_agent))
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    /// Get the last agent in the chain (current executing agent).
    pub fn current_agent(&self) -> Option<&str> {
        self.entries.last().map(|e| e.to_agent.as_str())
    }
}

impl Default for TransferChain {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Transfer Handler
// ============================================================================

/// Handles agent transfer events during execution.
///
/// When an `AgentTransfer` event is detected, the handler:
/// 1. Validates the transfer request
/// 2. Looks up the target agent in the registry
/// 3. Records the transfer in the chain
/// 4. Creates a new context for the target agent
/// 5. Executes the target agent
pub struct TransferHandler {
    /// Registry of available agents.
    registry: Arc<ComposerRegistry>,
    /// Transfer chain tracking.
    chain: TransferChain,
}

impl TransferHandler {
    /// Create a new transfer handler with the given registry.
    pub fn new(registry: Arc<ComposerRegistry>) -> Self {
        Self {
            registry,
            chain: TransferChain::new(),
        }
    }

    /// Create a new transfer handler with a custom max transfer depth.
    pub fn with_max_depth(registry: Arc<ComposerRegistry>, max_depth: usize) -> Self {
        Self {
            registry,
            chain: TransferChain::with_max_depth(max_depth),
        }
    }

    /// Handle an AgentTransfer event.
    ///
    /// Looks up the target agent, records the transfer, creates a new context,
    /// and executes the target agent.
    pub async fn handle_transfer(
        &mut self,
        from_agent: &str,
        target: &str,
        message: &str,
        base_ctx: &AgentContext,
    ) -> AppResult<AgentEventStream> {
        // Look up target agent
        let target_agent = self.registry.get(target).ok_or_else(|| {
            AppError::not_found(format!(
                "Transfer target agent '{}' not found in registry",
                target
            ))
        })?;

        // Record the transfer
        self.chain.record_transfer(from_agent, target, message)?;

        // Create new context for the target agent
        let mut new_ctx = base_ctx.clone();
        // Update shared state with transfer metadata
        {
            let mut shared = new_ctx.shared_state.write().await;
            shared.insert("__transfer_from".to_string(), serde_json::json!(from_agent));
            shared.insert("__transfer_message".to_string(), serde_json::json!(message));
            shared.insert(
                "__transfer_depth".to_string(),
                serde_json::json!(self.chain.depth()),
            );
        }

        // Update input with the transfer message
        new_ctx.input =
            crate::services::agent_composer::types::AgentInput::Text(message.to_string());

        // Execute the target agent
        target_agent.run(new_ctx).await
    }

    /// Get a reference to the transfer chain.
    pub fn chain(&self) -> &TransferChain {
        &self.chain
    }

    /// Get a mutable reference to the transfer chain.
    pub fn chain_mut(&mut self) -> &mut TransferChain {
        &mut self.chain
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // TransferChain Tests
    // ========================================================================

    #[test]
    fn test_transfer_chain_new() {
        let chain = TransferChain::new();
        assert_eq!(chain.depth(), 0);
        assert!(chain.entries().is_empty());
        assert_eq!(chain.chain_summary(), "(empty)");
        assert!(chain.current_agent().is_none());
    }

    #[test]
    fn test_transfer_chain_with_max_depth() {
        let chain = TransferChain::with_max_depth(5);
        assert_eq!(chain.max_depth, 5);
        assert_eq!(chain.depth(), 0);
    }

    #[test]
    fn test_transfer_chain_record_single() {
        let mut chain = TransferChain::new();
        chain
            .record_transfer("agent-a", "agent-b", "handle this")
            .unwrap();

        assert_eq!(chain.depth(), 1);
        assert!(chain.has_visited("agent-a"));
        assert!(chain.has_visited("agent-b"));
        assert!(!chain.has_visited("agent-c"));
        assert_eq!(chain.current_agent(), Some("agent-b"));
        assert_eq!(chain.chain_summary(), "agent-a -> agent-b");
    }

    #[test]
    fn test_transfer_chain_record_multiple() {
        let mut chain = TransferChain::new();
        chain
            .record_transfer("agent-a", "agent-b", "first transfer")
            .unwrap();
        chain
            .record_transfer("agent-b", "agent-c", "second transfer")
            .unwrap();

        assert_eq!(chain.depth(), 2);
        assert!(chain.has_visited("agent-a"));
        assert!(chain.has_visited("agent-b"));
        assert!(chain.has_visited("agent-c"));
        assert_eq!(chain.current_agent(), Some("agent-c"));
    }

    #[test]
    fn test_transfer_chain_cycle_detection() {
        let mut chain = TransferChain::new();
        chain
            .record_transfer("agent-a", "agent-b", "first")
            .unwrap();
        chain
            .record_transfer("agent-b", "agent-c", "second")
            .unwrap();

        // Trying to transfer back to agent-a should fail
        let result = chain.record_transfer("agent-c", "agent-a", "cycle");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cycle detected"));
        assert!(err.contains("agent-a"));
    }

    #[test]
    fn test_transfer_chain_depth_limit() {
        let mut chain = TransferChain::with_max_depth(3);
        chain.record_transfer("a", "b", "1").unwrap();
        chain.record_transfer("b", "c", "2").unwrap();
        chain.record_transfer("c", "d", "3").unwrap();

        // Fourth transfer should exceed depth limit
        let result = chain.record_transfer("d", "e", "4");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("depth limit exceeded"));
    }

    #[test]
    fn test_transfer_chain_entries() {
        let mut chain = TransferChain::new();
        chain.record_transfer("agent-a", "agent-b", "msg1").unwrap();
        chain.record_transfer("agent-b", "agent-c", "msg2").unwrap();

        let entries = chain.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].from_agent, "agent-a");
        assert_eq!(entries[0].to_agent, "agent-b");
        assert_eq!(entries[0].message, "msg1");
        assert_eq!(entries[1].from_agent, "agent-b");
        assert_eq!(entries[1].to_agent, "agent-c");
        assert_eq!(entries[1].message, "msg2");
    }

    #[test]
    fn test_transfer_chain_default() {
        let chain = TransferChain::default();
        assert_eq!(chain.depth(), 0);
        assert_eq!(chain.max_depth, DEFAULT_MAX_TRANSFER_DEPTH);
    }

    #[test]
    fn test_transfer_chain_summary_format() {
        let mut chain = TransferChain::new();
        chain
            .record_transfer("planner", "coder", "implement feature")
            .unwrap();
        chain
            .record_transfer("coder", "reviewer", "review changes")
            .unwrap();

        assert_eq!(
            chain.chain_summary(),
            "planner -> coder -> coder -> reviewer"
        );
    }

    // ========================================================================
    // TransferRequest Tests
    // ========================================================================

    #[test]
    fn test_transfer_request_serialization() {
        let req = TransferRequest {
            target_agent: "reviewer".to_string(),
            message: "Please review this code".to_string(),
            context: {
                let mut m = HashMap::new();
                m.insert("file".to_string(), serde_json::json!("src/main.rs"));
                m
            },
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: TransferRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.target_agent, "reviewer");
        assert_eq!(parsed.message, "Please review this code");
        assert!(parsed.context.contains_key("file"));
    }

    #[test]
    fn test_transfer_request_default_context() {
        let json = r#"{"target_agent": "test", "message": "hello"}"#;
        let req: TransferRequest = serde_json::from_str(json).unwrap();
        assert!(req.context.is_empty());
    }

    // ========================================================================
    // TransferHandler Tests
    // ========================================================================

    #[test]
    fn test_transfer_handler_new() {
        let registry = Arc::new(ComposerRegistry::new());
        let handler = TransferHandler::new(registry);
        assert_eq!(handler.chain().depth(), 0);
    }

    #[test]
    fn test_transfer_handler_with_max_depth() {
        let registry = Arc::new(ComposerRegistry::new());
        let handler = TransferHandler::with_max_depth(registry, 5);
        assert_eq!(handler.chain().max_depth, 5);
    }

    #[tokio::test]
    async fn test_transfer_handler_target_not_found() {
        let registry = Arc::new(ComposerRegistry::new());
        let mut handler = TransferHandler::new(registry);

        use crate::services::agent_composer::types::*;
        use crate::services::orchestrator::hooks::AgenticHooks;
        use std::path::PathBuf;
        use tokio::sync::RwLock;

        // Create a mock provider
        struct MockProv {
            config: crate::services::llm::ProviderConfig,
        }
        impl MockProv {
            fn new() -> Self {
                Self {
                    config: crate::services::llm::ProviderConfig::default(),
                }
            }
        }
        #[async_trait::async_trait]
        impl crate::services::llm::LlmProvider for MockProv {
            fn name(&self) -> &'static str {
                "mock"
            }
            fn model(&self) -> &str {
                "mock"
            }
            fn supports_thinking(&self) -> bool {
                false
            }
            fn supports_tools(&self) -> bool {
                false
            }
            async fn send_message(
                &self,
                _: Vec<crate::services::llm::Message>,
                _: Option<String>,
                _: Vec<crate::services::llm::ToolDefinition>,
                _: crate::services::llm::LlmRequestOptions,
            ) -> crate::services::llm::LlmResult<crate::services::llm::LlmResponse> {
                unimplemented!()
            }
            async fn stream_message(
                &self,
                _: Vec<crate::services::llm::Message>,
                _: Option<String>,
                _: Vec<crate::services::llm::ToolDefinition>,
                _: tokio::sync::mpsc::Sender<crate::services::streaming::UnifiedStreamEvent>,
                _: crate::services::llm::LlmRequestOptions,
            ) -> crate::services::llm::LlmResult<crate::services::llm::LlmResponse> {
                unimplemented!()
            }
            async fn health_check(&self) -> crate::services::llm::LlmResult<()> {
                Ok(())
            }
            fn config(&self) -> &crate::services::llm::ProviderConfig {
                &self.config
            }
        }

        let ctx = AgentContext {
            session_id: "test".to_string(),
            project_root: PathBuf::from("/tmp"),
            provider: Arc::new(MockProv::new()),
            tool_executor: Arc::new(crate::services::tools::ToolExecutor::new(&PathBuf::from(
                "/tmp",
            ))),
            plugin_manager: None,
            hooks: Arc::new(AgenticHooks::new()),
            input: AgentInput::Text("test".to_string()),
            shared_state: Arc::new(RwLock::new(HashMap::new())),
            config: AgentConfig::default(),
            orchestrator_ctx: None,
        };

        let result = handler
            .handle_transfer("agent-a", "nonexistent", "hello", &ctx)
            .await;
        assert!(result.is_err());
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("Expected error"),
        };
        assert!(err.contains("not found"));
    }
}
