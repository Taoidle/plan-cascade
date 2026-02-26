//! Step Validator
//!
//! Post-step validation using the adapter's validate_step method.
//! Validates step outputs against completion criteria.

use std::sync::Arc;

use crate::services::llm::provider::LlmProvider;

use super::adapter::DomainAdapter;
use super::types::{CriterionResult, PlanStep, StepOutput};

/// Validate a step's output against its completion criteria.
pub async fn validate_step_output(
    step: &PlanStep,
    output: &mut StepOutput,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
) -> Vec<CriterionResult> {
    let results = adapter.validate_step(step, output, provider).await;

    // Store results in the output
    output.criteria_met = results.clone();

    results
}

/// Check if all criteria are met for a step output.
pub fn all_criteria_met(results: &[CriterionResult]) -> bool {
    results.iter().all(|r| r.met)
}

/// Generate a summary of validation results.
pub fn validation_summary(results: &[CriterionResult]) -> String {
    let total = results.len();
    let met = results.iter().filter(|r| r.met).count();

    if total == 0 {
        return "No criteria to validate".to_string();
    }

    let mut summary = format!("{met}/{total} criteria met");

    let failed: Vec<_> = results.iter().filter(|r| !r.met).collect();
    if !failed.is_empty() {
        summary.push_str("\nUnmet criteria:");
        for r in failed {
            summary.push_str(&format!("\n  - {}: {}", r.criterion, r.explanation));
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_criteria_met() {
        let results = vec![
            CriterionResult {
                criterion: "c1".to_string(),
                met: true,
                explanation: "ok".to_string(),
            },
            CriterionResult {
                criterion: "c2".to_string(),
                met: true,
                explanation: "ok".to_string(),
            },
        ];
        assert!(all_criteria_met(&results));

        let results_with_fail = vec![
            CriterionResult {
                criterion: "c1".to_string(),
                met: true,
                explanation: "ok".to_string(),
            },
            CriterionResult {
                criterion: "c2".to_string(),
                met: false,
                explanation: "missing".to_string(),
            },
        ];
        assert!(!all_criteria_met(&results_with_fail));
    }

    #[test]
    fn test_validation_summary() {
        let results = vec![
            CriterionResult {
                criterion: "Has introduction".to_string(),
                met: true,
                explanation: "Present".to_string(),
            },
            CriterionResult {
                criterion: "Has conclusion".to_string(),
                met: false,
                explanation: "Missing conclusion section".to_string(),
            },
        ];

        let summary = validation_summary(&results);
        assert!(summary.contains("1/2 criteria met"));
        assert!(summary.contains("Has conclusion"));
        assert!(summary.contains("Missing conclusion section"));
    }

    #[test]
    fn test_validation_summary_empty() {
        assert_eq!(validation_summary(&[]), "No criteria to validate");
    }
}
