//! Guardrail Security System
//!
//! Provides content validation guardrails that detect sensitive data, security
//! vulnerabilities, and enforce user-defined rules. Guardrails integrate into
//! the agentic lifecycle via `AgenticHooks` to inspect user input and tool output.
//!
//! ## Architecture
//!
//! - `Guardrail` trait: async validation interface
//! - `SensitiveDataGuardrail`: detects API keys, passwords, secrets
//! - `CodeSecurityGuardrail`: detects SQL injection, command injection, eval
//! - `CustomGuardrail`: user-defined regex/keyword rules
//! - `GuardrailRegistry`: manages all guardrails with enable/disable support
//! - `register_guardrail_hooks`: wires registry into `AgenticHooks`

pub mod code_security;
pub mod custom;
pub mod registry;
pub mod schema_validation;
pub mod sensitive_data;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// Re-export key types
pub use code_security::CodeSecurityGuardrail;
pub use custom::CustomGuardrail;
pub use registry::{GuardrailRegistry, register_guardrail_hooks};
pub use schema_validation::SchemaValidationGuardrail;
pub use sensitive_data::SensitiveDataGuardrail;

/// Direction of content flow being validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// User input to the system
    Input,
    /// LLM-generated output (code, text)
    Output,
    /// Tool execution results
    Tool,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Input => write!(f, "input"),
            Direction::Output => write!(f, "output"),
            Direction::Tool => write!(f, "tool"),
        }
    }
}

/// Result of a guardrail validation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuardrailResult {
    /// Content passed validation with no issues.
    Pass,
    /// Content has potential issues but is not blocked.
    Warn {
        /// Description of the warning
        message: String,
    },
    /// Content is blocked and should not be processed further.
    Block {
        /// Reason the content was blocked
        reason: String,
    },
    /// Content was modified to remove sensitive information.
    Redact {
        /// The content with redacted portions
        redacted_content: String,
        /// List of items that were redacted
        redacted_items: Vec<String>,
    },
}

impl GuardrailResult {
    /// Returns true if the result is Pass.
    pub fn is_pass(&self) -> bool {
        matches!(self, GuardrailResult::Pass)
    }

    /// Returns true if the result is Block.
    pub fn is_block(&self) -> bool {
        matches!(self, GuardrailResult::Block { .. })
    }

    /// Returns true if the result is Warn.
    pub fn is_warn(&self) -> bool {
        matches!(self, GuardrailResult::Warn { .. })
    }

    /// Returns true if the result is Redact.
    pub fn is_redact(&self) -> bool {
        matches!(self, GuardrailResult::Redact { .. })
    }

    /// Returns a human-readable type string for logging.
    pub fn result_type(&self) -> &'static str {
        match self {
            GuardrailResult::Pass => "pass",
            GuardrailResult::Warn { .. } => "warn",
            GuardrailResult::Block { .. } => "block",
            GuardrailResult::Redact { .. } => "redact",
        }
    }
}

/// Action to take when a guardrail rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardrailAction {
    /// Emit a warning but allow content through
    Warn,
    /// Block the content entirely
    Block,
    /// Redact the matched portions
    Redact,
}

impl std::fmt::Display for GuardrailAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardrailAction::Warn => write!(f, "warn"),
            GuardrailAction::Block => write!(f, "block"),
            GuardrailAction::Redact => write!(f, "redact"),
        }
    }
}

impl GuardrailAction {
    /// Parse an action from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "warn" => Some(GuardrailAction::Warn),
            "block" => Some(GuardrailAction::Block),
            "redact" => Some(GuardrailAction::Redact),
            _ => None,
        }
    }
}

/// A pattern for detecting sensitive data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivePattern {
    /// Human-readable name of the pattern (e.g., "OpenAI API Key")
    pub name: String,
    /// Regex pattern string
    pub regex: String,
    /// Description of what the pattern detects
    pub description: String,
}

/// A rule for detecting code security issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRule {
    /// Human-readable name of the rule (e.g., "SQL Injection")
    pub name: String,
    /// Regex pattern string
    pub pattern: String,
    /// Description of what the rule detects
    pub description: String,
}

/// Configuration for guardrail settings (serialization/deserialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailConfig {
    /// Whether SensitiveDataGuardrail is enabled
    pub sensitive_data_enabled: bool,
    /// Whether CodeSecurityGuardrail is enabled
    pub code_security_enabled: bool,
    /// Custom guardrail rules
    pub custom_rules: Vec<CustomRuleConfig>,
}

/// Configuration for a single custom rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRuleConfig {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Regex pattern string
    pub pattern: String,
    /// Action to take on match
    pub action: GuardrailAction,
    /// Whether this rule is enabled
    pub enabled: bool,
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        Self {
            sensitive_data_enabled: true,
            code_security_enabled: true,
            custom_rules: Vec::new(),
        }
    }
}

/// Information about a guardrail for frontend display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailInfo {
    /// Guardrail name
    pub name: String,
    /// Type: "builtin" or "custom"
    pub guardrail_type: String,
    /// Whether currently enabled
    pub enabled: bool,
    /// Description
    pub description: String,
}

/// A single trigger log entry stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerLogEntry {
    /// Auto-generated ID
    pub id: i64,
    /// Name of the guardrail that triggered
    pub guardrail_name: String,
    /// Direction of the content
    pub direction: String,
    /// Result type (pass, warn, block, redact)
    pub result_type: String,
    /// Short snippet of content that triggered (truncated for storage)
    pub content_snippet: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
}

