use std::collections::HashSet;
use std::sync::Arc;

use crate::services::analytics::send_message_tracked;
use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};

use super::types::{
    ArtifactRequirement, DependencyEvidenceMode, PlanStep, StepDeliverableFormat,
    StepEvidenceBundle, StepEvidenceSummary, StepFailureBucket, StepOutcomeStatus, StepOutput,
    StepReviewReason, StepValidationResult, StepValidationStatus, ValidationCheckResult,
    ValidationEvidenceRef, ValidationSeverity,
};

pub async fn validate_step_contract(
    step: &PlanStep,
    output: &StepOutput,
    provider: Arc<dyn LlmProvider>,
) -> StepValidationResult {
    let mut checks = Vec::new();
    let content = if output.full_content.trim().is_empty() {
        output.content.trim()
    } else {
        output.full_content.trim()
    };

    checks.extend(validate_evidence(step, &output.evidence_bundle));
    checks.extend(validate_structure(step, content, &output.evidence_bundle));

    let semantic_checks =
        validate_semantics(step, content, &output.evidence_summary, provider).await;
    checks.extend(semantic_checks);

    finalize_validation(step, checks)
}

pub fn summarize_evidence(bundle: &StepEvidenceBundle) -> StepEvidenceSummary {
    StepEvidenceSummary {
        files_read_count: bundle.files_read.len(),
        files_written_count: bundle.files_written.len(),
        tool_call_count: bundle.tool_calls.len(),
        search_query_count: bundle.search_queries.len(),
        artifact_count: bundle.artifacts.len(),
        dependency_input_count: bundle.dependency_inputs.len(),
        coverage_markers: bundle.coverage_markers.clone(),
    }
}

fn validate_evidence(step: &PlanStep, bundle: &StepEvidenceBundle) -> Vec<ValidationCheckResult> {
    let mut checks = Vec::new();

    if step.evidence_requirements.min_files_read > 0 {
        let passed = bundle.files_read.len() >= step.evidence_requirements.min_files_read;
        checks.push(ValidationCheckResult {
            name: "minimum files read".to_string(),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                format!(
                    "Read {} files, meeting minimum {}",
                    bundle.files_read.len(),
                    step.evidence_requirements.min_files_read
                )
            } else {
                format!(
                    "Read {} files, below minimum {}",
                    bundle.files_read.len(),
                    step.evidence_requirements.min_files_read
                )
            },
            evidence_refs: bundle
                .files_read
                .iter()
                .map(|entry| ValidationEvidenceRef {
                    reference_type: "file_read".to_string(),
                    value: entry.path.clone(),
                })
                .collect(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![format!(
                    "Need {} more file reads",
                    step.evidence_requirements
                        .min_files_read
                        .saturating_sub(bundle.files_read.len())
                )]
            },
            confidence: Some(1.0),
        });
    }

    for required_path in &step.evidence_requirements.required_paths {
        let passed = bundle
            .files_read
            .iter()
            .any(|entry| entry.path.contains(required_path))
            || bundle
                .files_written
                .iter()
                .any(|path| path.contains(required_path))
            || bundle
                .artifacts
                .iter()
                .any(|artifact| artifact.value.contains(required_path));
        checks.push(ValidationCheckResult {
            name: format!("required path: {required_path}"),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                format!("Execution touched required path `{required_path}`")
            } else {
                format!("Execution did not touch required path `{required_path}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![required_path.clone()]
            },
            confidence: Some(1.0),
        });
    }

    let used_tools: HashSet<String> = bundle
        .tool_calls
        .iter()
        .map(|call| call.tool_name.to_ascii_lowercase())
        .collect();
    for required_tool in &step.evidence_requirements.required_tools {
        let passed = used_tools.contains(&required_tool.to_ascii_lowercase());
        checks.push(ValidationCheckResult {
            name: format!("required tool: {required_tool}"),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                format!("Execution used required tool `{required_tool}`")
            } else {
                format!("Execution did not use required tool `{required_tool}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![required_tool.clone()]
            },
            confidence: Some(1.0),
        });
    }

    for search in &step.evidence_requirements.required_searches {
        let passed = bundle
            .search_queries
            .iter()
            .any(|query| query.contains(search));
        checks.push(ValidationCheckResult {
            name: format!("required search: {search}"),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Soft,
            explanation: if passed {
                format!("Observed required search marker `{search}`")
            } else {
                format!("Did not observe required search marker `{search}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![search.clone()]
            },
            confidence: Some(1.0),
        });
    }

    for artifact_type in &step.evidence_requirements.required_artifact_types {
        let passed = bundle
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_type.eq_ignore_ascii_case(artifact_type));
        checks.push(ValidationCheckResult {
            name: format!("required artifact type: {artifact_type}"),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                format!("Produced artifact type `{artifact_type}`")
            } else {
                format!("Missing artifact type `{artifact_type}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![artifact_type.clone()]
            },
            confidence: Some(1.0),
        });
    }

    if matches!(
        step.evidence_requirements.dependency_evidence_mode,
        DependencyEvidenceMode::Required
    ) {
        let passed = !step.dependencies.is_empty() && !bundle.dependency_inputs.is_empty();
        checks.push(ValidationCheckResult {
            name: "dependency evidence".to_string(),
            category: "evidence".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                "Execution references dependency inputs".to_string()
            } else {
                "Dependency evidence is required but missing".to_string()
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                step.dependencies.clone()
            },
            confidence: Some(1.0),
        });
    }

    checks
}

