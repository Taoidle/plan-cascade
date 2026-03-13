//! Guardrail Security System
//!
//! Production guardrails for native execution flows. Guardrails validate
//! content across multiple execution surfaces, can block/redact content before
//! it reaches downstream systems, and emit sanitized audit events.

pub mod code_security;
pub mod custom;
pub mod registry;
pub mod schema_validation;
pub mod sensitive_data;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use code_security::CodeSecurityGuardrail;
pub use custom::CustomGuardrail;
pub use registry::{register_guardrail_hooks, shared_guardrail_registry, GuardrailRegistry};
pub use schema_validation::SchemaValidationGuardrail;
pub use sensitive_data::SensitiveDataGuardrail;

/// Execution surface being validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Input,
    ToolCall,
    Tool,
    Output,
    Artifact,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Input => "input",
            Direction::ToolCall => "tool_call",
            Direction::Tool => "tool_result",
            Direction::Output => "assistant_output",
            Direction::Artifact => "artifact",
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Runtime metadata passed to guardrail evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuardrailRuntimeContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<String>,
}

/// Result of a guardrail validation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuardrailResult {
    Pass,
    Warn { message: String },
    Block { reason: String },
    Redact {
        redacted_content: String,
        redacted_items: Vec<String>,
    },
}

impl GuardrailResult {
    pub fn is_pass(&self) -> bool {
        matches!(self, GuardrailResult::Pass)
    }

    pub fn is_block(&self) -> bool {
        matches!(self, GuardrailResult::Block { .. })
    }

    pub fn is_warn(&self) -> bool {
        matches!(self, GuardrailResult::Warn { .. })
    }

    pub fn is_redact(&self) -> bool {
        matches!(self, GuardrailResult::Redact { .. })
    }

    pub fn result_type(&self) -> &'static str {
        match self {
            GuardrailResult::Pass => "pass",
            GuardrailResult::Warn { .. } => "warn",
            GuardrailResult::Block { .. } => "block",
            GuardrailResult::Redact { .. } => "redact",
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            GuardrailResult::Warn { message } => Some(message.as_str()),
            GuardrailResult::Block { reason } => Some(reason.as_str()),
            GuardrailResult::Redact { .. } | GuardrailResult::Pass => None,
        }
    }
}

/// Action used by user-defined rules and surfaced in UI metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailAction {
    Warn,
    Block,
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
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "warn" => Some(Self::Warn),
            "block" => Some(Self::Block),
            "redact" => Some(Self::Redact),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivePattern {
    pub name: String,
    pub regex: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRule {
    pub name: String,
    pub pattern: String,
    pub description: String,
}

/// Guardrail metadata returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailInfo {
    pub id: String,
    pub name: String,
    pub guardrail_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub enabled: bool,
    pub scope: Vec<Direction>,
    pub action: String,
    pub editable: bool,
    pub description: String,
}

/// Sanitized audit event stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailEventEntry {
    pub id: i64,
    pub rule_id: String,
    pub rule_name: String,
    pub surface: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    pub decision: String,
    pub content_hash: String,
    pub safe_preview: String,
    pub timestamp: String,
}

/// Creation/update payload for custom rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRuleConfig {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub action: GuardrailAction,
    pub enabled: bool,
    pub scope: Vec<Direction>,
    #[serde(default)]
    pub description: String,
}

#[async_trait]
pub trait Guardrail: Send + Sync {
    fn id(&self) -> &str;

    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    fn builtin_key(&self) -> Option<&str> {
        None
    }

    fn default_scopes(&self) -> Vec<Direction>;

    fn default_action_label(&self) -> &'static str;

    fn editable(&self) -> bool {
        false
    }

    fn redact_preview(&self, _content: &str) -> Option<String> {
        None
    }

    async fn validate(
        &self,
        content: &str,
        direction: Direction,
        runtime: &GuardrailRuntimeContext,
    ) -> GuardrailResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_display_uses_surface_names() {
        assert_eq!(Direction::Input.to_string(), "input");
        assert_eq!(Direction::ToolCall.to_string(), "tool_call");
        assert_eq!(Direction::Tool.to_string(), "tool_result");
        assert_eq!(Direction::Output.to_string(), "assistant_output");
        assert_eq!(Direction::Artifact.to_string(), "artifact");
    }

    #[test]
    fn guardrail_action_parse_is_case_insensitive() {
        assert_eq!(GuardrailAction::parse("BLOCK"), Some(GuardrailAction::Block));
        assert_eq!(GuardrailAction::parse(" redact "), Some(GuardrailAction::Redact));
        assert_eq!(GuardrailAction::parse("unknown"), None);
    }
}
