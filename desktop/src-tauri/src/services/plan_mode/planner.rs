//! Plan Mode Planner
//!
//! Phase 3: LLM-powered plan decomposition.
//! Takes a task description and produces a Plan with steps and batches.

use std::sync::Arc;

use crate::services::analytics::send_message_tracked;
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};
use crate::utils::error::{AppError, AppResult};

use super::adapter::DomainAdapter;
use super::types::{
    calculate_plan_batches_with_parallel, ArtifactRequirement, ClarificationAnswer,
    DependencyEvidenceMode, FailureSeverity, Plan, PlanExecutionConfig, PlanStep,
    StepDeliverableContract, StepDeliverableFormat, StepDeliverableType,
    StepEvidenceRequirements, StepFailurePolicy, StepPriority, StepQualityRequirements,
    StepValidationProfile, TaskDomain, ValidationCheck, ValidationSeverity,
};

/// Generate a plan by decomposing the task into steps.
pub async fn generate_plan(
    description: &str,
    domain: &TaskDomain,
    adapter: Arc<dyn DomainAdapter>,
    clarifications: &[ClarificationAnswer],
    conversation_context: Option<&str>,
    language_instruction: &str,
    provider: Arc<dyn LlmProvider>,
) -> AppResult<Plan> {
    let persona = adapter.planning_persona();

    // Build context from clarifications
    let clarification_context = if clarifications.is_empty() {
        None
    } else {
        let mut ctx = String::from("## User Clarifications\n");
        for ca in clarifications {
            if !ca.skipped {
                ctx.push_str(&format!("- Q: ({})\n  A: {}\n", ca.question_id, ca.answer));
            }
        }
        Some(ctx)
    };

    // Combine contexts
    let full_context = match (clarification_context, conversation_context) {
        (Some(cl), Some(conv)) => Some(format!("{cl}\n\n{conv}")),
        (Some(cl), None) => Some(cl),
        (None, Some(conv)) => Some(conv.to_string()),
        (None, None) => None,
    };

    let system = format!(
        "{}\n\n{}\n\n## Output Language\n{}",
        persona.identity_prompt, persona.thinking_style, language_instruction
    );

    let decomposition_prompt = adapter.decomposition_prompt(description, full_context.as_deref());

    let messages = vec![Message::text(MessageRole::User, decomposition_prompt)];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = send_message_tracked(provider.as_ref(), messages, Some(system), vec![], options)
        .await
        .map_err(|e| AppError::Internal(format!("Plan generation LLM error: {e}")))?;

    let text = response.content.as_deref().unwrap_or("");

    // Parse plan from response, with retry-with-repair on failure
    match parse_plan(text, domain, adapter.id()) {
        Ok(plan) => Ok(plan),
        Err(parse_error) => {
            // Retry with repair prompt
            retry_parse_plan(
                description,
                text,
                &parse_error.to_string(),
                domain,
                adapter.clone(),
                provider,
            )
            .await
        }
    }
}

/// Parse an LLM response into a Plan.
fn parse_plan(text: &str, domain: &TaskDomain, adapter_name: &str) -> AppResult<Plan> {
    let json_str = extract_json_object(text)
        .ok_or_else(|| AppError::Internal("No JSON found in plan response".to_string()))?;

    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| AppError::Internal(format!("Failed to parse plan JSON: {e}")))?;

    let title = parsed
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Plan")
        .to_string();

    let description = parsed
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let steps_array = parsed
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Internal("Missing 'steps' array in plan".to_string()))?;

    let mut steps = Vec::new();
    for step_val in steps_array {
        let step = parse_step(step_val)?;
        steps.push(step);
    }

    if steps.is_empty() {
        return Err(AppError::Internal("Plan has no steps".to_string()));
    }

    let max_parallel = parsed
        .get("executionConfig")
        .and_then(|v| v.get("maxParallel"))
        .and_then(|v| v.as_u64())
        .map(|value| value as usize)
        .unwrap_or(4)
        .clamp(1, 8);
    let max_step_iterations = parsed
        .get("executionConfig")
        .and_then(|v| v.get("maxStepIterations"))
        .and_then(|v| v.as_u64())
        .map(|value| value as u32)
        .unwrap_or(36)
        .clamp(12, 96);
    let execution_config = PlanExecutionConfig {
        max_parallel,
        max_step_iterations,
        retry: Default::default(),
    };
    let batches = calculate_plan_batches_with_parallel(&steps, execution_config.max_parallel);

    Ok(Plan {
        title,
        description,
        domain: domain.clone(),
        adapter_name: adapter_name.to_string(),
        steps,
        batches,
        execution_config,
    })
}