fn validate_structure(
    step: &PlanStep,
    content: &str,
    bundle: &StepEvidenceBundle,
) -> Vec<ValidationCheckResult> {
    let mut checks = Vec::new();

    match step.deliverable.format {
        StepDeliverableFormat::Json => {
            let passed = serde_json::from_str::<serde_json::Value>(content).is_ok();
            checks.push(simple_structure_check(
                "json format",
                passed,
                ValidationSeverity::Hard,
                if passed {
                    "Output is valid JSON"
                } else {
                    "Output is not valid JSON"
                },
            ));
        }
        StepDeliverableFormat::Code => {
            let passed = content.contains("```") || !bundle.files_written.is_empty();
            checks.push(simple_structure_check(
                "code format",
                passed,
                ValidationSeverity::Soft,
                if passed {
                    "Output includes code content or file writes"
                } else {
                    "Expected code-like output or a written file"
                },
            ));
        }
        StepDeliverableFormat::Markdown => {
            let passed = content.contains('#') || content.contains("- ") || content.contains("## ");
            checks.push(simple_structure_check(
                "markdown structure",
                passed,
                ValidationSeverity::Soft,
                if passed {
                    "Output has markdown structure"
                } else {
                    "Output lacks clear markdown structure"
                },
            ));
        }
        StepDeliverableFormat::Mixed | StepDeliverableFormat::Text => {}
    }

    for section in &step.deliverable.required_sections {
        let passed = content
            .to_ascii_lowercase()
            .contains(&section.to_ascii_lowercase());
        checks.push(ValidationCheckResult {
            name: format!("required section: {section}"),
            category: "deliverable".to_string(),
            passed,
            severity: ValidationSeverity::Hard,
            explanation: if passed {
                format!("Output includes required section `{section}`")
            } else {
                format!("Output is missing required section `{section}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![section.clone()]
            },
            confidence: Some(0.95),
        });
    }

    for artifact in &step.deliverable.required_artifacts {
        checks.push(validate_required_artifact(artifact, bundle));
    }

    checks
}

fn validate_required_artifact(
    artifact: &ArtifactRequirement,
    bundle: &StepEvidenceBundle,
) -> ValidationCheckResult {
    let passed = bundle.artifacts.iter().any(|candidate| {
        candidate
            .artifact_type
            .eq_ignore_ascii_case(&artifact.artifact_type)
            || artifact
                .path_hint
                .as_ref()
                .map(|hint| candidate.value.contains(hint))
                .unwrap_or(false)
    });
    ValidationCheckResult {
        name: format!("required artifact: {}", artifact.artifact_type),
        category: "deliverable".to_string(),
        passed,
        severity: ValidationSeverity::Hard,
        explanation: if passed {
            format!("Found required artifact `{}`", artifact.artifact_type)
        } else {
            format!("Missing required artifact `{}`", artifact.artifact_type)
        },
        evidence_refs: Vec::new(),
        missing_items: if passed {
            Vec::new()
        } else {
            vec![artifact.artifact_type.clone()]
        },
        confidence: Some(1.0),
    }
}

async fn validate_semantics(
    step: &PlanStep,
    content: &str,
    evidence_summary: &StepEvidenceSummary,
    provider: Arc<dyn LlmProvider>,
) -> Vec<ValidationCheckResult> {
    let mut checks = Vec::new();
    let mut missing_topics = Vec::new();
    for topic in &step.quality_requirements.must_cover_topics {
        let passed = content
            .to_ascii_lowercase()
            .contains(&topic.to_ascii_lowercase());
        if !passed {
            missing_topics.push(topic.clone());
        }
        checks.push(ValidationCheckResult {
            name: format!("topic coverage: {topic}"),
            category: "semantic".to_string(),
            passed,
            severity: ValidationSeverity::Soft,
            explanation: if passed {
                format!("Output covers topic `{topic}`")
            } else {
                format!("Output does not clearly cover topic `{topic}`")
            },
            evidence_refs: Vec::new(),
            missing_items: if passed {
                Vec::new()
            } else {
                vec![topic.clone()]
            },
            confidence: Some(if passed { 0.9 } else { 0.45 }),
        });
    }

    if step.quality_requirements.must_reference_evidence {
        let passed = !evidence_summary.coverage_markers.is_empty()
            || content.to_ascii_lowercase().contains("evidence")
            || content.to_ascii_lowercase().contains("source");
        checks.push(simple_structure_check(
            "evidence references",
            passed,
            ValidationSeverity::Soft,
            if passed {
                "Output references execution evidence"
            } else {
                "Output does not clearly reference evidence"
            },
        ));
    }

    if step.quality_requirements.must_include_reasoning_links {
        let lower = content.to_ascii_lowercase();
        let passed =
            lower.contains("because") || lower.contains("therefore") || lower.contains("因此");
        checks.push(simple_structure_check(
            "reasoning links",
            passed,
            ValidationSeverity::Soft,
            if passed {
                "Output includes reasoning links"
            } else {
                "Output lacks explicit reasoning links"
            },
        ));
    }

    if missing_topics.is_empty() && step.quality_requirements.semantic_expectations.is_empty() {
        return checks;
    }

    let llm_check = llm_semantic_check(step, content, evidence_summary, provider).await;
    checks.push(llm_check);
    checks
}

async fn llm_semantic_check(
    step: &PlanStep,
    content: &str,
    evidence_summary: &StepEvidenceSummary,
    provider: Arc<dyn LlmProvider>,
) -> ValidationCheckResult {
    let prompt = format!(
        "You are validating whether a plan step output satisfies semantic quality expectations.\n\
Return strict JSON with keys met (bool), confidence (0-1), explanation (string), missing_items (string[]), severity_recommendation (hard|soft|review).\n\n\
Step title: {}\n\
Validation profile: {:?}\n\
Required topics: {}\n\
Semantic expectations: {}\n\
Must reference evidence: {}\n\
Must include reasoning links: {}\n\
Evidence summary: files_read={}, files_written={}, tool_calls={}, searches={}, artifacts={}\n\
Output summary:\n{}\n",
        step.title,
        step.validation_profile,
        step.quality_requirements.must_cover_topics.join(", "),
        step.quality_requirements.semantic_expectations.join(", "),
        step.quality_requirements.must_reference_evidence,
        step.quality_requirements.must_include_reasoning_links,
        evidence_summary.files_read_count,
        evidence_summary.files_written_count,
        evidence_summary.tool_call_count,
        evidence_summary.search_query_count,
        evidence_summary.artifact_count,
        truncate_for_semantic_review(content, 6000)
    );

    let response = send_message_tracked(
        provider.as_ref(),
        vec![Message::text(MessageRole::User, prompt)],
        Some(
            "Validate semantic quality only. Do not discuss tooling. Output strict JSON."
                .to_string(),
        ),
        vec![],
        LlmRequestOptions {
            temperature_override: Some(0.1),
            ..Default::default()
        },
    )
    .await;

    let Ok(response) = response else {
        return ValidationCheckResult {
            name: "semantic quality review".to_string(),
            category: "semantic".to_string(),
            passed: false,
            severity: ValidationSeverity::Review,
            explanation: "Semantic review could not be completed automatically".to_string(),
            evidence_refs: Vec::new(),
            missing_items: Vec::new(),
            confidence: Some(0.0),
        };
    };
    let raw = response.content.unwrap_or_default();
    let parsed = extract_json_object(&raw)
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok());
    let Some(parsed) = parsed else {
        return ValidationCheckResult {
            name: "semantic quality review".to_string(),
            category: "semantic".to_string(),
            passed: false,
            severity: ValidationSeverity::Review,
            explanation: "Semantic review returned invalid JSON".to_string(),
            evidence_refs: Vec::new(),
            missing_items: Vec::new(),
            confidence: Some(0.0),
        };
    };

    let severity = match parsed
        .get("severity_recommendation")
        .and_then(|v| v.as_str())
    {
        Some("hard") => ValidationSeverity::Hard,
        Some("review") => ValidationSeverity::Review,
        _ => ValidationSeverity::Soft,
    };
    ValidationCheckResult {
        name: "semantic quality review".to_string(),
        category: "semantic".to_string(),
        passed: parsed.get("met").and_then(|v| v.as_bool()).unwrap_or(false),
        severity,
        explanation: parsed
            .get("explanation")
            .and_then(|v| v.as_str())
            .unwrap_or("No explanation")
            .to_string(),
        evidence_refs: Vec::new(),
        missing_items: parsed
            .get("missing_items")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        confidence: parsed.get("confidence").and_then(|v| v.as_f64()),
    }
}

