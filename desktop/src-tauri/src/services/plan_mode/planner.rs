//! Plan Mode Planner
//!
//! Phase 3: LLM-powered plan decomposition.
//! Takes a task description and produces a Plan with steps and batches.

use std::sync::Arc;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};
use crate::utils::error::{AppError, AppResult};

use super::adapter::DomainAdapter;
use super::types::{
    calculate_plan_batches, ClarificationAnswer, Plan, PlanStep, StepPriority, TaskDomain,
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

    let decomposition_prompt =
        adapter.decomposition_prompt(description, full_context.as_deref());

    let messages = vec![Message::text(MessageRole::User, decomposition_prompt)];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = provider
        .send_message(messages, Some(system), vec![], options)
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

    let batches = calculate_plan_batches(&steps);

    Ok(Plan {
        title,
        description,
        domain: domain.clone(),
        adapter_name: adapter_name.to_string(),
        steps,
        batches,
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

    let completion_criteria = val
        .get("completionCriteria")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let expected_output = val
        .get("expectedOutput")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(PlanStep {
        id,
        title,
        description,
        priority,
        dependencies,
        completion_criteria,
        expected_output,
        metadata: std::collections::HashMap::new(),
    })
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

    let response = provider
        .send_message(messages, Some(system), vec![], options)
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
      "completionCriteria": ["Found 3+ sources"],
      "expectedOutput": "Research notes"
    },
    {
      "id": "step-2",
      "title": "Draft",
      "description": "Write the draft",
      "priority": "high",
      "dependencies": ["step-1"],
      "completionCriteria": ["1000+ words"],
      "expectedOutput": "Blog post draft"
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
            "completionCriteria": ["Done"],
            "expectedOutput": "Result"
        });
        let step = parse_step(&val).unwrap();
        assert_eq!(step.id, "step-1");
        assert_eq!(step.priority, StepPriority::High);
        assert_eq!(step.dependencies, vec!["step-0"]);
    }
}
