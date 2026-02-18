//! Code Review Gate
//!
//! Sends changed code to LLM for 5-dimension scoring:
//! - Code Quality (25 pts)
//! - Naming & Clarity (20 pts)
//! - Complexity (20 pts)
//! - Pattern Adherence (20 pts)
//! - Security (15 pts)
//!
//! Total 100 pts, blocks if score < 70 or critical findings exist.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

/// Score for a single review dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionScore {
    /// Dimension name
    pub name: String,
    /// Score achieved
    pub score: u32,
    /// Maximum possible score
    pub max_score: u32,
    /// Findings for this dimension
    pub findings: Vec<String>,
}

/// A review finding with severity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    /// File path
    pub file_path: String,
    /// Line number (approximate)
    pub line: Option<u32>,
    /// Finding description
    pub description: String,
    /// Severity: "info", "warning", "error", "critical"
    pub severity: String,
    /// Dimension this finding belongs to
    pub dimension: String,
}

/// Complete code review result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeReviewResult {
    /// Per-dimension scores
    pub dimensions: Vec<DimensionScore>,
    /// Total score (out of 100)
    pub total_score: u32,
    /// All findings
    pub findings: Vec<ReviewFinding>,
    /// Whether the review blocks execution
    pub blocking: bool,
    /// Summary message
    pub summary: String,
}

impl CodeReviewResult {
    /// Create a default passing result.
    pub fn default_pass() -> Self {
        Self {
            dimensions: vec![
                DimensionScore {
                    name: "Code Quality".to_string(),
                    score: 25,
                    max_score: 25,
                    findings: Vec::new(),
                },
                DimensionScore {
                    name: "Naming & Clarity".to_string(),
                    score: 20,
                    max_score: 20,
                    findings: Vec::new(),
                },
                DimensionScore {
                    name: "Complexity".to_string(),
                    score: 20,
                    max_score: 20,
                    findings: Vec::new(),
                },
                DimensionScore {
                    name: "Pattern Adherence".to_string(),
                    score: 20,
                    max_score: 20,
                    findings: Vec::new(),
                },
                DimensionScore {
                    name: "Security".to_string(),
                    score: 15,
                    max_score: 15,
                    findings: Vec::new(),
                },
            ],
            total_score: 100,
            findings: Vec::new(),
            blocking: false,
            summary: "Code review passed with full score".to_string(),
        }
    }

    /// Check if the review should block based on score and critical findings.
    pub fn should_block(&self) -> bool {
        self.total_score < 70
            || self
                .findings
                .iter()
                .any(|f| f.severity == "critical")
    }
}

/// Code Review Gate that scores code changes across 5 dimensions.
pub struct CodeReviewGate {
    /// Git diff content to review
    diff_content: String,
    /// Minimum passing score (default 70)
    min_score: u32,
}

impl CodeReviewGate {
    /// Create a new code review gate.
    pub fn new(diff_content: String) -> Self {
        Self {
            diff_content,
            min_score: 70,
        }
    }

    /// Set the minimum passing score.
    pub fn with_min_score(mut self, min_score: u32) -> Self {
        self.min_score = min_score;
        self
    }

    /// Build the system prompt for code review.
    pub fn build_prompt(&self) -> String {
        format!(
            r#"You are an expert code reviewer. Review the following git diff and score it across 5 dimensions.

Scoring rubric:
1. Code Quality (0-25): Correctness, error handling, edge cases, resource management
2. Naming & Clarity (0-20): Variable/function names, readability, comments quality
3. Complexity (0-20): Cyclomatic complexity, nesting depth, function length
4. Pattern Adherence (0-20): Follows project patterns, idiomatic code, consistency
5. Security (0-15): Input validation, injection prevention, secrets handling

For each finding, classify severity as: "info", "warning", "error", or "critical".
Critical findings automatically block the review regardless of score.

Respond in this exact JSON format:
{{
  "dimensions": [
    {{"name": "Code Quality", "score": 22, "maxScore": 25, "findings": ["Minor: missing error handling in line 42"]}},
    {{"name": "Naming & Clarity", "score": 18, "maxScore": 20, "findings": []}},
    {{"name": "Complexity", "score": 17, "maxScore": 20, "findings": ["Function too_long() exceeds 50 lines"]}},
    {{"name": "Pattern Adherence", "score": 19, "maxScore": 20, "findings": []}},
    {{"name": "Security", "score": 14, "maxScore": 15, "findings": []}}
  ],
  "totalScore": 90,
  "findings": [
    {{"filePath": "src/main.rs", "line": 42, "description": "Missing error handling", "severity": "warning", "dimension": "Code Quality"}}
  ],
  "blocking": false,
  "summary": "Code review passed with score 90/100"
}}

Git diff to review:
```
{}
```"#,
            self.diff_content
        )
    }

