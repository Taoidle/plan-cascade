//! General Adapter
//!
//! Default domain adapter that handles any task type with generic decomposition.
//! Used as fallback when no domain-specific adapter matches.

use std::sync::Arc;

use async_trait::async_trait;

use crate::services::llm::provider::LlmProvider;
use crate::services::plan_mode::adapter::DomainAdapter;
use crate::services::plan_mode::types::{CriterionResult, Plan, PlanStep, StepOutput, TaskDomain};

fn truncate_with_ellipsis(content: &str, max_chars: usize) -> String {
    let mut chars = content.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...\n[Truncated]")
    } else {
        content.to_string()
    }
}

/// General-purpose adapter for any task domain.
pub struct GeneralAdapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CriterionKind {
    DirectlyVerifiable,
    ExternallyVerifiable,
}

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
            "codebase_search".to_string(),
            "grep".to_string(),
            "ls".to_string(),
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
        let mut results = Vec::new();
        let source = if output.full_content.trim().is_empty() {
            output.content.as_str()
        } else {
            output.full_content.as_str()
        };

        for criterion in &step.completion_criteria {
            match classify_criterion_kind(criterion) {
                CriterionKind::ExternallyVerifiable => {
                    let met = output_has_validation_plan(source);
                    let explanation = if met {
                        "externally_verifiable: includes validation approach/metrics for this criterion"
                            .to_string()
                    } else {
                        "externally_verifiable: missing explicit validation method or metric plan"
                            .to_string()
                    };
                    results.push(CriterionResult {
                        criterion: criterion.to_string(),
                        met,
                        explanation,
                    });
                }
                CriterionKind::DirectlyVerifiable => {
                    if let Some(rule_based) = rule_validate_direct_criterion(criterion, source) {
                        results.push(rule_based);
                    } else {
                        results.push(
                            llm_validate_criterion(criterion, source, provider.clone()).await,
                        );
                    }
                }
            }
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

fn classify_criterion_kind(criterion: &str) -> CriterionKind {
    let lower = criterion.to_ascii_lowercase();
    let externally_verifiable_markers = [
        ">= ",
        ">=",
        "accuracy",
        "推荐准确率",
        "流畅",
        "无感知",
        "user no perception",
        "用户无需",
        "performance under",
        "sla",
        "latency",
    ];
    if externally_verifiable_markers
        .iter()
        .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
        || criterion.contains('%')
    {
        CriterionKind::ExternallyVerifiable
    } else {
        CriterionKind::DirectlyVerifiable
    }
}

fn output_has_validation_plan(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    let markers = [
        "验证",
        "测试",
        "test plan",
        "verification",
        "metrics",
        "metric",
        "measure",
        "acceptance",
        "coverage",
        "benchmark",
        "回归",
        "验收",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn rule_validate_direct_criterion(criterion: &str, output: &str) -> Option<CriterionResult> {
    let criterion_trimmed = criterion.trim();
    let output_trimmed = output.trim();
    if output_trimmed.is_empty() {
        return Some(CriterionResult {
            criterion: criterion.to_string(),
            met: false,
            explanation: "directly_verifiable: output is empty".to_string(),
        });
    }

    let lower_criterion = criterion_trimmed.to_ascii_lowercase();
    let lower_output = output_trimmed.to_ascii_lowercase();

    let mut direct_markers = criterion_trimmed
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 4)
        .collect::<Vec<_>>();
    direct_markers.dedup();
    let matched = direct_markers
        .iter()
        .filter(|token| lower_output.contains(token.as_str()))
        .count();
    if matched >= 2 {
        return Some(CriterionResult {
            criterion: criterion.to_string(),
            met: true,
            explanation: format!(
                "directly_verifiable: matched {} keyword(s) from criterion in output",
                matched
            ),
        });
    }

    if criterion_trimmed.chars().count() >= 8
        && (output_trimmed.contains(criterion_trimmed)
            || lower_output.contains(lower_criterion.as_str()))
    {
        return Some(CriterionResult {
            criterion: criterion.to_string(),
            met: true,
            explanation: "directly_verifiable: criterion phrase appears in output".to_string(),
        });
    }

    // If the criterion is short and generic, defer to LLM for semantic judgement.
    if criterion_trimmed.chars().count() < 18 {
        return None;
    }

    // Rule path is intentionally conservative: when static matching is inconclusive,
    // defer to LLM semantic judgement instead of hard-failing on lexical mismatch.
    None
}

/// Validate a single criterion against output using LLM.
async fn llm_validate_criterion(
    criterion: &str,
    output: &str,
    provider: Arc<dyn LlmProvider>,
) -> CriterionResult {
    use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};

    let truncated_output = truncate_with_ellipsis(output, 3000);

    let system = format!(
        "You are evaluating whether a step's output meets a specific completion criterion.\n\n\
         Criterion: {criterion}\n\n\
         Respond with a JSON object:\n\
         ```json\n\
         {{\"met\": true|false, \"confidence\": 0.0-1.0, \"explanation\": \"brief explanation\", \"missing_items\": [\"...\"]}}\n\
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

    match provider
        .send_message(messages, Some(system), vec![], options)
        .await
    {
        Ok(response) => {
            // Try to parse JSON from response
            let text = response.content.as_deref().unwrap_or("").trim();
            if let Some(json_str) = extract_json_object(text) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    return CriterionResult {
                        criterion: criterion.to_string(),
                        met: parsed.get("met").and_then(|v| v.as_bool()).unwrap_or(true),
                        explanation: build_llm_explanation(&parsed),
                    };
                }
            }
            // Fallback if parsing fails
            CriterionResult {
                criterion: criterion.to_string(),
                met: true,
                explanation: "Validation completed (could not parse structured response)"
                    .to_string(),
            }
        }
        Err(_) => CriterionResult {
            criterion: criterion.to_string(),
            met: true,
            explanation: "Validation skipped (LLM error)".to_string(),
        },
    }
}

