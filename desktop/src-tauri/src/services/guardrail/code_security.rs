//! Code Security Guardrail
//!
//! Detects code security issues like SQL injection, command injection, and eval usage
//! in LLM-generated code output and tool results.

use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;

use super::{Direction, Guardrail, GuardrailResult, SecurityRule};

/// Compiled security rule with pre-compiled regex.
struct CompiledRule {
    name: String,
    regex: Regex,
    #[allow(dead_code)]
    description: String,
}

/// Get compiled default security rules (initialized once).
fn default_rules() -> &'static Vec<CompiledRule> {
    static RULES: OnceLock<Vec<CompiledRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let raw = default_security_rules();
        raw.into_iter()
            .filter_map(|r| {
                Regex::new(&r.pattern).ok().map(|rx| CompiledRule {
                    name: r.name,
                    regex: rx,
                    description: r.description,
                })
            })
            .collect()
    })
}

/// Returns the default set of code security rules.
pub fn default_security_rules() -> Vec<SecurityRule> {
    vec![
        // SQL injection: format! with SQL keywords and interpolation
        SecurityRule {
            name: "SQL Injection".to_string(),
            pattern: r#"format!\s*\([^)]*(?:SELECT|INSERT|UPDATE|DELETE)\s+.*\{"#.to_string(),
            description: "SQL injection via format! string interpolation".to_string(),
        },
        // Command injection: Command::new with variable arguments
        SecurityRule {
            name: "Command Injection".to_string(),
            pattern: r"Command::new\s*\(\s*[a-z_][a-z_0-9]*\s*\)".to_string(),
            description: "Command injection via variable in Command::new".to_string(),
        },
        // eval() usage in JavaScript/Python
        SecurityRule {
            name: "Eval Usage".to_string(),
            pattern: r"\beval\s*\(".to_string(),
            description: "Dangerous eval() function usage".to_string(),
        },
        // exec() usage in Python/JS
        SecurityRule {
            name: "Exec Usage".to_string(),
            pattern: r"\bexec\s*\(".to_string(),
            description: "Dangerous exec() function usage".to_string(),
        },
        // Rust unsafe block detection
        SecurityRule {
            name: "Unsafe Block".to_string(),
            pattern: r"\bunsafe\s*\{".to_string(),
            description: "Rust unsafe block usage".to_string(),
        },
    ]
}

/// Guardrail that detects code security issues in LLM output.
///
/// Only active for `Direction::Output` (LLM-generated code) and
/// `Direction::Tool` (tool results that may contain code).
/// `Direction::Input` is passed through since user code should not be blocked.
pub struct CodeSecurityGuardrail {
    /// Additional custom security rules
    custom_rules: Vec<CompiledRule>,
}

impl CodeSecurityGuardrail {
    /// Create a new CodeSecurityGuardrail with default rules.
    pub fn new() -> Self {
        Self {
            custom_rules: Vec::new(),
        }
    }

    /// Add a custom security rule.
    pub fn add_rule(&mut self, rule: SecurityRule) {
        if let Ok(regex) = Regex::new(&rule.pattern) {
            self.custom_rules.push(CompiledRule {
                name: rule.name,
                regex,
                description: rule.description,
            });
        }
    }

    /// Detect security issues in content.
    fn detect(&self, content: &str) -> Vec<String> {
        let mut violations = Vec::new();

        // Check default rules
        for rule in default_rules() {
            if rule.regex.is_match(content) {
                violations.push(rule.name.clone());
            }
        }

        // Check custom rules
        for rule in &self.custom_rules {
            if rule.regex.is_match(content) {
                violations.push(rule.name.clone());
            }
        }

        violations
    }
}

impl Default for CodeSecurityGuardrail {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Guardrail for CodeSecurityGuardrail {
    fn name(&self) -> &str {
        "CodeSecurity"
    }

    fn description(&self) -> &str {
        "Detects SQL injection, command injection, eval/exec usage, and unsafe blocks"
    }

    async fn validate(&self, content: &str, direction: Direction) -> GuardrailResult {
        match direction {
            Direction::Input => {
                // User input: pass through (don't block user from discussing code)
                GuardrailResult::Pass
            }
            Direction::Output | Direction::Tool => {
                // LLM output / tool results: detect and warn
                let violations = self.detect(content);
                if violations.is_empty() {
                    GuardrailResult::Pass
                } else {
                    GuardrailResult::Warn {
                        message: format!(
                            "Code security issues detected: {}",
                            violations.join(", ")
                        ),
                    }
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
    async fn test_detect_sql_injection() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"let query = format!("SELECT * FROM users WHERE id = {}", user_id);"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("SQL Injection"));
        }
    }

    #[tokio::test]
    async fn test_detect_command_injection() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"let output = Command::new(user_cmd).output();"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Command Injection"));
        }
    }

    #[tokio::test]
    async fn test_detect_eval_usage() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"eval("document.cookie")"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Eval Usage"));
        }
    }

    #[tokio::test]
    async fn test_detect_exec_usage() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"exec("import os; os.system('rm -rf /')")"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Exec Usage"));
        }
    }

    #[tokio::test]
    async fn test_detect_unsafe_block() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"unsafe { std::ptr::null_mut() }"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Unsafe Block"));
        }
    }

    #[tokio::test]
    async fn test_safe_code_passes() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"
        fn main() {
            let users = db.query("SELECT * FROM users WHERE id = ?", &[&user_id]);
            println!("{:?}", users);
        }
        "#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_input_direction_passes() {
        let guard = CodeSecurityGuardrail::new();
        // Even dangerous code in user input should pass (user is asking about it)
        let content = r#"eval("malicious code")"#;
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_tool_direction_warns() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"eval("something")"#;
        let result = guard.validate(content, Direction::Tool).await;
        assert!(result.is_warn());
    }

    #[tokio::test]
    async fn test_command_new_with_literal_passes() {
        let guard = CodeSecurityGuardrail::new();
        // Command::new with a string literal should NOT be flagged
        let content = r#"Command::new("ls").arg("-la").output()"#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_custom_rule() {
        let mut guard = CodeSecurityGuardrail::new();
        guard.add_rule(SecurityRule {
            name: "Hardcoded IP".to_string(),
            pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
            description: "Hardcoded IP address".to_string(),
        });
        let content = "let host = \"192.168.1.100\";";
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Hardcoded IP"));
        }
    }

    #[tokio::test]
    async fn test_multiple_violations() {
        let guard = CodeSecurityGuardrail::new();
        let content = r#"
        eval("something");
        unsafe { ptr::null_mut() }
        "#;
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_warn());
        if let GuardrailResult::Warn { message } = result {
            assert!(message.contains("Eval Usage"));
            assert!(message.contains("Unsafe Block"));
        }
    }

    #[test]
    fn test_name_and_description() {
        let guard = CodeSecurityGuardrail::new();
        assert_eq!(guard.name(), "CodeSecurity");
        assert!(!guard.description().is_empty());
    }

    #[test]
    fn test_default_rules_compiled() {
        let rules = default_rules();
        assert!(
            rules.len() >= 5,
            "Should have at least 5 default security rules"
        );
    }
}
