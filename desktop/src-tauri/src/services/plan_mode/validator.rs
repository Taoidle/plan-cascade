use std::sync::Arc;

use crate::services::llm::provider::LlmProvider;

use super::adapter::DomainAdapter;
use super::types::{CriterionResult, PlanStep, StepOutput, StepValidationResult};
use super::validation_engine::{summarize_evidence, validate_step_contract};

pub async fn validate_step_output(
    step: &PlanStep,
    output: &mut StepOutput,
    _adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
) -> StepValidationResult {
    output.evidence_summary = summarize_evidence(&output.evidence_bundle);
    let result = validate_step_contract(step, output, provider).await;
    output.criteria_met = derive_legacy_criteria_results(&result);
    output.validation_result = result.clone();
    output.outcome_status = result.outcome_status.clone();
    output.review_reason = result.review_reason.clone();
    result
}

pub fn validation_summary(result: &StepValidationResult) -> String {
    result.summary.clone()
}

pub fn derive_legacy_criteria_results(result: &StepValidationResult) -> Vec<CriterionResult> {
    result
        .checks
        .iter()
        .map(|check| CriterionResult {
            criterion: check.name.clone(),
            met: check.passed,
            explanation: check.explanation.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plan_mode::types::{
        StepOutcomeStatus, StepValidationStatus, ValidationCheckResult, ValidationSeverity,
    };

    #[test]
    fn test_validation_summary_uses_structured_result() {
        let result = StepValidationResult {
            status: StepValidationStatus::SoftFailed,
            outcome_status: StepOutcomeStatus::SoftFailed,
            summary: "2 validation issue(s): topic coverage: architecture".to_string(),
            checks: vec![ValidationCheckResult {
                name: "topic coverage: architecture".to_string(),
                category: "semantic".to_string(),
                passed: false,
                severity: ValidationSeverity::Soft,
                explanation: "missing".to_string(),
                evidence_refs: Vec::new(),
                missing_items: vec!["architecture".to_string()],
                confidence: Some(0.4),
            }],
            unmet_checks: Vec::new(),
            failure_bucket: None,
            confidence: Some(0.4),
            retry_guidance: Vec::new(),
            review_reason: None,
        };
        assert!(validation_summary(&result).contains("validation issue"));
    }

    #[test]
    fn test_derive_legacy_criteria_results() {
        let result = StepValidationResult {
            status: StepValidationStatus::Passed,
            outcome_status: StepOutcomeStatus::Completed,
            summary: String::new(),
            checks: vec![ValidationCheckResult {
                name: "required section".to_string(),
                category: "deliverable".to_string(),
                passed: true,
                severity: ValidationSeverity::Hard,
                explanation: "ok".to_string(),
                evidence_refs: Vec::new(),
                missing_items: Vec::new(),
                confidence: Some(1.0),
            }],
            unmet_checks: Vec::new(),
            failure_bucket: None,
            confidence: Some(1.0),
            retry_guidance: Vec::new(),
            review_reason: None,
        };
        let legacy = derive_legacy_criteria_results(&result);
        assert_eq!(legacy.len(), 1);
        assert!(legacy[0].met);
    }
}