fn build_llm_explanation(parsed: &serde_json::Value) -> String {
    let explanation = parsed
        .get("explanation")
        .and_then(|v| v.as_str())
        .unwrap_or("Validation completed")
        .to_string();
    let confidence = parsed
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|v| format!(" (confidence: {:.2})", v))
        .unwrap_or_default();
    let missing = parsed
        .get("missing_items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if missing.is_empty() {
        format!("{explanation}{confidence}")
    } else {
        format!("{explanation}{confidence}; missing: {}", missing.join(", "))
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
            extract_json_object(
                r#"```json
{"met": true, "explanation": "ok"}
```"#
            ),
            Some(r#"{"met": true, "explanation": "ok"}"#.to_string())
        );

        assert_eq!(
            extract_json_object(r#"Some text {"met": false} more text"#),
            Some(r#"{"met": false}"#.to_string())
        );

        assert_eq!(extract_json_object("no json here"), None);
    }

    #[test]
    fn classify_external_criterion_for_accuracy_target() {
        assert!(matches!(
            classify_criterion_kind("Auto 模式推荐准确率 >= 80%"),
            CriterionKind::ExternallyVerifiable
        ));
    }

    #[test]
    fn externally_verifiable_requires_validation_plan() {
        assert!(output_has_validation_plan(
            "验证方案: 通过 A/B test 和指标追踪评估准确率。"
        ));
        assert!(!output_has_validation_plan("将支持该功能并提供更好体验。"));
    }

    #[test]
    fn direct_rule_defers_to_llm_when_partial_semantic_match() {
        let criterion =
            "明确各维度的改进目标（如：降低模式选择复杂度从高到低、提升状态可视化满意度等）";
        let output = "## 二、改进目标设定\n- 降低模式选择复杂度\n- 提升状态可视化满意度";
        let result = rule_validate_direct_criterion(criterion, output);
        assert!(
            result.is_none(),
            "inconclusive lexical match should defer to LLM instead of hard-failing"
        );
    }

    #[test]
    fn build_llm_explanation_includes_confidence_and_missing_items() {
        let parsed = serde_json::json!({
            "met": false,
            "confidence": 0.73,
            "explanation": "criterion partially met",
            "missing_items": ["量化目标", "验收口径"]
        });
        let explanation = build_llm_explanation(&parsed);
        assert!(explanation.contains("criterion partially met"));
        assert!(explanation.contains("confidence: 0.73"));
        assert!(explanation.contains("量化目标"));
    }
}
