//! Sensitive Data Guardrail
//!
//! Detects API keys, passwords, and sensitive environment variables in content.
//! Uses compiled regex patterns for performance.

use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;

use super::{Direction, Guardrail, GuardrailResult, SensitivePattern};

/// Compiled regex patterns for sensitive data detection.
/// Compiled once using OnceLock for performance.
struct CompiledPattern {
    name: String,
    regex: Regex,
    #[allow(dead_code)]
    description: String,
}

/// Get compiled default patterns (initialized once).
fn default_patterns() -> &'static Vec<CompiledPattern> {
    static PATTERNS: OnceLock<Vec<CompiledPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let raw = default_sensitive_patterns();
        raw.into_iter()
            .filter_map(|p| {
                Regex::new(&p.regex).ok().map(|r| CompiledPattern {
                    name: p.name,
                    regex: r,
                    description: p.description,
                })
            })
            .collect()
    })
}

/// Returns the default set of sensitive patterns.
pub fn default_sensitive_patterns() -> Vec<SensitivePattern> {
    vec![
        // OpenAI API keys
        SensitivePattern {
            name: "OpenAI API Key".to_string(),
            regex: r"sk-[a-zA-Z0-9]{20,}".to_string(),
            description: "OpenAI API key (sk-...)".to_string(),
        },
        // AWS Access Key ID
        SensitivePattern {
            name: "AWS Access Key".to_string(),
            regex: r"AKIA[0-9A-Z]{16}".to_string(),
            description: "AWS access key ID (AKIA...)".to_string(),
        },
        // GitHub Personal Access Token
        SensitivePattern {
            name: "GitHub PAT".to_string(),
            regex: r"ghp_[a-zA-Z0-9]{36,}".to_string(),
            description: "GitHub personal access token (ghp_...)".to_string(),
        },
        // Slack tokens
        SensitivePattern {
            name: "Slack Token".to_string(),
            regex: r"xox[bps]-[a-zA-Z0-9\-]+".to_string(),
            description: "Slack bot/user/app token (xox[bps]-...)".to_string(),
        },
        // Password patterns in config-like strings
        SensitivePattern {
            name: "Password Assignment".to_string(),
            regex: r#"(?i)(password|passwd|secret|token)\s*[=:]\s*["']?[^\s"']{4,}"#.to_string(),
            description: "Password/secret assignment in config".to_string(),
        },
        // Sensitive env vars
        SensitivePattern {
            name: "DATABASE_URL".to_string(),
            regex: r"(?i)DATABASE_URL\s*=\s*\S+".to_string(),
            description: "Database connection string".to_string(),
        },
        SensitivePattern {
            name: "PRIVATE_KEY".to_string(),
            regex: r"(?i)PRIVATE_KEY\s*=\s*\S+".to_string(),
            description: "Private key assignment".to_string(),
        },
        SensitivePattern {
            name: "JWT_SECRET".to_string(),
            regex: r"(?i)JWT_SECRET\s*=\s*\S+".to_string(),
            description: "JWT secret assignment".to_string(),
        },
        SensitivePattern {
            name: "AWS_SECRET_ACCESS_KEY".to_string(),
            regex: r"(?i)AWS_SECRET_ACCESS_KEY\s*=\s*\S+".to_string(),
            description: "AWS secret access key assignment".to_string(),
        },
    ]
}

/// Guardrail that detects sensitive data (API keys, passwords, secrets).
///
/// For `Direction::Input`: detects and redacts (replaces with `[REDACTED:type]`).
/// For `Direction::Tool`: detects and warns.
/// For `Direction::Output`: passes through (LLM output not expected to contain user secrets).
pub struct SensitiveDataGuardrail {
    /// Additional custom patterns (beyond defaults)
    custom_patterns: Vec<CompiledPattern>,
}

impl SensitiveDataGuardrail {
    /// Create a new SensitiveDataGuardrail with default patterns only.
    pub fn new() -> Self {
        Self {
            custom_patterns: Vec::new(),
        }
    }

    /// Add a custom sensitive pattern.
    pub fn add_pattern(&mut self, pattern: SensitivePattern) {
        if let Ok(regex) = Regex::new(&pattern.regex) {
            self.custom_patterns.push(CompiledPattern {
                name: pattern.name,
                regex,
                description: pattern.description,
            });
        }
    }

    /// Detect sensitive data in content and return matched pattern names.
    fn detect(&self, content: &str) -> Vec<(String, String)> {
        let mut matches = Vec::new();

        // Check default patterns
        for pattern in default_patterns() {
            if let Some(m) = pattern.regex.find(content) {
                matches.push((pattern.name.clone(), m.as_str().to_string()));
            }
        }

        // Check custom patterns
        for pattern in &self.custom_patterns {
            if let Some(m) = pattern.regex.find(content) {
                matches.push((pattern.name.clone(), m.as_str().to_string()));
            }
        }

        matches
    }

    /// Redact all sensitive data in content.
    fn redact(&self, content: &str) -> (String, Vec<String>) {
        let mut redacted = content.to_string();
        let mut redacted_items = Vec::new();

        // Apply default patterns
        for pattern in default_patterns() {
            if pattern.regex.is_match(&redacted) {
                redacted_items.push(pattern.name.clone());
                redacted = pattern
                    .regex
                    .replace_all(&redacted, format!("[REDACTED:{}]", pattern.name))
                    .to_string();
            }
        }

        // Apply custom patterns
        for pattern in &self.custom_patterns {
            if pattern.regex.is_match(&redacted) {
                redacted_items.push(pattern.name.clone());
                redacted = pattern
                    .regex
                    .replace_all(&redacted, format!("[REDACTED:{}]", pattern.name))
                    .to_string();
            }
        }

        (redacted, redacted_items)
    }
}

