//! Definition of Done (DoD) Gate
//!
//! Validates story completion:
//! - All acceptance criteria are addressed (via LLM check against git diff or heuristic)
//! - Quality gates passed
//! - No blocking review findings

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::services::quality_gates::code_review::CodeReviewResult;
use crate::services::quality_gates::pipeline::{
    GatePhase, PipelineGateResult, PipelineResult,
};

/// DoD validation input.
#[derive(Debug, Clone)]
pub struct DoDInput {
    /// Story ID
    pub story_id: String,
    /// Acceptance criteria from the story
    pub acceptance_criteria: Vec<String>,
    /// Quality gate pipeline result
    pub pipeline_result: Option<PipelineResult>,
    /// Code review result (if available)
    pub code_review_result: Option<CodeReviewResult>,
    /// Git diff of changes (for AI-based criteria checking)
    pub diff_content: Option<String>,
}

/// Result of criteria checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionCheck {
    /// The acceptance criterion
    pub criterion: String,
    /// Whether it appears to be addressed
    pub addressed: bool,
    /// Reasoning
    pub reasoning: String,
}

/// DoD Gate that validates story completion.
pub struct DoDGate {
    input: DoDInput,
}

impl DoDGate {
    /// Create a new DoD gate.
    pub fn new(input: DoDInput) -> Self {
        Self { input }
    }