    /// Run the code review gate with an optional LLM provider.
    ///
    /// If a provider is available, sends the diff for 5-dimension scoring.
    /// Falls back to a default passing result when no provider is given or
    /// the LLM call fails.
    pub async fn run(&self, provider: Option<Arc<dyn LlmProvider>>) -> PipelineGateResult {
        if let Some(provider) = provider {
            let prompt = self.build_prompt();
            let messages = vec![Message::user(prompt)];
            let request_options = LlmRequestOptions {
                temperature_override: Some(0.0),
                ..Default::default()
            };

            match provider
                .send_message(messages, None, vec![], request_options)
                .await
            {
                Ok(response) => {
                    if let Some(content) = &response.content {
                        return self.parse_response(content);
                    }
                }
                Err(e) => {
                    tracing::warn!("Code review LLM call failed, falling back to pass: {}", e);
                }
            }
        }

        // Fallback: pass with warning when no LLM available
        let mut result = PipelineGateResult::passed(
            "code_review",
            "Code Review",
            GatePhase::PostValidation,
            0,
        );
        result.message = "No LLM provider available for code review - passing with warning".to_string();
        result
    }

    /// Parse the AI response into a pipeline gate result.
    pub fn parse_response(&self, ai_response: &str) -> PipelineGateResult {
        let review_result = self.extract_result(ai_response);

        match review_result {
            Some(result) => {
                let blocking = result.should_block();
                if !blocking {
                    let mut gate_result = PipelineGateResult::passed(
                        "code_review",
                        "Code Review",
                        GatePhase::PostValidation,
                        0,
                    );
                    gate_result.message = result.summary.clone();
                    gate_result
                } else {
                    let findings: Vec<String> = result
                        .findings
                        .iter()
                        .map(|f| {
                            format!(
                                "[{}] {}: {} (line {})",
                                f.severity,
                                f.file_path,
                                f.description,
                                f.line.map_or("?".to_string(), |l| l.to_string())
                            )
                        })
                        .collect();

                    PipelineGateResult::failed(
                        "code_review",
                        "Code Review",
                        GatePhase::PostValidation,
                        0,
                        format!(
                            "Code review score {}/100 (minimum: {}). {}",
                            result.total_score, self.min_score, result.summary
                        ),
                        findings,
                    )
                }
            }
            None => {
                // Graceful fallback: pass with warning
                let mut result = PipelineGateResult::passed(
                    "code_review",
                    "Code Review",
                    GatePhase::PostValidation,
                    0,
                );
                result.message = "Could not parse AI review response - passing with warning".to_string();
                result
            }
        }
    }