impl Default for SensitiveDataGuardrail {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Guardrail for SensitiveDataGuardrail {
    fn name(&self) -> &str {
        "SensitiveData"
    }

    fn description(&self) -> &str {
        "Detects API keys, passwords, and sensitive environment variables"
    }

    async fn validate(&self, content: &str, direction: Direction) -> GuardrailResult {
        match direction {
            Direction::Input => {
                // For user input: detect and redact
                let detections = self.detect(content);
                if detections.is_empty() {
                    GuardrailResult::Pass
                } else {
                    let (redacted_content, redacted_items) = self.redact(content);
                    GuardrailResult::Redact {
                        redacted_content,
                        redacted_items,
                    }
                }
            }
            Direction::Tool => {
                // For tool output: detect and warn
                let detections = self.detect(content);
                if detections.is_empty() {
                    GuardrailResult::Pass
                } else {
                    let names: Vec<String> = detections.iter().map(|(n, _)| n.clone()).collect();
                    GuardrailResult::Warn {
                        message: format!(
                            "Sensitive data detected in tool output: {}",
                            names.join(", ")
                        ),
                    }
                }
            }
            Direction::Output => {
                // LLM output: pass through (not expected to contain user secrets)
                GuardrailResult::Pass
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
    async fn test_detect_openai_api_key() {
        let guard = SensitiveDataGuardrail::new();
        let content = "My API key is sk-abcdefghijklmnopqrstuvwxyz123456789012345678";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact {
            redacted_content,
            redacted_items,
        } = result
        {
            assert!(redacted_content.contains("[REDACTED:OpenAI API Key]"));
            assert!(redacted_items.contains(&"OpenAI API Key".to_string()));
        }
    }

    #[tokio::test]
    async fn test_detect_aws_access_key() {
        let guard = SensitiveDataGuardrail::new();
        let content = "AWS key: AKIAIOSFODNN7EXAMPLE";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.contains(&"AWS Access Key".to_string()));
        }
    }

    #[tokio::test]
    async fn test_detect_github_pat() {
        let guard = SensitiveDataGuardrail::new();
        let content = "Token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.contains(&"GitHub PAT".to_string()));
        }
    }

    #[tokio::test]
    async fn test_detect_slack_token() {
        let guard = SensitiveDataGuardrail::new();
        let content = "Slack: xoxb-123456-abcdef-ghijkl";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.contains(&"Slack Token".to_string()));
        }
    }

    #[tokio::test]
    async fn test_detect_password_assignment() {
        let guard = SensitiveDataGuardrail::new();
        let content = "password = 'my-secret-pass123'";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.contains(&"Password Assignment".to_string()));
        }
    }

    #[tokio::test]
    async fn test_detect_database_url() {
        let guard = SensitiveDataGuardrail::new();
        let content = "DATABASE_URL = postgres://user:pass@localhost:5432/db";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
    }

    #[tokio::test]
    async fn test_detect_jwt_secret() {
        let guard = SensitiveDataGuardrail::new();
        let content = "JWT_SECRET = supersecretvalue123";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
    }

    #[tokio::test]
    async fn test_detect_aws_secret_key() {
        let guard = SensitiveDataGuardrail::new();
        let content = "AWS_SECRET_ACCESS_KEY = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
    }

    #[tokio::test]
    async fn test_no_sensitive_data_passes() {
        let guard = SensitiveDataGuardrail::new();
        let content = "This is a normal message with no secrets.";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_tool_direction_warns() {
        let guard = SensitiveDataGuardrail::new();
        let content = "File contains: sk-abcdefghijklmnopqrstuvwxyz12345678901234";
        let result = guard.validate(content, Direction::Tool).await;
        assert!(result.is_warn());
    }

    #[tokio::test]
    async fn test_output_direction_passes() {
        let guard = SensitiveDataGuardrail::new();
        let content = "sk-abcdefghijklmnopqrstuvwxyz12345678901234";
        let result = guard.validate(content, Direction::Output).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn test_custom_pattern() {
        let mut guard = SensitiveDataGuardrail::new();
        guard.add_pattern(SensitivePattern {
            name: "Internal API".to_string(),
            regex: r"internal-api-[a-z0-9]{16}".to_string(),
            description: "Internal API key".to_string(),
        });
        let content = "Key: internal-api-abcdef1234567890";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.contains(&"Internal API".to_string()));
        }
    }

    #[tokio::test]
    async fn test_multiple_detections() {
        let guard = SensitiveDataGuardrail::new();
        let content = "API: sk-abcdefghijklmnopqrst12345678901234 and password = mypass123";
        let result = guard.validate(content, Direction::Input).await;
        assert!(result.is_redact());
        if let GuardrailResult::Redact { redacted_items, .. } = result {
            assert!(redacted_items.len() >= 2);
        }
    }

    #[test]
    fn test_name_and_description() {
        let guard = SensitiveDataGuardrail::new();
        assert_eq!(guard.name(), "SensitiveData");
        assert!(!guard.description().is_empty());
    }

    #[test]
    fn test_default_patterns_compiled() {
        // Verify all default patterns compile successfully
        let patterns = default_patterns();
        assert!(
            patterns.len() >= 9,
            "Should have at least 9 default patterns"
        );
    }
}