/// Parse a single step from a JSON value.
fn parse_step(val: &serde_json::Value) -> AppResult<PlanStep> {
    let id = val
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Step missing 'id'".to_string()))?
        .to_string();

    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Step")
        .to_string();

    let description = val
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let priority = match val.get("priority").and_then(|v| v.as_str()) {
        Some("high") => StepPriority::High,
        Some("low") => StepPriority::Low,
        _ => StepPriority::Medium,
    };

    let dependencies = val
        .get("dependencies")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let deliverable = parse_deliverable_contract(val.get("deliverable"));
    let evidence_requirements = parse_evidence_requirements(val.get("evidenceRequirements"));
    let quality_requirements = parse_quality_requirements(val.get("qualityRequirements"));
    let validation_profile = parse_validation_profile(val.get("validationProfile"));
    let failure_policy = parse_failure_policy(val.get("failurePolicy"));

    let completion_criteria = derive_legacy_completion_criteria(
        &deliverable,
        &evidence_requirements,
        &quality_requirements,
    );

    let expected_output = if !deliverable.expected_output_summary.is_empty() {
        deliverable.expected_output_summary.clone()
    } else {
        val.get("expectedOutput")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    Ok(PlanStep {
        id,
        title,
        description,
        priority,
        dependencies,
        deliverable,
        evidence_requirements,
        quality_requirements,
        validation_profile,
        failure_policy,
        completion_criteria,
        expected_output,
        metadata: std::collections::HashMap::new(),
    })
}

fn parse_deliverable_contract(value: Option<&serde_json::Value>) -> StepDeliverableContract {
    let Some(value) = value else {
        return StepDeliverableContract::default();
    };
    let deliverable_type = match value.get("deliverableType").and_then(|v| v.as_str()) {
        Some("report") => StepDeliverableType::Report,
        Some("markdown") => StepDeliverableType::Markdown,
        Some("json") => StepDeliverableType::Json,
        Some("file_patch") => StepDeliverableType::FilePatch,
        Some("code_change") => StepDeliverableType::CodeChange,
        Some("artifact_bundle") => StepDeliverableType::ArtifactBundle,
        Some("research_summary") => StepDeliverableType::ResearchSummary,
        Some("analysis_memo") => StepDeliverableType::AnalysisMemo,
        _ => StepDeliverableType::Custom,
    };
    let format = match value.get("format").and_then(|v| v.as_str()) {
        Some("markdown") => StepDeliverableFormat::Markdown,
        Some("json") => StepDeliverableFormat::Json,
        Some("code") => StepDeliverableFormat::Code,
        Some("mixed") => StepDeliverableFormat::Mixed,
        _ => StepDeliverableFormat::Text,
    };
    let required_sections = parse_string_array(value.get("requiredSections"));
    let required_artifacts = value
        .get("requiredArtifacts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|artifact| ArtifactRequirement {
                    artifact_type: artifact
                        .get("artifactType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    path_hint: artifact
                        .get("pathHint")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    description: artifact
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();
    let expected_output_summary = value
        .get("expectedOutputSummary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    StepDeliverableContract {
        deliverable_type,
        format,
        required_sections,
        required_artifacts,
        expected_output_summary,
    }
}

fn parse_evidence_requirements(value: Option<&serde_json::Value>) -> StepEvidenceRequirements {
    let Some(value) = value else {
        return StepEvidenceRequirements::default();
    };
    let dependency_evidence_mode =
        match value.get("dependencyEvidenceMode").and_then(|v| v.as_str()) {
            Some("none") => DependencyEvidenceMode::None,
            Some("required") => DependencyEvidenceMode::Required,
            _ => DependencyEvidenceMode::Optional,
        };
    StepEvidenceRequirements {
        min_files_read: value
            .get("minFilesRead")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        required_paths: parse_string_array(value.get("requiredPaths")),
        required_tools: parse_string_array(value.get("requiredTools")),
        required_searches: parse_string_array(value.get("requiredSearches")),
        required_artifact_types: parse_string_array(value.get("requiredArtifactTypes")),
        dependency_evidence_mode,
    }
}

fn parse_quality_requirements(value: Option<&serde_json::Value>) -> StepQualityRequirements {
    let Some(value) = value else {
        return StepQualityRequirements::default();
    };
    let must_pass_checks = value
        .get("mustPassChecks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|check| ValidationCheck {
                    name: check
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    description: check
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    severity: match check.get("severity").and_then(|v| v.as_str()) {
                        Some("hard") => ValidationSeverity::Hard,
                        Some("review") => ValidationSeverity::Review,
                        _ => ValidationSeverity::Soft,
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    StepQualityRequirements {
        must_cover_topics: parse_string_array(value.get("mustCoverTopics")),
        must_reference_evidence: value
            .get("mustReferenceEvidence")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        must_include_reasoning_links: value
            .get("mustIncludeReasoningLinks")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        must_pass_checks,
        semantic_expectations: parse_string_array(value.get("semanticExpectations")),
    }
}

fn parse_validation_profile(value: Option<&serde_json::Value>) -> StepValidationProfile {
    match value.and_then(|v| v.as_str()) {
        Some("report") => StepValidationProfile::Report,
        Some("analysis") => StepValidationProfile::Analysis,
        Some("research") => StepValidationProfile::Research,
        Some("code_change") => StepValidationProfile::CodeChange,
        Some("documentation") => StepValidationProfile::Documentation,
        _ => StepValidationProfile::Mixed,
    }
}

fn parse_failure_policy(value: Option<&serde_json::Value>) -> StepFailurePolicy {
    let Some(value) = value else {
        return StepFailurePolicy::default();
    };
    StepFailurePolicy {
        severity: match value.get("severity").and_then(|v| v.as_str()) {
            Some("soft") => FailureSeverity::Soft,
            Some("review") => FailureSeverity::Review,
            _ => FailureSeverity::Hard,
        },
        max_auto_retries: value
            .get("maxAutoRetries")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize,
        allow_downstream_on_soft_fail: value
            .get("allowDownstreamOnSoftFail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn parse_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn derive_legacy_completion_criteria(
    deliverable: &StepDeliverableContract,
    evidence: &StepEvidenceRequirements,
    quality: &StepQualityRequirements,
) -> Vec<String> {
    let mut criteria = Vec::new();
    if !deliverable.required_sections.is_empty() {
        criteria.push(format!(
            "Include sections: {}",
            deliverable.required_sections.join(", ")
        ));
    }
    if evidence.min_files_read > 0 {
        criteria.push(format!("Read at least {} files", evidence.min_files_read));
    }
    if !evidence.required_tools.is_empty() {
        criteria.push(format!(
            "Use tools: {}",
            evidence.required_tools.join(", ")
        ));
    }
    for topic in &quality.must_cover_topics {
        criteria.push(format!("Cover topic: {topic}"));
    }
    if criteria.is_empty() && !deliverable.expected_output_summary.is_empty() {
        criteria.push(deliverable.expected_output_summary.clone());
    }
    criteria
}

/// Retry plan parsing with a repair prompt when initial parse fails.
async fn retry_parse_plan(
    description: &str,
    previous_response: &str,
    parse_error: &str,
    domain: &TaskDomain,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
) -> AppResult<Plan> {
    let repair_prompt = format!(
        "Your previous response could not be parsed as a valid plan.\n\n\
         ## Previous Response\n{previous_response}\n\n\
         ## Parse Error\n{parse_error}\n\n\
         Please fix your response and return valid JSON for the plan.\n\
         The original task was: {description}\n\n\
         Return ONLY the corrected JSON, no additional text."
    );

    let messages = vec![Message::text(MessageRole::User, repair_prompt)];

    let options = LlmRequestOptions {
        temperature_override: Some(0.1), // Low temperature for repair
        ..Default::default()
    };

    let persona = adapter.planning_persona();
    let system = format!("{}\n\n{}", persona.identity_prompt, persona.thinking_style);

    let response = send_message_tracked(provider.as_ref(), messages, Some(system), vec![], options)
        .await
        .map_err(|e| AppError::Internal(format!("Plan repair LLM error: {e}")))?;

    let text = response.content.as_deref().unwrap_or("");
    parse_plan(text, domain, adapter.id())
}

/// Extract the first JSON object from a text that may contain markdown fences.
fn extract_json_object(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        let after_lang = if let Some(nl) = after_fence.find('\n') {
            &after_fence[nl + 1..]
        } else {
            after_fence
        };
        if let Some(end) = after_lang.find("```") {
            let content = after_lang[..end].trim();
            if content.starts_with('{') {
                return Some(content.to_string());
            }
        }
    }
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
    fn test_parse_plan_valid() {
        let json = r#"```json
{
  "title": "Blog Post Plan",
  "description": "Write a blog post about AI",
  "steps": [
    {
      "id": "step-1",
      "title": "Research",
      "description": "Research AI trends",
      "priority": "high",
      "dependencies": [],
      "deliverable": {
        "deliverableType": "research_summary",
        "format": "markdown",
        "requiredSections": ["Sources", "Findings"],
        "requiredArtifacts": [],
        "expectedOutputSummary": "Research notes"
      },
      "evidenceRequirements": {
        "minFilesRead": 3,
        "requiredPaths": [],
        "requiredTools": ["web_search"],
        "requiredSearches": [],
        "requiredArtifactTypes": [],
        "dependencyEvidenceMode": "none"
      },
      "qualityRequirements": {
        "mustCoverTopics": ["AI trends"],
        "mustReferenceEvidence": true,
        "mustIncludeReasoningLinks": false,
        "mustPassChecks": [],
        "semanticExpectations": ["Summarize the findings clearly"]
      },
      "validationProfile": "research",
      "failurePolicy": {
        "severity": "hard",
        "maxAutoRetries": 1,
        "allowDownstreamOnSoftFail": false
      }
    },
    {
      "id": "step-2",
      "title": "Draft",
      "description": "Write the draft",
      "priority": "high",
      "dependencies": ["step-1"],
      "deliverable": {
        "deliverableType": "markdown",
        "format": "markdown",
        "requiredSections": ["Introduction", "Conclusion"],
        "requiredArtifacts": [],
        "expectedOutputSummary": "Blog post draft"
      },
      "evidenceRequirements": {
        "minFilesRead": 0,
        "requiredPaths": [],
        "requiredTools": [],
        "requiredSearches": [],
        "requiredArtifactTypes": [],
        "dependencyEvidenceMode": "required"
      },
      "qualityRequirements": {
        "mustCoverTopics": ["AI trends"],
        "mustReferenceEvidence": true,
        "mustIncludeReasoningLinks": false,
        "mustPassChecks": [],
        "semanticExpectations": ["Produce a coherent draft"]
      },
      "validationProfile": "report",
      "failurePolicy": {
        "severity": "soft",
        "maxAutoRetries": 1,
        "allowDownstreamOnSoftFail": false
      }
    }
  ]
}
```"#;

        let plan = parse_plan(json, &TaskDomain::Writing, "writing").unwrap();
        assert_eq!(plan.title, "Blog Post Plan");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.batches.len(), 2);
        assert_eq!(plan.steps[0].id, "step-1");
        assert_eq!(plan.steps[1].dependencies, vec!["step-1"]);
    }

    #[test]
    fn test_parse_plan_missing_steps() {
        let json = r#"{"title": "Empty Plan", "description": "No steps"}"#;
        let result = parse_plan(json, &TaskDomain::General, "general");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_step() {
        let val = serde_json::json!({
            "id": "step-1",
            "title": "Do something",
            "description": "Details",
            "priority": "high",
            "dependencies": ["step-0"],
            "deliverable": {
                "deliverableType": "analysis_memo",
                "format": "markdown",
                "requiredSections": ["Summary"],
                "requiredArtifacts": [],
                "expectedOutputSummary": "Result"
            },
            "evidenceRequirements": {
                "minFilesRead": 1,
                "requiredPaths": [],
                "requiredTools": ["read_file"],
                "requiredSearches": [],
                "requiredArtifactTypes": [],
                "dependencyEvidenceMode": "optional"
            },
            "qualityRequirements": {
                "mustCoverTopics": ["Done"],
                "mustReferenceEvidence": true,
                "mustIncludeReasoningLinks": false,
                "mustPassChecks": [],
                "semanticExpectations": ["Provide the result"]
            },
            "validationProfile": "analysis",
            "failurePolicy": {
                "severity": "hard",
                "maxAutoRetries": 1,
                "allowDownstreamOnSoftFail": false
            }
        });
        let step = parse_step(&val).unwrap();
        assert_eq!(step.id, "step-1");
        assert_eq!(step.priority, StepPriority::High);
        assert_eq!(step.dependencies, vec!["step-0"]);
        assert_eq!(step.deliverable.expected_output_summary, "Result");
    }
}