/// Core guardrail validation trait.
///
/// Implementors validate content flowing in a given direction and return
/// a `GuardrailResult` indicating whether the content is safe.
#[async_trait]
pub trait Guardrail: Send + Sync {
    /// Human-readable name of this guardrail.
    fn name(&self) -> &str;

    /// Validate content flowing in the given direction.
    async fn validate(&self, content: &str, direction: Direction) -> GuardrailResult;

    /// Short description of what this guardrail checks.
    fn description(&self) -> &str {
        ""
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_display() {
        assert_eq!(Direction::Input.to_string(), "input");
        assert_eq!(Direction::Output.to_string(), "output");
        assert_eq!(Direction::Tool.to_string(), "tool");
    }

    #[test]
    fn test_direction_equality() {
        assert_eq!(Direction::Input, Direction::Input);
        assert_ne!(Direction::Input, Direction::Output);
    }

    #[test]
    fn test_guardrail_result_pass() {
        let result = GuardrailResult::Pass;
        assert!(result.is_pass());
        assert!(!result.is_block());
        assert!(!result.is_warn());
        assert!(!result.is_redact());
        assert_eq!(result.result_type(), "pass");
    }

    #[test]
    fn test_guardrail_result_warn() {
        let result = GuardrailResult::Warn {
            message: "potential issue".to_string(),
        };
        assert!(result.is_warn());
        assert!(!result.is_pass());
        assert_eq!(result.result_type(), "warn");
    }

    #[test]
    fn test_guardrail_result_block() {
        let result = GuardrailResult::Block {
            reason: "blocked for safety".to_string(),
        };
        assert!(result.is_block());
        assert!(!result.is_pass());
        assert_eq!(result.result_type(), "block");
    }

    #[test]
    fn test_guardrail_result_redact() {
        let result = GuardrailResult::Redact {
            redacted_content: "my key is [REDACTED:api_key]".to_string(),
            redacted_items: vec!["api_key".to_string()],
        };
        assert!(result.is_redact());
        assert!(!result.is_pass());
        assert_eq!(result.result_type(), "redact");
    }

    #[test]
    fn test_guardrail_action_display() {
        assert_eq!(GuardrailAction::Warn.to_string(), "warn");
        assert_eq!(GuardrailAction::Block.to_string(), "block");
        assert_eq!(GuardrailAction::Redact.to_string(), "redact");
    }

    #[test]
    fn test_guardrail_action_from_str() {
        assert_eq!(GuardrailAction::parse("warn"), Some(GuardrailAction::Warn));
        assert_eq!(GuardrailAction::parse("BLOCK"), Some(GuardrailAction::Block));
        assert_eq!(GuardrailAction::parse("Redact"), Some(GuardrailAction::Redact));
        assert_eq!(GuardrailAction::parse("invalid"), None);
    }

    #[test]
    fn test_guardrail_config_default() {
        let config = GuardrailConfig::default();
        assert!(config.sensitive_data_enabled);
        assert!(config.code_security_enabled);
        assert!(config.custom_rules.is_empty());
    }

    #[test]
    fn test_sensitive_pattern_fields() {
        let pattern = SensitivePattern {
            name: "API Key".to_string(),
            regex: r"sk-[a-zA-Z0-9]{48}".to_string(),
            description: "OpenAI API key".to_string(),
        };
        assert_eq!(pattern.name, "API Key");
        assert!(!pattern.regex.is_empty());
    }

    #[test]
    fn test_security_rule_fields() {
        let rule = SecurityRule {
            name: "SQL Injection".to_string(),
            pattern: r"format!\(.*SELECT.*\{".to_string(),
            description: "SQL injection via string interpolation".to_string(),
        };
        assert_eq!(rule.name, "SQL Injection");
        assert!(!rule.pattern.is_empty());
    }

    #[test]
    fn test_guardrail_config_serialization() {
        let config = GuardrailConfig {
            sensitive_data_enabled: false,
            code_security_enabled: true,
            custom_rules: vec![CustomRuleConfig {
                id: "rule-1".to_string(),
                name: "No TODOs".to_string(),
                pattern: r"TODO".to_string(),
                action: GuardrailAction::Warn,
                enabled: true,
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: GuardrailConfig = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.sensitive_data_enabled);
        assert!(deserialized.code_security_enabled);
        assert_eq!(deserialized.custom_rules.len(), 1);
        assert_eq!(deserialized.custom_rules[0].name, "No TODOs");
    }

    #[test]
    fn test_trigger_log_entry_fields() {
        let entry = TriggerLogEntry {
            id: 1,
            guardrail_name: "SensitiveData".to_string(),
            direction: "input".to_string(),
            result_type: "redact".to_string(),
            content_snippet: "sk-abc***".to_string(),
            timestamp: "2026-02-17T12:00:00Z".to_string(),
        };
        assert_eq!(entry.id, 1);
        assert_eq!(entry.guardrail_name, "SensitiveData");
    }

    #[test]
    fn test_guardrail_info_fields() {
        let info = GuardrailInfo {
            name: "SensitiveData".to_string(),
            guardrail_type: "builtin".to_string(),
            enabled: true,
            description: "Detects API keys and passwords".to_string(),
        };
        assert_eq!(info.name, "SensitiveData");
        assert_eq!(info.guardrail_type, "builtin");
        assert!(info.enabled);
    }
}