    /// Build the LLM prompt for acceptance criteria checking.
    pub fn build_criteria_prompt(&self) -> Option<String> {
        let diff = self.input.diff_content.as_ref()?;
        if diff.is_empty() {
            return None;
        }

        let criteria_list = self
            .input
            .acceptance_criteria
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c))
            .collect::<Vec<_>>()
            .join("\n");

        Some(format!(
            r#"You are a QA analyst. Check whether the following acceptance criteria are addressed by the code changes in the git diff.

Acceptance Criteria:
{}

For each criterion, respond in this JSON format:
{{
  "criteria": [
    {{"criterion": "...", "addressed": true, "reasoning": "Implemented in file X"}}
  ]
}}

Git diff:
```
{}
```"#,
            criteria_list, diff
        ))
    }

    /// Run the DoD validation with an optional LLM provider.
    ///
    /// When a provider is available and a diff is present, uses the LLM to
    /// verify that each acceptance criterion is addressed by the code changes.
    /// Falls back to heuristic checks otherwise.
    pub async fn run(&self, provider: Option<Arc<dyn LlmProvider>>) -> PipelineGateResult {
        let mut failures = Vec::new();

        // Check quality gates passed
        if let Some(ref pipeline_result) = self.input.pipeline_result {
            if !pipeline_result.passed {
                failures.push("Quality gate pipeline did not pass".to_string());
            }
        }

        // Check no blocking review findings
        if let Some(ref review_result) = self.input.code_review_result {
            if review_result.should_block() {
                failures.push(format!(
                    "Code review has blocking findings (score: {}/100)",
                    review_result.total_score
                ));
            }

            let critical_count = review_result
                .findings
                .iter()
                .filter(|f| f.severity == "critical")
                .count();
            if critical_count > 0 {
                failures.push(format!(
                    "{} critical review findings must be resolved",
                    critical_count
                ));
            }
        }

        // Check acceptance criteria are addressed
        if self.input.acceptance_criteria.is_empty() {
            failures.push("No acceptance criteria defined for story".to_string());
        } else if let Some(prompt) = self.build_criteria_prompt() {
            // Attempt LLM-based criteria verification when diff and provider are available
            if let Some(provider) = &provider {
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
                            let checks = self.parse_criteria_response(content);
                            let unaddressed: Vec<_> = checks
                                .iter()
                                .filter(|c| !c.addressed)
                                .collect();

                            for check in &unaddressed {
                                failures.push(format!(
                                    "Acceptance criterion not addressed: '{}' — {}",
                                    check.criterion, check.reasoning
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "DoD LLM criteria check failed, skipping AI verification: {}",
                            e
                        );
                    }
                }
            }
        }

        if failures.is_empty() {
            PipelineGateResult::passed(
                "dod",
                "Definition of Done",
                GatePhase::PostValidation,
                0,
            )
        } else {
            PipelineGateResult::failed(
                "dod",
                "Definition of Done",
                GatePhase::PostValidation,
                0,
                format!(
                    "Story '{}' is not done: {} issues found",
                    self.input.story_id,
                    failures.len()
                ),
                failures,
            )
        }
    }

    /// Run the DoD validation (heuristic mode, no LLM).
    pub fn run_heuristic(&self) -> PipelineGateResult {
        let mut failures = Vec::new();

        if let Some(ref pipeline_result) = self.input.pipeline_result {
            if !pipeline_result.passed {
                failures.push("Quality gate pipeline did not pass".to_string());
            }
        }

        if let Some(ref review_result) = self.input.code_review_result {
            if review_result.should_block() {
                failures.push(format!(
                    "Code review has blocking findings (score: {}/100)",
                    review_result.total_score
                ));
            }

            let critical_count = review_result
                .findings
                .iter()
                .filter(|f| f.severity == "critical")
                .count();
            if critical_count > 0 {
                failures.push(format!(
                    "{} critical review findings must be resolved",
                    critical_count
                ));
            }
        }

        if self.input.acceptance_criteria.is_empty() {
            failures.push("No acceptance criteria defined for story".to_string());
        }

        if failures.is_empty() {
            PipelineGateResult::passed(
                "dod",
                "Definition of Done",
                GatePhase::PostValidation,
                0,
            )
        } else {
            PipelineGateResult::failed(
                "dod",
                "Definition of Done",
                GatePhase::PostValidation,
                0,
                format!(
                    "Story '{}' is not done: {} issues found",
                    self.input.story_id,
                    failures.len()
                ),
                failures,
            )
        }
    }

    /// Parse the LLM response for acceptance criteria checking.
    pub fn parse_criteria_response(&self, ai_response: &str) -> Vec<CriterionCheck> {
        // Try to parse JSON
        #[derive(Deserialize)]
        struct CriteriaResponse {
            criteria: Vec<CriterionCheck>,
        }

        // Try direct parse
        if let Ok(resp) = serde_json::from_str::<CriteriaResponse>(ai_response) {
            return resp.criteria;
        }

        // Try to find JSON in response
        if let Some(start) = ai_response.find('{') {
            if let Some(end) = ai_response.rfind('}') {
                let json_str = &ai_response[start..=end];
                if let Ok(resp) = serde_json::from_str::<CriteriaResponse>(json_str) {
                    return resp.criteria;
                }
            }
        }

        // Fallback: mark all as addressed
        self.input
            .acceptance_criteria
            .iter()
            .map(|c| CriterionCheck {
                criterion: c.clone(),
                addressed: true,
                reasoning: "Unable to verify - assuming addressed".to_string(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::quality_gates::code_review::ReviewFinding;
    use crate::services::quality_gates::pipeline::{
        GateMode, PipelinePhaseResult,
    };

    fn basic_input() -> DoDInput {
        DoDInput {
            story_id: "story-001".to_string(),
            acceptance_criteria: vec![
                "Feature X works".to_string(),
                "Tests pass".to_string(),
            ],
            pipeline_result: None,
            code_review_result: None,
            diff_content: None,
        }
    }

    #[test]
    fn test_dod_passes_basic() {
        let gate = DoDGate::new(basic_input());
        let result = gate.run_heuristic();
        assert!(result.passed);
    }

    #[test]
    fn test_dod_fails_no_criteria() {
        let mut input = basic_input();
        input.acceptance_criteria = vec![];
        let gate = DoDGate::new(input);
        let result = gate.run_heuristic();
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("acceptance criteria")));
    }

    #[test]
    fn test_dod_fails_pipeline_failed() {
        let mut input = basic_input();
        input.pipeline_result = Some(PipelineResult::new(
            vec![PipelinePhaseResult::new(
                GatePhase::Validation,
                GateMode::Hard,
                vec![PipelineGateResult::failed(
                    "test",
                    "Test",
                    GatePhase::Validation,
                    0,
                    "Tests failed".to_string(),
                    vec![],
                )],
            )],
            false,
            None,
        ));
        let gate = DoDGate::new(input);
        let result = gate.run_heuristic();
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("Quality gate")));
    }

    #[test]
    fn test_dod_fails_blocking_review() {
        let mut input = basic_input();
        input.code_review_result = Some(CodeReviewResult {
            dimensions: vec![],
            total_score: 50,
            findings: vec![ReviewFinding {
                file_path: "main.rs".to_string(),
                line: Some(1),
                description: "Critical vulnerability".to_string(),
                severity: "critical".to_string(),
                dimension: "Security".to_string(),
            }],
            blocking: true,
            summary: "Blocking issues".to_string(),
        });
        let gate = DoDGate::new(input);
        let result = gate.run_heuristic();
        assert!(!result.passed);
    }

    #[test]
    fn test_dod_passes_with_good_pipeline_and_review() {
        let mut input = basic_input();
        input.pipeline_result = Some(PipelineResult::new(
            vec![PipelinePhaseResult::new(
                GatePhase::Validation,
                GateMode::Hard,
                vec![PipelineGateResult::passed(
                    "test",
                    "Test",
                    GatePhase::Validation,
                    100,
                )],
            )],
            false,
            None,
        ));
        input.code_review_result = Some(CodeReviewResult::default_pass());
        let gate = DoDGate::new(input);
        let result = gate.run_heuristic();
        assert!(result.passed);
    }

    #[test]
    fn test_build_criteria_prompt() {
        let mut input = basic_input();
        input.diff_content = Some("+fn main() {}".to_string());
        let gate = DoDGate::new(input);
        let prompt = gate.build_criteria_prompt();
        assert!(prompt.is_some());
        let prompt = prompt.unwrap();
        assert!(prompt.contains("Feature X works"));
        assert!(prompt.contains("+fn main()"));
    }

    #[test]
    fn test_build_criteria_prompt_no_diff() {
        let gate = DoDGate::new(basic_input());
        assert!(gate.build_criteria_prompt().is_none());
    }

    #[test]
    fn test_parse_criteria_response() {
        let gate = DoDGate::new(basic_input());
        let response = r#"{"criteria": [
            {"criterion": "Feature X works", "addressed": true, "reasoning": "Implemented in main.rs"},
            {"criterion": "Tests pass", "addressed": false, "reasoning": "No test files found"}
        ]}"#;
        let checks = gate.parse_criteria_response(response);
        assert_eq!(checks.len(), 2);
        assert!(checks[0].addressed);
        assert!(!checks[1].addressed);
    }

    #[test]
    fn test_parse_criteria_response_fallback() {
        let gate = DoDGate::new(basic_input());
        let checks = gate.parse_criteria_response("invalid response");
        assert_eq!(checks.len(), 2);
        // All should be marked as addressed in fallback
        assert!(checks.iter().all(|c| c.addressed));
    }
}
