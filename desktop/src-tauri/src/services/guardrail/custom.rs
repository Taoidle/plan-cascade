//! Custom Guardrail
//!
//! User-defined guardrail rules with configurable regex/keyword patterns
//! and actions (Warn, Block, Redact).

use async_trait::async_trait;
use regex::Regex;

use super::{Direction, Guardrail, GuardrailAction, GuardrailResult};

/// A user-defined guardrail rule.
pub struct CustomGuardrail {
    /// Unique identifier
    id: String,
    /// Human-readable name
    rule_name: String,
    /// Compiled regex pattern
    regex: Regex,
    /// Action to take on match
    action: GuardrailAction,
}

impl CustomGuardrail {
    /// Create a new custom guardrail from a regex pattern string.
    /// Returns None if the pattern fails to compile.
    pub fn new(id: String, name: String, pattern: &str, action: GuardrailAction) -> Option<Self> {
        Regex::new(pattern).ok().map(|regex| Self {
            id,
            rule_name: name,
            regex,
            action,
        })
    }

    /// Get the rule ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the regex pattern string.
    pub fn pattern(&self) -> &str {
        self.regex.as_str()
    }

    /// Get the action.
    pub fn action(&self) -> GuardrailAction {
        self.action
    }
}

#[async_trait]
impl Guardrail for CustomGuardrail {
    fn name(&self) -> &str {
        &self.rule_name
    }

    fn description(&self) -> &str {
        "User-defined guardrail rule"
    }

    async fn validate(&self, content: &str, _direction: Direction) -> GuardrailResult {
        if !self.regex.is_match(content) {
            return GuardrailResult::Pass;
        }

        match self.action {
            GuardrailAction::Warn => GuardrailResult::Warn {
                message: format!("Custom rule '{}' matched", self.rule_name),
            },
            GuardrailAction::Block => GuardrailResult::Block {
                reason: format!("Blocked by custom rule '{}'", self.rule_name),
            },
            GuardrailAction::Redact => {
                let redacted = self
                    .regex
                    .replace_all(content, format!("[REDACTED:{}]", self.rule_name))
                    .to_string();
                GuardrailResult::Redact {
                    redacted_content: redacted,
                    redacted_items: vec![self.rule_name.clone()],
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_custom_warn_rule() {
        let guard = CustomGuardrail::new(
            "r1".to_string(),
            "No TODO".to_string(),
            r"TODO",
            GuardrailAction::Warn,
        )
        .unwrap();
        let result = guard.validate("// TODO: fix this", Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("No TODO"));
        }
    }

    #[tokio::test]
    async fn test_custom_block_rule() {
        let guard = CustomGuardrail::new(
            "r2".to_string(),
            "No rm -rf".to_string(),
            r"rm\s+-rf\s+/",
            GuardrailAction::Block,
        )
        .unwrap();
        let result = guard.validate("rm -rf /", Direction::Input).await;
        assert!(result.is_block());
        if let GuardrailResult::Block { reason } = result {
            assert!(reason.contains("No rm -rf"));
        }
    }

    #[tokio::test]
    async fn test_custom_redact_rule() {
        let guard = CustomGuardrail::new(
            "r3".to_string(),
            "Internal ID".to_string(),
            r"INT-\d{6}",
            GuardrailAction::Redact,
        )
        .unwrap();
        let result = guard.validate("Ticket: INT-123456", Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact {
            redacted_content,
            redacted_items,
        } = result
        {
            assert!(redacted_content.contains("[REDACTED:Internal ID]"));
            assert!(redacted_items.contains(&"Internal ID".to_string()));
        }
    }

    #[tokio::test]
    async fn test_no_match_passes() {
        let guard = CustomGuardrail::new(
            "r4".to_string(),
            "Test".to_string(),
            r"SPECIFIC_PATTERN",
            GuardrailAction::Block,
        )
        .unwrap();
        let result = guard.validate("normal content", Direction::Input).await;
        assert!(result.is_pass());
    }

    #[test]
    fn test_invalid_pattern_returns_none() {
        let guard = CustomGuardrail::new(
            "r5".to_string(),
            "Bad".to_string(),
            r"[invalid",
            GuardrailAction::Warn,
        );
        assert!(guard.is_none());
    }

    #[test]
    fn test_accessors() {
        let guard = CustomGuardrail::new(
            "r6".to_string(),
            "Test Rule".to_string(),
            r"\btest\b",
            GuardrailAction::Warn,
        )
        .unwrap();
        assert_eq!(guard.id(), "r6");
        assert_eq!(guard.name(), "Test Rule");
        assert_eq!(guard.pattern(), r"\btest\b");
        assert_eq!(guard.action(), GuardrailAction::Warn);
    }
}
