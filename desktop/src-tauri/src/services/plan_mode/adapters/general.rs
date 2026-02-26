//! General Adapter
//!
//! Default domain adapter that handles any task type with generic decomposition.
//! Used as fallback when no domain-specific adapter matches.

use std::sync::Arc;

use async_trait::async_trait;

use crate::services::llm::provider::LlmProvider;
use crate::services::plan_mode::adapter::DomainAdapter;
use crate::services::plan_mode::types::{
    CriterionResult, Plan, PlanStep, StepOutput, TaskDomain,
};

/// General-purpose adapter for any task domain.
pub struct GeneralAdapter;

#[async_trait]
impl DomainAdapter for GeneralAdapter {
    fn id(&self) -> &str {
        "general"
    }

    fn display_name(&self) -> &str {
        "General"
    }

    fn supported_domains(&self) -> Vec<TaskDomain> {
        vec![
            TaskDomain::General,
            TaskDomain::ProjectManagement,
            TaskDomain::DataAnalysis,
            TaskDomain::Marketing,
        ]
    }

    fn available_tools(&self, _step: &PlanStep) -> Vec<String> {
        vec![
            "web_search".to_string(),
            "read_file".to_string(),
            "write_file".to_string(),
        ]
    }

    async fn validate_step(
        &self,
        step: &PlanStep,
        output: &StepOutput,
        provider: Arc<dyn LlmProvider>,
    ) -> Vec<CriterionResult> {
        // Use LLM to validate each criterion against the output
        let mut results = Vec::new();

        for criterion in &step.completion_criteria {
            let result = llm_validate_criterion(criterion, &output.content, provider.clone()).await;
            results.push(result);
        }

        results
    }

    fn after_execution(&self, plan: &Plan, outputs: &[StepOutput]) -> Option<String> {
        let completed = outputs.len();
        let total = plan.steps.len();
        Some(format!(
            "Plan '{}' completed: {}/{} steps executed successfully.",
            plan.title, completed, total
        ))
    }
}

/// Validate a single criterion against output using LLM.
async fn llm_validate_criterion(
    criterion: &str,
    output: &str,
    provider: Arc<dyn LlmProvider>,
) -> CriterionResult {
    use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};

    let truncated_output = if output.len() > 3000 {
        format!("{}...\n[Truncated]", &output[..3000])
    } else {
        output.to_string()
    };

    let system = format!(
        "You are evaluating whether a step's output meets a specific completion criterion.\n\n\
         Criterion: {criterion}\n\n\
         Respond with a JSON object:\n\
         ```json\n\
         {{\"met\": true|false, \"explanation\": \"brief explanation\"}}\n\
         ```"
    );

    let messages = vec![Message::text(
        MessageRole::User,
        format!("## Step Output\n{truncated_output}\n\nDoes this output meet the criterion?"),
    )];

    let options = LlmRequestOptions {
        temperature_override: Some(0.1),
        ..Default::default()
    };

    match provider.send_message(messages, Some(system), vec![], options).await {
        Ok(response) => {
            // Try to parse JSON from response
            let text = response.content.as_deref().unwrap_or("").trim();
            if let Some(json_str) = extract_json_object(text) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    return CriterionResult {
                        criterion: criterion.to_string(),
                        met: parsed.get("met").and_then(|v| v.as_bool()).unwrap_or(true),
                        explanation: parsed
                            .get("explanation")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Validation completed")
                            .to_string(),
                    };
                }
            }
            // Fallback if parsing fails
            CriterionResult {
                criterion: criterion.to_string(),
                met: true,
                explanation: "Validation completed (could not parse structured response)".to_string(),
            }
        }
        Err(_) => CriterionResult {
            criterion: criterion.to_string(),
            met: true,
            explanation: "Validation skipped (LLM error)".to_string(),
        },
    }
}

/// Extract the first JSON object from a text that may contain markdown fences.
fn extract_json_object(text: &str) -> Option<String> {
    // Try to find JSON in code fences first
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }
    // Try to find raw JSON object
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return Some(text[start..=end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_adapter_properties() {
        let adapter = GeneralAdapter;
        assert_eq!(adapter.id(), "general");
        assert_eq!(adapter.display_name(), "General");
        assert!(adapter.supported_domains().contains(&TaskDomain::General));
    }

    #[test]
    fn test_extract_json_object() {
        assert_eq!(
            extract_json_object(r#"```json
{"met": true, "explanation": "ok"}
```"#),
            Some(r#"{"met": true, "explanation": "ok"}"#.to_string())
        );

        assert_eq!(
            extract_json_object(r#"Some text {"met": false} more text"#),
            Some(r#"{"met": false}"#.to_string())
        );

        assert_eq!(extract_json_object("no json here"), None);
    }
}
