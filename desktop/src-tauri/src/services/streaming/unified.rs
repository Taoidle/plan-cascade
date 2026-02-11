//! Unified Stream Event Types
//!
//! Provider-agnostic event types that all adapters convert to.

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
        #[serde(skip_serializing_if = "Option::is_none")]
        task_type: Option<String>,
    },

    /// A sub-agent task has completed
    SubAgentEnd {
        sub_agent_id: String,
        success: bool,
        usage: serde_json::Value,
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
    fn test_thinking_events_serialization() {
        let start = UnifiedStreamEvent::ThinkingStart {
            thinking_id: Some("t1".to_string()),
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.contains("\"type\":\"thinking_start\""));

        let delta = UnifiedStreamEvent::ThinkingDelta {
            content: "reasoning...".to_string(),
            thinking_id: None,
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("\"type\":\"thinking_delta\""));
        assert!(!json.contains("thinking_id")); // None should be skipped

        let end = UnifiedStreamEvent::ThinkingEnd { thinking_id: None };
        let json = serde_json::to_string(&end).unwrap();
        assert!(json.contains("\"type\":\"thinking_end\""));
    }

    #[test]
    fn test_tool_events_serialization() {
        let start = UnifiedStreamEvent::ToolStart {
            tool_id: "tool_1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: Some("{\"path\": \"/foo\"}".to_string()),
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.contains("\"type\":\"tool_start\""));
        assert!(json.contains("\"tool_name\":\"read_file\""));

        let result = UnifiedStreamEvent::ToolResult {
            tool_id: "tool_1".to_string(),
            result: Some("file contents".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));
    }

    #[test]
    fn test_usage_serialization() {
        let usage = UnifiedStreamEvent::Usage {
            input_tokens: 100,
            output_tokens: 50,
            thinking_tokens: Some(20),
            cache_read_tokens: None,
            cache_creation_tokens: None,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"type\":\"usage\""));
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"thinking_tokens\":20"));
        assert!(!json.contains("cache_read_tokens")); // None skipped
    }

    #[test]
    fn test_analysis_phase_event_serialization() {
        let event = UnifiedStreamEvent::AnalysisPhaseStart {
            phase_id: "structure_discovery".to_string(),
            title: "Structure Discovery".to_string(),
            objective: "Map project entrypoints and manifests".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"analysis_phase_start\""));
        assert!(json.contains("\"phase_id\":\"structure_discovery\""));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, parsed);
    }

    #[test]
    fn test_analysis_run_events_serialization() {
        let started = UnifiedStreamEvent::AnalysisRunStarted {
            run_id: "run-123".to_string(),
            run_dir: "/tmp/run-123".to_string(),
            request: "Analyze this project".to_string(),
        };
        let started_json = serde_json::to_string(&started).unwrap();
        assert!(started_json.contains("\"type\":\"analysis_run_started\""));

        let planned = UnifiedStreamEvent::AnalysisPhasePlanned {
            run_id: "run-123".to_string(),
            phase_id: "structure_discovery".to_string(),
            title: "Structure Discovery".to_string(),
            objective: "map".to_string(),
            worker_count: 2,
            layers: vec!["Layer 1".to_string()],
        };
        let planned_json = serde_json::to_string(&planned).unwrap();
        assert!(planned_json.contains("\"type\":\"analysis_phase_planned\""));

        let indexed = UnifiedStreamEvent::AnalysisIndexBuilt {
            run_id: "run-123".to_string(),
            inventory_total_files: 320,
            test_files_total: 42,
            chunk_count: 18,
        };
        let indexed_json = serde_json::to_string(&indexed).unwrap();
        assert!(indexed_json.contains("\"type\":\"analysis_index_built\""));

        let chunk_started = UnifiedStreamEvent::AnalysisChunkStarted {
            run_id: "run-123".to_string(),
            phase_id: "architecture_trace".to_string(),
            chunk_id: "python-core-001".to_string(),
            component: "python-core".to_string(),
            file_count: 24,
        };
        let chunk_started_json = serde_json::to_string(&chunk_started).unwrap();
        assert!(chunk_started_json.contains("\"type\":\"analysis_chunk_started\""));

        let chunk_completed = UnifiedStreamEvent::AnalysisChunkCompleted {
            run_id: "run-123".to_string(),
            phase_id: "architecture_trace".to_string(),
            chunk_id: "python-core-001".to_string(),
            observed_paths: 19,
            read_files: 3,
        };
        let chunk_completed_json = serde_json::to_string(&chunk_completed).unwrap();
        assert!(chunk_completed_json.contains("\"type\":\"analysis_chunk_completed\""));

        let completed = UnifiedStreamEvent::AnalysisRunCompleted {
            run_id: "run-123".to_string(),
            success: true,
            manifest_path: "/tmp/run-123/manifest.json".to_string(),
            report_path: Some("/tmp/run-123/final/report.md".to_string()),
        };
        let completed_json = serde_json::to_string(&completed).unwrap();
        assert!(completed_json.contains("\"type\":\"analysis_run_completed\""));

        let merged = UnifiedStreamEvent::AnalysisMergeCompleted {
            run_id: "run-123".to_string(),
            phase_count: 3,
            chunk_summary_count: 26,
        };
        let merged_json = serde_json::to_string(&merged).unwrap();
        assert!(merged_json.contains("\"type\":\"analysis_merge_completed\""));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&completed_json).unwrap();
        assert_eq!(completed, parsed);
    }

    #[test]
    fn test_analysis_attempt_and_gate_events_serialization() {
        let start = UnifiedStreamEvent::AnalysisPhaseAttemptStart {
            phase_id: "structure_discovery".to_string(),
            attempt: 1,
            max_attempts: 3,
            required_tools: vec!["Cwd".to_string(), "LS".to_string()],
        };
        let start_json = serde_json::to_string(&start).unwrap();
        assert!(start_json.contains("\"type\":\"analysis_phase_attempt_start\""));
        assert!(start_json.contains("\"attempt\":1"));

        let end = UnifiedStreamEvent::AnalysisPhaseAttemptEnd {
            phase_id: "structure_discovery".to_string(),
            attempt: 1,
            success: false,
            metrics: serde_json::json!({ "tool_calls": 2, "read_calls": 0 }),
            gate_failures: vec!["read_calls 0 < required 1".to_string()],
        };
        let end_json = serde_json::to_string(&end).unwrap();
        assert!(end_json.contains("\"type\":\"analysis_phase_attempt_end\""));
        assert!(end_json.contains("\"success\":false"));

        let gate = UnifiedStreamEvent::AnalysisGateFailure {
            phase_id: "structure_discovery".to_string(),
            attempt: 1,
            reasons: vec!["required tool 'LS' not used".to_string()],
        };
        let gate_json = serde_json::to_string(&gate).unwrap();
        assert!(gate_json.contains("\"type\":\"analysis_gate_failure\""));

        let degraded = UnifiedStreamEvent::AnalysisPhaseDegraded {
            phase_id: "architecture_trace".to_string(),
            attempt: 2,
            reasons: vec!["token budget pressure".to_string()],
        };
        let degraded_json = serde_json::to_string(&degraded).unwrap();
        assert!(degraded_json.contains("\"type\":\"analysis_phase_degraded\""));

        let summary = UnifiedStreamEvent::AnalysisRunSummary {
            success: false,
            phase_results: vec!["successful_phases=2".to_string()],
            total_metrics: serde_json::json!({ "iterations": 9 }),
        };
        let summary_json = serde_json::to_string(&summary).unwrap();
        assert!(summary_json.contains("\"type\":\"analysis_run_summary\""));

        let partial = UnifiedStreamEvent::AnalysisPartial {
            successful_phases: 2,
            partial_phases: 1,
            failed_phases: 0,
            reason: "best effort".to_string(),
        };
        let partial_json = serde_json::to_string(&partial).unwrap();
        assert!(partial_json.contains("\"type\":\"analysis_partial\""));

        let parsed: UnifiedStreamEvent = serde_json::from_str(&summary_json).unwrap();
        assert_eq!(summary, parsed);
    }

    #[test]
    fn test_error_serialization() {
        let error = UnifiedStreamEvent::Error {
            message: "Rate limit exceeded".to_string(),
            code: Some("rate_limit".to_string()),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"Rate limit exceeded\""));
    }

    #[test]
    fn test_complete_serialization() {
        let complete = UnifiedStreamEvent::Complete {
            stop_reason: Some("end_turn".to_string()),
        };
        let json = serde_json::to_string(&complete).unwrap();
        assert!(json.contains("\"type\":\"complete\""));
        assert!(json.contains("\"stop_reason\":\"end_turn\""));
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
