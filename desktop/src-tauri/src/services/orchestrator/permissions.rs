//! Tool Permission Types and Classification
//!
//! Defines permission levels, tool risk categories, and the classification
//! logic that determines whether a tool call needs user approval.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Session-level permission mode. Determines which tool categories require approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// All write and dangerous operations require approval
    Strict,
    /// Only dangerous operations require approval (default)
    Standard,
    /// All operations auto-approved
    Permissive,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::Standard
    }
}

/// Risk classification for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolRisk {
    /// Read-only operations: never require approval
    ReadOnly,
    /// Safe write operations (file create/edit): require approval in Strict mode
    SafeWrite,
    /// Dangerous operations (shell, browser, sub-agents): require approval in Strict + Standard
    Dangerous,
}

impl ToolRisk {
    /// Serialization name for the streaming event
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolRisk::ReadOnly => "ReadOnly",
            ToolRisk::SafeWrite => "SafeWrite",
            ToolRisk::Dangerous => "Dangerous",
        }
    }
}

/// Classify the risk level of a tool invocation.
///
/// | Tool                | Risk        |
/// |---------------------|-------------|
/// | Read, Glob, Grep, LS, Cwd, CodebaseSearch, WebFetch, WebSearch, Analyze | ReadOnly |
/// | Write, Edit, NotebookEdit | SafeWrite |
/// | Bash, Browser, Task | Dangerous |
pub fn classify_tool_risk(tool_name: &str, _args: &Value) -> ToolRisk {
    match tool_name {
        // Read-only tools
        "Read" | "Glob" | "Grep" | "LS" | "Cwd" | "CodebaseSearch" | "WebFetch"
        | "WebSearch" | "Analyze" => ToolRisk::ReadOnly,

        // Safe write tools
        "Write" | "Edit" | "NotebookEdit" => ToolRisk::SafeWrite,

        // Dangerous tools
        "Bash" | "Browser" | "Task" => ToolRisk::Dangerous,

        // Unknown tools default to Dangerous for safety
        _ => ToolRisk::Dangerous,
    }
}

/// Determine whether a tool invocation needs user approval given the session's permission level.
pub fn needs_approval(tool_name: &str, args: &Value, level: PermissionLevel) -> bool {
    let risk = classify_tool_risk(tool_name, args);
    match level {
        PermissionLevel::Strict => matches!(risk, ToolRisk::SafeWrite | ToolRisk::Dangerous),
        PermissionLevel::Standard => matches!(risk, ToolRisk::Dangerous),
        PermissionLevel::Permissive => false,
    }
}

/// Request payload sent from backend to frontend for tool approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub request_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub risk: ToolRisk,
}

/// Response from frontend to backend for tool approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    pub request_id: String,
    /// Whether the tool execution is allowed
    pub allowed: bool,
    /// If true, auto-allow this tool for the remainder of the session
    pub always_allow: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_read_only_tools() {
        let args = serde_json::json!({});
        for name in &[
            "Read", "Glob", "Grep", "LS", "Cwd", "CodebaseSearch", "WebFetch", "WebSearch",
            "Analyze",
        ] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::ReadOnly,
                "{} should be ReadOnly",
                name
            );
        }
    }

    #[test]
    fn test_classify_safe_write_tools() {
        let args = serde_json::json!({});
        for name in &["Write", "Edit", "NotebookEdit"] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::SafeWrite,
                "{} should be SafeWrite",
                name
            );
        }
    }

    #[test]
    fn test_classify_dangerous_tools() {
        let args = serde_json::json!({});
        for name in &["Bash", "Browser", "Task"] {
            assert_eq!(
                classify_tool_risk(name, &args),
                ToolRisk::Dangerous,
                "{} should be Dangerous",
                name
            );
        }
    }

    #[test]
    fn test_classify_unknown_tool_defaults_to_dangerous() {
        let args = serde_json::json!({});
        assert_eq!(classify_tool_risk("UnknownTool", &args), ToolRisk::Dangerous);
    }

    #[test]
    fn test_needs_approval_strict() {
        let args = serde_json::json!({});
        // ReadOnly: no approval
        assert!(!needs_approval("Read", &args, PermissionLevel::Strict));
        assert!(!needs_approval("Grep", &args, PermissionLevel::Strict));
        // SafeWrite: approval needed
        assert!(needs_approval("Write", &args, PermissionLevel::Strict));
        assert!(needs_approval("Edit", &args, PermissionLevel::Strict));
        // Dangerous: approval needed
        assert!(needs_approval("Bash", &args, PermissionLevel::Strict));
        assert!(needs_approval("Task", &args, PermissionLevel::Strict));
    }

    #[test]
    fn test_needs_approval_standard() {
        let args = serde_json::json!({});
        // ReadOnly: no approval
        assert!(!needs_approval("Read", &args, PermissionLevel::Standard));
        // SafeWrite: no approval in Standard
        assert!(!needs_approval("Write", &args, PermissionLevel::Standard));
        assert!(!needs_approval("Edit", &args, PermissionLevel::Standard));
        // Dangerous: approval needed
        assert!(needs_approval("Bash", &args, PermissionLevel::Standard));
        assert!(needs_approval("Browser", &args, PermissionLevel::Standard));
        assert!(needs_approval("Task", &args, PermissionLevel::Standard));
    }

    #[test]
    fn test_needs_approval_permissive() {
        let args = serde_json::json!({});
        // Nothing needs approval in Permissive
        assert!(!needs_approval("Read", &args, PermissionLevel::Permissive));
        assert!(!needs_approval("Write", &args, PermissionLevel::Permissive));
        assert!(!needs_approval("Bash", &args, PermissionLevel::Permissive));
        assert!(!needs_approval("Browser", &args, PermissionLevel::Permissive));
        assert!(!needs_approval("Task", &args, PermissionLevel::Permissive));
    }

    #[test]
    fn test_default_permission_level_is_standard() {
        assert_eq!(PermissionLevel::default(), PermissionLevel::Standard);
    }

    #[test]
    fn test_tool_risk_as_str() {
        assert_eq!(ToolRisk::ReadOnly.as_str(), "ReadOnly");
        assert_eq!(ToolRisk::SafeWrite.as_str(), "SafeWrite");
        assert_eq!(ToolRisk::Dangerous.as_str(), "Dangerous");
    }
}