fn finalize_validation(
    step: &PlanStep,
    checks: Vec<ValidationCheckResult>,
) -> StepValidationResult {
    let unmet_checks: Vec<ValidationCheckResult> = checks
        .iter()
        .filter(|check| !check.passed)
        .cloned()
        .collect();
    let hard_fail = unmet_checks
        .iter()
        .any(|check| check.severity == ValidationSeverity::Hard);
    let review_needed = unmet_checks
        .iter()
        .any(|check| check.severity == ValidationSeverity::Review)
        || unmet_checks
            .iter()
            .any(|check| check.confidence.unwrap_or(1.0) < 0.45);
    let soft_fail = unmet_checks
        .iter()
        .any(|check| check.severity == ValidationSeverity::Soft);

    let (status, outcome_status, failure_bucket, review_reason) = if hard_fail {
        (
            StepValidationStatus::HardFailed,
            StepOutcomeStatus::HardFailed,
            Some(primary_failure_bucket(&unmet_checks)),
            None,
        )
    } else if review_needed
        || matches!(
            step.failure_policy.severity,
            super::types::FailureSeverity::Review
        )
    {
        (
            StepValidationStatus::NeedsReview,
            StepOutcomeStatus::NeedsReview,
            Some(primary_failure_bucket(&unmet_checks)),
            Some(StepReviewReason::ReviewRequired),
        )
    } else if soft_fail
        || matches!(
            step.failure_policy.severity,
            super::types::FailureSeverity::Soft
        )
    {
        (
            StepValidationStatus::SoftFailed,
            StepOutcomeStatus::SoftFailed,
            Some(primary_failure_bucket(&unmet_checks)),
            None,
        )
    } else {
        (
            StepValidationStatus::Passed,
            StepOutcomeStatus::Completed,
            None,
            None,
        )
    };

    let confidence = aggregate_confidence(&checks);
    let summary = if unmet_checks.is_empty() {
        "All validation checks passed".to_string()
    } else {
        format!(
            "{} validation issue(s): {}",
            unmet_checks.len(),
            unmet_checks
                .iter()
                .map(|check| check.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let retry_guidance = unmet_checks
        .iter()
        .flat_map(build_retry_guidance)
        .collect::<Vec<_>>();

    StepValidationResult {
        status,
        outcome_status,
        failure_bucket,
        checks,
        unmet_checks,
        confidence,
        summary,
        retry_guidance,
        review_reason,
    }
}

fn build_retry_guidance(check: &ValidationCheckResult) -> Vec<String> {
    match check.category.as_str() {
        "evidence" => vec![format!(
            "Collect missing execution evidence for `{}` and cite it directly.",
            check.name
        )],
        "deliverable" => vec![format!(
            "Amend the deliverable to satisfy `{}`.",
            check.name
        )],
        "semantic" => vec![format!(
            "Strengthen semantic coverage for `{}` with explicit evidence mapping.",
            check.name
        )],
        _ => Vec::new(),
    }
}

fn primary_failure_bucket(checks: &[ValidationCheckResult]) -> StepFailureBucket {
    if checks.iter().any(|check| check.category == "evidence") {
        StepFailureBucket::MissingEvidence
    } else if checks.iter().any(|check| check.category == "deliverable") {
        StepFailureBucket::DeliverableIncomplete
    } else if checks
        .iter()
        .any(|check| check.severity == ValidationSeverity::Review)
    {
        StepFailureBucket::ReviewRequired
    } else {
        StepFailureBucket::SemanticGap
    }
}

fn aggregate_confidence(checks: &[ValidationCheckResult]) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0usize;
    for check in checks {
        if let Some(confidence) = check.confidence {
            total += confidence;
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

fn simple_structure_check(
    name: &str,
    passed: bool,
    severity: ValidationSeverity,
    explanation: &str,
) -> ValidationCheckResult {
    ValidationCheckResult {
        name: name.to_string(),
        category: "deliverable".to_string(),
        passed,
        severity,
        explanation: explanation.to_string(),
        evidence_refs: Vec::new(),
        missing_items: Vec::new(),
        confidence: Some(if passed { 0.95 } else { 0.4 }),
    }
}

fn truncate_for_semantic_review(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let truncated: String = content.chars().take(max_chars).collect();
    format!("{truncated}\n\n[truncated]")
}

fn extract_json_object(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return Some(text[start..=end].to_string());
        }
    }
    None
}
