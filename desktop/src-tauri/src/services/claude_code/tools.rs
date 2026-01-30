//! ToolTracker
//!
//! Tracks tool executions from Claude Code, maintaining history and status.
//! Extracts file paths for file operations and provides queryable execution history.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Status of a tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    /// Tool execution is pending (not yet started)
    Pending,
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Success,
    /// Tool encountered an error
    Error,
}

impl Default for ToolStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Record of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    /// Unique tool call ID
    pub tool_id: String,
    /// Name of the tool (Read, Write, Edit, Glob, Grep, Bash, etc.)
    pub tool_name: String,
    /// Arguments passed to the tool (JSON string)
    pub arguments: Option<String>,
    /// Current status
    pub status: ToolStatus,
    /// Result content if successful
    pub result: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Start timestamp (ISO 8601)
    pub started_at: String,
    /// Completion timestamp (ISO 8601)
    pub completed_at: Option<String>,
    /// Extracted file paths (for Read, Write, Edit, Glob, Grep)
    pub file_paths: Vec<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

impl ToolExecution {
    /// Create a new tool execution record
    pub fn new(tool_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            tool_name: tool_name.into(),
            arguments: None,
            status: ToolStatus::Pending,
            result: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            file_paths: Vec::new(),
            duration_ms: None,
        }
    }

    /// Set the arguments
    pub fn with_arguments(mut self, args: impl Into<String>) -> Self {
        let args_str = args.into();
        self.file_paths = Self::extract_file_paths(&self.tool_name, &args_str);
        self.arguments = Some(args_str);
        self
    }

    /// Mark as running
    pub fn mark_running(&mut self) {
        self.status = ToolStatus::Running;
    }

    /// Mark as successful with result
    pub fn mark_success(&mut self, result: Option<String>) {
        self.status = ToolStatus::Success;
        self.result = result;
        self.complete();
    }

    /// Mark as failed with error
    pub fn mark_error(&mut self, error: impl Into<String>) {
        self.status = ToolStatus::Error;
        self.error = Some(error.into());
        self.complete();
    }

    /// Set completion time and calculate duration
    fn complete(&mut self) {
        let now = chrono::Utc::now();
        self.completed_at = Some(now.to_rfc3339());

        // Calculate duration
        if let Ok(started) = chrono::DateTime::parse_from_rfc3339(&self.started_at) {
            let duration = now.signed_duration_since(started.with_timezone(&chrono::Utc));
            self.duration_ms = Some(duration.num_milliseconds() as u64);
        }
    }

    /// Extract file paths from tool arguments based on tool name
    fn extract_file_paths(tool_name: &str, args: &str) -> Vec<String> {
        let mut paths = Vec::new();

        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(args) {
            match tool_name.to_lowercase().as_str() {
                "read" => {
                    // Read has file_path parameter
                    if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                        paths.push(path.to_string());
                    }
                }
                "write" => {
                    // Write has file_path parameter
                    if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                        paths.push(path.to_string());
                    }
                }
                "edit" => {
                    // Edit has file_path parameter
                    if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                        paths.push(path.to_string());
                    }
                }
                "glob" => {
                    // Glob has path and pattern
                    if let Some(path) = json.get("path").and_then(|v| v.as_str()) {
                        paths.push(path.to_string());
                    }
                    if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                        paths.push(pattern.to_string());
                    }
                }
                "grep" => {
                    // Grep has path parameter
                    if let Some(path) = json.get("path").and_then(|v| v.as_str()) {
                        paths.push(path.to_string());
                    }
                }
                "bash" => {
                    // Try to extract paths from command
                    if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                        // Simple heuristic: look for paths starting with / or containing \
                        for word in cmd.split_whitespace() {
                            if word.starts_with('/') || word.starts_with("./") || word.contains('\\') {
                                // Remove quotes if present
                                let cleaned = word.trim_matches(|c| c == '"' || c == '\'');
                                if !cleaned.is_empty() {
                                    paths.push(cleaned.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        paths
    }

    /// Check if this is a file-related tool
    pub fn is_file_tool(&self) -> bool {
        matches!(
            self.tool_name.to_lowercase().as_str(),
            "read" | "write" | "edit" | "glob" | "grep"
        )
    }
}

/// Tracks tool executions across a session
#[derive(Debug, Default)]
pub struct ToolTracker {
    /// Map of tool_id to execution record
    executions: HashMap<String, ToolExecution>,
    /// Ordered list of tool IDs for history
    execution_order: Vec<String>,
}

impl ToolTracker {
    /// Create a new tool tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a tool start event
    pub fn on_tool_start(
        &mut self,
        tool_id: &str,
        tool_name: &str,
        arguments: Option<&str>,
    ) -> &ToolExecution {
        let mut execution = ToolExecution::new(tool_id, tool_name);
        execution.mark_running();

        if let Some(args) = arguments {
            execution = execution.with_arguments(args);
        }

        self.execution_order.push(tool_id.to_string());
        self.executions.insert(tool_id.to_string(), execution);
        self.executions.get(tool_id).unwrap()
    }

    /// Handle a tool result event
    pub fn on_tool_result(
        &mut self,
        tool_id: &str,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Option<&ToolExecution> {
        if let Some(execution) = self.executions.get_mut(tool_id) {
            if let Some(err) = error {
                execution.mark_error(err);
            } else {
                execution.mark_success(result.map(|s| s.to_string()));
            }
            Some(execution)
        } else {
            None
        }
    }

    /// Get a tool execution by ID
    pub fn get_execution(&self, tool_id: &str) -> Option<&ToolExecution> {
        self.executions.get(tool_id)
    }

    /// Get all active (running) tools
    pub fn get_active_tools(&self) -> Vec<&ToolExecution> {
        self.executions
            .values()
            .filter(|e| e.status == ToolStatus::Running)
            .collect()
    }

    /// Get tool execution history
    pub fn get_tool_history(&self, tool_name_filter: Option<&str>) -> Vec<&ToolExecution> {
        let filter_lower = tool_name_filter.map(|s| s.to_lowercase());

        self.execution_order
            .iter()
            .filter_map(|id| self.executions.get(id))
            .filter(|e| {
                filter_lower
                    .as_ref()
                    .map(|f| e.tool_name.to_lowercase() == *f)
                    .unwrap_or(true)
            })
            .collect()
    }

    /// Get tool history as owned values (for serialization)
    pub fn get_tool_history_owned(&self, tool_name_filter: Option<&str>) -> Vec<ToolExecution> {
        self.get_tool_history(tool_name_filter)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get count of tools by status
    pub fn count_by_status(&self, status: ToolStatus) -> usize {
        self.executions.values().filter(|e| e.status == status).count()
    }

    /// Get total tool count
    pub fn total_count(&self) -> usize {
        self.executions.len()
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.total_count();
        if total == 0 {
            return 100.0;
        }

        let success = self.count_by_status(ToolStatus::Success);
        (success as f64 / total as f64) * 100.0
    }

    /// Get all file paths touched by tools
    pub fn get_all_file_paths(&self) -> Vec<String> {
        self.executions
            .values()
            .flat_map(|e| e.file_paths.iter().cloned())
            .collect()
    }

    /// Clear all executions
    pub fn clear(&mut self) {
        self.executions.clear();
        self.execution_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_execution_creation() {
        let exec = ToolExecution::new("tool-1", "Read");
        assert_eq!(exec.tool_id, "tool-1");
        assert_eq!(exec.tool_name, "Read");
        assert_eq!(exec.status, ToolStatus::Pending);
    }

    #[test]
    fn test_tool_execution_with_arguments() {
        let exec = ToolExecution::new("tool-1", "Read")
            .with_arguments(r#"{"file_path": "/path/to/file.rs"}"#);

        assert!(exec.arguments.is_some());
        assert_eq!(exec.file_paths, vec!["/path/to/file.rs"]);
    }

    #[test]
    fn test_tool_execution_mark_success() {
        let mut exec = ToolExecution::new("tool-1", "Read");
        exec.mark_running();
        assert_eq!(exec.status, ToolStatus::Running);

        exec.mark_success(Some("file contents".to_string()));
        assert_eq!(exec.status, ToolStatus::Success);
        assert_eq!(exec.result, Some("file contents".to_string()));
        assert!(exec.completed_at.is_some());
    }

    #[test]
    fn test_tool_execution_mark_error() {
        let mut exec = ToolExecution::new("tool-1", "Read");
        exec.mark_running();

        exec.mark_error("File not found");
        assert_eq!(exec.status, ToolStatus::Error);
        assert_eq!(exec.error, Some("File not found".to_string()));
        assert!(exec.completed_at.is_some());
    }

    #[test]
    fn test_extract_file_paths_read() {
        let exec = ToolExecution::new("tool-1", "Read")
            .with_arguments(r#"{"file_path": "/src/main.rs"}"#);
        assert_eq!(exec.file_paths, vec!["/src/main.rs"]);
    }

    #[test]
    fn test_extract_file_paths_write() {
        let exec = ToolExecution::new("tool-1", "Write")
            .with_arguments(r#"{"file_path": "/src/lib.rs", "content": "test"}"#);
        assert_eq!(exec.file_paths, vec!["/src/lib.rs"]);
    }

    #[test]
    fn test_extract_file_paths_edit() {
        let exec = ToolExecution::new("tool-1", "Edit")
            .with_arguments(r#"{"file_path": "/src/mod.rs", "old_string": "a", "new_string": "b"}"#);
        assert_eq!(exec.file_paths, vec!["/src/mod.rs"]);
    }

    #[test]
    fn test_extract_file_paths_glob() {
        let exec = ToolExecution::new("tool-1", "Glob")
            .with_arguments(r#"{"path": "/src", "pattern": "**/*.rs"}"#);
        assert!(exec.file_paths.contains(&"/src".to_string()));
        assert!(exec.file_paths.contains(&"**/*.rs".to_string()));
    }

    #[test]
    fn test_extract_file_paths_grep() {
        let exec = ToolExecution::new("tool-1", "Grep")
            .with_arguments(r#"{"path": "/src", "pattern": "fn main"}"#);
        assert_eq!(exec.file_paths, vec!["/src"]);
    }

    #[test]
    fn test_is_file_tool() {
        assert!(ToolExecution::new("t1", "Read").is_file_tool());
        assert!(ToolExecution::new("t1", "Write").is_file_tool());
        assert!(ToolExecution::new("t1", "Edit").is_file_tool());
        assert!(ToolExecution::new("t1", "Glob").is_file_tool());
        assert!(ToolExecution::new("t1", "Grep").is_file_tool());
        assert!(!ToolExecution::new("t1", "Bash").is_file_tool());
        assert!(!ToolExecution::new("t1", "WebFetch").is_file_tool());
    }

    #[test]
    fn test_tool_tracker_creation() {
        let tracker = ToolTracker::new();
        assert_eq!(tracker.total_count(), 0);
    }

    #[test]
    fn test_tool_tracker_on_tool_start() {
        let mut tracker = ToolTracker::new();

        let exec = tracker.on_tool_start(
            "tool-1",
            "Read",
            Some(r#"{"file_path": "/test.rs"}"#),
        );

        assert_eq!(exec.tool_id, "tool-1");
        assert_eq!(exec.status, ToolStatus::Running);
        assert_eq!(tracker.total_count(), 1);
    }

    #[test]
    fn test_tool_tracker_on_tool_result_success() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        let exec = tracker.on_tool_result("tool-1", Some("contents"), None);

        assert!(exec.is_some());
        let exec = exec.unwrap();
        assert_eq!(exec.status, ToolStatus::Success);
        assert_eq!(exec.result, Some("contents".to_string()));
    }

    #[test]
    fn test_tool_tracker_on_tool_result_error() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        let exec = tracker.on_tool_result("tool-1", None, Some("File not found"));

        assert!(exec.is_some());
        let exec = exec.unwrap();
        assert_eq!(exec.status, ToolStatus::Error);
        assert_eq!(exec.error, Some("File not found".to_string()));
    }

    #[test]
    fn test_tool_tracker_get_active_tools() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        tracker.on_tool_start("tool-2", "Write", None);
        tracker.on_tool_result("tool-1", Some("done"), None);

        let active = tracker.get_active_tools();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].tool_id, "tool-2");
    }

    #[test]
    fn test_tool_tracker_get_tool_history() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        tracker.on_tool_start("tool-2", "Write", None);
        tracker.on_tool_start("tool-3", "Read", None);

        let history = tracker.get_tool_history(None);
        assert_eq!(history.len(), 3);

        let read_history = tracker.get_tool_history(Some("Read"));
        assert_eq!(read_history.len(), 2);
    }

    #[test]
    fn test_tool_tracker_count_by_status() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        tracker.on_tool_start("tool-2", "Write", None);
        tracker.on_tool_result("tool-1", Some("done"), None);
        tracker.on_tool_result("tool-2", None, Some("error"));

        assert_eq!(tracker.count_by_status(ToolStatus::Success), 1);
        assert_eq!(tracker.count_by_status(ToolStatus::Error), 1);
        assert_eq!(tracker.count_by_status(ToolStatus::Running), 0);
    }

    #[test]
    fn test_tool_tracker_success_rate() {
        let mut tracker = ToolTracker::new();

        // Empty tracker should return 100%
        assert_eq!(tracker.success_rate(), 100.0);

        tracker.on_tool_start("tool-1", "Read", None);
        tracker.on_tool_start("tool-2", "Write", None);
        tracker.on_tool_result("tool-1", Some("done"), None);
        tracker.on_tool_result("tool-2", None, Some("error"));

        assert_eq!(tracker.success_rate(), 50.0);
    }

    #[test]
    fn test_tool_tracker_get_all_file_paths() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", Some(r#"{"file_path": "/a.rs"}"#));
        tracker.on_tool_start("tool-2", "Write", Some(r#"{"file_path": "/b.rs"}"#));

        let paths = tracker.get_all_file_paths();
        assert!(paths.contains(&"/a.rs".to_string()));
        assert!(paths.contains(&"/b.rs".to_string()));
    }

    #[test]
    fn test_tool_tracker_clear() {
        let mut tracker = ToolTracker::new();

        tracker.on_tool_start("tool-1", "Read", None);
        tracker.on_tool_start("tool-2", "Write", None);
        assert_eq!(tracker.total_count(), 2);

        tracker.clear();
        assert_eq!(tracker.total_count(), 0);
    }

    #[test]
    fn test_tool_execution_serialization() {
        let mut exec = ToolExecution::new("tool-1", "Read");
        exec.mark_running();

        let json = serde_json::to_string(&exec).unwrap();
        assert!(json.contains("\"tool_id\":\"tool-1\""));
        assert!(json.contains("\"tool_name\":\"Read\""));
        assert!(json.contains("\"status\":\"running\""));
    }
}