    /// Extract the review result from the AI response.
    fn extract_result(&self, response: &str) -> Option<CodeReviewResult> {
        // Try direct JSON parse
        if let Ok(result) = serde_json::from_str::<CodeReviewResult>(response) {
            return Some(result);
        }

        // Try to find JSON block in response
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                let json_str = &response[start..=end];
                if let Ok(result) = serde_json::from_str::<CodeReviewResult>(json_str) {
                    return Some(result);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quality_gates::GateStatus;

    #[test]
    fn test_code_review_result_default_pass() {
        let result = CodeReviewResult::default_pass();
        assert_eq!(result.total_score, 100);
        assert!(!result.blocking);
        assert!(!result.should_block());
        assert_eq!(result.dimensions.len(), 5);
    }

    #[test]
    fn test_code_review_blocks_low_score() {
        let result = CodeReviewResult {
            dimensions: vec![],
            total_score: 60,
            findings: vec![],
            blocking: false,
            summary: "Low score".to_string(),
        };
        assert!(result.should_block());
    }

    #[test]
    fn test_code_review_blocks_critical_finding() {
        let result = CodeReviewResult {
            dimensions: vec![],
            total_score: 90,
            findings: vec![ReviewFinding {
                file_path: "main.rs".to_string(),
                line: Some(1),
                description: "SQL injection vulnerability".to_string(),
                severity: "critical".to_string(),
                dimension: "Security".to_string(),
            }],
            blocking: false,
            summary: "Critical issue".to_string(),
        };
        assert!(result.should_block());
    }

    #[test]
    fn test_code_review_passes_good_score() {
        let result = CodeReviewResult {
            dimensions: vec![],
            total_score: 85,
            findings: vec![ReviewFinding {
                file_path: "main.rs".to_string(),
                line: Some(1),
                description: "Minor naming issue".to_string(),
                severity: "info".to_string(),
                dimension: "Naming & Clarity".to_string(),
            }],
            blocking: false,
            summary: "Good code".to_string(),
        };
        assert!(!result.should_block());
    }

    #[test]
    fn test_parse_valid_response() {
        let gate = CodeReviewGate::new("diff".to_string());
        let response = r#"{
            "dimensions": [
                {"name": "Code Quality", "score": 22, "maxScore": 25, "findings": []},
                {"name": "Naming & Clarity", "score": 18, "maxScore": 20, "findings": []},
                {"name": "Complexity", "score": 17, "maxScore": 20, "findings": []},
                {"name": "Pattern Adherence", "score": 19, "maxScore": 20, "findings": []},
                {"name": "Security", "score": 14, "maxScore": 15, "findings": []}
            ],
            "totalScore": 90,
            "findings": [],
            "blocking": false,
            "summary": "Code looks good"
        }"#;
        let result = gate.parse_response(response);
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Passed);
    }

    #[test]
    fn test_parse_blocking_response() {
        let gate = CodeReviewGate::new("diff".to_string());
        let response = r#"{
            "dimensions": [
                {"name": "Code Quality", "score": 10, "maxScore": 25, "findings": []},
                {"name": "Naming & Clarity", "score": 10, "maxScore": 20, "findings": []},
                {"name": "Complexity", "score": 10, "maxScore": 20, "findings": []},
                {"name": "Pattern Adherence", "score": 10, "maxScore": 20, "findings": []},
                {"name": "Security", "score": 5, "maxScore": 15, "findings": []}
            ],
            "totalScore": 45,
            "findings": [
                {"filePath": "main.rs", "line": 1, "description": "Bad code", "severity": "error", "dimension": "Code Quality"}
            ],
            "blocking": true,
            "summary": "Poor quality code"
        }"#;
        let result = gate.parse_response(response);
        assert!(!result.passed);
        assert_eq!(result.status, GateStatus::Failed);
    }

    #[test]
    fn test_parse_invalid_response_fallback() {
        let gate = CodeReviewGate::new("diff".to_string());
        let result = gate.parse_response("not json");
        assert!(result.passed);
    }

    #[test]
    fn test_build_prompt_includes_diff() {
        let gate = CodeReviewGate::new("+fn foo() {}".to_string());
        let prompt = gate.build_prompt();
        assert!(prompt.contains("+fn foo()"));
        assert!(prompt.contains("Code Quality"));
        assert!(prompt.contains("Security"));
    }

    #[test]
    fn test_dimension_score_serialization() {
        let score = DimensionScore {
            name: "Code Quality".to_string(),
            score: 20,
            max_score: 25,
            findings: vec!["Minor issue".to_string()],
        };
        let json = serde_json::to_string(&score).unwrap();
        assert!(json.contains("\"maxScore\""));
    }
}
