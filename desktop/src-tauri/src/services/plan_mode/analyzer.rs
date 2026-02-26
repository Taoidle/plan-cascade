//! Plan Mode Analyzer
//!
//! Phase 1: Classify the task domain, estimate complexity,
//! determine if clarification is needed, and select the appropriate adapter.

use std::sync::Arc;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};
use crate::utils::error::{AppError, AppResult};

use super::adapter_registry::AdapterRegistry;
use super::types::{PlanAnalysis, TaskDomain};

/// Analyze a task description and produce a PlanAnalysis.
pub async fn analyze_task(
    description: &str,
    conversation_context: Option<&str>,
    language_instruction: &str,
    provider: Arc<dyn LlmProvider>,
    registry: &AdapterRegistry,
) -> AppResult<PlanAnalysis> {
    let persona = super::adapter::build_plan_persona(super::types::PlanPersonaRole::Planner);

    let context_section = conversation_context
        .map(|c| format!("\n\n## Conversation Context\n{}", c))
        .unwrap_or_default();

    let system = format!(
        "{}\n\n{}\n\n\
         You are analyzing a task to determine its domain, complexity, and optimal decomposition approach.\n\n\
         ## Output Language\n\
         {language_instruction}\n\n\
         Analyze the task and respond with a JSON object:\n\
         ```json\n\
         {{\n\
           \"domain\": \"general|writing|research|marketing|data_analysis|project_management\",\n\
           \"complexity\": 1-10,\n\
           \"estimatedSteps\": 2-8,\n\
           \"needsClarification\": true|false,\n\
           \"reasoning\": \"Brief explanation of your analysis\",\n\
           \"suggestedApproach\": \"High-level approach description\"\n\
         }}\n\
         ```\n\n\
         Domain guidelines:\n\
         - \"writing\": Content creation (blog posts, articles, reports, documentation)\n\
         - \"research\": Investigation, analysis, market research, literature review\n\
         - \"marketing\": Campaigns, copy, strategy, social media\n\
         - \"data_analysis\": Data processing, visualization, statistical analysis\n\
         - \"project_management\": Planning, scheduling, resource allocation\n\
         - \"general\": Anything else\n\n\
         Set needsClarification=true ONLY when critical information is missing.\n\
         IMPORTANT: The \"reasoning\" and \"suggestedApproach\" fields MUST follow the output language instruction above.",
        persona.identity_prompt,
        persona.thinking_style,
    );

    let messages = vec![Message::text(
        MessageRole::User,
        format!("## Task\n{description}{context_section}\n\nAnalyze this task."),
    )];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = provider
        .send_message(messages, Some(system), vec![], options)
        .await
        .map_err(|e| AppError::Internal(format!("Plan analysis LLM error: {e}")))?;

    let text = response.content.as_deref().unwrap_or("");

    // Parse JSON from response
    let analysis = parse_analysis(text, registry)?;

    Ok(analysis)
}

/// Parse the LLM analysis response into a PlanAnalysis.
fn parse_analysis(text: &str, registry: &AdapterRegistry) -> AppResult<PlanAnalysis> {
    let json_str = extract_json_object(text)
        .ok_or_else(|| AppError::Internal("No JSON found in analysis response".to_string()))?;

    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| AppError::Internal(format!("Failed to parse analysis JSON: {e}")))?;

    let domain_str = parsed
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("general");

    let domain = match domain_str {
        "writing" => TaskDomain::Writing,
        "research" => TaskDomain::Research,
        "marketing" => TaskDomain::Marketing,
        "data_analysis" => TaskDomain::DataAnalysis,
        "project_management" => TaskDomain::ProjectManagement,
        _ => TaskDomain::General,
    };

    let adapter = registry.find_for_domain(&domain);

    Ok(PlanAnalysis {
        domain,
        complexity: parsed
            .get("complexity")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as u8,
        estimated_steps: parsed
            .get("estimatedSteps")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize,
        needs_clarification: parsed
            .get("needsClarification")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        reasoning: parsed
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("Analysis completed")
            .to_string(),
        adapter_name: adapter.id().to_string(),
        suggested_approach: parsed
            .get("suggestedApproach")
            .and_then(|v| v.as_str())
            .unwrap_or("Standard decomposition approach")
            .to_string(),
    })
}

/// Extract the first JSON object from a text that may contain markdown fences.
pub(super) fn extract_json_object(text: &str) -> Option<String> {
    // Try to find JSON in code fences first
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        // Skip optional language identifier on first line
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
    fn test_parse_analysis() {
        let registry = AdapterRegistry::with_builtins();
        let json = r#"```json
{
  "domain": "writing",
  "complexity": 4,
  "estimatedSteps": 5,
  "needsClarification": false,
  "reasoning": "This is a writing task",
  "suggestedApproach": "Outline then draft"
}
```"#;
        let analysis = parse_analysis(json, &registry).unwrap();
        assert_eq!(analysis.domain, TaskDomain::Writing);
        assert_eq!(analysis.complexity, 4);
        assert_eq!(analysis.estimated_steps, 5);
        assert!(!analysis.needs_clarification);
        assert_eq!(analysis.adapter_name, "writing");
    }

    #[test]
    fn test_extract_json_object() {
        let text = r#"Some preamble
```json
{"key": "value"}
```
Some postamble"#;
        assert_eq!(
            extract_json_object(text),
            Some(r#"{"key": "value"}"#.to_string())
        );
    }
}
