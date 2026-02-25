//! Unified Stream Event Types
//!
//! Provider-agnostic event types and adapter trait for processing real-time
//! LLM responses from multiple providers. These types are shared across the
//! LLM crate (provider implementations) and the main crate (orchestrator,
//! agent executor, etc.).

use serde::{Deserialize, Serialize};

/// Unified streaming event that all provider adapters convert to.
/// This provides a consistent interface for the frontend regardless of LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedStreamEvent {
    /// Text content delta from the model
    TextDelta { content: String },

    /// Replace previously streamed text with cleaned version.
    /// Used after prompt-fallback tool call extraction to remove raw tool call
    /// XML/blocks that were streamed before parsing.
    TextReplace { content: String },

    /// Start of a thinking/reasoning block
    ThinkingStart {
        /// Optional thinking block ID for correlation
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// Thinking content delta
    ThinkingDelta {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// End of a thinking/reasoning block
    ThinkingEnd {
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_id: Option<String>,
    },

    /// Start of a tool call
    ToolStart {
        tool_id: String,
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<String>,
    },

    /// Tool call complete with accumulated arguments
    ToolComplete {
        tool_id: String,
        tool_name: String,
        /// Complete JSON string of tool arguments
        arguments: String,
    },

    /// Tool execution result
    ToolResult {
        tool_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Token usage information
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_creation_tokens: Option<u32>,
    },

    /// Search citation data from provider-native web search (Qwen enable_search, GLM web_search, etc.)
    SearchCitations {
        citations: Vec<SearchCitationEntry>,
    },

    /// Error during streaming
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },

    /// Stream complete
    Complete {
        #[serde(skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
    },

    // ========================================================================
    // Sub-agent events (for Task tool)
    // ========================================================================
    /// A sub-agent task has started
    SubAgentStart {
        sub_agent_id: String,
        /// Truncated prompt summary
        prompt: String,
        /// Legacy task type for backward compatibility
        #[serde(skip_serializing_if = "Option::is_none")]
        task_type: Option<String>,
        /// Sub-agent type: "general-purpose", "explore", "plan", "bash"
        #[serde(skip_serializing_if = "Option::is_none")]
        subagent_type: Option<String>,
        /// Nesting depth (root = 0)
        #[serde(default)]
        depth: u32,
    },

    /// A sub-agent task has completed
    SubAgentEnd {
        sub_agent_id: String,
        success: bool,
        usage: serde_json::Value,
    },

    /// An event originating from a sub-agent, tagged with sub-agent context.
    /// Wraps the inner event's data as JSON to avoid recursive enum nesting
    /// (`Box<UnifiedStreamEvent>` is incompatible with `#[serde(tag = "type")]`).
    SubAgentEvent {
        sub_agent_id: String,
        depth: u32,
        /// The original event type (e.g., "text_delta", "tool_start")
        event_type: String,
        /// The original event's fields (excluding "type")
        event_data: serde_json::Value,
    },

    // ========================================================================
    // Agent transfer events (runtime agent-to-agent handoff)
    // ========================================================================
    /// An agent transfer has started — execution is being handed off.
    AgentTransferStart {
        /// Name of the source agent initiating the transfer.
        from_agent: String,
        /// Name of the target agent receiving execution.
        to_agent: String,
        /// Transfer message/context passed to the target agent.
        message: String,
        /// Current depth of the transfer chain.
        depth: usize,
    },

    /// An agent transfer has completed.
    AgentTransferEnd {
        /// Name of the source agent that initiated the transfer.
        from_agent: String,
        /// Name of the target agent that was executed.
        to_agent: String,
        /// Whether the transfer completed successfully.
        success: bool,
        /// Error message if the transfer failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // ========================================================================
    // Analysis pipeline events (evidence-first project analysis mode)
    // ========================================================================
    /// Analysis run has started and artifacts will be persisted to disk.
    AnalysisRunStarted {
        run_id: String,
        run_dir: String,
        request: String,
    },

    /// Phase planning metadata was produced before execution.
    AnalysisPhasePlanned {
        run_id: String,
        phase_id: String,
        title: String,
        objective: String,
        worker_count: usize,
        layers: Vec<String>,
    },

    /// A phase worker (sub-agent task) was planned.
    AnalysisSubAgentPlanned {
        run_id: String,
        phase_id: String,
        sub_agent_id: String,
        role: String,
        objective: String,
    },

    /// Sub-agent progress signal for the analysis planner.
    AnalysisSubAgentProgress {
        run_id: String,
        phase_id: String,
        sub_agent_id: String,
        status: String,
        message: String,
    },

    /// Inventory index has been built for analysis.
    AnalysisIndexBuilt {
        run_id: String,
        inventory_total_files: usize,
        test_files_total: usize,
        chunk_count: usize,
    },

    /// Analysis phase has started
    AnalysisPhaseStart {
        phase_id: String,
        title: String,
        objective: String,
    },

    /// Analysis phase attempt has started
    AnalysisPhaseAttemptStart {
        phase_id: String,
        attempt: u32,
        max_attempts: u32,
        required_tools: Vec<String>,
    },

    /// Analysis phase progress update
    AnalysisPhaseProgress { phase_id: String, message: String },

    /// Chunk-level processing started.
    AnalysisChunkStarted {
        run_id: String,
        phase_id: String,
        chunk_id: String,
        component: String,
        file_count: usize,
    },

    /// Chunk-level processing completed.
    AnalysisChunkCompleted {
        run_id: String,
        phase_id: String,
        chunk_id: String,
        observed_paths: usize,
        read_files: usize,
    },

    /// Evidence captured during analysis (from tool activity)
    AnalysisEvidence {
        phase_id: String,
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        file_path: Option<String>,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        success: Option<bool>,
    },

    /// Analysis phase has completed
    AnalysisPhaseEnd {
        phase_id: String,
        success: bool,
        usage: serde_json::Value,
        metrics: serde_json::Value,
    },

    /// Analysis phase attempt has completed
    AnalysisPhaseAttemptEnd {
        phase_id: String,
        attempt: u32,
        success: bool,
        metrics: serde_json::Value,
        gate_failures: Vec<String>,
    },

    /// Analysis phase completed with partial evidence due to budget or gate constraints
    AnalysisPhaseDegraded {
        phase_id: String,
        attempt: u32,
        reasons: Vec<String>,
    },

    /// Analysis gate failure detail
    AnalysisGateFailure {
        phase_id: String,
        attempt: u32,
        reasons: Vec<String>,
    },

    /// Validation result emitted near the end of analysis
    AnalysisValidation { status: String, issues: Vec<String> },

    /// Overall analysis run summary
    AnalysisRunSummary {
        success: bool,
        phase_results: Vec<String>,
        total_metrics: serde_json::Value,
    },

    /// Coverage metrics updated while processing analysis artifacts.
    AnalysisCoverageUpdated {
        run_id: String,
        metrics: serde_json::Value,
    },

    /// Analysis run completed and persisted final artifacts.
    AnalysisRunCompleted {
        run_id: String,
        success: bool,
        manifest_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        report_path: Option<String>,
    },

    /// Batch summary merge completed before final synthesis.
    AnalysisMergeCompleted {
        run_id: String,
        phase_count: usize,
        chunk_summary_count: usize,
    },

    /// Overall analysis returned a usable but partial result
    AnalysisPartial {
        successful_phases: usize,
        partial_phases: usize,
        failed_phases: usize,
        reason: String,
    },

    // ========================================================================
    // Session-based execution events (for standalone mode)
    // ========================================================================
    /// Session progress update
    SessionProgress {
        session_id: String,
        progress: serde_json::Value,
    },

    /// Session execution complete
    SessionComplete {
        session_id: String,
        success: bool,
        completed_stories: usize,
        total_stories: usize,
    },

    /// Story execution started
    StoryStart {
        session_id: String,
        story_id: String,
        story_title: String,
        story_index: usize,
        total_stories: usize,
    },

    /// Story execution complete
    StoryComplete {
        session_id: String,
        story_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Quality gates execution result
    QualityGatesResult {
        session_id: String,
        story_id: String,
        passed: bool,
        summary: serde_json::Value,
    },

    /// Context compaction occurred (messages were summarized to reduce context size)
    ContextCompaction {
        /// Number of messages that were compacted into a summary
        messages_compacted: usize,
        /// Number of recent messages preserved as-is
        messages_preserved: usize,
        /// Token count of the compaction summary
        compaction_tokens: u32,
    },

    // ========================================================================
    // Tool permission events (runtime approval gate)
    // ========================================================================
    /// Backend requests frontend approval for a tool execution
    ToolPermissionRequest {
        /// Unique identifier for this approval request
        request_id: String,
        /// Session that owns this tool call
        session_id: String,
        /// Name of the tool being invoked (e.g., "Bash", "Write")
        tool_name: String,
        /// JSON-serialized arguments to the tool
        arguments: String,
        /// Risk classification: "ReadOnly", "SafeWrite", or "Dangerous"
        risk: String,
    },

    /// Frontend responds to a tool permission request
    ToolPermissionResponse {
        /// Matches the request_id from ToolPermissionRequest
        request_id: String,
        /// Whether the tool execution is allowed
        allowed: bool,
    },
}

/// A single search citation entry from provider-native web search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchCitationEntry {
    /// Citation index (position in search results)
    pub index: i32,
    /// Title of the search result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// URL of the search result
    pub url: String,
    /// Name of the source website
    pub site_name: String,
    /// Icon URL of the source website
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Errors that can occur during stream adaptation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdapterError {
    /// Invalid format that couldn't be parsed
    InvalidFormat(String),
    /// JSON/data parsing error
    ParseError(String),
    /// Event type not supported by this adapter
    UnsupportedEvent(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            AdapterError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AdapterError::UnsupportedEvent(msg) => write!(f, "Unsupported event: {}", msg),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Trait for adapting provider-specific stream formats to unified events.
///
/// All provider adapters (Claude, OpenAI, DeepSeek, Ollama) implement this trait
/// to provide a consistent interface for stream processing.
pub trait StreamAdapter: Send + Sync {
    /// Returns the provider name for logging and identification.
    fn provider_name(&self) -> &'static str;

    /// Returns whether this adapter/provider supports thinking blocks.
    fn supports_thinking(&self) -> bool;

    /// Returns whether this adapter/provider supports tool calls.
    fn supports_tools(&self) -> bool;

    /// Adapt a raw stream line/chunk to unified events.
    ///
    /// A single input line may produce zero, one, or multiple events.
    fn adapt(&mut self, input: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError>;

    /// Reset adapter state for a new stream.
    fn reset(&mut self) {
        // Default implementation does nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta_serialization() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text_delta\""));
        assert!(json.contains("\"content\":\"Hello\""));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_sub_agent_event_serialization() {
        let event = UnifiedStreamEvent::SubAgentEvent {
            sub_agent_id: "agent-1".to_string(),
            depth: 1,
            event_type: "text_delta".to_string(),
            event_data: serde_json::json!({"content": "hello"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"sub_agent_event\""));
        assert!(json.contains("\"sub_agent_id\":\"agent-1\""));
        assert!(json.contains("\"event_type\":\"text_delta\""));
        assert!(json.contains("\"depth\":1"));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_adapter_error_display() {
        let err = AdapterError::InvalidFormat("bad json".to_string());
        assert_eq!(err.to_string(), "Invalid format: bad json");

        let err = AdapterError::ParseError("unexpected token".to_string());
        assert_eq!(err.to_string(), "Parse error: unexpected token");

        let err = AdapterError::UnsupportedEvent("ping".to_string());
        assert_eq!(err.to_string(), "Unsupported event: ping");
    }
}
